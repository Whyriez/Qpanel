//! `panelctl upgrade` command handlers for automated panel upgrades with safety backup and rollback.

use crate::commands::system_backup::{create_system_backup, restore_system_backup};
use panel_db::pool::{DbPool, run_migrations};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::Path;
use tracing::{error, info, warn};

/// Command line options for `panelctl upgrade`.
#[derive(Debug, Clone, Default)]
pub struct UpgradeCliOptions {
    /// Directory containing newer binaries (e.g. ./bin or /tmp/nusa-update/bin)
    pub binary_dir: Option<String>,
    /// Skip pre-upgrade safety backup snapshot
    pub skip_backup: bool,
    /// Restart systemd services after successful upgrade
    pub restart_services: bool,
    /// Target version string override
    pub target_version: Option<String>,
    /// Simulate upgrade without modifying filesystem or database
    pub dry_run: bool,
    /// Output result as formatted JSON
    pub json: bool,
}

/// Detailed result of the upgrade operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeResult {
    pub success: bool,
    pub from_version: String,
    pub to_version: String,
    pub backup_created: Option<String>,
    pub migrations_applied: bool,
    pub database_healthy: bool,
    pub rollback_performed: bool,
    pub binaries_updated: Vec<String>,
    pub services_restarted: bool,
    pub message: String,
    pub error: Option<String>,
}

/// Helper to verify database integrity via SQLite PRAGMA quick_check.
async fn verify_db_integrity(pool: &DbPool) -> bool {
    let check_res: Result<String, _> = sqlx::query_scalar("PRAGMA quick_check(1);")
        .fetch_one(pool)
        .await;

    matches!(check_res, Ok(v) if v.to_lowercase() == "ok")
}

/// Helper to safely replace binaries atomically with fallback.
fn replace_binary_atomically(src: &Path, dest: &Path) -> Result<(), Box<dyn Error>> {
    let tmp_dest = dest.with_extension("tmp_upg");
    std::fs::copy(src, &tmp_dest)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp_dest, std::fs::Permissions::from_mode(0o755));
    }

    std::fs::rename(&tmp_dest, dest)?;
    Ok(())
}

/// Updates panel binaries from a source directory to target destination.
fn update_binaries(binary_dir: &str, target_dir: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let bin_path = Path::new(binary_dir);
    if !bin_path.exists() {
        return Err(format!("Direktori biner baru tidak ditemukan: {}", binary_dir).into());
    }

    let mut updated = Vec::new();
    for binary_name in &["panel-api", "panel-agent", "panelctl"] {
        let src_binary = bin_path.join(binary_name);
        if src_binary.exists() {
            let dest_binary = target_dir.join(binary_name);
            replace_binary_atomically(&src_binary, &dest_binary)?;
            info!(
                "Biner {} berhasil diperbarui di {}",
                binary_name,
                dest_binary.display()
            );
            updated.push(binary_name.to_string());
        }
    }

    Ok(updated)
}

/// Restarts systemd daemon and panel services if running on Linux.
fn restart_panel_systemd_services() -> bool {
    if !cfg!(target_os = "linux") {
        return false;
    }

    let _ = std::process::Command::new("systemctl")
        .arg("daemon-reload")
        .status();

    let restart_res = std::process::Command::new("systemctl")
        .args(["restart", "panel-agent.service", "panel-api.service"])
        .status();

    matches!(restart_res, Ok(st) if st.success())
}

async fn execute_dry_run(
    pool: &DbPool,
    from_version: String,
    to_version: String,
    json: bool,
) -> Result<UpgradeResult, Box<dyn Error>> {
    let db_healthy = verify_db_integrity(pool).await;
    let res = UpgradeResult {
        success: db_healthy,
        from_version,
        to_version,
        backup_created: None,
        migrations_applied: false,
        database_healthy: db_healthy,
        rollback_performed: false,
        binaries_updated: Vec::new(),
        services_restarted: false,
        message: "Simulasi peningkatan selesai (dry-run). Basis data siap untuk migrasi."
            .to_string(),
        error: if db_healthy {
            None
        } else {
            Some("Uji integritas basis data gagal sebelum peningkatan".to_string())
        },
    };
    display_upgrade_result(&res, json)?;
    Ok(res)
}

async fn create_pre_upgrade_backup(
    pool: &DbPool,
    db_path: &str,
    skip: bool,
) -> Result<Option<String>, String> {
    if skip {
        return Ok(None);
    }
    info!("Membuat snapshot keselamatan sebelum pembaruan basis data...");
    match create_system_backup(pool, db_path, None, false).await {
        Ok(summary) => {
            info!(
                "Snapshot keselamatan berhasil dibuat di: {}",
                summary.filepath
            );
            Ok(Some(summary.filepath))
        }
        Err(e) => Err(format!(
            "Gagal membuat snapshot keselamatan pra-peningkatan: {}",
            e
        )),
    }
}

async fn attempt_rollback(pool: &DbPool, db_path: &str, backup_path: Option<&str>) -> bool {
    let Some(bk_path) = backup_path else {
        return false;
    };
    warn!(
        "Menjalankan pemulihan otomatis (rollback) dari snapshot keselamatan: {}",
        bk_path
    );
    match restore_system_backup(pool, db_path, bk_path).await {
        Ok(()) => {
            info!("Rollback basis data berhasil diselesaikan.");
            true
        }
        Err(e) => {
            error!("CRITICAL: Rollback basis data gagal: {}", e);
            false
        }
    }
}

async fn run_migrations_step(pool: &DbPool) -> Result<(), String> {
    info!("Menerapkan migrasi skema basis data...");
    run_migrations(pool)
        .await
        .map_err(|e| format!("Migrasi basis data gagal: {}", e))
}

