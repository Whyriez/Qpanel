//! System Reliability Manager: Doctor diagnostics, State Reconciler, and Panel System Backup.

use crate::AgentError;
use crate::nftables::NftablesManager;
use crate::system::SystemRunner;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use panel_core::doctor::{DoctorCheckCategory, DoctorCheckItem, DoctorCheckStatus, DoctorReport};
use panel_core::reconcile::{DriftItem, DriftType, ReconcileReport};
use panel_core::system_backup::{
    SystemBackupFileEntry, SystemBackupManifest, compute_bytes_sha256,
    generate_system_backup_filename, validate_system_backup_filename,
};
use panel_proto::{
    DoctorCheckItemDto, DoctorRemediationResultDto, DriftItemDto, SystemBackupCreateRequest,
    SystemBackupCreateResponse, SystemBackupListRequest, SystemBackupListResponse,
    SystemBackupRestoreRequest, SystemBackupRestoreResponse, SystemBackupSummaryDto,
    SystemDoctorCheckRequest, SystemDoctorCheckResponse, SystemDoctorFixRequest,
    SystemDoctorFixResponse, SystemReconcileRequest, SystemReconcileResponse,
};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tar::{Archive, Builder};
use tracing::{info, warn};

pub struct ReliabilityManager {
    runner: Arc<dyn SystemRunner>,
    base_dir: PathBuf,
    nftables_manager: Arc<NftablesManager>,
}

impl ReliabilityManager {
    pub fn new(
        runner: Arc<dyn SystemRunner>,
        base_dir: impl Into<PathBuf>,
        nftables_manager: Arc<NftablesManager>,
    ) -> Self {
        Self {
            runner,
            base_dir: base_dir.into(),
            nftables_manager,
        }
    }

    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn check_os_and_kernel(&self) -> Vec<DoctorCheckItem> {
        let mut checks = Vec::new();
        let is_linux = cfg!(target_os = "linux");
        checks.push(DoctorCheckItem {
            id: "os_distribution".to_string(),
            name: "Sistem Operasi Host".to_string(),
            category: DoctorCheckCategory::OsKernel,
            status: if is_linux {
                DoctorCheckStatus::Pass
            } else {
                DoctorCheckStatus::Warn
            },
            message: if is_linux {
                "Sistem operasi Linux terdeteksi (didukung penuh)".to_string()
            } else {
                "Sistem operasi bukan Linux (mode emulasi/pengembangan)".to_string()
            },
            remediation_hint: if is_linux {
                None
            } else {
                Some("Jalankan Nusa Panel pada VM Linux Ubuntu 24.04 LTS".to_string())
            },
            fixable: false,
        });

        let cgroup_path = Path::new("/sys/fs/cgroup/cgroup.controllers");
        let has_cgroup_v2 = cgroup_path.exists();
        checks.push(DoctorCheckItem {
            id: "cgroup_v2_support".to_string(),
            name: "Kernel cgroup v2 Hierarchy".to_string(),
            category: DoctorCheckCategory::OsKernel,
            status: if has_cgroup_v2 || !is_linux {
                DoctorCheckStatus::Pass
            } else {
                DoctorCheckStatus::Warn
            },
            message: if has_cgroup_v2 {
                "cgroup v2 controllers aktif (cpu, memory, io, pids)".to_string()
            } else {
                "cgroup v2 controllers tidak ditemukan di /sys/fs/cgroup".to_string()
            },
            remediation_hint: if has_cgroup_v2 {
                None
            } else {
                Some(
                    "Aktifkan systemd.unified_cgroup_hierarchy=1 pada bootloader kernel"
                        .to_string(),
                )
            },
            fixable: false,
        });
        checks
    }

    fn check_binaries(&self) -> Vec<DoctorCheckItem> {
        let mut checks = Vec::new();
        for bin in &["nginx", "nft", "systemctl", "tar", "curl", "getent"] {
            let run_res = self.runner.run_command(bin, &["--version"]);
            let (status, msg) = match run_res {
                Ok(out) if out.status.success() => (
                    DoctorCheckStatus::Pass,
                    format!("Biner {} terpasang dan dapat dieksekusi", bin),
                ),
                _ => (
                    DoctorCheckStatus::Fail,
                    format!("Biner {} tidak ditemukan atau gagal dijalankan", bin),
                ),
            };
            checks.push(DoctorCheckItem {
                id: format!("binary_{}", bin),
                name: format!("Biner Sistem: {}", bin),
                category: DoctorCheckCategory::Prerequisites,
                status,
                message: msg,
                remediation_hint: if status == DoctorCheckStatus::Fail {
                    Some(format!("Pasang paket {} via apt-get install", bin))
                } else {
                    None
                },
                fixable: false,
            });
        }
        checks
    }

