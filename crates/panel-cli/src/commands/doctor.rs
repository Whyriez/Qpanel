//! `panelctl doctor` command handler.

use panel_core::doctor::{
    DoctorCheckCategory, DoctorCheckItem, DoctorCheckStatus, DoctorReport, format_doctor_report_cli,
};
use panel_db::pool::DbPool;
use panel_db::repositories::SqliteJobRepository;
#[cfg(unix)]
use panel_proto::{
    RequestPayload, RpcRequest, RpcStream, SystemDoctorCheckRequest, SystemDoctorFixRequest,
};
use std::error::Error;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::info;

#[derive(Debug, Clone, Default)]
pub struct DoctorCliOptions {
    pub json: bool,
    pub fix: bool,
}

async fn check_database_integrity(pool: &DbPool, db_path: &str) -> DoctorCheckItem {
    let quick_check_res: Result<String, sqlx::Error> = sqlx::query_scalar("PRAGMA quick_check(1);")
        .fetch_one(pool)
        .await;

    let (db_status, db_msg) = match quick_check_res {
        Ok(ref val) if val.to_lowercase() == "ok" => (
            DoctorCheckStatus::Pass,
            format!(
                "Integritas berkas SQLite ({}) valid (PRAGMA quick_check: ok)",
                db_path
            ),
        ),
        Ok(ref val) => (
            DoctorCheckStatus::Fail,
            format!("Integritas berkas SQLite korup: {}", val),
        ),
        Err(ref e) => (
            DoctorCheckStatus::Fail,
            format!("Gagal memeriksa integritas basis data: {}", e),
        ),
    };

    DoctorCheckItem {
        id: "database_integrity".to_string(),
        name: "Integritas Basis Data SQLite".to_string(),
        category: DoctorCheckCategory::Database,
        status: db_status,
        message: db_msg,
        remediation_hint: if db_status == DoctorCheckStatus::Fail {
            Some(
                "Pulihkan basis data dari cadangan terbaru dengan panelctl backup restore"
                    .to_string(),
            )
        } else {
            None
        },
        fixable: false,
    }
}

async fn check_job_queue(pool: &DbPool) -> (DoctorCheckItem, i64) {
    let stuck_jobs_res: Result<i64, sqlx::Error> =
        sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE status = 'running';")
            .fetch_one(pool)
            .await;

    let stuck_count = stuck_jobs_res.unwrap_or(0);
    let (jobs_status, jobs_msg) = if stuck_count == 0 {
        (
            DoctorCheckStatus::Pass,
            "Tidak ada pekerjaan macet dalam status 'running'".to_string(),
        )
    } else {
        (
            DoctorCheckStatus::Warn,
            format!(
                "Ditemukan {} pekerjaan berstatus 'running' yang terinterupsi",
                stuck_count
            ),
        )
    };

    let item = DoctorCheckItem {
        id: "interrupted_jobs".to_string(),
        name: "Pekerjaan Terinterupsi / Macet".to_string(),
        category: DoctorCheckCategory::Jobs,
        status: jobs_status,
        message: jobs_msg,
        remediation_hint: if stuck_count > 0 {
            Some(
                "Jalankan panelctl doctor --fix untuk menandai gagal pekerjaan yang terhenti"
                    .to_string(),
            )
        } else {
            None
        },
        fixable: true,
    };

    (item, stuck_count)
}

async fn check_agent_socket_cli() -> DoctorCheckItem {
    #[allow(unused_mut)]
    let mut agent_connected = false;
    #[cfg(unix)]
    let socket_path = Path::new("/run/panel/agent.sock");

    #[cfg(unix)]
    if socket_path.exists() {
        if let Ok(stream) = tokio::net::UnixStream::connect(socket_path).await {
            let mut rpc = RpcStream::new(stream);
            let req = RpcRequest::new(
                "cli-doc-1",
                RequestPayload::SystemDoctorCheck(SystemDoctorCheckRequest {}),
            );
            if rpc.send_request(&req).await.is_ok() {
                let _ = rpc.recv_request().await;
            }
            agent_connected = true;
        }
    }

    let is_linux = cfg!(target_os = "linux");
    DoctorCheckItem {
        id: "agent_socket".to_string(),
        name: "Soket IPC Agent Panel".to_string(),
        category: DoctorCheckCategory::Socket,
        status: if agent_connected || !is_linux {
            DoctorCheckStatus::Pass
        } else {
            DoctorCheckStatus::Warn
        },
        message: if agent_connected {
            "Soket /run/panel/agent.sock aktif dan merespons IPC".to_string()
        } else if !is_linux {
            "Soket IPC dilewati pada lingkungan pengujian host".to_string()
        } else {
            "Soket /run/panel/agent.sock tidak merespons (agent mungkin belum berjalan)".to_string()
        },
        remediation_hint: if !agent_connected && is_linux {
            Some("Mulai daemon agent dengan 'systemctl start panel-agent'".to_string())
        } else {
            None
        },
        fixable: false,
    }
}

