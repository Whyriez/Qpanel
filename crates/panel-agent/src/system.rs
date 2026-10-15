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
    "getfacl",
    "pkill",
    "systemctl",
    "setquota",
    "repquota",
    "xfs_quota",
    "findmnt",
    "mount",
    "getent",
    "nginx",
    "apt-get",
    "apt-cache",
    "add-apt-repository",
    "dpkg-query",
    "phpenmod",
    "phpdismod",
    "systemd-analyze",
    "php-fpm7.4",
    "php-fpm8.0",
    "php-fpm8.1",
    "php-fpm8.2",
    "php-fpm8.3",
    "php-fpm8.4",
    "sshd",
    "curl",
    "node",
    "corepack",
    "npm",
    "pnpm",
    "yarn",
    "uv",
    "/usr/local/bin/uv",
    "/usr/bin/uv",
    "python",
    "python3",
    "gunicorn",
    "uvicorn",
    "systemd-run",
    "journalctl",
    "ssh-keygen",
    "git",
    "ssh",
    "sh",
    "bash",
    "mariadb",
    "mysql",
    "mariadb-dump",
    "mysqldump",
    "psql",
    "pg_dump",
    "createdb",
    "dropdb",
    "createuser",
    "dropuser",
    "tar",
    "zstd",
    "gzip",
    "sysctl",
    "nft",
];

/// Trait abstracting underlying operating system commands and filesystem operations.
pub trait SystemRunner: Send + Sync {
    fn run_command(&self, binary: &str, args: &[&str]) -> Result<std::process::Output, AgentError>;
    fn create_dir_all(&self, path: &Path, mode: u32, uid: u32, gid: u32) -> Result<(), AgentError>;
    fn set_permissions(&self, path: &Path, mode: u32) -> Result<(), AgentError>;
    fn set_ownership(&self, path: &Path, uid: u32, gid: u32) -> Result<(), AgentError>;
    fn set_facl(&self, path: &Path, user: &str, permissions: &str) -> Result<(), AgentError>;
    fn move_path(&self, from: &Path, to: &Path) -> Result<(), AgentError>;
    fn create_symlink(&self, original: &Path, link: &Path) -> Result<(), AgentError>;
    fn read_link(&self, path: &Path) -> Result<PathBuf, AgentError>;
    fn remove_dir_all(&self, path: &Path) -> Result<(), AgentError>;
    fn path_exists(&self, path: &Path) -> bool;
    fn write_file(&self, path: &Path, content: &[u8], mode: u32) -> Result<(), AgentError>;
    fn read_file(&self, path: &Path) -> Result<Vec<u8>, AgentError>;
    fn remove_file(&self, path: &Path) -> Result<(), AgentError>;
    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, AgentError>;
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