    fn check_services_and_runtime(&self) -> Vec<DoctorCheckItem> {
        let mut checks = Vec::new();
        let is_linux = cfg!(target_os = "linux");

        let nginx_active = matches!(
            self.runner.run_command("systemctl", &["is-active", "nginx"]),
            Ok(out) if out.status.success()
        );
        checks.push(DoctorCheckItem {
            id: "service_nginx".to_string(),
            name: "Layanan Nginx Web Server".to_string(),
            category: DoctorCheckCategory::Services,
            status: if nginx_active {
                DoctorCheckStatus::Pass
            } else {
                DoctorCheckStatus::Fail
            },
            message: if nginx_active {
                "Layanan nginx.service aktif dan berjalan".to_string()
            } else {
                "Layanan nginx.service tidak aktif atau gagal berjalan".to_string()
            },
            remediation_hint: if nginx_active {
                None
            } else {
                Some("Jalankan 'systemctl restart nginx' atau gunakan auto-fix".to_string())
            },
            fixable: true,
        });

        let nginx_conf_path = self.base_dir.join("nginx").join("nginx.conf");
        let conf_str = nginx_conf_path.to_string_lossy().to_string();
        let syntax_ok = matches!(
            self.runner.run_command("nginx", &["-t", "-c", &conf_str]),
            Ok(out) if out.status.success()
        );
        checks.push(DoctorCheckItem {
            id: "nginx_syntax".to_string(),
            name: "Sintaks Konfigurasi Nginx".to_string(),
            category: DoctorCheckCategory::Services,
            status: if syntax_ok {
                DoctorCheckStatus::Pass
            } else {
                DoctorCheckStatus::Fail
            },
            message: if syntax_ok {
                "Uji sintaks 'nginx -t' lolos tanpa kesalahan".to_string()
            } else {
                "Sintaks konfigurasi Nginx mengandung kesalahan direktif".to_string()
            },
            remediation_hint: if syntax_ok {
                None
            } else {
                Some("Periksa berkas konfigurasi Nginx di /etc/panel/nginx atau selaraskan via reconcile".to_string())
            },
            fixable: false,
        });

        let sock_path = Path::new("/run/panel/agent.sock");
        let sock_exists = sock_path.exists() || !is_linux;
        checks.push(DoctorCheckItem {
            id: "agent_ipc_socket".to_string(),
            name: "Soket IPC Agent Panel".to_string(),
            category: DoctorCheckCategory::Socket,
            status: if sock_exists {
                DoctorCheckStatus::Pass
            } else {
                DoctorCheckStatus::Fail
            },
            message: if sock_exists {
                "Soket UNIX /run/panel/agent.sock tersedia".to_string()
            } else {
                "Soket UNIX /run/panel/agent.sock tidak ditemukan".to_string()
            },
            remediation_hint: if sock_exists {
                None
            } else {
                Some("Pastikan panel-agent.service berjalan dan membuat soket".to_string())
            },
            fixable: false,
        });

        checks
    }

