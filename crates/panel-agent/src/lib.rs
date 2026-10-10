//! `panel-agent`: Privileged root daemon executing typed system operations.

pub mod audit;
pub mod crypto;
pub mod fs_manager;
pub mod hardening;
pub mod node;
pub mod node_site;
pub mod php;
pub mod proxy;
pub mod pty;
pub mod python_site;
pub mod quota;
pub mod resource;
pub mod slice;
pub mod ssh;
pub mod system;
pub mod user;
pub mod webserver;

use panel_proto::{
    PROTOCOL_VERSION, ProtoError, RequestPayload, ResponsePayload, RpcResponse, RpcStream,
    VersionInfo,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tracing::{info, warn};

#[cfg(unix)]
pub type IpcStream = tokio::net::UnixStream;
#[cfg(not(unix))]
pub type IpcStream = tokio::net::TcpStream;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Protocol error: {0}")]
    Proto(#[from] ProtoError),
    #[error("Authentication/Authorization failed: unauthorized peer {0}")]
    UnauthorizedPeer(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Security violation: {0}")]
    SecurityViolation(String),
    #[error("Command {binary} failed with exit code {code:?}: {stderr}")]
    CommandFailed {
        binary: String,
        code: Option<i32>,
        stderr: String,
    },
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Agent daemon configuration.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub socket_path: PathBuf,
    pub allowed_uid: Option<u32>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            socket_path: PathBuf::from("/run/panel/agent.sock"),
            allowed_uid: None,
        }
    }
}

pub struct AgentServer {
    config: AgentConfig,
    runner: Arc<dyn system::SystemRunner>,
    user_manager: Arc<user::UserLifecycleManager>,
    slice_manager: Arc<slice::SliceManager>,
    quota_manager: Arc<quota::QuotaManager>,
    resource_reader: Arc<resource::CgroupReader>,
    hardening_manager: Arc<hardening::HardeningManager>,
    webserver_manager: Arc<webserver::NginxManager>,
    fs_manager: Arc<fs_manager::FsManager>,
    pty_manager: Arc<pty::PtySessionManager>,
    ssh_manager: Arc<ssh::SshManager>,
}

fn create_default_pty_manager() -> Arc<pty::PtySessionManager> {
    #[cfg(unix)]
    let backend: Arc<dyn pty::PtyBackend> = Arc::new(pty::UnixPtyBackend::new());
    #[cfg(not(unix))]
    let backend: Arc<dyn pty::PtyBackend> = Arc::new(pty::MockPtyBackend::new());

    let mgr = Arc::new(pty::PtySessionManager::new(
        pty::PtyManagerConfig::default(),
        backend,
    ));
    std::mem::drop(mgr.start_idle_reaper(std::time::Duration::from_secs(30)));
    mgr
}

impl AgentServer {
    pub fn new(config: AgentConfig) -> Self {
        let runner = Arc::new(system::RealSystemRunner);
        let user_manager = Arc::new(user::UserLifecycleManager::new(
            Arc::clone(&runner) as Arc<dyn system::SystemRunner>
        ));
        let slice_manager = Arc::new(slice::SliceManager::new(
            Arc::clone(&runner) as Arc<dyn system::SystemRunner>
        ));
        let quota_manager = Arc::new(quota::QuotaManager::new(
            Arc::clone(&runner) as Arc<dyn system::SystemRunner>
        ));
        let resource_reader = Arc::new(resource::CgroupReader::default());
        let hardening_manager = Arc::new(hardening::HardeningManager::new(
            Arc::clone(&runner) as Arc<dyn system::SystemRunner>
        ));
        let webserver_manager = Arc::new(webserver::NginxManager::new(
            Arc::clone(&runner) as Arc<dyn system::SystemRunner>
        ));
        let fs_manager = Arc::new(fs_manager::FsManager::new(
            Arc::clone(&runner) as Arc<dyn system::SystemRunner>
        ));
        let pty_manager = create_default_pty_manager();
        let ssh_manager = Arc::new(ssh::SshManager::new(
            Arc::clone(&runner) as Arc<dyn system::SystemRunner>
        ));
        Self {
            config,
            runner,
            user_manager,
            slice_manager,
            quota_manager,
            resource_reader,
            hardening_manager,
            webserver_manager,
            fs_manager,
            pty_manager,
            ssh_manager,
        }
    }

    pub fn with_user_manager(
        config: AgentConfig,
        user_manager: Arc<user::UserLifecycleManager>,
    ) -> Self {
        let runner = Arc::new(system::RealSystemRunner);
        let slice_manager = Arc::new(slice::SliceManager::new(
            Arc::clone(&runner) as Arc<dyn system::SystemRunner>
        ));
        let quota_manager = Arc::new(quota::QuotaManager::new(
            Arc::clone(&runner) as Arc<dyn system::SystemRunner>
        ));
        let resource_reader = Arc::new(resource::CgroupReader::default());
        let hardening_manager = Arc::new(hardening::HardeningManager::new(
            Arc::clone(&runner) as Arc<dyn system::SystemRunner>
        ));
        let webserver_manager = Arc::new(webserver::NginxManager::new(
            Arc::clone(&runner) as Arc<dyn system::SystemRunner>
        ));
        let fs_manager = Arc::new(fs_manager::FsManager::new(
            Arc::clone(&runner) as Arc<dyn system::SystemRunner>
        ));
        let pty_manager = create_default_pty_manager();
        let ssh_manager = Arc::new(ssh::SshManager::new(
            Arc::clone(&runner) as Arc<dyn system::SystemRunner>
        ));
        Self {
            config,
            runner,
            user_manager,
            slice_manager,
            quota_manager,
            resource_reader,
            hardening_manager,
            webserver_manager,
            fs_manager,
            pty_manager,
            ssh_manager,
        }
    }

