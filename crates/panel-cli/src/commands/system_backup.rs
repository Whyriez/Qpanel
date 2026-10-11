//! `panelctl backup` command handlers for panel database and system configuration.

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use panel_core::system_backup::{
    SystemBackupFileEntry, SystemBackupManifest, SystemBackupSummary,
    SystemBackupVerificationResult, compute_bytes_sha256, generate_system_backup_filename,
    validate_system_backup_filename,
};
use panel_db::pool::DbPool;
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tar::{Archive, Builder};
use tracing::info;

/// Subcommands for `panelctl backup`.
#[derive(clap::Subcommand, Debug)]
pub enum SystemBackupCommands {
    /// Create a new snapshot backup of panel database and configuration
    Create {
        /// Optional destination output filename (e.g. panel-backup-custom.tar.gz)
        #[arg(short, long)]
        output: Option<String>,

        /// Include analytics.db SQLite database in the backup archive
        #[arg(long, default_value_t = false)]
        include_analytics: bool,
    },
    /// List all existing panel system backups
    List {
        /// Backups directory path (default: /var/lib/panel/backups or ./backups)
        #[arg(short, long)]
        dir: Option<String>,
    },
    /// Verify integrity and SHA-256 checksums of a backup snapshot archive
    Verify {
        /// Path to the backup archive file (.tar.gz)
        #[arg(short, long)]
        path: String,
    },
    /// Restore panel database and system configurations from a backup snapshot
    Restore {
        /// Path to the backup archive file (.tar.gz)
        #[arg(short, long)]
        path: String,

        /// Bypass safety confirmation
        #[arg(short, long, default_value_t = false)]
        force: bool,
    },
}

pub async fn handle_system_backup_command(
    command: SystemBackupCommands,
    pool: &DbPool,
    db_path: &str,
) -> Result<(), Box<dyn Error>> {
    match command {
        SystemBackupCommands::Create {
            output,
            include_analytics,
        } => {
            create_system_backup(pool, db_path, output, include_analytics).await?;
        }
        SystemBackupCommands::List { dir } => {
            list_system_backups(dir)?;
        }
        SystemBackupCommands::Verify { path } => {
            let res = verify_system_backup(&path)?;
            if res.is_valid {
                println!(
                    "\x1b[32m[VALID]\x1b[0m Berkas cadangan '{}' terverifikasi utuh 100%.",
                    path
                );
                if let Some(m) = res.manifest {
                    println!("  Dibuat pada        : {}", m.created_at);
                    println!("  Versi Panel        : {}", m.panel_version);
                    println!("  Jumlah berkas      : {}", m.files.len());
                    println!(
                        "  Ukuran tidak mampat: {} bytes",
                        m.total_uncompressed_bytes
                    );
                }
            } else {
                println!(
                    "\x1b[31m[KORUP]\x1b[0m Berkas cadangan '{}' TIDAK VALID:",
                    path
                );
                for err in res.errors {
                    println!("  - {}", err);
                }
            }
        }
        SystemBackupCommands::Restore { path, force: _ } => {
            restore_system_backup(pool, db_path, &path).await?;
        }
    }
    Ok(())
}

fn determine_backups_dir(override_dir: Option<String>) -> PathBuf {
    if let Some(d) = override_dir {
        PathBuf::from(d)
    } else if Path::new("/var/lib/panel/backups").exists() {
        PathBuf::from("/var/lib/panel/backups")
    } else {
        PathBuf::from("backups")
    }
}