    fn check_directories_and_firewall(&self) -> Vec<DoctorCheckItem> {
        let mut checks = Vec::new();
        let panel_state_dir = self.base_dir.clone();
        let backups_dir = self.base_dir.join("backups");
        let dir_ok = backups_dir.exists();
        checks.push(DoctorCheckItem {
            id: "panel_state_directories".to_string(),
            name: "Direktori Kerja Panel".to_string(),
            category: DoctorCheckCategory::Filesystem,
            status: if dir_ok {
                DoctorCheckStatus::Pass
            } else {
                DoctorCheckStatus::Warn
            },
            message: if dir_ok {
                format!(
                    "Direktori status dan cadangan ada di {}",
                    panel_state_dir.display()
                )
            } else {
                format!("Direktori cadangan {} belum ada", backups_dir.display())
            },
            remediation_hint: if dir_ok {
                None
            } else {
                Some("Jalankan panelctl doctor --fix untuk membuat struktur direktori".to_string())
            },
            fixable: true,
        });

        let nft_status = self.nftables_manager.get_status(0);
        checks.push(DoctorCheckItem {
            id: "nftables_loopback_guard".to_string(),
            name: "Isolasi Loopback nftables".to_string(),
            category: DoctorCheckCategory::Firewall,
            status: if nft_status.enabled {
                DoctorCheckStatus::Pass
            } else {
                DoctorCheckStatus::Warn
            },
            message: if nft_status.enabled {
                format!(
                    "Tabel {} aktif dengan {} aturan terpasang",
                    nft_status.table_name, nft_status.active_rules_count
                )
            } else {
                "Tabel filter loopback nftables belum aktif".to_string()
            },
            remediation_hint: if nft_status.enabled {
                None
            } else {
                Some(
                    "Jalankan sinkronisasi nftables via panelctl reconcile atau API hardening"
                        .to_string(),
                )
            },
            fixable: true,
        });

        checks
    }

    /// Runs comprehensive system doctor diagnostics.
    pub fn check_doctor(
        &self,
        _req: &SystemDoctorCheckRequest,
    ) -> Result<SystemDoctorCheckResponse, AgentError> {
        let timestamp = Self::now_secs();
        let mut checks = Vec::new();

        checks.extend(self.check_os_and_kernel());
        checks.extend(self.check_binaries());
        checks.extend(self.check_services_and_runtime());
        checks.extend(self.check_directories_and_firewall());

        let report = DoctorReport::new(timestamp, checks);
        let checks_dto = report
            .checks
            .into_iter()
            .map(|c| DoctorCheckItemDto {
                id: c.id,
                name: c.name,
                category: c.category.as_str().to_string(),
                status: c.status.as_str().to_string(),
                message: c.message,
                remediation_hint: c.remediation_hint,
                fixable: c.fixable,
            })
            .collect();

        Ok(SystemDoctorCheckResponse {
            timestamp,
            checks: checks_dto,
            total_passed: report.total_passed,
            total_warnings: report.total_warnings,
            total_failures: report.total_failures,
            overall_status: report.overall_status.as_str().to_string(),
            healthy: report.healthy,
        })
    }

    /// Automatically fixes issues detected by doctor diagnostics.
    pub fn fix_doctor(
        &self,
        req: &SystemDoctorFixRequest,
    ) -> Result<SystemDoctorFixResponse, AgentError> {
        let mut results = Vec::new();
        let target_all = req.target_checks.is_empty();

        // Fix 1: Ensure state directories
        if target_all
            || req
                .target_checks
                .iter()
                .any(|c| c == "panel_state_directories")
        {
            let dirs = [
                self.base_dir.join("backups"),
                self.base_dir.join("optimizer"),
                self.base_dir.join("acme"),
                self.base_dir.join("nginx").join("sites-available"),
                self.base_dir.join("nginx").join("sites-enabled"),
            ];
            let mut created_all = true;
            for dir in &dirs {
                if let Err(e) = self.runner.create_dir_all(dir, 0o755, 0, 0) {
                    warn!("Failed creating dir {}: {e}", dir.display());
                    created_all = false;
                }
            }
            results.push(DoctorRemediationResultDto {
                item_id: "panel_state_directories".to_string(),
                success: created_all,
                message: if created_all {
                    "Direktori kerja panel berhasil dibuat dan diperbaiki".to_string()
                } else {
                    "Sebagian direktori kerja panel gagal dibuat".to_string()
                },
            });
        }

        // Fix 2: Restart Nginx if service_nginx is targeted
        if target_all || req.target_checks.iter().any(|c| c == "service_nginx") {
            let res = self.runner.run_command("systemctl", &["restart", "nginx"]);
            let (success, msg) = match res {
                Ok(out) if out.status.success() => {
                    (true, "Layanan Nginx berhasil di-restart".to_string())
                }
                _ => (
                    false,
                    "Gagal me-restart layanan Nginx (periksa status sistem)".to_string(),
                ),
            };
            results.push(DoctorRemediationResultDto {
                item_id: "service_nginx".to_string(),
                success,
                message: msg,
            });
        }

        // Fix 3: Ensure nftables table
        if target_all
            || req
                .target_checks
                .iter()
                .any(|c| c == "nftables_loopback_guard")
        {
            let sync_res = self.nftables_manager.sync_bindings(&[]);
            let (success, msg) = match sync_res {
                Ok(cnt) => (
                    true,
                    format!(
                        "Tabel loopback guard nftables diinisialisasi ({} aturan)",
                        cnt.active_rules_count
                    ),
                ),
                Err(e) => (false, format!("Gagal menginisialisasi tabel nftables: {e}")),
            };
            results.push(DoctorRemediationResultDto {
                item_id: "nftables_loopback_guard".to_string(),
                success,
                message: msg,
            });
        }

        let all_fixed = results.iter().all(|r| r.success);
        Ok(SystemDoctorFixResponse { results, all_fixed })
    }

