//! `panel-agent`: Privileged root daemon executing typed system operations.

pub mod audit;
pub mod hardening;
pub mod php;
pub mod quota;
pub mod resource;
pub mod slice;
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
        Self {
            config,
            runner,
            user_manager,
            slice_manager,
            quota_manager,
            resource_reader,
            hardening_manager,
            webserver_manager,
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
        Self {
            config,
            runner,
            user_manager,
            slice_manager,
            quota_manager,
            resource_reader,
            hardening_manager,
            webserver_manager,
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
        Self {
            config,
            runner,
            user_manager,
            slice_manager,
            quota_manager,
            resource_reader,
            hardening_manager,
            webserver_manager,
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
        Self {
            config,
            runner,
            user_manager,
            slice_manager,
            quota_manager,
            resource_reader,
            hardening_manager,
            webserver_manager,
        }
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