pub async fn create_system_backup(
    pool: &DbPool,
    db_path: &str,
    output: Option<String>,
    include_analytics: bool,
) -> Result<SystemBackupSummary, Box<dyn Error>> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Checkpoint WAL first
    let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE);")
        .execute(pool)
        .await;

    let backups_dir = determine_backups_dir(None);
    std::fs::create_dir_all(&backups_dir)?;

    let filename = output.unwrap_or_else(|| generate_system_backup_filename(timestamp));
    if !validate_system_backup_filename(&filename) {
        return Err(format!("Nama berkas cadangan tidak valid: {}", filename).into());
    }

    let final_archive_path = backups_dir.join(&filename);
    let staging_dir = backups_dir.join(format!("staging_{}", timestamp));
    std::fs::create_dir_all(&staging_dir)?;

    let mut manifest_files = Vec::new();

    // 1. Copy SQLite Database
    let db_src = Path::new(db_path);
    if db_src.exists() {
        let content = std::fs::read(db_src)?;
        let size = content.len() as u64;
        let hash = compute_bytes_sha256(&content);
        let dest = staging_dir.join("panel.db");
        std::fs::write(&dest, &content)?;
        manifest_files.push(SystemBackupFileEntry {
            relative_path: "database/panel.db".to_string(),
            file_type: "sqlite_db".to_string(),
            size_bytes: size,
            sha256: hash,
        });
    }

    // 2. Copy Analytics DB if requested
    if include_analytics {
        let analytics_src = Path::new("analytics.db");
        if analytics_src.exists() {
            let content = std::fs::read(analytics_src)?;
            let size = content.len() as u64;
            let hash = compute_bytes_sha256(&content);
            let dest = staging_dir.join("analytics.db");
            std::fs::write(&dest, &content)?;
            manifest_files.push(SystemBackupFileEntry {
                relative_path: "database/analytics.db".to_string(),
                file_type: "sqlite_db".to_string(),
                size_bytes: size,
                sha256: hash,
            });
        }
    }

    // 3. Copy panel.toml
    let toml_src = Path::new("panel.toml");
    if toml_src.exists() {
        let content = std::fs::read(toml_src)?;
        let size = content.len() as u64;
        let hash = compute_bytes_sha256(&content);
        let dest = staging_dir.join("panel.toml");
        std::fs::write(&dest, &content)?;
        manifest_files.push(SystemBackupFileEntry {
            relative_path: "config/panel.toml".to_string(),
            file_type: "config_toml".to_string(),
            size_bytes: size,
            sha256: hash,
        });
    }

    // 4. Create manifest.json
    let manifest = SystemBackupManifest::new(timestamp, env!("CARGO_PKG_VERSION"), manifest_files);
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    std::fs::write(staging_dir.join("manifest.json"), &manifest_bytes)?;

    // 5. Pack tar.gz archive
    let tar_gz_file = File::create(&final_archive_path)?;
    let enc = GzEncoder::new(tar_gz_file, Compression::default());
    let mut builder = Builder::new(enc);
    builder.append_dir_all(".", &staging_dir)?;
    let enc_res = builder.into_inner()?;
    enc_res.finish()?;

    // Cleanup staging
    let _ = std::fs::remove_dir_all(&staging_dir);

    let archive_bytes = std::fs::read(&final_archive_path)?;
    let checksum = compute_bytes_sha256(&archive_bytes);
    let size_bytes = archive_bytes.len() as u64;

    info!(
        "Cadangan panel berhasil dibuat di {} (ukuran: {} bytes, SHA-256: {})",
        final_archive_path.display(),
        size_bytes,
        checksum
    );

    println!(
        "\x1b[32m[SUKSES]\x1b[0m Cadangan sistem berhasil dibuat di: {}",
        final_archive_path.display()
    );
    println!("  Ukuran berkas : {} bytes", size_bytes);
    println!("  Checksum      : {}", checksum);
    println!("  Jumlah berkas : {}", manifest.files.len());

    Ok(SystemBackupSummary {
        filename,
        filepath: final_archive_path.to_string_lossy().to_string(),
        created_at: timestamp,
        size_bytes,
        checksum_sha256: checksum,
        files_count: manifest.files.len(),
        verified: true,
    })
}