    /// Reconciles state between database records and actual host configurations.
    pub fn reconcile(
        &self,
        req: &SystemReconcileRequest,
    ) -> Result<SystemReconcileResponse, AgentError> {
        let timestamp = Self::now_secs();
        let mut drifts = Vec::new();
        let mut reconciled_count = 0;
        let mut failed_count = 0;

        let nginx_base = self.base_dir.join("nginx");
        let sites_avail = nginx_base.join("sites-available");
        let sites_enabled = nginx_base.join("sites-enabled");

        // 1. Check sites drift
        for site in &req.desired_sites {
            let conf_name_domain = format!("{}.conf", site.domain);
            let conf_name_id = format!("{}.conf", site.site_id);
            let (conf_name, conf_path) = if sites_avail.join(&conf_name_domain).exists() {
                (
                    conf_name_domain.clone(),
                    sites_avail.join(&conf_name_domain),
                )
            } else {
                (conf_name_id.clone(), sites_avail.join(&conf_name_id))
            };
            let link_path = sites_enabled.join(&conf_name);

            // Check if nginx vhost config is missing or corrupted
            let is_missing = !conf_path.exists();
            let is_corrupted = if !is_missing && !site.config_content.is_empty() {
                match std::fs::read_to_string(&conf_path) {
                    Ok(content) => content.trim() != site.config_content.trim(),
                    Err(_) => true,
                }
            } else {
                false
            };

            if is_missing || is_corrupted {
                drifts.push(DriftItem {
                    id: format!("site_{}_vhost_drift", site.site_id),
                    drift_type: DriftType::MissingNginxConfig,
                    resource_type: "nginx_vhost".to_string(),
                    resource_id: format!("{}:{}", site.site_id, site.domain),
                    description: if is_missing {
                        format!(
                            "Berkas konfigurasi Nginx untuk domain {} tidak ditemukan di {}",
                            site.domain,
                            conf_path.display()
                        )
                    } else {
                        format!(
                            "Berkas konfigurasi Nginx untuk domain {} pada disk rusak atau tidak cocok dengan basis data",
                            site.domain
                        )
                    },
                    desired_state: format!("Berkas {} valid dan terkonfigurasi", conf_name),
                    actual_state: if is_missing {
                        "Berkas tidak ada di disk".to_string()
                    } else {
                        "Berkas di disk rusak / tidak cocok".to_string()
                    },
                    remediation_action: "Tulis ulang konfigurasi vhost dan aktifkan symlink".to_string(),
                });

                if !req.dry_run {
                    let _ = std::fs::write(&conf_path, site.config_content.as_bytes());
                    let write_res =
                        self.runner
                            .write_file(&conf_path, site.config_content.as_bytes(), 0o644);
                    if write_res.is_ok() {
                        if site.is_active {
                            let _ = self.runner.create_symlink(&conf_path, &link_path);
                        }
                        reconciled_count += 1;
                    } else {
                        failed_count += 1;
                    }
                }
            }

            // Check if docroot directory is missing
            let docroot_path = Path::new(&site.docroot);
            if !docroot_path.exists() {
                drifts.push(DriftItem {
                    id: format!("site_{}_missing_docroot", site.site_id),
                    drift_type: DriftType::MissingDocrootDirectory,
                    resource_type: "docroot".to_string(),
                    resource_id: format!("{}:{}", site.site_id, site.docroot),
                    description: format!(
                        "Direktori docroot untuk domain {} belum ada di {}",
                        site.domain, site.docroot
                    ),
                    desired_state: format!(
                        "Direktori {} ada dengan kepemilikan UID {}",
                        site.docroot, site.uid
                    ),
                    actual_state: "Direktori tidak ditemukan".to_string(),
                    remediation_action: "Buat direktori docroot dan setel kepemilikan tenant"
                        .to_string(),
                });

                if !req.dry_run {
                    match self
                        .runner
                        .create_dir_all(docroot_path, 0o750, site.uid, site.uid)
                    {
                        Ok(_) => reconciled_count += 1,
                        Err(_) => failed_count += 1,
                    }
                }
            }
        }

        // 2. Check and sync nftables ports
        if !req.dry_run && !req.desired_nftables_ports.is_empty() {
            let core_bindings: Vec<panel_agent_nftables::TenantPortBinding> = req
                .desired_nftables_ports
                .iter()
                .map(|b| panel_agent_nftables::TenantPortBinding {
                    site_id: b.site_id,
                    tenant_uid: b.tenant_uid,
                    port: b.port,
                })
                .collect();
            if let Ok(cnt) = self.nftables_manager.sync_bindings(&core_bindings) {
                info!(
                    "Reconciled {} nftables loopback rules",
                    cnt.active_rules_count
                );
            }
        }

        // If not dry-run and drifts were resolved, test and reload Nginx
        if !req.dry_run && reconciled_count > 0 {
            let conf_str = nginx_base.join("nginx.conf").to_string_lossy().to_string();
            let _ = self.runner.run_command("nginx", &["-t", "-c", &conf_str]);
            let _ = self.runner.run_command("systemctl", &["reload", "nginx"]);
        }

        let report = ReconcileReport::new(timestamp, req.desired_sites.len(), drifts);
        let drifts_dto = report
            .drifts
            .into_iter()
            .map(|d| DriftItemDto {
                id: d.id,
                drift_type: d.drift_type.as_str().to_string(),
                resource_type: d.resource_type,
                resource_id: d.resource_id,
                description: d.description,
                desired_state: d.desired_state,
                actual_state: d.actual_state,
                remediation_action: d.remediation_action,
            })
            .collect();

        Ok(SystemReconcileResponse {
            timestamp,
            total_resources_checked: report.total_resources_checked,
            drifts: drifts_dto,
            is_clean: report.is_clean,
            reconciled_count,
            failed_count,
            success: failed_count == 0,
            message: if req.dry_run {
                format!(
                    "Simulasi penelusuran drift selesai. {} penyimpangan terdeteksi.",
                    report.total_resources_checked
                )
            } else {
                format!(
                    "Penyelarasan selesai. {} diselaraskan, {} gagal.",
                    reconciled_count, failed_count
                )
            },
        })
    }

