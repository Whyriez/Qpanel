//! System command execution and filesystem abstraction for host operations.

use crate::AgentError;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::warn;

pub const ALLOWED_BINARIES: &[&str] = &[
    "useradd",
    "usermod",
    "userdel",
    "groupadd",
    "groupdel",
    "setfacl",
    "pkill",
    "systemctl",
    "setquota",
    "repquota",
    "xfs_quota",
    "findmnt",
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
    fn write_file(&self, path: &Path, content: &[u8], mode: u32) -> Result<(), AgentError>;
    fn remove_file(&self, path: &Path) -> Result<(), AgentError>;
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
                || (binary == "pkill" && output.status.code() == Some(1));

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
            use std::os::unix::fs::chown;
            chown(path, Some(uid), Some(gid))?;
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

    fn write_file(&self, path: &Path, content: &[u8], mode: u32) -> Result<(), AgentError> {
        if let Some(parent) = path.parent().filter(|p| !p.exists()) {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        self.set_permissions(path, mode)?;
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> Result<(), AgentError> {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
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
    WriteFile {
        path: PathBuf,
        content: Vec<u8>,
        mode: u32,
    },
    RemoveFile {
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
        let status = {
            use std::os::unix::process::ExitStatusExt;
            std::process::ExitStatus::from_raw(0)
        };
        #[cfg(not(unix))]
        let status = {
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

    fn write_file(&self, path: &Path, content: &[u8], mode: u32) -> Result<(), AgentError> {
        if let Ok(mut lock) = self.actions.lock() {
            lock.push(MockAction::WriteFile {
                path: path.to_path_buf(),
                content: content.to_vec(),
                mode,
            });
        }
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> Result<(), AgentError> {
        if let Ok(mut lock) = self.actions.lock() {
            lock.push(MockAction::RemoveFile {
                path: path.to_path_buf(),
            });
        }
        Ok(())
    }
}