    pub fn with_managers(
        config: AgentConfig,
        user_manager: Arc<user::UserLifecycleManager>,
        slice_manager: Arc<slice::SliceManager>,
        quota_manager: Arc<quota::QuotaManager>,
        resource_reader: Arc<resource::CgroupReader>,
        hardening_manager: Arc<hardening::HardeningManager>,
    ) -> Self {
        let runner = Arc::new(system::RealSystemRunner);
        let webserver_manager = Arc::new(webserver::NginxManager::new(
            Arc::clone(&runner) as Arc<dyn system::SystemRunner>
        ));
        let fs_manager = Arc::new(fs_manager::FsManager::new(
            Arc::clone(&runner) as Arc<dyn system::SystemRunner>
        ));
        let pty_manager = create_default_pty_manager();
        let ssh_manager = Arc::new(ssh::SshManager::new(
            Arc::clone(&runner) as Arc<dyn system::SystemRunner>
        ));
        Self {
            config,
            runner,
            user_manager,
            slice_manager,
            quota_manager,
            resource_reader,
            hardening_manager,
            webserver_manager,
            fs_manager,
            pty_manager,
            ssh_manager,
        }
    }

    pub fn with_all_managers(
        config: AgentConfig,
        user_manager: Arc<user::UserLifecycleManager>,
        slice_manager: Arc<slice::SliceManager>,
        quota_manager: Arc<quota::QuotaManager>,
        resource_reader: Arc<resource::CgroupReader>,
        hardening_manager: Arc<hardening::HardeningManager>,
        webserver_manager: Arc<webserver::NginxManager>,
    ) -> Self {
        let runner = Arc::clone(webserver_manager.runner());
        let fs_manager = Arc::new(fs_manager::FsManager::new(
            Arc::clone(&runner) as Arc<dyn system::SystemRunner>
        ));
        let pty_manager = create_default_pty_manager();
        let ssh_manager = Arc::new(ssh::SshManager::new(
            Arc::clone(&runner) as Arc<dyn system::SystemRunner>
        ));
        Self {
            config,
            runner,
            user_manager,
            slice_manager,
            quota_manager,
            resource_reader,
            hardening_manager,
            webserver_manager,
            fs_manager,
            pty_manager,
            ssh_manager,
        }
    }

    pub fn with_fs_manager(mut self, fs_manager: Arc<fs_manager::FsManager>) -> Self {
        self.fs_manager = fs_manager;
        self
    }

    pub fn with_quota_manager(mut self, quota_manager: Arc<quota::QuotaManager>) -> Self {
        self.quota_manager = quota_manager;
        self
    }

    pub fn with_pty_manager(mut self, pty_manager: Arc<pty::PtySessionManager>) -> Self {
        self.pty_manager = pty_manager;
        self
    }

    pub fn with_ssh_manager(mut self, ssh_manager: Arc<ssh::SshManager>) -> Self {
        self.ssh_manager = ssh_manager;
        self
    }

    pub fn pty_manager(&self) -> &Arc<pty::PtySessionManager> {
        &self.pty_manager
    }