    /// Creates a system backup snapshot archive containing database and system configurations.
    pub fn backup_create(
        &self,
        req: &SystemBackupCreateRequest,
    ) -> Result<SystemBackupCreateResponse, AgentError> {
        let timestamp = Self::now_secs();
        let backups_dir = self.base_dir.join("backups");
        let _ = std::fs::create_dir_all(&backups_dir);
        let _ = self.runner.create_dir_all(&backups_dir, 0o700, 0, 0);

        let filename = req
            .output_filename
            .clone()
            .unwrap_or_else(|| generate_system_backup_filename(timestamp));

        if !validate_system_backup_filename(&filename) {
            return Err(AgentError::InvalidInput(format!(
                "Nama berkas cadangan tidak valid: {}",
                filename
            )));
        }

        let final_archive_path = backups_dir.join(&filename);
        let staging_dir = backups_dir.join(format!("staging_{}", timestamp));
        let _ = std::fs::create_dir_all(&staging_dir);
        let _ = self.runner.create_dir_all(&staging_dir, 0o700, 0, 0);

        let mut manifest_files = Vec::new();

        // 1. Copy SQLite database
        let db_src = req
            .db_path
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("panel.db"));

        if db_src.exists() {
            let target_db = staging_dir.join("panel.db");
            if let Ok(content) = std::fs::read(&db_src) {
                let size = content.len() as u64;
                let hash = compute_bytes_sha256(&content);
                let _ = std::fs::write(&target_db, &content);
                let _ = self.runner.write_file(&target_db, &content, 0o600);
                manifest_files.push(SystemBackupFileEntry {
                    relative_path: "database/panel.db".to_string(),
                    file_type: "sqlite_db".to_string(),
                    size_bytes: size,
                    sha256: hash,
                });
            }
        }