        let mut command = panel_core::production::system_command(binary);
        // Candidate parsing must not depend on the server's display language.
        if binary == "apt-cache" {
            command.env("LC_ALL", "C");
        }
        let output = command.args(args).output().map_err(AgentError::Io)?;

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
            if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)) {
                if e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(1) {
                    warn!(
                        "set_permissions on {} to {:o} failed ({}: Operation not permitted). Continuing without chmod.",
                        path.display(),
                        mode,
                        e
                    );
                } else {
                    return Err(AgentError::Io(e));
                }
            }
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
            if let Err(e) = chown(path, Some(uid), Some(gid)) {
                if e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(1) {
                    warn!(
                        "set_ownership on {} to {}:{} failed ({}: Operation not permitted). Continuing without chown.",
                        path.display(),
                        uid,
                        gid,
                        e
                    );
                } else {
                    return Err(AgentError::Io(e));
                }
            }
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

    fn create_symlink(&self, original: &Path, link: &Path) -> Result<(), AgentError> {
        if let Some(parent) = link.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent)?;
        }
        if link.is_symlink() || link.exists() {
            let _ = std::fs::remove_file(link);
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(original, link).map_err(AgentError::Io)?;
        }
        #[cfg(windows)]
        {
            if original.is_dir() {
                std::os::windows::fs::symlink_dir(original, link).map_err(AgentError::Io)?;
            } else {
                std::os::windows::fs::symlink_file(original, link).map_err(AgentError::Io)?;
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (original, link);
        }
        Ok(())
    }

    fn read_link(&self, path: &Path) -> Result<PathBuf, AgentError> {
        std::fs::read_link(path).map_err(AgentError::Io)
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

    fn read_file(&self, path: &Path) -> Result<Vec<u8>, AgentError> {
        std::fs::read(path).map_err(AgentError::Io)
    }

    fn remove_file(&self, path: &Path) -> Result<(), AgentError> {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, AgentError> {
        if !path.exists() {
            return Ok(Vec::new());
        }
        let entries = std::fs::read_dir(path)
            .map_err(AgentError::Io)?
            .filter_map(|e| e.ok().map(|ent| ent.path()))
            .collect();
        Ok(entries)
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
    CreateSymlink {
        original: PathBuf,
        link: PathBuf,
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

pub type MockCommandResponse = (i32, Vec<u8>, Vec<u8>);

/// Helper to determine if a path refers to a host system directory that should
/// never leak into test mocks.
pub fn is_mock_system_path(path: &Path) -> bool {
    let p = path.to_string_lossy();
    p.starts_with("/etc")
        || p.starts_with("/usr")
        || p.starts_with("/var")
        || p.starts_with("/run")
        || p.starts_with("/sys")
        || p.starts_with("/proc")
        || p.starts_with("/bin")
        || p.starts_with("/sbin")
        || p.starts_with("/dev")
}

/// In-memory mock runner for test assertions and safe non-root validation.
#[derive(Debug, Clone, Default)]
pub struct MockSystemRunner {
    pub actions: Arc<Mutex<Vec<MockAction>>>,
    pub responses: Arc<Mutex<std::collections::HashMap<String, MockCommandResponse>>>,
    pub files: Arc<Mutex<std::collections::HashMap<PathBuf, Vec<u8>>>>,
    pub symlinks: Arc<Mutex<std::collections::HashMap<PathBuf, PathBuf>>>,
}

impl MockSystemRunner {
    pub fn new() -> Self {
        Self {
            actions: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(std::collections::HashMap::new())),
            files: Arc::new(Mutex::new(std::collections::HashMap::new())),
            symlinks: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub fn set_file_content(&self, path: impl AsRef<Path>, content: &[u8]) {
        if let Ok(mut lock) = self.files.lock() {
            lock.insert(path.as_ref().to_path_buf(), content.to_vec());
        }
    }

    pub fn set_response(&self, key: &str, exit_code: i32, stdout: &[u8], stderr: &[u8]) {
        if let Ok(mut lock) = self.responses.lock() {
            lock.insert(
                key.to_string(),
                (exit_code, stdout.to_vec(), stderr.to_vec()),
            );
        }
    }

    pub fn recorded_actions(&self) -> Vec<MockAction> {
        self.actions.lock().map(|l| l.clone()).unwrap_or_default()
    }

    pub fn get_commands(&self) -> Vec<String> {
        self.recorded_actions()
            .into_iter()
            .filter_map(|action| match action {
                MockAction::Command { binary, args } => {
                    Some(format!("{} {}", binary, args.join(" ")))
                }
                _ => None,
            })
            .collect()
    }

    pub fn add_existing_path(&self, path: impl AsRef<Path>) {
        self.set_file_content(path, b"");
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

        let cmd_key = format!("{binary} {}", args.join(" "));
        let (exit_code, stdout, stderr) = if let Ok(lock) = self.responses.lock() {
            if let Some((code, out, err)) = lock.get(&cmd_key).or_else(|| lock.get(binary)) {
                (*code, out.clone(), err.clone())
            } else {
                (0, Vec::new(), Vec::new())
            }
        } else {
            (0, Vec::new(), Vec::new())
        };

        #[cfg(unix)]
        let status = {
            use std::os::unix::process::ExitStatusExt;
            std::process::ExitStatus::from_raw(exit_code)
        };
        #[cfg(not(unix))]
        let status = {
            std::process::Command::new("cmd")
                .args(["/c", &format!("exit {exit_code}")])
                .status()
                .unwrap_or_else(|_| panic!("Failed to get dummy ExitStatus"))
        };

        if !status.success() {
            let stderr_str = String::from_utf8_lossy(&stderr).to_string();
            let is_ignorable = (binary == "groupadd" && stderr_str.contains("already exists"))
                || (binary == "groupdel" && stderr_str.contains("does not exist"))
                || (binary == "pkill" && status.code() == Some(1));

            if !is_ignorable {
                return Err(AgentError::CommandFailed {
                    binary: binary.to_string(),
                    code: status.code(),
                    stderr: stderr_str,
                });
            }
        }

        Ok(std::process::Output {
            status,
            stdout,
            stderr,
        })
    }

    fn create_dir_all(&self, path: &Path, mode: u32, uid: u32, gid: u32) -> Result<(), AgentError> {
        if let Ok(mut files) = self.files.lock() {
            files.insert(path.to_path_buf(), Vec::new());
        }
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
        if let Ok(mut files) = self.files.lock() {
            let content = files.remove(from);
            if let Some(content) = content {
                files.insert(to.to_path_buf(), content);
            }
        }
        if let Ok(mut symlinks) = self.symlinks.lock() {
            let target = symlinks.remove(from);
            if let Some(target) = target {
                symlinks.insert(to.to_path_buf(), target);
            }
        }
        if let Ok(mut lock) = self.actions.lock() {
            lock.push(MockAction::MovePath {
                from: from.to_path_buf(),
                to: to.to_path_buf(),
            });
        }
        Ok(())
    }

    fn create_symlink(&self, original: &Path, link: &Path) -> Result<(), AgentError> {
        if let Ok(mut lock) = self.symlinks.lock() {
            lock.insert(link.to_path_buf(), original.to_path_buf());
        }
        if let Ok(mut lock) = self.actions.lock() {
            lock.push(MockAction::CreateSymlink {
                original: original.to_path_buf(),
                link: link.to_path_buf(),
            });
        }
        Ok(())
    }

    fn read_link(&self, path: &Path) -> Result<PathBuf, AgentError> {
        if let Ok(lock) = self.symlinks.lock()
            && let Some(target) = lock.get(path)
        {
            return Ok(target.clone());
        }
        if path.is_symlink() {
            std::fs::read_link(path).map_err(AgentError::Io)
        } else {
            Err(AgentError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Mock symlink not found: {:?}", path),
            )))
        }
    }

    fn remove_dir_all(&self, path: &Path) -> Result<(), AgentError> {
        if let Ok(mut lock) = self.actions.lock() {
            lock.push(MockAction::RemoveDir {
                path: path.to_path_buf(),
            });
        }
        if !is_mock_system_path(path) && path.exists() {
            let _ = std::fs::remove_dir_all(path);
        }
        Ok(())
    }

    fn path_exists(&self, path: &Path) -> bool {
        if self
            .files
            .lock()
            .ok()
            .is_some_and(|lock| lock.contains_key(path))
        {
            return true;
        }
        if self
            .symlinks
            .lock()
            .ok()
            .is_some_and(|lock| lock.contains_key(path))
        {
            return true;
        }
        if is_mock_system_path(path) {
            return false;
        }
        path.exists()
    }

    fn write_file(&self, path: &Path, content: &[u8], mode: u32) -> Result<(), AgentError> {
        if let Ok(mut lock) = self.files.lock() {
            lock.insert(path.to_path_buf(), content.to_vec());
        }
        if let Ok(mut lock) = self.actions.lock() {
            lock.push(MockAction::WriteFile {
                path: path.to_path_buf(),
                content: content.to_vec(),
                mode,
            });
        }
        Ok(())
    }

    fn read_file(&self, path: &Path) -> Result<Vec<u8>, AgentError> {
        if let Some(content) = self
            .files
            .lock()
            .ok()
            .and_then(|lock| lock.get(path).cloned())
        {
            return Ok(content);
        }
        if !is_mock_system_path(path) && path.exists() {
            std::fs::read(path).map_err(AgentError::Io)
        } else {
            Err(AgentError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Mock file not found: {:?}", path),
            )))
        }
    }

    fn remove_file(&self, path: &Path) -> Result<(), AgentError> {
        if let Ok(mut lock) = self.files.lock() {
            lock.remove(path);
        }
        if let Ok(mut lock) = self.symlinks.lock() {
            lock.remove(path);
        }
        if let Ok(mut lock) = self.actions.lock() {
            lock.push(MockAction::RemoveFile {
                path: path.to_path_buf(),
            });
        }
        if !is_mock_system_path(path) && path.exists() {
            let _ = std::fs::remove_file(path);
        }
        Ok(())
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, AgentError> {
        let mut results = Vec::new();
        if let Ok(files) = self.files.lock() {
            for key in files.keys() {
                if key.parent() == Some(path) {
                    results.push(key.clone());
                }
            }
        }
        if !is_mock_system_path(path)
            && let Ok(entries) = std::fs::read_dir(path)
        {
            for entry in entries.flatten() {
                let p = entry.path();
                if !results.contains(&p) {
                    results.push(p);
                }
            }
        }
        Ok(results)
    }
}
