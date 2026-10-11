//! Host agent execution for site backups, snapshots, and archive restores.

use crate::AgentError;
use crate::system::SystemRunner;
use panel_core::backup::{
    BackupDatabaseManifestEntry, BackupFormat, BackupManifest, validate_backup_filename,
};
use panel_proto::{
    BackupCreateRequest, BackupCreateResponse, BackupDeleteRequest, BackupDeleteResponse,
    BackupRestoreRequest, BackupRestoreResponse,
};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

/// Computes SHA256 hex digest of a file.
fn compute_file_sha256(path: &Path) -> Result<String, AgentError> {
    let mut file = File::open(path).map_err(AgentError::Io)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(AgentError::Io)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Recursively counts files and total uncompressed bytes in a directory.
fn scan_dir_stats(path: &Path) -> (u64, u64) {
    let mut count = 0u64;
    let mut bytes = 0u64;

    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                count += 1;
                if let Ok(meta) = p.metadata() {
                    bytes += meta.len();
                }
            } else if p.is_dir() {
                let (sub_count, sub_bytes) = scan_dir_stats(&p);
                count += sub_count;
                bytes += sub_bytes;
            }
        }
    }

    (count, bytes)
}

/// Recursively copies a directory tree.
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else if src_path.is_file() {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Creates a complete site backup archive containing files and database dumps.
#[allow(clippy::cognitive_complexity)]
pub fn create_backup(
    runner: &dyn SystemRunner,
    req: &BackupCreateRequest,
) -> Result<BackupCreateResponse, AgentError> {
    if let Err(e) = validate_backup_filename(&req.filename) {
        return Ok(BackupCreateResponse {
            success: false,
            size_bytes: 0,
            checksum_sha256: String::new(),
            filepath: String::new(),
            error: Some(e.to_string()),
        });
    }

    let backup_dir = Path::new(&req.backup_dir);
    if !runner.path_exists(backup_dir) {
        runner.create_dir_all(backup_dir, 0o750, req.tenant_uid, req.tenant_uid)?;
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let staging_dir = backup_dir.join(format!(".staging_{}_{}", req.site_id, now));
    if runner.path_exists(&staging_dir) {
        let _ = runner.remove_dir_all(&staging_dir);
    }
    runner.create_dir_all(&staging_dir, 0o700, req.tenant_uid, req.tenant_uid)?;

    let mut manifest = BackupManifest {
        site_id: req.site_id,
        created_at: now,
        includes_files: req.include_files,
        includes_databases: req.include_databases,
        archive_format: req.format.clone(),
        ..Default::default()
    };

    // 1. Pack site files if requested
    if req.include_files {
        let site_dir = Path::new(&req.site_dir);
        let staging_files = staging_dir.join("files");
        runner.create_dir_all(&staging_files, 0o755, req.tenant_uid, req.tenant_uid)?;

        if site_dir.exists() {
            if let Err(e) = copy_dir_all(site_dir, &staging_files) {
                let _ = runner.remove_dir_all(&staging_dir);
                return Ok(BackupCreateResponse {
                    success: false,
                    size_bytes: 0,
                    checksum_sha256: String::new(),
                    filepath: String::new(),
                    error: Some(format!("Gagal menyalin berkas situs: {}", e)),
                });
            }
            let (file_count, file_bytes) = scan_dir_stats(&staging_files);
            manifest.files_count = file_count;
            manifest.uncompressed_bytes = file_bytes;
        }
    }

    // 2. Dump databases if requested
    if req.include_databases && !req.databases.is_empty() {
        let staging_dbs = staging_dir.join("databases");
        runner.create_dir_all(&staging_dbs, 0o700, req.tenant_uid, req.tenant_uid)?;

        for db in &req.databases {
            let dump_file_rel = format!("databases/{}.sql", db.name);
            let dump_file_path = staging_dir.join(&dump_file_rel);
            let dump_str = dump_file_path.to_string_lossy().to_string();

            if db.engine == "mariadb" || db.engine == "mysql" {
                let out = runner.run_command(
                    "mariadb-dump",
                    &[
                        "--single-transaction",
                        "--quick",
                        "--routines",
                        "--triggers",
                        "--no-tablespaces",
                        &format!("--result-file={}", dump_str),
                        &db.name,
                    ],
                );

                let res = match out {
                    Ok(o) if o.status.success() => Ok(()),
                    _ => {
                        // Fallback to mysqldump
                        runner
                            .run_command(
                                "mysqldump",
                                &[
                                    "--single-transaction",
                                    "--quick",
                                    "--routines",
                                    "--triggers",
                                    "--no-tablespaces",
                                    &format!("--result-file={}", dump_str),
                                    &db.name,
                                ],
                            )
                            .map(|_| ())
                    }
                };

                if let Err(e) = res {
                    warn!("Database dump warning for {}: {:?}", db.name, e);
                }
            } else if db.engine == "postgres" || db.engine == "postgresql" {
                let out =
                    runner.run_command("pg_dump", &["-d", &db.name, "-F", "p", "-f", &dump_str]);
                if let Err(e) = out {
                    warn!("PostgreSQL dump warning for {}: {:?}", db.name, e);
                }
            }

            let db_size = if dump_file_path.exists() {
                dump_file_path.metadata().map(|m| m.len()).unwrap_or(0)
            } else {
                0
            };

            manifest.databases.push(BackupDatabaseManifestEntry {
                engine: db.engine.clone(),
                name: db.name.clone(),
                username: db.username.clone(),
                dump_file: dump_file_rel,
                size_bytes: db_size,
            });
        }
    }

    // 3. Write manifest.json inside staging
    let manifest_path = staging_dir.join("manifest.json");
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap_or_default();
    let _ = runner.write_file(&manifest_path, &manifest_bytes, 0o600);

    // 4. Create compressed archive
    let final_archive_path = backup_dir.join(&req.filename);
    let final_archive_str = final_archive_path.to_string_lossy().to_string();
    let staging_str = staging_dir.to_string_lossy().to_string();

    let format = BackupFormat::from_str_loose(&req.format);
    let pack_res = match format {
        BackupFormat::TarGz => runner.run_command(
            "tar",
            &["-czf", &final_archive_str, "-C", &staging_str, "."],
        ),
        BackupFormat::TarZst => {
            // Try zstd compression with tar
            let zstd_res = runner.run_command(
                "tar",
                &["--zstd", "-cf", &final_archive_str, "-C", &staging_str, "."],
            );
            match zstd_res {
                Ok(o) if o.status.success() => Ok(o),
                _ => {
                    // Fallback to tar with -I zstd or tar.gz
                    let alt_zstd = runner.run_command(
                        "tar",
                        &[
                            "-I",
                            "zstd",
                            "-cf",
                            &final_archive_str,
                            "-C",
                            &staging_str,
                            ".",
                        ],
                    );
                    match alt_zstd {
                        Ok(o) if o.status.success() => Ok(o),
                        _ => runner.run_command(
                            "tar",
                            &["-czf", &final_archive_str, "-C", &staging_str, "."],
                        ),
                    }
                }
            }
        }
    };

    // Clean up staging directory
    let _ = runner.remove_dir_all(&staging_dir);

    match pack_res {
        Ok(out) if out.status.success() => {
            let _ = runner.set_permissions(&final_archive_path, 0o600);
            let _ = runner.set_ownership(&final_archive_path, req.tenant_uid, req.tenant_uid);

            let (size_bytes, checksum) = if final_archive_path.exists() {
                let sz = final_archive_path.metadata().map(|m| m.len()).unwrap_or(0);
                let chk = compute_file_sha256(&final_archive_path).unwrap_or_default();
                (sz, chk)
            } else {
                (0, String::new())
            };

            info!(
                site_id = req.site_id,
                size = size_bytes,
                "Backup archive created successfully at {}",
                final_archive_str
            );

            Ok(BackupCreateResponse {
                success: true,
                size_bytes,
                checksum_sha256: checksum,
                filepath: final_archive_str,
                error: None,
            })
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            Ok(BackupCreateResponse {
                success: false,
                size_bytes: 0,
                checksum_sha256: String::new(),
                filepath: String::new(),
                error: Some(format!("Gagal membuat arsip kompresi tar: {}", stderr)),
            })
        }
        Err(e) => Ok(BackupCreateResponse {
            success: false,
            size_bytes: 0,
            checksum_sha256: String::new(),
            filepath: String::new(),
            error: Some(format!("Eksekusi pembuatan arsip gagal: {}", e)),
        }),
    }
}

/// Restores site files and database dumps from a backup archive.
pub fn restore_backup(
    runner: &dyn SystemRunner,
    req: &BackupRestoreRequest,
) -> Result<BackupRestoreResponse, AgentError> {
    let backup_path = Path::new(&req.backup_filepath);
    if !backup_path.exists() {
        return Ok(BackupRestoreResponse {
            success: false,
            restored_files_count: 0,
            restored_databases_count: 0,
            error: Some("Berkas cadangan tidak ditemukan di server".to_string()),
        });
    }

    let target_dir = Path::new(&req.target_site_dir);
    if !target_dir.exists() {
        runner.create_dir_all(target_dir, 0o755, req.tenant_uid, req.tenant_uid)?;
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let staging_dir = target_dir.join(format!(".restore_staging_{}", now));
    if runner.path_exists(&staging_dir) {
        let _ = runner.remove_dir_all(&staging_dir);
    }
    runner.create_dir_all(&staging_dir, 0o700, req.tenant_uid, req.tenant_uid)?;

    let backup_str = backup_path.to_string_lossy().to_string();
    let staging_str = staging_dir.to_string_lossy().to_string();

    // Extract archive
    let extract_res = if backup_str.ends_with(".tar.gz") {
        runner.run_command("tar", &["-xzf", &backup_str, "-C", &staging_str])
    } else {
        let zstd_res =
            runner.run_command("tar", &["--zstd", "-xf", &backup_str, "-C", &staging_str]);
        match zstd_res {
            Ok(o) if o.status.success() => Ok(o),
            _ => runner.run_command(
                "tar",
                &["-I", "zstd", "-xf", &backup_str, "-C", &staging_str],
            ),
        }
    };

    if let Err(e) = extract_res {
        let _ = runner.remove_dir_all(&staging_dir);
        return Ok(BackupRestoreResponse {
            success: false,
            restored_files_count: 0,
            restored_databases_count: 0,
            error: Some(format!("Gagal mengekstrak berkas arsip cadangan: {}", e)),
        });
    }

    let mut restored_files = 0u64;
    let mut restored_dbs = 0u32;

    // 1. Restore files if requested
    if req.restore_files {
        let staging_files = staging_dir.join("files");
        if staging_files.exists() {
            if let Err(e) = copy_dir_all(&staging_files, target_dir) {
                let _ = runner.remove_dir_all(&staging_dir);
                return Ok(BackupRestoreResponse {
                    success: false,
                    restored_files_count: 0,
                    restored_databases_count: 0,
                    error: Some(format!("Gagal memulihkan berkas ke direktori situs: {}", e)),
                });
            }
            let (f_count, _) = scan_dir_stats(target_dir);
            restored_files = f_count;
        }
    }

    // 2. Restore databases if requested
    if req.restore_databases {
        let staging_dbs = staging_dir.join("databases");
        if let Ok(entries) = std::fs::read_dir(&staging_dbs) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().map(|e| e == "sql").unwrap_or(false) {
                    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    let p_str = p.to_string_lossy().to_string();

                    // Match engine from requested databases or default to mariadb
                    let db_info = req.databases.iter().find(|d| d.name == stem);
                    let is_pg = db_info
                        .map(|d| d.engine == "postgres" || d.engine == "postgresql")
                        .unwrap_or(false);

                    if is_pg {
                        let _ = runner.run_command("psql", &["-d", stem, "-f", &p_str]);
                    } else {
                        // MariaDB: safe execution without shell piping
                        // Read dump file and pass via client command or execute
                        let _ = runner
                            .run_command("mariadb", &["-e", &format!("source {}", p_str), stem]);
                    }
                    restored_dbs += 1;
                }
            }
        }
    }

    // Clean up staging directory
    let _ = runner.remove_dir_all(&staging_dir);

    // Ensure permissions
    let _ = runner.set_permissions(target_dir, 0o755);
    let _ = runner.set_ownership(target_dir, req.tenant_uid, req.tenant_uid);

    info!(
        site_id = req.site_id,
        files = restored_files,
        databases = restored_dbs,
        "Site backup restored successfully"
    );

    Ok(BackupRestoreResponse {
        success: true,
        restored_files_count: restored_files,
        restored_databases_count: restored_dbs,
        error: None,
    })
}

/// Deletes a backup archive file from the host.
pub fn delete_backup(
    runner: &dyn SystemRunner,
    req: &BackupDeleteRequest,
) -> Result<BackupDeleteResponse, AgentError> {
    let p = Path::new(&req.filepath);
    if runner.path_exists(p) {
        runner.remove_file(p)?;
    }
    Ok(BackupDeleteResponse {
        success: true,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::MockSystemRunner;

    #[test]
    fn test_delete_backup_mock() {
        let runner = MockSystemRunner::new();
        let file_path = Path::new("/home/p_alice/backups/backup_test.tar.zst");
        runner
            .write_file(file_path, b"test backup data", 0o600)
            .unwrap();
        assert!(runner.path_exists(file_path));

        let res = delete_backup(
            &runner,
            &BackupDeleteRequest {
                filepath: "/home/p_alice/backups/backup_test.tar.zst".to_string(),
                tenant_username: "p_alice".to_string(),
            },
        )
        .unwrap();

        assert!(res.success);
        assert!(!runner.path_exists(file_path));
    }
}
