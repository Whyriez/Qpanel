//! Linux tenant user lifecycle management.
//!
//! Rule 4.3: Privilege separation and command execution rules.
//! Rule 4.4: 1 tenant = 1 user Linux `p_<slug>` (UID 10000–59999), without login password,
//!           home 0700 + ACL u:<nginx-user>:--x, subdirectories, group panel-tenants.
//! Rule 4: No string interpolation for shell commands, allowlisted binaries only.

use crate::AgentError;
use panel_core::Username;
use panel_core::tenant::TenantUid;
use panel_proto::{
    UserCreateRequest, UserCreatedResponse, UserDeleteRequest, UserDeletedResponse,
    UserSuspendRequest, UserSuspendedResponse, UserUnsuspendRequest, UserUnsuspendedResponse,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

pub const ALLOWED_BINARIES: &[&str] = &[
    "useradd", "usermod", "userdel", "groupadd", "groupdel", "setfacl", "pkill",
];

pub const ALLOWED_SHELLS: &[&str] = &[
    "/usr/sbin/nologin",
    "/bin/false",
    "/usr/lib/openssh/sftp-server",
    "/bin/bash",
    "/bin/sh",
];

/// Trait abstracting underlying operating system commands and filesystem operations.
pub trait SystemRunner: Send + Sync {
    fn run_command(&self, binary: &str, args: &[&str]) -> Result<std::process::Output, AgentError>;
    fn create_dir_all(&self, path: &Path, mode: u32, uid: u32, gid: u32) -> Result<(), AgentError>;
    fn set_permissions(&self, path: &Path, mode: u32) -> Result<(), AgentError>;
    fn set_ownership(&self, path: &Path, uid: u32, gid: u32) -> Result<(), AgentError>;
    fn set_facl(&self, path: &Path, user: &str, permissions: &str) -> Result<(), AgentError>;
    fn move_path(&self, from: &Path, to: &Path) -> Result<(), AgentError>;
    fn remove_dir_all(&self, path: &Path) -> Result<(), AgentError>;
    fn path_exists(&self, path: &Path) -> bool;
}

/// Real system runner executing actual Linux system binaries and system calls.
pub struct RealSystemRunner;

impl SystemRunner for RealSystemRunner {
    fn run_command(&self, binary: &str, args: &[&str]) -> Result<std::process::Output, AgentError> {
        if !ALLOWED_BINARIES.contains(&binary) {
            return Err(AgentError::SecurityViolation(format!(
                "Binary '{}' is not in the allowed binaries list",
                binary
            )));
        }

        let output = std::process::Command::new(binary)
            .args(args)
            .output()
            .map_err(AgentError::Io)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            // Allow group already exists or process kill non-zero if no processes matched
            let is_ignorable = (binary == "groupadd" && stderr.contains("already exists"))
                || (binary == "groupdel" && stderr.contains("does not exist"))
                || (binary == "pkill" && output.status.code() == Some(1)); // code 1 = no process matched

            if !is_ignorable {
                return Err(AgentError::CommandFailed {
                    binary: binary.to_string(),
                    code: output.status.code(),
                    stderr,
                });
            }
        }

        Ok(output)
    }

    fn create_dir_all(&self, path: &Path, mode: u32, uid: u32, gid: u32) -> Result<(), AgentError> {
        std::fs::create_dir_all(path)?;
        self.set_permissions(path, mode)?;
        self.set_ownership(path, uid, gid)?;
        Ok(())
    }

    fn set_permissions(&self, path: &Path, mode: u32) -> Result<(), AgentError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
        }
        #[cfg(not(unix))]
        {
            let _ = (path, mode);
        }
        Ok(())
    }

    fn set_ownership(&self, path: &Path, uid: u32, gid: u32) -> Result<(), AgentError> {
        #[cfg(unix)]
        {
            std::os::unix::fs::chown(path, Some(uid), Some(gid))?;
        }
        #[cfg(not(unix))]
        {
            let _ = (path, uid, gid);
        }
        Ok(())
    }

    fn set_facl(&self, path: &Path, user: &str, permissions: &str) -> Result<(), AgentError> {
        let path_str = path.to_str().ok_or_else(|| {
            AgentError::InvalidInput(format!("Path contains invalid UTF-8: {:?}", path))
        })?;
        let acl_spec = format!("u:{user}:{permissions}");
        match self.run_command("setfacl", &["-m", &acl_spec, path_str]) {
            Ok(_) => Ok(()),
            Err(e) => {
                warn!(
                    "setfacl failed on {}: {}. Continuing without ACL.",
                    path_str, e
                );
                Ok(())
            }
        }
    }

    fn move_path(&self, from: &Path, to: &Path) -> Result<(), AgentError> {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(from, to)?;
        Ok(())
    }

    fn remove_dir_all(&self, path: &Path) -> Result<(), AgentError> {
        if path.exists() {
            std::fs::remove_dir_all(path)?;
        }
        Ok(())
    }

    fn path_exists(&self, path: &Path) -> bool {
        path.exists()
    }
}