    pub fn ssh_manager(&self) -> &Arc<ssh::SshManager> {
        &self.ssh_manager
    }

    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    /// Verifies peer credentials over Unix Domain Socket using SO_PEERCRED on Unix.
    fn verify_peer(&self, stream: &IpcStream) -> Result<(), AgentError> {
        #[cfg(unix)]
        {
            if let Some(expected_uid) = self.config.allowed_uid {
                let cred = stream.peer_cred()?;
                if cred.uid() != expected_uid {
                    return Err(AgentError::UnauthorizedPeer(format!(
                        "Peer UID {} does not match expected allowed UID {}",
                        cred.uid(),
                        expected_uid
                    )));
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = stream;
        }
        Ok(())
    }

    /// Starts the server listening on the configured socket path.
    pub async fn run(
        self: Arc<Self>,
        shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<(), AgentError> {
        self.run_listener(shutdown_rx).await
    }

    #[cfg(unix)]
    async fn run_listener(
        self: Arc<Self>,
        mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<(), AgentError> {
        let socket_path = &self.config.socket_path;
        if socket_path.exists() {
            let _ = std::fs::remove_file(socket_path);
        }

        if let Some(parent) = socket_path.parent().filter(|p| !p.exists()) {
            std::fs::create_dir_all(parent)?;
        }

        let listener = tokio::net::UnixListener::bind(socket_path)?;
        info!("Agent listening on Unix socket: {}", socket_path.display());

        // Ensure socket permissions are restricted (0660 root:panel)
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o660);
        if let Err(e) = std::fs::set_permissions(socket_path, permissions) {
            warn!("Failed to set permissions on agent socket: {}", e);
        }

        loop {
            tokio::select! {
                accept_res = listener.accept() => {
                    if let Ok((stream, _addr)) = accept_res {
                        let server = Arc::clone(&self);
                        tokio::spawn(async move {
                            if let Err(e) = server.handle_connection(stream).await {
                                warn!("Error handling agent client connection: {}", e);
                            }
                        });
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Agent received shutdown signal, shutting down listener...");
                    break;
                }
            }
        }

        if socket_path.exists() {
            let _ = std::fs::remove_file(socket_path);
        }

        Ok(())
    }

    #[cfg(not(unix))]
    async fn run_listener(
        self: Arc<Self>,
        mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<(), AgentError> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let local_addr = listener.local_addr()?;

        if let Some(parent) = self.config.socket_path.parent().filter(|p| !p.exists()) {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.config.socket_path, local_addr.port().to_string())?;

        info!(
            "Agent listening on fallback TCP loopback: {} (development mode)",
            local_addr
        );

        loop {
            tokio::select! {
                accept_res = listener.accept() => {
                    if let Ok((stream, _addr)) = accept_res {
                        let server = Arc::clone(&self);
                        tokio::spawn(async move {
                            if let Err(e) = server.handle_connection(stream).await {
                                warn!("Error handling agent client connection: {}", e);
                            }
                        });
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Agent received shutdown signal, shutting down listener...");
                    break;
                }
            }
        }

        if self.config.socket_path.exists() {
            let _ = std::fs::remove_file(&self.config.socket_path);
        }

        Ok(())
    }

    #[allow(clippy::cognitive_complexity)]
    async fn handle_connection(&self, stream: IpcStream) -> Result<(), AgentError> {
        self.verify_peer(&stream)?;
        let mut rpc = RpcStream::new(stream);

        loop {
            let req = match rpc.recv_request().await {
                Ok(req) => req,
                Err(ProtoError::ConnectionClosed) => break,
                Err(e) => return Err(AgentError::Proto(e)),
            };

            let (action, target) = match &req.payload {
                RequestPayload::Ping => ("ping".to_string(), "system".to_string()),
                RequestPayload::Version => ("version".to_string(), "system".to_string()),
                RequestPayload::UserCreate(d) => ("user.create".to_string(), d.username.clone()),
                RequestPayload::UserSuspend(d) => ("user.suspend".to_string(), d.username.clone()),
                RequestPayload::UserUnsuspend(d) => {
                    ("user.unsuspend".to_string(), d.username.clone())
                }
                RequestPayload::UserDelete(d) => ("user.delete".to_string(), d.username.clone()),
                RequestPayload::SliceApply(d) => {
                    ("slice.apply".to_string(), format!("panel-u{}.slice", d.uid))
                }
                RequestPayload::SliceRemove(d) => (
                    "slice.remove".to_string(),
                    format!("panel-u{}.slice", d.uid),
                ),
                RequestPayload::QuotaSet(d) => ("quota.set".to_string(), d.username.clone()),
                RequestPayload::QuotaGet(d) => ("quota.get".to_string(), d.username.clone()),
                RequestPayload::QuotaScan(d) => ("quota.scan".to_string(), d.username.clone()),
                RequestPayload::ResourceRead(d) => (
                    "resource.read".to_string(),
                    format!("panel-u{}.slice", d.uid),
                ),
                RequestPayload::HardeningCheck => {
                    ("hardening.check".to_string(), "host".to_string())
                }
                RequestPayload::HardeningApply(_) => {
                    ("hardening.apply".to_string(), "host".to_string())
                }
                RequestPayload::NginxBaselineApply(d) => (
                    "nginx.baseline_apply".to_string(),
                    d.base_dir
                        .clone()
                        .unwrap_or_else(|| "/etc/panel/nginx".to_string()),
                ),
                RequestPayload::SiteApply(d) => ("nginx.site_apply".to_string(), d.site_id.clone()),
                RequestPayload::SiteRemove(d) => {
                    ("nginx.site_remove".to_string(), d.site_id.clone())
                }
                RequestPayload::SiteProvision(d) => {
                    ("nginx.site_provision".to_string(), d.username.clone())
                }
                RequestPayload::AcmeChallengeDeploy(d) => {
                    ("acme.challenge_deploy".to_string(), d.token.clone())
                }
                RequestPayload::AcmeChallengeCleanup(d) => {
                    ("acme.challenge_cleanup".to_string(), d.token.clone())
                }
                RequestPayload::SslCertDeploy(d) => {
                    ("ssl.cert_deploy".to_string(), d.domain.clone())
                }
                RequestPayload::SiteLogsRead(d) => (
                    "nginx.site_logs_read".to_string(),
                    format!("{}/{}", d.username, d.site_domain),
                ),
                RequestPayload::PhpRepoSetup(_) => {
                    ("php.repo_setup".to_string(), "sury_ppa".to_string())
                }
                RequestPayload::PhpRuntimeList(_) => {
                    ("php.runtime_list".to_string(), "host".to_string())
                }
                RequestPayload::PhpRuntimeInstall(d) => {
                    ("php.runtime_install".to_string(), d.version.clone())
                }
                RequestPayload::PhpRuntimeRemove(d) => {
                    ("php.runtime_remove".to_string(), d.version.clone())
                }
                RequestPayload::PhpExtensionList(d) => {
                    ("php.extension_list".to_string(), d.version.clone())
                }
                RequestPayload::PhpExtensionInstall(d) => (
                    "php.extension_install".to_string(),
                    format!("php{}-{}", d.version, d.extension),
                ),
                RequestPayload::PhpExtensionToggle(d) => (
                    "php.extension_toggle".to_string(),
                    format!("php{}-{}", d.version, d.extension),
                ),
                RequestPayload::PhpExtensionRemove(d) => (
                    "php.extension_remove".to_string(),
                    format!("php{}-{}", d.version, d.extension),
                ),
                RequestPayload::PhpFpmMasterEnsure(d) => (
                    "php.fpm_master_ensure".to_string(),
                    format!("{}-{}", d.username, d.version),
                ),
                RequestPayload::PhpFpmPoolApply(d) => (
                    "php.fpm_pool_apply".to_string(),
                    format!("{}-{}/{}", d.username, d.version, d.pool_name),
                ),
                RequestPayload::PhpFpmPoolRemove(d) => (
                    "php.fpm_pool_remove".to_string(),
                    format!("{}-{}/{}", d.username, d.version, d.pool_name),
                ),
                RequestPayload::PhpFpmMasterStatus(d) => (
                    "php.fpm_master_status".to_string(),
                    format!("{}-{}", d.username, d.version),
                ),
                RequestPayload::PhpFpmMasterReload(d) => (
                    "php.fpm_master_reload".to_string(),
                    format!("{}-{}", d.username, d.version),
                ),
                RequestPayload::PhpFpmMasterRestart(d) => (
                    "php.fpm_master_restart".to_string(),
                    format!("{}-{}", d.username, d.version),
                ),
                RequestPayload::PhpFpmHealthCheck(d) => (
                    "php.fpm_health_check".to_string(),
                    format!("{}-{}/{}", d.username, d.version, d.pool_name),
                ),
                RequestPayload::PhpFpmPoolStatus(d) => (
                    "php.fpm_pool_status".to_string(),
                    format!("{}-{}/{}", d.username, d.version, d.pool_name),
                ),
                RequestPayload::PhpLogsRead(d) => (
                    "php.logs_read".to_string(),
                    format!("{}-{}/{}", d.username, d.version, d.log_type),
                ),
                RequestPayload::FsList(d) => {
                    ("fs.list".to_string(), format!("{}/{}", d.username, d.path))
                }
                RequestPayload::FsStat(d) => {
                    ("fs.stat".to_string(), format!("{}/{}", d.username, d.path))
                }
                RequestPayload::FsMkdir(d) => {
                    ("fs.mkdir".to_string(), format!("{}/{}", d.username, d.path))
                }
                RequestPayload::FsRename(d) => (
                    "fs.rename".to_string(),
                    format!("{}:{}->{}", d.username, d.source, d.destination),
                ),
                RequestPayload::FsCopy(d) => (
                    "fs.copy".to_string(),
                    format!("{}:{}->{}", d.username, d.source, d.destination),
                ),
                RequestPayload::FsTrash(d) => {
                    ("fs.trash".to_string(), format!("{}/{}", d.username, d.path))
                }
                RequestPayload::FsTrashList(d) => ("fs.trash_list".to_string(), d.username.clone()),
                RequestPayload::FsTrashRestore(d) => (
                    "fs.trash_restore".to_string(),
                    format!("{}/{}", d.username, d.trash_id),
                ),
                RequestPayload::FsTrashEmpty(d) => {
                    ("fs.trash_empty".to_string(), d.username.clone())
                }
                RequestPayload::FsDelete(d) => (
                    "fs.delete".to_string(),
                    format!("{}/{}", d.username, d.path),
                ),
                RequestPayload::FsUploadInit(d) => (
                    "fs.upload_init".to_string(),
                    format!("{}:{}/{}", d.username, d.target_path, d.file_name),
                ),
                RequestPayload::FsUploadGetSession(d) => (
                    "fs.upload_get_session".to_string(),
                    format!("{}:{}", d.username, d.upload_id),
                ),
                RequestPayload::FsUploadAppendChunk(d) => (
                    "fs.upload_append_chunk".to_string(),
                    format!("{}:{}:{}", d.username, d.upload_id, d.offset),
                ),
                RequestPayload::FsReadRange(d) => (
                    "fs.read_range".to_string(),
                    format!("{}:{}:{}-{}", d.username, d.path, d.start, d.length),
                ),
                RequestPayload::FsZipDirectory(d) => (
                    "fs.zip_directory".to_string(),
                    format!("{}:{}", d.username, d.path),
                ),
                RequestPayload::FsArchiveCompress(d) => (
                    "fs.archive_compress".to_string(),
                    format!("{}:{}", d.username, d.destination),
                ),
                RequestPayload::FsArchiveExtract(d) => (
                    "fs.archive_extract".to_string(),
                    format!("{}:{}", d.username, d.archive_path),
                ),
                RequestPayload::FsChmod(d) => (
                    "fs.chmod".to_string(),
                    format!("{}:{}:{:04o}", d.username, d.path, d.mode),
                ),
                RequestPayload::FsChmodRecursive(d) => (
                    "fs.chmod_recursive".to_string(),
                    format!(
                        "{}:{}:{:04o}:{:04o}",
                        d.username, d.path, d.file_mode, d.dir_mode
                    ),
                ),
                RequestPayload::FsChown(d) => (
                    "fs.chown".to_string(),
                    format!("{}:{}:{:?}:{:?}", d.username, d.path, d.uid, d.gid),
                ),
                RequestPayload::FsReadFile(d) => (
                    "fs.read_file".to_string(),
                    format!("{}:{}", d.username, d.path),
                ),
                RequestPayload::FsWriteAtomic(d) => (
                    "fs.write_atomic".to_string(),
                    format!("{}:{}:{}b", d.username, d.path, d.data.len()),
                ),
                RequestPayload::FsSearch(d) => (
                    "fs.search".to_string(),
                    format!("{}:{}:{}", d.username, d.path, d.query),
                ),
                RequestPayload::PtyOpen(d) => (
                    "pty.open".to_string(),
                    format!("{}:{:?}", d.username, d.cwd),
                ),
                RequestPayload::PtyInput(d) => ("pty.input".to_string(), d.session_id.clone()),
                RequestPayload::PtyReadOutput(d) => {
                    ("pty.read_output".to_string(), d.session_id.clone())
                }
                RequestPayload::PtyResize(d) => (
                    "pty.resize".to_string(),
                    format!("{}:{}x{}", d.session_id, d.rows, d.cols),
                ),
                RequestPayload::PtyClose(d) => ("pty.close".to_string(), d.session_id.clone()),
                RequestPayload::PtyList(d) => ("pty.list".to_string(), format!("{:?}", d.username)),
                RequestPayload::SshKeysSync(d) => {
                    ("ssh.sync_keys".to_string(), d.linux_username.clone())
                }
                RequestPayload::SshPolicyApply(d) => (
                    "ssh.policy_apply".to_string(),
                    format!("{}:{}", d.linux_username, d.policy),
                ),
                RequestPayload::SshDropinApply(_) => (
                    "ssh.dropin_apply".to_string(),
                    "/etc/ssh/sshd_config.d/50-panel.conf".to_string(),
                ),
                RequestPayload::NodeRuntimeList(_) => {
                    ("node.runtime_list".to_string(), "host".to_string())
                }
                RequestPayload::NodeRuntimeInstall(d) => {
                    ("node.runtime_install".to_string(), d.version.clone())
                }
                RequestPayload::NodeRuntimeRemove(d) => {
                    ("node.runtime_remove".to_string(), d.version.clone())
                }
                RequestPayload::NodeRuntimeCorepack(d) => {
                    ("node.runtime_corepack".to_string(), d.version.clone())
                }
                RequestPayload::NodeSiteApply(d) => {
                    ("node.site_apply".to_string(), d.site_id.clone())
                }
                RequestPayload::NodeSiteRemove(d) => {
                    ("node.site_remove".to_string(), d.site_id.clone())
                }
                RequestPayload::NodeSiteAction(d) => (
                    format!("node.site_{:?}", d.action).to_lowercase(),
                    d.site_id.clone(),
                ),
                RequestPayload::NodeSiteStatus(d) => {
                    ("node.site_status".to_string(), d.site_id.clone())
                }
                RequestPayload::NodeSiteJob(d) => (
                    "node.site_job".to_string(),
                    format!("{}:{:?}", d.site_id, d.job_type),
                ),
                RequestPayload::PythonRuntimeList(_) => {
                    ("python.runtime_list".to_string(), "host".to_string())
                }
                RequestPayload::PythonRuntimeInstall(d) => {
                    ("python.runtime_install".to_string(), d.version.clone())
                }
                RequestPayload::PythonRuntimeRemove(d) => {
                    ("python.runtime_remove".to_string(), d.version.clone())
                }
                RequestPayload::PythonSiteApply(d) => {
                    ("python.site_apply".to_string(), d.site_id.clone())
                }
                RequestPayload::PythonSiteRemove(d) => {
                    ("python.site_remove".to_string(), d.site_id.clone())
                }
                RequestPayload::PythonSiteAction(d) => (
                    format!("python.site_{:?}", d.action).to_lowercase(),
                    d.site_id.clone(),
                ),
                RequestPayload::PythonSiteStatus(d) => {
                    ("python.site_status".to_string(), d.site_id.clone())
                }
                RequestPayload::PythonSiteJob(d) => (
                    "python.site_job".to_string(),
                    format!("{}:{:?}", d.site_id, d.job_type),
                ),
                RequestPayload::ProxyUpstreamProbe(d) => {
                    ("proxy.upstream_probe".to_string(), d.upstream_url.clone())
                }
                RequestPayload::SiteEnvEncrypt(d) => {
                    ("site.env_encrypt".to_string(), d.site_id.clone())
                }
                RequestPayload::SiteEnvDecrypt(d) => {
                    ("site.env_decrypt".to_string(), d.site_id.clone())
                }
                RequestPayload::SiteEnvApply(d) => {
                    ("site.env_apply".to_string(), d.site_id.clone())
                }
            };

            let response = match req.payload {
                RequestPayload::Ping => RpcResponse::ok(&req.request_id, ResponsePayload::Pong),
                RequestPayload::Version => {
                    let info = VersionInfo {
                        version: env!("CARGO_PKG_VERSION").to_string(),
                        proto_version: PROTOCOL_VERSION,
                        os: std::env::consts::OS.to_string(),
                        arch: std::env::consts::ARCH.to_string(),
                    };
                    RpcResponse::ok(&req.request_id, ResponsePayload::Version(info))
                }
                RequestPayload::UserCreate(data) => match self.user_manager.create_user(data) {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::UserCreated(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::UserSuspend(data) => match self.user_manager.suspend_user(data) {
                    Ok(res) => {
                        RpcResponse::ok(&req.request_id, ResponsePayload::UserSuspended(res))
                    }
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::UserUnsuspend(data) => {
                    match self.user_manager.unsuspend_user(data) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::UserUnsuspended(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::UserDelete(data) => match self.user_manager.delete_user(data) {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::UserDeleted(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::SliceApply(data) => match self.slice_manager.apply_slice(data) {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::SliceApplied(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::SliceRemove(data) => match self.slice_manager.remove_slice(data) {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::SliceRemoved(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::QuotaSet(data) => match self.quota_manager.set_quota(data) {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::QuotaSet(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::QuotaGet(data) => match self.quota_manager.get_quota(data) {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::QuotaGet(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::QuotaScan(data) => match self.quota_manager.scan_quota(data) {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::QuotaScan(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::ResourceRead(data) => {
                    match self.resource_reader.read_slice_resources(data.uid) {
                        Ok(metrics) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::ResourceRead(panel_proto::ResourceReadResponse {
                                metrics,
                            }),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::HardeningCheck => RpcResponse::ok(
                    &req.request_id,
                    ResponsePayload::HardeningStatus(Box::new(self.hardening_manager.check_all())),
                ),
                RequestPayload::HardeningApply(data) => match self.hardening_manager.apply(data) {
                    Ok(res) => {
                        RpcResponse::ok(&req.request_id, ResponsePayload::HardeningApplied(res))
                    }
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::NginxBaselineApply(data) => {
                    match self.webserver_manager.apply_baseline(data) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::NginxBaselineApplied(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::SiteApply(data) => match self.webserver_manager.apply_site(data) {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::SiteApplied(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::SiteRemove(data) => {
                    match self.webserver_manager.remove_site(data) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::SiteRemoved(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::SiteProvision(data) => {
                    match self.webserver_manager.provision_site(data) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::SiteProvisioned(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::AcmeChallengeDeploy(data) => {
                    match self.webserver_manager.deploy_acme_challenge(data) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::AcmeChallengeDeployed(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::AcmeChallengeCleanup(data) => {
                    match self.webserver_manager.cleanup_acme_challenge(data) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::AcmeChallengeCleaned(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::SslCertDeploy(data) => {
                    match self.webserver_manager.deploy_ssl_cert(data) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::SslCertDeployed(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::SiteLogsRead(data) => {
                    match self.webserver_manager.read_site_logs(data) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::SiteLogsRead(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PhpRepoSetup(_) => {
                    match php::setup_php_repository(self.runner.as_ref()) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::PhpRepoSetup(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PhpRuntimeList(_) => {
                    match php::list_php_runtimes(self.runner.as_ref()) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::PhpRuntimeList(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PhpRuntimeInstall(data) => {
                    match php::install_php_runtime(self.runner.as_ref(), &data.version) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::PhpRuntimeInstalled(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PhpRuntimeRemove(data) => {
                    match php::remove_php_runtime(self.runner.as_ref(), &data.version) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::PhpRuntimeRemoved(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PhpExtensionList(data) => {
                    match php::list_php_extensions(self.runner.as_ref(), &data.version) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::PhpExtensionList(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PhpExtensionInstall(data) => {
                    match php::install_php_extension(
                        self.runner.as_ref(),
                        &data.version,
                        &data.extension,
                    ) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::PhpExtensionAction(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PhpExtensionToggle(data) => {
                    match php::toggle_php_extension(
                        self.runner.as_ref(),
                        &data.version,
                        &data.extension,
                        data.enabled,
                    ) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::PhpExtensionAction(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PhpExtensionRemove(data) => {
                    match php::remove_php_extension(
                        self.runner.as_ref(),
                        &data.version,
                        &data.extension,
                    ) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::PhpExtensionAction(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PhpFpmMasterEnsure(data) => {
                    match php::ensure_php_fpm_master(self.runner.as_ref(), data) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::PhpFpmMasterEnsure(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PhpFpmPoolApply(data) => {
                    match php::apply_php_fpm_pool(self.runner.as_ref(), data) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::PhpFpmPoolApply(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PhpFpmPoolRemove(data) => {
                    match php::remove_php_fpm_pool(self.runner.as_ref(), data) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::PhpFpmPoolRemove(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PhpFpmMasterStatus(data) => {
                    match php::get_php_fpm_master_status(self.runner.as_ref(), data) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::PhpFpmMasterStatus(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PhpFpmMasterReload(data) => {
                    match php::reload_php_fpm_master(self.runner.as_ref(), data) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::PhpFpmMasterAction(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PhpFpmMasterRestart(data) => {
                    match php::restart_php_fpm_master(self.runner.as_ref(), data) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::PhpFpmMasterAction(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PhpFpmHealthCheck(data) => {
                    match php::check_php_fpm_health(self.runner.as_ref(), data) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::PhpFpmHealthChecked(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PhpFpmPoolStatus(data) => {
                    match php::get_php_fpm_pool_status(self.runner.as_ref(), data) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::PhpFpmPoolStatus(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PhpLogsRead(data) => {
                    match php::read_php_logs(self.runner.as_ref(), data) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::PhpLogsRead(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::FsList(data) => match self.fs_manager.list_dir(data).await {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::FsList(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::FsStat(data) => match self.fs_manager.stat(data).await {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::FsStat(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::FsMkdir(data) => match self.fs_manager.mkdir(data).await {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::FsAction(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::FsRename(data) => match self.fs_manager.rename(data).await {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::FsAction(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::FsCopy(data) => match self.fs_manager.copy(data).await {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::FsAction(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::FsTrash(data) => match self.fs_manager.trash(data).await {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::FsTrash(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::FsTrashList(data) => match self.fs_manager.trash_list(data).await {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::FsTrashList(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::FsTrashRestore(data) => {
                    match self.fs_manager.trash_restore(data).await {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::FsTrashRestore(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::FsTrashEmpty(data) => match self.fs_manager.trash_empty(data).await
                {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::FsTrashEmpty(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::FsDelete(data) => match self.fs_manager.delete(data).await {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::FsAction(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::FsUploadInit(data) => {
                    match self.fs_manager.upload_init(data).await {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::FsUploadInit(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::FsUploadGetSession(data) => {
                    match self.fs_manager.upload_get_session(data).await {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::FsUploadGetSession(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::FsUploadAppendChunk(data) => {
                    match self.fs_manager.upload_append_chunk(data).await {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::FsUploadAppendChunk(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::FsReadRange(data) => match self.fs_manager.read_range(data).await {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::FsReadRange(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::FsZipDirectory(data) => {
                    match self.fs_manager.zip_directory(data).await {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::FsZipDirectory(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::FsArchiveCompress(data) => {
                    match self.fs_manager.archive_compress(data).await {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::FsArchiveCompress(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::FsArchiveExtract(data) => {
                    match self.fs_manager.archive_extract(data).await {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::FsArchiveExtract(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::FsChmod(data) => match self.fs_manager.chmod(data).await {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::FsChmod(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::FsChmodRecursive(data) => {
                    match self.fs_manager.chmod_recursive(data).await {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::FsChmodRecursive(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::FsChown(data) => match self.fs_manager.chown(data).await {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::FsChown(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::FsReadFile(data) => match self.fs_manager.read_file(data).await {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::FsReadFile(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::FsWriteAtomic(data) => {
                    match self.fs_manager.write_atomic(data).await {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::FsWriteAtomic(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::FsSearch(data) => match self.fs_manager.search(data).await {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::FsSearch(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::PtyOpen(data) => match self.pty_manager.open_session(data).await {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::PtyOpen(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::PtyInput(data) => match self.pty_manager.write_input(data).await {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::PtyInput(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::PtyReadOutput(data) => {
                    match self.pty_manager.read_output(data).await {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::PtyReadOutput(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PtyResize(data) => match self.pty_manager.resize(data).await {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::PtyResize(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::PtyClose(data) => match self.pty_manager.close(data).await {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::PtyClose(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::PtyList(data) => match self.pty_manager.list_sessions(data).await {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::PtyList(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::SshKeysSync(data) => match self
                    .ssh_manager
                    .sync_authorized_keys(&data)
                {
                    Ok(res) => RpcResponse::ok(&req.request_id, ResponsePayload::SshKeysSync(res)),
                    Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                },
                RequestPayload::SshPolicyApply(data) => {
                    match self.ssh_manager.apply_user_shell_policy(&data) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::SshPolicyApply(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::SshDropinApply(data) => {
                    match self.ssh_manager.apply_sshd_dropin(&data) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::SshDropinApply(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::NodeRuntimeList(data) => {
                    match node::list_node_runtimes(self.runner.as_ref(), data.base_dir.as_deref()) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::NodeRuntimeList(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::NodeRuntimeInstall(data) => {
                    match node::install_node_runtime(self.runner.as_ref(), &data) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::NodeRuntimeInstalled(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::NodeRuntimeRemove(data) => {
                    match node::remove_node_runtime(self.runner.as_ref(), &data) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::NodeRuntimeRemoved(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::NodeRuntimeCorepack(data) => {
                    match node::enable_node_corepack(self.runner.as_ref(), &data) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::NodeRuntimeCorepack(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::NodeSiteApply(data) => {
                    match node_site::apply_node_site(self.runner.as_ref(), &data) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::NodeSiteApplied(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::NodeSiteRemove(data) => {
                    match node_site::remove_node_site(self.runner.as_ref(), &data) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::NodeSiteRemoved(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::NodeSiteAction(data) => {
                    match node_site::action_node_site(self.runner.as_ref(), &data) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::NodeSiteAction(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::NodeSiteStatus(data) => {
                    match node_site::status_node_site(self.runner.as_ref(), &data) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::NodeSiteStatus(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::NodeSiteJob(data) => {
                    match node_site::run_node_site_job(self.runner.as_ref(), &data) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::NodeSiteJob(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PythonRuntimeList(data) => {
                    match python_site::list_python_runtimes(self.runner.as_ref(), &data) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::PythonRuntimeList(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PythonRuntimeInstall(data) => {
                    match python_site::install_python_runtime(self.runner.as_ref(), &data) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::PythonRuntimeInstalled(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PythonRuntimeRemove(data) => {
                    match python_site::remove_python_runtime(self.runner.as_ref(), &data) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::PythonRuntimeRemoved(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PythonSiteApply(data) => {
                    match python_site::apply_python_site(self.runner.as_ref(), &data) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::PythonSiteApplied(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PythonSiteRemove(data) => {
                    match python_site::remove_python_site(self.runner.as_ref(), &data) {
                        Ok(res) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::PythonSiteRemoved(res),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PythonSiteAction(data) => {
                    match python_site::action_python_site(self.runner.as_ref(), &data) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::PythonSiteAction(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PythonSiteStatus(data) => {
                    match python_site::status_python_site(self.runner.as_ref(), &data) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::PythonSiteStatus(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::PythonSiteJob(data) => {
                    match python_site::run_python_site_job(self.runner.as_ref(), &data) {
                        Ok(res) => {
                            RpcResponse::ok(&req.request_id, ResponsePayload::PythonSiteJob(res))
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::ProxyUpstreamProbe(data) => {
                    let res = proxy::probe_upstream(&data).await;
                    RpcResponse::ok(&req.request_id, ResponsePayload::ProxyUpstreamProbed(res))
                }
                RequestPayload::SiteEnvEncrypt(data) => {
                    match crypto::get_or_create_master_key(self.runner.as_ref(), None) {
                        Ok(master_key) => {
                            match crypto::encrypt_env_vars(&master_key, &data.env_vars) {
                                Ok(encrypted_vars) => RpcResponse::ok(
                                    &req.request_id,
                                    ResponsePayload::SiteEnvEncrypted(
                                        panel_proto::SiteEnvEncryptResponse {
                                            site_id: data.site_id,
                                            encrypted_vars,
                                        },
                                    ),
                                ),
                                Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                            }
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::SiteEnvDecrypt(data) => {
                    match crypto::get_or_create_master_key(self.runner.as_ref(), None) {
                        Ok(master_key) => {
                            match crypto::decrypt_env_vars(&master_key, &data.encrypted_vars) {
                                Ok(env_vars) => RpcResponse::ok(
                                    &req.request_id,
                                    ResponsePayload::SiteEnvDecrypted(
                                        panel_proto::SiteEnvDecryptResponse {
                                            site_id: data.site_id,
                                            env_vars,
                                        },
                                    ),
                                ),
                                Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                            }
                        }
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
                RequestPayload::SiteEnvApply(data) => {
                    match crypto::apply_site_env_file(
                        self.runner.as_ref(),
                        &data.site_id,
                        &data.username,
                        data.env_file_path.as_deref(),
                        &data.encrypted_vars,
                        None,
                    ) {
                        Ok(path) => RpcResponse::ok(
                            &req.request_id,
                            ResponsePayload::SiteEnvApplied(panel_proto::SiteEnvApplyResponse {
                                site_id: data.site_id,
                                env_file_path: path,
                                success: true,
                                message: "Variabel lingkungan terenkripsi berhasil dipasang"
                                    .to_string(),
                            }),
                        ),
                        Err(e) => RpcResponse::err(&req.request_id, e.to_string()),
                    }
                }
            };

            let audit_record = audit::AgentAuditRecord::new(
                "panel-api",
                action,
                target,
                if response.success { "success" } else { "error" },
                Some(format!("req_id:{}", req.request_id)),
            );
            audit::record_audit(&audit_record);

            rpc.send_response(&response).await?;
        }

        Ok(())
    }
}

pub async fn run_agent_socket(path: impl AsRef<Path>) -> Result<(), AgentError> {
    let config = AgentConfig {
        socket_path: path.as_ref().to_path_buf(),
        allowed_uid: None,
    };
    let (tx, rx) = tokio::sync::broadcast::channel(1);
    let server = Arc::new(AgentServer::new(config));
    tokio::select! {
        res = server.run(rx) => res,
        _ = tokio::signal::ctrl_c() => {
            let _ = tx.send(());
            Ok(())
        }
    }
}