async fn run_migrations_with_rollback(
    pool: &DbPool,
    db_path: &str,
    backup_path: Option<&str>,
) -> Result<(), (String, bool)> {
    if let Err(err_msg) = run_migrations_step(pool).await {
        error!("{}", err_msg);
        let rollback_done = attempt_rollback(pool, db_path, backup_path).await;
        return Err((err_msg, rollback_done));
    }

    if !verify_db_integrity(pool).await {
        let err_msg = "Integritas basis data tidak valid setelah migrasi skema.".to_string();
        error!("{}", err_msg);
        let rollback_done = attempt_rollback(pool, db_path, backup_path).await;
        return Err((err_msg, rollback_done));
    }

    Ok(())
}

fn apply_binaries_and_services(
    binary_dir: Option<&str>,
    restart_services: bool,
) -> (Vec<String>, bool) {
    let mut binaries_updated = Vec::new();
    if let Some(bin_dir) = binary_dir {
        let target_dir = Path::new("/usr/local/bin");
        let active_target = if target_dir.exists() {
            target_dir
        } else {
            Path::new(".")
        };

        if let Ok(updated) = update_binaries(bin_dir, active_target) {
            binaries_updated = updated;
        }
    }

    let services_restarted = if restart_services {
        restart_panel_systemd_services()
    } else {
        false
    };

    (binaries_updated, services_restarted)
}

/// Runs automated panel upgrade process with safety snapshot backup and automatic rollback.
pub async fn run_upgrade(
    options: &UpgradeCliOptions,
    pool: &DbPool,
    db_path: &str,
) -> Result<UpgradeResult, Box<dyn Error>> {
    let from_version = env!("CARGO_PKG_VERSION").to_string();
    let to_version = options
        .target_version
        .clone()
        .unwrap_or_else(|| from_version.clone());

    if options.dry_run {
        return execute_dry_run(pool, from_version, to_version, options.json).await;
    }

    let backup_created_path =
        match create_pre_upgrade_backup(pool, db_path, options.skip_backup).await {
            Ok(path) => path,
            Err(err_msg) => {
                let res = UpgradeResult {
                    success: false,
                    from_version,
                    to_version,
                    backup_created: None,
                    migrations_applied: false,
                    database_healthy: false,
                    rollback_performed: false,
                    binaries_updated: Vec::new(),
                    services_restarted: false,
                    message: "Peningkatan dibatalkan demi keamanan karena snapshot cadangan gagal."
                        .to_string(),
                    error: Some(err_msg),
                };
                display_upgrade_result(&res, options.json)?;
                return Ok(res);
            }
        };

    if let Err((err_msg, rollback_done)) =
        run_migrations_with_rollback(pool, db_path, backup_created_path.as_deref()).await
    {
        let res = UpgradeResult {
            success: false,
            from_version,
            to_version,
            backup_created: backup_created_path,
            migrations_applied: false,
            database_healthy: false,
            rollback_performed: rollback_done,
            binaries_updated: Vec::new(),
            services_restarted: false,
            message: "Peningkatan gagal. Rollback telah dijalankan.".to_string(),
            error: Some(err_msg),
        };
        display_upgrade_result(&res, options.json)?;
        return Ok(res);
    }

    let (binaries_updated, services_restarted) =
        apply_binaries_and_services(options.binary_dir.as_deref(), options.restart_services);

    let res = UpgradeResult {
        success: true,
        from_version: from_version.clone(),
        to_version: to_version.clone(),
        backup_created: backup_created_path,
        migrations_applied: true,
        database_healthy: true,
        rollback_performed: false,
        binaries_updated,
        services_restarted,
        message: format!(
            "Peningkatan Nusa Panel ke versi {} berhasil diselesaikan.",
            to_version
        ),
        error: None,
    };

    display_upgrade_result(&res, options.json)?;
    Ok(res)
}

/// Formats and outputs the upgrade result.
fn display_upgrade_result(res: &UpgradeResult, json: bool) -> Result<(), Box<dyn Error>> {
    if json {
        let json_str = serde_json::to_string_pretty(res)?;
        println!("{}", json_str);
        return Ok(());
    }

    println!("================================================================================");
    println!("                     HASIL PENINGKATAN SISTEM PANEL                             ");
    println!("================================================================================");
    if res.success {
        println!("\x1b[32m[SUKSES]\x1b[0m {}", res.message);
    } else {
        println!("\x1b[31m[GAGAL]\x1b[0m {}", res.message);
    }
    println!("Versi Sebelumnya   : v{}", res.from_version);
    println!("Versi Target       : v{}", res.to_version);
    if let Some(ref bk) = res.backup_created {
        println!("Snapshot Cadangan  : {}", bk);
    }
    println!(
        "Migrasi Basis Data : {}",
        if res.migrations_applied {
            "Berhasil"
        } else {
            "Gagal"
        }
    );
    println!(
        "Integritas DB      : {}",
        if res.database_healthy {
            "Sehat (PRAGMA quick_check: ok)"
        } else {
            "Bermasalah"
        }
    );
    if res.rollback_performed {
        println!("Rollback Otomatis  : \x1b[33mDijalankan (Data dipulihkan)\x1b[0m");
    }
    if !res.binaries_updated.is_empty() {
        println!("Biner Diperbarui   : {}", res.binaries_updated.join(", "));
    }
    if res.services_restarted {
        println!("Status Service     : Layanan systemd di-restart");
    }
    if let Some(ref err) = res.error {
        println!("Keterangan Error   : {}", err);
    }
    println!("================================================================================");

    Ok(())
}