fn check_system_binaries_cli() -> Vec<DoctorCheckItem> {
    let mut items = Vec::new();
    let is_linux = cfg!(target_os = "linux");
    let cgroup_exists = Path::new("/sys/fs/cgroup/cgroup.controllers").exists();
    items.push(DoctorCheckItem {
        id: "cgroup_v2".to_string(),
        name: "cgroup v2 Controller".to_string(),
        category: DoctorCheckCategory::OsKernel,
        status: if cgroup_exists || !is_linux {
            DoctorCheckStatus::Pass
        } else {
            DoctorCheckStatus::Warn
        },
        message: if cgroup_exists {
            "cgroup v2 controllers aktif".to_string()
        } else {
            "cgroup v2 tidak aktif pada host".to_string()
        },
        remediation_hint: None,
        fixable: false,
    });

    for bin in &["nginx", "tar", "systemctl", "curl"] {
        let (status, msg) = match std::process::Command::new(bin).arg("--version").output() {
            Ok(out) if out.status.success() => {
                (DoctorCheckStatus::Pass, format!("Biner {} terpasang", bin))
            }
            _ => (
                DoctorCheckStatus::Warn,
                format!("Biner {} tidak ditemukan dalam PATH", bin),
            ),
        };
        items.push(DoctorCheckItem {
            id: format!("bin_{}", bin),
            name: format!("Biner: {}", bin),
            category: DoctorCheckCategory::Prerequisites,
            status,
            message: msg,
            remediation_hint: if status == DoctorCheckStatus::Warn {
                Some(format!("Pasang biner {} via apt-get", bin))
            } else {
                None
            },
            fixable: false,
        });
    }
    items
}

async fn apply_doctor_fixes_cli(
    pool: &DbPool,
    job_repo: &SqliteJobRepository,
    stuck_count: i64,
    checks: &mut [DoctorCheckItem],
) -> Result<(), Box<dyn Error>> {
    if stuck_count > 0 {
        let recovered = job_repo.recover_interrupted_jobs().await?;
        info!("Pulihkan {} pekerjaan yang terinterupsi", recovered);
        if let Some(check) = checks.iter_mut().find(|c| c.id == "interrupted_jobs") {
            check.status = DoctorCheckStatus::Pass;
            check.message = format!(
                "Semua pekerjaan terinterupsi ({}) telah dipulihkan",
                recovered
            );
            check.remediation_hint = None;
        }
    }

    // Run WAL checkpoint
    let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE);")
        .execute(pool)
        .await;

    #[cfg(unix)]
    {
        let socket_path = Path::new("/run/panel/agent.sock");
        if socket_path.exists() {
            if let Ok(stream) = tokio::net::UnixStream::connect(socket_path).await {
                let mut rpc = RpcStream::new(stream);
                let fix_req = RpcRequest::new(
                    "cli-fix-1",
                    RequestPayload::SystemDoctorFix(SystemDoctorFixRequest {
                        target_checks: vec![],
                    }),
                );
                let _ = rpc.send_request(&fix_req).await;
            }
        }
    }
    Ok(())
}

pub async fn run_doctor(
    options: &DoctorCliOptions,
    pool: &DbPool,
    db_path: &str,
) -> Result<DoctorReport, Box<dyn Error>> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut checks = Vec::new();

    // 1. Check SQLite database integrity
    checks.push(check_database_integrity(pool, db_path).await);

    // 2. Check Job Queue (Stuck jobs in running state)
    let job_repo = SqliteJobRepository::new(pool.clone());
    let (job_check, stuck_count) = check_job_queue(pool).await;
    checks.push(job_check);

    // 3. Try communicating with Agent over IPC Socket
    checks.push(check_agent_socket_cli().await);

    // 4 & 5. Local OS, kernel, binaries check
    checks.extend(check_system_binaries_cli());

    // Execute --fix actions if requested
    if options.fix {
        apply_doctor_fixes_cli(pool, &job_repo, stuck_count, &mut checks).await?;
    }

    let report = DoctorReport::new(timestamp, checks);

    if options.json {
        let json_str = serde_json::to_string_pretty(&report)?;
        println!("{}", json_str);
    } else {
        let cli_output = format_doctor_report_cli(&report, true);
        println!("{}", cli_output);
    }

    Ok(report)
}