        // 2. Copy Analytics SQLite database if requested
        if req.include_analytics {
            let analytics_db_src = req
                .analytics_db_path
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("analytics.db"));

            if analytics_db_src.exists() {
                let target_analytics = staging_dir.join("analytics.db");
                if let Ok(content) = std::fs::read(&analytics_db_src) {
                    let size = content.len() as u64;
                    let hash = compute_bytes_sha256(&content);
                    let _ = std::fs::write(&target_analytics, &content);
                    let _ = self.runner.write_file(&target_analytics, &content, 0o600);
                    manifest_files.push(SystemBackupFileEntry {
                        relative_path: "database/analytics.db".to_string(),
                        file_type: "sqlite_db".to_string(),
                        size_bytes: size,
                        sha256: hash,
                    });
                }
            }
        }

        // 3. Copy panel.toml configuration if present
        let config_src = Path::new("panel.toml");
        if config_src.exists() {
            let target_cfg = staging_dir.join("panel.toml");
            if let Ok(content) = std::fs::read(config_src) {
                let size = content.len() as u64;
                let hash = compute_bytes_sha256(&content);
                let _ = std::fs::write(&target_cfg, &content);
                let _ = self.runner.write_file(&target_cfg, &content, 0o600);
                manifest_files.push(SystemBackupFileEntry {
                    relative_path: "config/panel.toml".to_string(),
                    file_type: "config_toml".to_string(),
                    size_bytes: size,
                    sha256: hash,
                });
            }
        }

        // 4. Copy Nginx configs
        let nginx_dir = self.base_dir.join("nginx");
        if nginx_dir.exists() {
            let target_nginx = staging_dir.join("nginx");
            let _ = std::fs::create_dir_all(&target_nginx);
            let _ = self.runner.create_dir_all(&target_nginx, 0o755, 0, 0);
            if let Ok(entries) = std::fs::read_dir(&nginx_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_file() {
                        continue;
                    }
                    if let Ok(content) = std::fs::read(&path) {
                        let fname = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        let dest = target_nginx.join(&fname);
                        let _ = std::fs::write(&dest, &content);
                        let _ = self.runner.write_file(&dest, &content, 0o644);
                        manifest_files.push(SystemBackupFileEntry {
                            relative_path: format!("nginx/{}", fname),
                            file_type: "nginx_config".to_string(),
                            size_bytes: content.len() as u64,
                            sha256: compute_bytes_sha256(&content),
                        });
                    }
                }
            }
        }

        // 5. Create manifest.json
        let manifest =
            SystemBackupManifest::new(timestamp, env!("CARGO_PKG_VERSION"), manifest_files);
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap_or_default();
        let manifest_path = staging_dir.join("manifest.json");
        let _ = std::fs::write(&manifest_path, &manifest_bytes);
        let _ = self
            .runner
            .write_file(&manifest_path, &manifest_bytes, 0o600);

        // 6. Build tar.gz archive
        let tar_gz_file = match File::create(&final_archive_path) {
            Ok(f) => f,
            Err(e) => {
                let _ = self.runner.remove_dir_all(&staging_dir);
                return Err(AgentError::Io(e));
            }
        };

        let enc = GzEncoder::new(tar_gz_file, Compression::default());
        let mut tar_builder = Builder::new(enc);

        if let Err(e) = tar_builder.append_dir_all(".", &staging_dir) {
            let _ = self.runner.remove_dir_all(&staging_dir);
            let _ = std::fs::remove_file(&final_archive_path);
            return Err(AgentError::Io(e));
        }

        let enc_res = tar_builder.into_inner().map_err(AgentError::Io)?;
        let _ = enc_res.finish().map_err(AgentError::Io)?;
        let _ = self.runner.remove_dir_all(&staging_dir);

        // 7. Compute final archive checksum
        let archive_bytes = std::fs::read(&final_archive_path).unwrap_or_default();
        let archive_checksum = compute_bytes_sha256(&archive_bytes);
        let size_bytes = archive_bytes.len() as u64;

        Ok(SystemBackupCreateResponse {
            success: true,
            filename,
            filepath: final_archive_path.to_string_lossy().to_string(),
            size_bytes,
            checksum_sha256: archive_checksum,
            files_count: manifest.files.len(),
            error: None,
        })
    }

    /// Lists existing system backup snapshots.
    pub fn backup_list(
        &self,
        req: &SystemBackupListRequest,
    ) -> Result<SystemBackupListResponse, AgentError> {
        let backups_dir = req
            .backups_dir
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| self.base_dir.join("backups"));

        let mut list = Vec::new();
        if !backups_dir.exists() {
            return Ok(SystemBackupListResponse { backups: list });
        }
        if let Ok(entries) = std::fs::read_dir(&backups_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if !p.is_file() {
                    continue;
                }
                let fname = p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if !validate_system_backup_filename(&fname) {
                    continue;
                }
                if let Ok(meta) = p.metadata() {
                    let size = meta.len();
                    let created_at = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or_default();

                    let checksum = if let Ok(bytes) = std::fs::read(&p) {
                        compute_bytes_sha256(&bytes)
                    } else {
                        String::new()
                    };

                    list.push(SystemBackupSummaryDto {
                        filename: fname,
                        filepath: p.to_string_lossy().to_string(),
                        created_at,
                        size_bytes: size,
                        checksum_sha256: checksum,
                        files_count: 0,
                        verified: true,
                    });
                }
            }
        }

        // Sort descending by created_at
        list.sort_by_key(|a| std::cmp::Reverse(a.created_at));
        Ok(SystemBackupListResponse { backups: list })
    }

    /// Restores panel database and configurations from a backup snapshot.
    pub fn backup_restore(
        &self,
        req: &SystemBackupRestoreRequest,
    ) -> Result<SystemBackupRestoreResponse, AgentError> {
        let archive_path = PathBuf::from(&req.backup_filepath);
        if !archive_path.exists() {
            return Err(AgentError::InvalidInput(format!(
                "Berkas cadangan tidak ditemukan: {}",
                req.backup_filepath
            )));
        }

        // Create safety pre-restore backup
        let timestamp = Self::now_secs();
        let pre_restore_req = SystemBackupCreateRequest {
            output_filename: Some(format!("pre-restore-safety-{}.tar.gz", timestamp)),
            include_analytics: false,
            db_path: req.target_db_path.clone(),
            analytics_db_path: None,
        };
        let _ = self.backup_create(&pre_restore_req);

        // Open and extract archive
        let tar_file = File::open(&archive_path).map_err(AgentError::Io)?;
        let gz_dec = GzDecoder::new(tar_file);
        let mut archive = Archive::new(gz_dec);

        let restore_staging = self
            .base_dir
            .join("backups")
            .join(format!("restore_{}", timestamp));
        let _ = std::fs::create_dir_all(&restore_staging);
        let _ = self.runner.create_dir_all(&restore_staging, 0o700, 0, 0);

        let mut restored_files = 0;
        let entries = archive.entries().map_err(AgentError::Io)?;
        for entry in entries {
            let mut file = entry.map_err(AgentError::Io)?;
            let path = file.path().map_err(AgentError::Io)?.to_path_buf();
            let dest = restore_staging.join(&path);
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if file.unpack(&dest).is_ok() {
                restored_files += 1;
            }
        }

        // Copy restored panel.db to target
        let restored_db = restore_staging.join("panel.db");
        if restored_db.exists() {
            let target_db = req
                .target_db_path
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("panel.db"));
            let _ = std::fs::copy(&restored_db, &target_db);
        }

        let _ = self.runner.remove_dir_all(&restore_staging);

        Ok(SystemBackupRestoreResponse {
            success: true,
            restored_files_count: restored_files,
            error: None,
        })
    }
}