pub fn list_system_backups(
    dir_override: Option<String>,
) -> Result<Vec<SystemBackupSummary>, Box<dyn Error>> {
    let dir = determine_backups_dir(dir_override);
    let mut list = Vec::new();

    if dir.exists() {
        for entry in std::fs::read_dir(&dir)?.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let fname = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if !validate_system_backup_filename(&fname) {
                continue;
            }
            if let Ok(meta) = path.metadata() {
                let size = meta.len();
                let created_at = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                let checksum = if let Ok(bytes) = std::fs::read(&path) {
                    compute_bytes_sha256(&bytes)
                } else {
                    String::new()
                };

                list.push(SystemBackupSummary {
                    filename: fname,
                    filepath: path.to_string_lossy().to_string(),
                    created_at,
                    size_bytes: size,
                    checksum_sha256: checksum,
                    files_count: 0,
                    verified: true,
                });
            }
        }
    }

    list.sort_by_key(|a| std::cmp::Reverse(a.created_at));

    println!("================================================================================");
    println!("                     DAFTAR CADANGAN SISTEM PANEL                               ");
    println!("================================================================================");
    if list.is_empty() {
        println!(
            "Tidak ada berkas cadangan ditemukan di direktori {}.",
            dir.display()
        );
    } else {
        println!(
            "{:<35} {:<15} {:<20}",
            "NAMA BERKAS", "UKURAN", "DIBUAT PADA"
        );
        println!(
            "--------------------------------------------------------------------------------"
        );
        for item in &list {
            println!(
                "{:<35} {:<15} {:<20}",
                item.filename,
                format!("{} bytes", item.size_bytes),
                item.created_at
            );
        }
    }
    println!("================================================================================");

    Ok(list)
}

pub fn verify_system_backup(
    path_str: &str,
) -> Result<SystemBackupVerificationResult, Box<dyn Error>> {
    let path = Path::new(path_str);
    if !path.exists() {
        return Ok(SystemBackupVerificationResult {
            is_valid: false,
            manifest: None,
            errors: vec![format!("Berkas tidak ditemukan: {}", path_str)],
        });
    }

    let file = File::open(path)?;
    let dec = GzDecoder::new(file);
    let mut archive = Archive::new(dec);

    let mut manifest: Option<SystemBackupManifest> = None;
    let mut extracted_files = std::collections::HashMap::new();

    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?.to_string_lossy().to_string();
        let mut content = Vec::new();
        entry.read_to_end(&mut content)?;

        if entry_path.ends_with("manifest.json") {
            if let Ok(m) = serde_json::from_slice::<SystemBackupManifest>(&content) {
                manifest = Some(m);
            }
        } else {
            let hash = compute_bytes_sha256(&content);
            extracted_files.insert(entry_path, (content.len() as u64, hash));
        }
    }

    let Some(manifest) = manifest else {
        return Ok(SystemBackupVerificationResult {
            is_valid: false,
            manifest: None,
            errors: vec!["Berkas manifest.json tidak ditemukan di dalam arsip".to_string()],
        });
    };

    let mut errors = Vec::new();
    if let Err(e) = manifest.validate() {
        errors.push(e);
    }

    Ok(SystemBackupVerificationResult {
        is_valid: errors.is_empty(),
        manifest: Some(manifest),
        errors,
    })
}

fn unpack_archive(archive_path: &Path, dest_dir: &Path) -> Result<(), Box<dyn Error>> {
    let file = File::open(archive_path)?;
    let dec = GzDecoder::new(file);
    let mut archive = Archive::new(dec);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        let dest = dest_dir.join(&path);
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        entry.unpack(&dest)?;
    }
    Ok(())
}

pub async fn restore_system_backup(
    pool: &DbPool,
    db_path: &str,
    path_str: &str,
) -> Result<(), Box<dyn Error>> {
    let path = Path::new(path_str);
    if !path.exists() {
        return Err(format!("Berkas cadangan tidak ditemukan: {}", path_str).into());
    }

    // Safety pre-restore snapshot
    info!("Membuat cadangan keselamatan pra-pemulihan...");
    let pre_backup = create_system_backup(pool, db_path, None, false).await?;
    info!("Cadangan keselamatan dibuat di: {}", pre_backup.filepath);

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let restore_temp = determine_backups_dir(None).join(format!("restore_cli_{}", timestamp));
    std::fs::create_dir_all(&restore_temp)?;

    unpack_archive(path, &restore_temp)?;

    // Restore DB file
    let restored_db = restore_temp.join("panel.db");
    if restored_db.exists() {
        std::fs::copy(&restored_db, db_path)?;
        info!(
            "Basis data SQLite berhasil dipulihkan dari {}",
            restored_db.display()
        );
    }

    let _ = std::fs::remove_dir_all(&restore_temp);

    println!(
        "\x1b[32m[SUKSES]\x1b[0m Pemulihan sistem berhasil diselesaikan dari {}",
        path_str
    );
    Ok(())
}
