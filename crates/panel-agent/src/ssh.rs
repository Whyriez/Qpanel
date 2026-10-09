//! SshManager: Manages host-level root-owned authorized_keys files for Linux tenants.
//!
//! Rule 4.5 / P6-T5: "file dikelola panel di lokasi root-owned"
//! Location: /etc/panel/ssh/authorized_keys/<linux_username> (mode 0644, root:root)
//!
//! Because the authorized_keys directory is owned by root with mode 0755 and individual files
//! are mode 0644 root:root, tenants cannot inject arbitrary options, tamper with, or delete
//! their authorized_keys files.

use crate::AgentError;
use crate::system::SystemRunner;
use panel_core::Username;
use panel_proto::{SshKeysSyncRequest, SshKeysSyncResponse};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::info;

pub const DEFAULT_SSH_KEYS_DIR: &str = "/etc/panel/ssh/authorized_keys";

#[derive(Clone)]
pub struct SshManager {
    _runner: Arc<dyn SystemRunner>,
    base_dir: PathBuf,
}

impl SshManager {
    pub fn new(runner: Arc<dyn SystemRunner>) -> Self {
        Self {
            _runner: runner,
            base_dir: PathBuf::from(DEFAULT_SSH_KEYS_DIR),
        }
    }

    pub fn with_base_dir(runner: Arc<dyn SystemRunner>, base_dir: impl Into<PathBuf>) -> Self {
        Self {
            _runner: runner,
            base_dir: base_dir.into(),
        }
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Atomically syncs authorized_keys for a given tenant linux user.
    pub fn sync_authorized_keys(
        &self,
        req: &SshKeysSyncRequest,
    ) -> Result<SshKeysSyncResponse, AgentError> {
        // Validate linux username strictly
        let validated_user = Username::parse(&req.linux_username)
            .map_err(|e| AgentError::InvalidInput(format!("Invalid linux username: {}", e)))?;
        let username_str = validated_user.as_str();

        // Validate that keys do not contain internal newline characters
        for key in &req.keys {
            if key.contains('\n') || key.contains('\r') {
                return Err(AgentError::InvalidInput(
                    "SSH key entry contains newline characters".to_string(),
                ));
            }
        }

        // Ensure base directory exists
        if !self.base_dir.exists() {
            fs::create_dir_all(&self.base_dir)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&self.base_dir, fs::Permissions::from_mode(0o755));
            }
        }

        let target_path = self.base_dir.join(username_str);
        let temp_path =
            self.base_dir
                .join(format!("{}.tmp.{}", username_str, uuid::Uuid::new_v4()));

        // Write content atomically: temp file -> sync -> set permissions -> rename
        {
            let mut file = fs::File::create(&temp_path)?;
            for key in &req.keys {
                let trimmed = key.trim();
                if !trimmed.is_empty() {
                    writeln!(file, "{}", trimmed)?;
                }
            }
            file.flush()?;
            file.sync_all()?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o644));
        }

        fs::rename(&temp_path, &target_path)?;

        info!(
            "Synchronized {} SSH public key(s) to root-owned file {} for tenant {}",
            req.keys.len(),
            target_path.display(),
            username_str
        );

        Ok(SshKeysSyncResponse {
            success: true,
            path: target_path.display().to_string(),
            count: req.keys.len(),
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::system::MockSystemRunner;
    use tempfile::tempdir;

    #[test]
    fn test_sync_authorized_keys_success() {
        let tmp = tempdir().expect("tempdir");
        let runner = Arc::new(MockSystemRunner::new());
        let manager = SshManager::with_base_dir(runner, tmp.path());

        let req = SshKeysSyncRequest {
            linux_username: "p_alice".to_string(),
            keys: vec![
                "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl key1".to_string(),
                "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl key2".to_string(),
            ],
        };

        let res = manager.sync_authorized_keys(&req).expect("sync keys");
        assert!(res.success);
        assert_eq!(res.count, 2);

        let target_file = tmp.path().join("p_alice");
        assert!(target_file.exists());
        let content = fs::read_to_string(&target_file).expect("read keys");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], req.keys[0]);
        assert_eq!(lines[1], req.keys[1]);
    }

    #[test]
    fn test_sync_empty_keys_clears_file() {
        let tmp = tempdir().expect("tempdir");
        let runner = Arc::new(MockSystemRunner::new());
        let manager = SshManager::with_base_dir(runner, tmp.path());

        // First write one key
        let req1 = SshKeysSyncRequest {
            linux_username: "p_bob".to_string(),
            keys: vec!["ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl key1".to_string()],
        };
        manager.sync_authorized_keys(&req1).expect("sync key");

        // Now sync empty keys list
        let req_empty = SshKeysSyncRequest {
            linux_username: "p_bob".to_string(),
            keys: vec![],
        };
        let res = manager
            .sync_authorized_keys(&req_empty)
            .expect("sync empty");
        assert_eq!(res.count, 0);

        let target_file = tmp.path().join("p_bob");
        let content = fs::read_to_string(&target_file).expect("read keys");
        assert!(content.trim().is_empty());
    }

    #[test]
    fn test_reject_invalid_username_or_path_traversal() {
        let tmp = tempdir().expect("tempdir");
        let runner = Arc::new(MockSystemRunner::new());
        let manager = SshManager::with_base_dir(runner, tmp.path());

        let req_traversal = SshKeysSyncRequest {
            linux_username: "../p_root".to_string(),
            keys: vec!["ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI... key".to_string()],
        };
        assert!(manager.sync_authorized_keys(&req_traversal).is_err());

        let req_invalid_prefix = SshKeysSyncRequest {
            linux_username: "alice".to_string(),
            keys: vec!["ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI... key".to_string()],
        };
        assert!(manager.sync_authorized_keys(&req_invalid_prefix).is_err());
    }

    #[test]
    fn test_reject_newline_injection() {
        let tmp = tempdir().expect("tempdir");
        let runner = Arc::new(MockSystemRunner::new());
        let manager = SshManager::with_base_dir(runner, tmp.path());

        let req_injection = SshKeysSyncRequest {
            linux_username: "p_charlie".to_string(),
            keys: vec![
                "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI...\ncommand=\"rm -rf /\" ssh-rsa ..."
                    .to_string(),
            ],
        };
        assert!(manager.sync_authorized_keys(&req_injection).is_err());
    }
}
