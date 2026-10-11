//! `panelctl uninstall` command handlers for clean panel uninstallation.

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::Path;
use tracing::info;

/// Options for `panelctl uninstall`.
#[derive(Debug, Clone, Default)]
pub struct UninstallCliOptions {
    /// Purge all panel databases, configuration files, and backup snapshots
    pub purge: bool,
    /// Automatically confirm uninstallation prompt
    pub yes: bool,
    /// Output result as formatted JSON
    pub json: bool,
}

/// Detailed result of the uninstall operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UninstallResult {
    pub success: bool,
    pub services_stopped: bool,
    pub unit_files_removed: bool,
    pub binaries_removed: bool,
    pub data_purged: bool,
    pub message: String,
}

fn stop_and_remove_systemd_services() -> (bool, bool) {
    if !cfg!(target_os = "linux") {
        return (false, false);
    }

    let _ = std::process::Command::new("systemctl")
        .args(["stop", "panel-api.service", "panel-agent.service"])
        .status();

    let _ = std::process::Command::new("systemctl")
        .args(["disable", "panel-api.service", "panel-agent.service"])
        .status();

    let mut unit_removed = false;
    for unit_file in &[
        "/etc/systemd/system/panel-api.service",
        "/etc/systemd/system/panel-agent.service",
    ] {
        let p = Path::new(unit_file);
        if p.exists() {
            let _ = std::fs::remove_file(p);
            unit_removed = true;
        }
    }

    let _ = std::process::Command::new("systemctl")
        .arg("daemon-reload")
        .status();

    (true, unit_removed)
}

fn remove_installed_binaries() -> bool {
    let mut removed = false;
    let bin_dir = Path::new("/usr/local/bin");
    for bin_name in &["panel-api", "panel-agent", "panelctl"] {
        let bin_path = bin_dir.join(bin_name);
        if bin_path.exists() {
            let _ = std::fs::remove_file(&bin_path);
            removed = true;
        }
    }
    let _ = std::fs::remove_dir_all("/run/panel");
    removed
}

fn purge_data_and_users(purge: bool) -> bool {
    if !purge {
        info!("Direktori data (/var/lib/panel, /var/backups/panel) dipertahankan (--keep-data).");
        return false;
    }

    info!("Membersihkan seluruh direktori data dan konfigurasi Nusa Panel (--purge)...");
    let mut purged = false;
    for dir_path in &[
        "/var/lib/panel",
        "/var/backups/panel",
        "/etc/panel",
        "/var/log/panel",
    ] {
        let p = Path::new(dir_path);
        if p.exists() {
            let _ = std::fs::remove_dir_all(p);
            purged = true;
        }
    }

    if cfg!(target_os = "linux") {
        let _ = std::process::Command::new("userdel")
            .arg("panel-api")
            .status();
        let _ = std::process::Command::new("groupdel").arg("panel").status();
    }
    purged
}

/// Executes clean uninstallation of Nusa Panel.
pub async fn run_uninstall(
    options: &UninstallCliOptions,
) -> Result<UninstallResult, Box<dyn Error>> {
    info!("Memulai proses pencopotan pemasangan (uninstall) Nusa Panel...");

    let (services_stopped, unit_files_removed) = stop_and_remove_systemd_services();
    let binaries_removed = remove_installed_binaries();
    let data_purged = purge_data_and_users(options.purge);

    let res = UninstallResult {
        success: true,
        services_stopped,
        unit_files_removed,
        binaries_removed,
        data_purged,
        message: if options.purge {
            "Nusa Panel berhasil dicopot sepenuhnya beserta seluruh data dan konfigurasi."
                .to_string()
        } else {
            "Nusa Panel berhasil dicopot. Berkas basis data dan cadangan dipertahankan.".to_string()
        },
    };

    if options.json {
        let json_str = serde_json::to_string_pretty(&res)?;
        println!("{}", json_str);
    } else {
        println!(
            "================================================================================"
        );
        println!(
            "                     HASIL PENCAPUTAN NUSA PANEL                                "
        );
        println!(
            "================================================================================"
        );
        println!("\x1b[32m[SUKSES]\x1b[0m {}", res.message);
        println!(
            "Layanan Dihentikan : {}",
            if res.services_stopped {
                "Ya"
            } else {
                "Tidak (Bukan Linux)"
            }
        );
        println!(
            "Unit Systemd       : {}",
            if res.unit_files_removed {
                "Dihapus"
            } else {
                "Tidak ditemukan"
            }
        );
        println!(
            "Biner Dihapus      : {}",
            if res.binaries_removed {
                "Ya"
            } else {
                "Tidak ditemukan"
            }
        );
        println!(
            "Status Data        : {}",
            if res.data_purged {
                "Dibersihkan (--purge)"
            } else {
                "Dipertahankan di /var/lib/panel"
            }
        );
        println!(
            "================================================================================"
        );
    }

    Ok(res)
}