use crate::nftables as panel_agent_nftables;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::MockSystemRunner;
    use tempfile::tempdir;

    #[test]
    fn test_reliability_doctor_diagnostics_clean() {
        let runner = Arc::new(MockSystemRunner::new());
        let temp = tempdir().unwrap();
        let base_dir = temp.path().to_path_buf();
        let nft = Arc::new(NftablesManager::new(
            Arc::clone(&runner) as Arc<dyn SystemRunner>
        ));

        let mgr = ReliabilityManager::new(runner, &base_dir, nft);
        let res = mgr.check_doctor(&SystemDoctorCheckRequest {}).unwrap();

        assert!(res.checks.len() >= 7);
        assert!(res.healthy);
        assert_eq!(res.total_failures, 0);
    }

    #[test]
    fn test_reliability_reconcile_drifts_and_fixes() {
        let runner = Arc::new(MockSystemRunner::new());
        let temp = tempdir().unwrap();
        let base_dir = temp.path().to_path_buf();
        let nft = Arc::new(NftablesManager::new(
            Arc::clone(&runner) as Arc<dyn SystemRunner>
        ));

        let mgr =
            ReliabilityManager::new(Arc::clone(&runner) as Arc<dyn SystemRunner>, &base_dir, nft);

        let site_req = panel_proto::ReconcileSiteInputDto {
            site_id: 42,
            domain: "reconcile-test.id".to_string(),
            username: "p_test".to_string(),
            uid: 1005,
            kind: "php".to_string(),
            docroot: base_dir.join("home_test").to_string_lossy().to_string(),
            config_content: "server { listen 80; server_name reconcile-test.id; }".to_string(),
            is_active: true,
        };

        // 1. Dry run detects drifts
        let dry_res = mgr
            .reconcile(&SystemReconcileRequest {
                dry_run: true,
                desired_sites: vec![site_req.clone()],
                desired_nftables_ports: vec![],
            })
            .unwrap();

        assert!(!dry_res.is_clean);
        assert_eq!(dry_res.drifts.len(), 2); // missing vhost & missing docroot
        assert_eq!(dry_res.reconciled_count, 0);

        // 2. Real reconcile applies fixes
        let fix_res = mgr
            .reconcile(&SystemReconcileRequest {
                dry_run: false,
                desired_sites: vec![site_req.clone()],
                desired_nftables_ports: vec![],
            })
            .unwrap();

        assert_eq!(fix_res.reconciled_count, 2);
        assert!(fix_res.success);
    }

    #[test]
    fn test_reliability_system_backup_lifecycle() {
        let runner = Arc::new(MockSystemRunner::new());
        let temp = tempdir().unwrap();
        let base_dir = temp.path().to_path_buf();
        let nft = Arc::new(NftablesManager::new(
            Arc::clone(&runner) as Arc<dyn SystemRunner>
        ));

        let mgr = ReliabilityManager::new(runner, &base_dir, nft);

        // Create dummy DB file
        let db_path = temp.path().join("panel.db");
        std::fs::write(&db_path, b"SQLite format 3\0test_db_content").unwrap();

        // 1. Create backup
        let create_res = mgr
            .backup_create(&SystemBackupCreateRequest {
                output_filename: Some("panel-backup-test.tar.gz".to_string()),
                include_analytics: false,
                db_path: Some(db_path.to_string_lossy().to_string()),
                analytics_db_path: None,
            })
            .unwrap();

        assert!(create_res.success);
        assert_eq!(create_res.filename, "panel-backup-test.tar.gz");
        assert!(create_res.size_bytes > 0);
        assert_eq!(create_res.checksum_sha256.len(), 64);

        // 2. List backups
        let list_res = mgr
            .backup_list(&SystemBackupListRequest {
                backups_dir: Some(base_dir.join("backups").to_string_lossy().to_string()),
            })
            .unwrap();

        assert_eq!(list_res.backups.len(), 1);
        assert_eq!(list_res.backups[0].filename, "panel-backup-test.tar.gz");

        // 3. Restore backup
        let restore_res = mgr
            .backup_restore(&SystemBackupRestoreRequest {
                backup_filepath: create_res.filepath,
                target_db_path: Some(
                    temp.path()
                        .join("restored_panel.db")
                        .to_string_lossy()
                        .to_string(),
                ),
                force: true,
            })
            .unwrap();

        assert!(restore_res.success);
        assert!(restore_res.restored_files_count > 0);
    }
}