/// Record of an action executed on the mock runner for testing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockAction {
    Command {
        binary: String,
        args: Vec<String>,
    },
    CreateDir {
        path: PathBuf,
        mode: u32,
        uid: u32,
        gid: u32,
    },
    SetPermissions {
        path: PathBuf,
        mode: u32,
    },
    SetOwnership {
        path: PathBuf,
        uid: u32,
        gid: u32,
    },
    SetFacl {
        path: PathBuf,
        user: String,
        permissions: String,
    },
    MovePath {
        from: PathBuf,
        to: PathBuf,
    },
    RemoveDir {
        path: PathBuf,
    },
}

/// In-memory mock runner for test assertions and safe non-root validation.
#[derive(Debug, Clone, Default)]
pub struct MockSystemRunner {
    pub actions: Arc<Mutex<Vec<MockAction>>>,
}

impl MockSystemRunner {
    pub fn new() -> Self {
        Self {
            actions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn recorded_actions(&self) -> Vec<MockAction> {
        self.actions.lock().map(|l| l.clone()).unwrap_or_default()
    }
}

impl SystemRunner for MockSystemRunner {
    fn run_command(&self, binary: &str, args: &[&str]) -> Result<std::process::Output, AgentError> {
        if !ALLOWED_BINARIES.contains(&binary) {
            return Err(AgentError::SecurityViolation(format!(
                "Binary '{}' is not in the allowed binaries list",
                binary
            )));
        }

        if let Ok(mut lock) = self.actions.lock() {
            lock.push(MockAction::Command {
                binary: binary.to_string(),
                args: args.iter().map(|s| s.to_string()).collect(),
            });
        }

        #[cfg(unix)]
        use std::os::unix::process::ExitStatusExt;
        #[cfg(unix)]
        let status = std::process::ExitStatus::from_raw(0);
        #[cfg(not(unix))]
        let status = {
            // Dummy successful command to acquire a zero ExitStatus
            std::process::Command::new("cmd")
                .args(["/c", "exit 0"])
                .status()
                .unwrap_or_else(|_| panic!("Failed to get dummy ExitStatus"))
        };

        Ok(std::process::Output {
            status,
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }

    fn create_dir_all(&self, path: &Path, mode: u32, uid: u32, gid: u32) -> Result<(), AgentError> {
        if let Ok(mut lock) = self.actions.lock() {
            lock.push(MockAction::CreateDir {
                path: path.to_path_buf(),
                mode,
                uid,
                gid,
            });
        }
        Ok(())
    }

    fn set_permissions(&self, path: &Path, mode: u32) -> Result<(), AgentError> {
        if let Ok(mut lock) = self.actions.lock() {
            lock.push(MockAction::SetPermissions {
                path: path.to_path_buf(),
                mode,
            });
        }
        Ok(())
    }

    fn set_ownership(&self, path: &Path, uid: u32, gid: u32) -> Result<(), AgentError> {
        if let Ok(mut lock) = self.actions.lock() {
            lock.push(MockAction::SetOwnership {
                path: path.to_path_buf(),
                uid,
                gid,
            });
        }
        Ok(())
    }

    fn set_facl(&self, path: &Path, user: &str, permissions: &str) -> Result<(), AgentError> {
        if let Ok(mut lock) = self.actions.lock() {
            lock.push(MockAction::SetFacl {
                path: path.to_path_buf(),
                user: user.to_string(),
                permissions: permissions.to_string(),
            });
        }
        Ok(())
    }

    fn move_path(&self, from: &Path, to: &Path) -> Result<(), AgentError> {
        if let Ok(mut lock) = self.actions.lock() {
            lock.push(MockAction::MovePath {
                from: from.to_path_buf(),
                to: to.to_path_buf(),
            });
        }
        Ok(())
    }

    fn remove_dir_all(&self, path: &Path) -> Result<(), AgentError> {
        if let Ok(mut lock) = self.actions.lock() {
            lock.push(MockAction::RemoveDir {
                path: path.to_path_buf(),
            });
        }
        Ok(())
    }

    fn path_exists(&self, _path: &Path) -> bool {
        true
    }
}

/// Lifecycle manager handling tenant users on the host system.
pub struct UserLifecycleManager {
    runner: Arc<dyn SystemRunner>,
}

impl UserLifecycleManager {
    pub fn new(runner: Arc<dyn SystemRunner>) -> Self {
        Self { runner }
    }

    /// Creates a new isolated Linux tenant user according to PRD 4.4 and Task P2-T1.
    pub fn create_user(&self, req: UserCreateRequest) -> Result<UserCreatedResponse, AgentError> {
        // 1. Rigorous input re-validation on the agent side
        let username =
            Username::parse(&req.username).map_err(|e| AgentError::InvalidInput(e.to_string()))?;
        let uid = TenantUid::parse(req.uid).map_err(|e| AgentError::InvalidInput(e.to_string()))?;

        let shell = req.shell.as_deref().unwrap_or("/usr/sbin/nologin");
        if !ALLOWED_SHELLS.contains(&shell) {
            return Err(AgentError::InvalidInput(format!(
                "Shell '{}' is not in the allowed shells list: {:?}",
                shell, ALLOWED_SHELLS
            )));
        }

        // Validate home directory: strictly canonical /home/<username>
        let expected_home = format!("/home/{}", username.as_str());
        let home_dir = req.home_dir.unwrap_or_else(|| expected_home.clone());
        if home_dir != expected_home {
            return Err(AgentError::SecurityViolation(format!(
                "Home directory must match canonical path '{}', got '{}'",
                expected_home, home_dir
            )));
        }

        let web_user = req.web_user.as_deref().unwrap_or("www-data");

        // 2. Ensure system supplemental group `panel-tenants` exists
        let _ = self
            .runner
            .run_command("groupadd", &["--system", "panel-tenants"]);

        // 3. Create private group for user with GID == UID
        let uid_str = uid.to_string();
        self.runner
            .run_command("groupadd", &["--gid", &uid_str, username.as_str()])?;

        // 4. Create user: without login password, disabled home creation by useradd, in group panel-tenants
        self.runner.run_command(
            "useradd",
            &[
                "--uid",
                &uid_str,
                "--gid",
                &uid_str,
                "--home-dir",
                &home_dir,
                "--shell",
                shell,
                "--no-create-home",
                "-G",
                "panel-tenants",
                username.as_str(),
            ],
        )?;

        // Ensure user password is locked
        let _ = self
            .runner
            .run_command("usermod", &["-L", username.as_str()]);

        // 5. Build secure home layout:
        //    Home directory: mode 0700 (owner only)
        //    ACL: u:<nginx-user>:--x on home directory to allow traverse
        let home_path = Path::new(&home_dir);
        let uid_val = uid.as_u32();
        self.runner
            .create_dir_all(home_path, 0o700, uid_val, uid_val)?;

        // Subdirectories: sites (0750), tmp (0700), .ssh (0700), .panel-trash (0700)
        let subdirs = [
            ("sites", 0o750),
            ("tmp", 0o700),
            (".ssh", 0o700),
            (".panel-trash", 0o700),
        ];

        for (subdir, mode) in subdirs {
            let p = home_path.join(subdir);
            self.runner.create_dir_all(&p, mode, uid_val, uid_val)?;
        }

        // Apply ACL on home directory for web server traversal (PRD 4.4)
        self.runner.set_facl(home_path, web_user, "--x")?;

        info!(
            "Tenant user {} created successfully with UID {}, home {}",
            username.as_str(),
            uid.as_u32(),
            home_dir
        );

        Ok(UserCreatedResponse {
            username: username.as_str().to_string(),
            uid: uid.as_u32(),
            home_dir,
        })
    }

    /// Suspends a tenant user: locks shadow entry, sets shell to nologin, and kills active processes.
    pub fn suspend_user(
        &self,
        req: UserSuspendRequest,
    ) -> Result<UserSuspendedResponse, AgentError> {
        let username =
            Username::parse(&req.username).map_err(|e| AgentError::InvalidInput(e.to_string()))?;
        let uid = TenantUid::parse(req.uid).map_err(|e| AgentError::InvalidInput(e.to_string()))?;

        // 1. Lock account and set shell to nologin
        self.runner.run_command(
            "usermod",
            &["-L", "-s", "/usr/sbin/nologin", username.as_str()],
        )?;

        // 2. Kill all tenant processes
        let uid_str = uid.to_string();
        let _ = self.runner.run_command("pkill", &["-KILL", "-u", &uid_str]);

        info!(
            "Tenant user {} (UID {}) suspended",
            username.as_str(),
            uid.as_u32()
        );

        Ok(UserSuspendedResponse {
            username: username.as_str().to_string(),
            uid: uid.as_u32(),
        })
    }

    /// Unsuspends a tenant user: unlocks shadow entry and restores configured shell.
    pub fn unsuspend_user(
        &self,
        req: UserUnsuspendRequest,
    ) -> Result<UserUnsuspendedResponse, AgentError> {
        let username =
            Username::parse(&req.username).map_err(|e| AgentError::InvalidInput(e.to_string()))?;
        let uid = TenantUid::parse(req.uid).map_err(|e| AgentError::InvalidInput(e.to_string()))?;

        let shell = req.shell.as_deref().unwrap_or("/usr/sbin/nologin");
        if !ALLOWED_SHELLS.contains(&shell) {
            return Err(AgentError::InvalidInput(format!(
                "Shell '{}' is not in the allowed shells list: {:?}",
                shell, ALLOWED_SHELLS
            )));
        }

        // Unlock account and restore shell
        self.runner
            .run_command("usermod", &["-U", "-s", shell, username.as_str()])?;

        info!(
            "Tenant user {} (UID {}) unsuspended",
            username.as_str(),
            uid.as_u32()
        );

        Ok(UserUnsuspendedResponse {
            username: username.as_str().to_string(),
            uid: uid.as_u32(),
        })
    }

    /// Deletes a tenant user: either with purge (permanent removal) or retention (archived home).
    pub fn delete_user(&self, req: UserDeleteRequest) -> Result<UserDeletedResponse, AgentError> {
        let username =
            Username::parse(&req.username).map_err(|e| AgentError::InvalidInput(e.to_string()))?;
        let uid = TenantUid::parse(req.uid).map_err(|e| AgentError::InvalidInput(e.to_string()))?;

        let uid_str = uid.to_string();
        // 1. Terminate all remaining processes
        let _ = self.runner.run_command("pkill", &["-KILL", "-u", &uid_str]);

        let home_path = PathBuf::from(format!("/home/{}", username.as_str()));

        let archived_path = if req.purge {
            // Permanent purge: remove home directory completely
            self.runner.remove_dir_all(&home_path)?;
            None
        } else {
            // Retention mode: archive home directory to trash location
            let default_trash = "/var/lib/panel/trash-index".to_string();
            let trash_base = req.trash_dir.unwrap_or(default_trash);
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let dest_name = format!("{}-{}", username.as_str(), now);
            let dest_path = Path::new(&trash_base).join(dest_name);
            self.runner.move_path(&home_path, &dest_path)?;
            Some(dest_path.to_string_lossy().to_string())
        };

        // 2. Remove Linux user
        self.runner.run_command("userdel", &[username.as_str()])?;

        // 3. Remove primary group (if still present)
        let _ = self.runner.run_command("groupdel", &[username.as_str()]);

        info!(
            "Tenant user {} (UID {}) deleted (purged={})",
            username.as_str(),
            uid.as_u32(),
            req.purge
        );

        Ok(UserDeletedResponse {
            username: username.as_str().to_string(),
            uid: uid.as_u32(),
            purged: req.purge,
            archived_path,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_create_user_flow_mock() {
        let runner = MockSystemRunner::new();
        let actions_ref = Arc::clone(&runner.actions);
        let manager = UserLifecycleManager::new(Arc::new(runner));

        let req = UserCreateRequest {
            username: "p_client1".to_string(),
            uid: 10001,
            shell: Some("/usr/sbin/nologin".to_string()),
            home_dir: None,
            web_user: Some("www-data".to_string()),
        };

        let res = manager.create_user(req).expect("create_user success");
        assert_eq!(res.username, "p_client1");
        assert_eq!(res.uid, 10001);
        assert_eq!(res.home_dir, "/home/p_client1");

        let actions = actions_ref.lock().map(|l| l.clone()).unwrap_or_default();
        // Verify groupadd panel-tenants
        assert!(actions.iter().any(|a| matches!(a, MockAction::Command { binary, args } if binary == "groupadd" && args.contains(&"panel-tenants".to_string()))));
        // Verify groupadd p_client1 with gid 10001
        assert!(actions.iter().any(|a| matches!(a, MockAction::Command { binary, args } if binary == "groupadd" && args.contains(&"10001".to_string()) && args.contains(&"p_client1".to_string()))));
        // Verify useradd
        assert!(actions.iter().any(|a| matches!(a, MockAction::Command { binary, args } if binary == "useradd" && args.contains(&"p_client1".to_string()) && args.contains(&"10001".to_string()))));
        // Verify home directory creation with mode 0700
        assert!(actions.iter().any(|a| matches!(a, MockAction::CreateDir { path, mode, .. } if path == Path::new("/home/p_client1") && *mode == 0o700)));
        // Verify subdirectories creation (sites with 0750)
        assert!(actions.iter().any(|a| matches!(a, MockAction::CreateDir { path, mode, .. } if path == Path::new("/home/p_client1/sites") && *mode == 0o750)));
        // Verify setfacl for www-data --x
        assert!(actions.iter().any(|a| matches!(a, MockAction::SetFacl { user, permissions, .. } if user == "www-data" && permissions == "--x")));
    }

    #[test]
    fn test_create_user_rejects_invalid_username_and_uid() {
        let runner = MockSystemRunner::new();
        let manager = UserLifecycleManager::new(Arc::new(runner));

        // Invalid username (reserved)
        let err1 = manager.create_user(UserCreateRequest {
            username: "p_root".to_string(),
            uid: 10001,
            shell: None,
            home_dir: None,
            web_user: None,
        });
        assert!(err1.is_err());

        // Invalid UID (out of range)
        let err2 = manager.create_user(UserCreateRequest {
            username: "p_validuser".to_string(),
            uid: 9999,
            shell: None,
            home_dir: None,
            web_user: None,
        });
        assert!(err2.is_err());

        // Invalid home directory (arbitrary path injection)
        let err3 = manager.create_user(UserCreateRequest {
            username: "p_validuser".to_string(),
            uid: 10001,
            shell: None,
            home_dir: Some("/etc".to_string()),
            web_user: None,
        });
        assert!(err3.is_err());
    }

    #[test]
    fn test_suspend_and_unsuspend_flow() {
        let runner = MockSystemRunner::new();
        let actions_ref = Arc::clone(&runner.actions);
        let manager = UserLifecycleManager::new(Arc::new(runner));

        // Suspend
        let res_suspend = manager
            .suspend_user(UserSuspendRequest {
                username: "p_client1".to_string(),
                uid: 10001,
            })
            .expect("suspend");
        assert_eq!(res_suspend.username, "p_client1");

        let actions = actions_ref.lock().map(|l| l.clone()).unwrap_or_default();
        assert!(actions.iter().any(|a| matches!(a, MockAction::Command { binary, args } if binary == "usermod" && args.contains(&"-L".to_string()))));
        assert!(actions.iter().any(|a| matches!(a, MockAction::Command { binary, args } if binary == "pkill" && args.contains(&"10001".to_string()))));

        // Unsuspend
        let res_unsuspend = manager
            .unsuspend_user(UserUnsuspendRequest {
                username: "p_client1".to_string(),
                uid: 10001,
                shell: Some("/bin/bash".to_string()),
            })
            .expect("unsuspend");
        assert_eq!(res_unsuspend.username, "p_client1");

        let actions2 = actions_ref.lock().map(|l| l.clone()).unwrap_or_default();
        assert!(actions2.iter().any(|a| matches!(a, MockAction::Command { binary, args } if binary == "usermod" && args.contains(&"-U".to_string()) && args.contains(&"/bin/bash".to_string()))));
    }

    #[test]
    fn test_delete_user_purge_and_retention() {
        let runner = MockSystemRunner::new();
        let actions_ref = Arc::clone(&runner.actions);
        let manager = UserLifecycleManager::new(Arc::new(runner));

        // Delete with retention
        let res_retention = manager
            .delete_user(UserDeleteRequest {
                username: "p_client1".to_string(),
                uid: 10001,
                purge: false,
                trash_dir: Some("/var/lib/panel/trash-index".to_string()),
            })
            .expect("delete retention");
        assert!(!res_retention.purged);
        assert!(res_retention.archived_path.is_some());

        // Delete with purge
        let res_purge = manager
            .delete_user(UserDeleteRequest {
                username: "p_client2".to_string(),
                uid: 10002,
                purge: true,
                trash_dir: None,
            })
            .expect("delete purge");
        assert!(res_purge.purged);
        assert!(res_purge.archived_path.is_none());

        let actions = actions_ref.lock().map(|l| l.clone()).unwrap_or_default();
        // Checked pkill
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, MockAction::Command { binary, .. } if binary == "pkill"))
        );
        // Checked userdel
        assert!(actions.iter().any(|a| matches!(a, MockAction::Command { binary, args } if binary == "userdel" && args.contains(&"p_client1".to_string()))));
        // Checked remove_dir for purged user
        assert!(actions.iter().any(
            |a| matches!(a, MockAction::RemoveDir { path } if path == Path::new("/home/p_client2"))
        ));
    }
}
