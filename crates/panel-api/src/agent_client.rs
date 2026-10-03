//! IPC client to communicate with `panel-agent` over a Unix domain socket.

use panel_proto::{
    HardeningApplyRequest, HardeningApplyResponse, HardeningStatus, ProtoError, QuotaGetRequest,
    QuotaGetResponse, QuotaScanRequest, QuotaScanResponse, QuotaSetRequest, QuotaSetResponse,
    RequestPayload, ResourceReadRequest, ResponsePayload, RpcRequest, RpcStream,
    SliceAppliedResponse, SliceApplyRequest, SliceRemoveRequest, SliceRemovedResponse,
    SliceResourceMetrics, UserCreateRequest, UserCreatedResponse, UserDeleteRequest,
    UserDeletedResponse, UserSuspendRequest, UserSuspendedResponse, UserUnsuspendRequest,
    UserUnsuspendedResponse, VersionInfo,
};
use std::path::PathBuf;
use thiserror::Error;

#[cfg(unix)]
type IpcStream = tokio::net::UnixStream;
#[cfg(not(unix))]
type IpcStream = tokio::net::TcpStream;

#[derive(Debug, Error)]
pub enum AgentClientError {
    #[error("Failed to connect to agent socket at {path}: {source}")]
    Connect {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Protocol error: {0}")]
    Proto(#[from] ProtoError),
    #[error("Agent returned error: {0}")]
    AgentError(String),
    #[error("Unexpected agent response: expected {expected}, got {got}")]
    UnexpectedResponse { expected: &'static str, got: String },
}

#[derive(Debug, Clone)]
pub struct AgentClient {
    socket_path: PathBuf,
}

impl AgentClient {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
        }
    }

    pub fn socket_path(&self) -> &std::path::Path {
        &self.socket_path
    }

    async fn connect(&self) -> Result<RpcStream<IpcStream>, AgentClientError> {
        #[cfg(unix)]
        {
            let stream = tokio::net::UnixStream::connect(&self.socket_path)
                .await
                .map_err(|e| AgentClientError::Connect {
                    path: self.socket_path.clone(),
                    source: e,
                })?;
            Ok(RpcStream::new(stream))
        }
        #[cfg(not(unix))]
        {
            let port_str = if self.socket_path.exists() {
                std::fs::read_to_string(&self.socket_path).unwrap_or_else(|_| "7701".to_string())
            } else {
                "7701".to_string()
            };
            let port: u16 = port_str.trim().parse().unwrap_or(7701);
            let addr = format!("127.0.0.1:{}", port);
            let stream = tokio::net::TcpStream::connect(&addr).await.map_err(|e| {
                AgentClientError::Connect {
                    path: self.socket_path.clone(),
                    source: e,
                }
            })?;
            Ok(RpcStream::new(stream))
        }
    }

    pub async fn ping(&self) -> Result<(), AgentClientError> {
        let mut rpc = self.connect().await?;
        let req_id = format!(
            "ping-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let req = RpcRequest::new(req_id, RequestPayload::Ping);
        rpc.send_request(&req).await?;
        let res = rpc.recv_response().await?;
        if !res.success {
            return Err(AgentClientError::AgentError(
                res.error
                    .unwrap_or_else(|| "Unknown agent error".to_string()),
            ));
        }
        match res.payload {
            ResponsePayload::Pong => Ok(()),
            other => Err(AgentClientError::UnexpectedResponse {
                expected: "Pong",
                got: format!("{other:?}"),
            }),
        }
    }

    pub async fn version(&self) -> Result<VersionInfo, AgentClientError> {
        let mut rpc = self.connect().await?;
        let req_id = format!(
            "ver-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let req = RpcRequest::new(req_id, RequestPayload::Version);
        rpc.send_request(&req).await?;
        let res = rpc.recv_response().await?;
        if !res.success {
            return Err(AgentClientError::AgentError(
                res.error
                    .unwrap_or_else(|| "Unknown agent error".to_string()),
            ));
        }
        match res.payload {
            ResponsePayload::Version(info) => Ok(info),
            other => Err(AgentClientError::UnexpectedResponse {
                expected: "Version",
                got: format!("{other:?}"),
            }),
        }
    }

    pub async fn create_user(
        &self,
        username: impl Into<String>,
        uid: u32,
        shell: Option<String>,
        home_dir: Option<String>,
        web_user: Option<String>,
    ) -> Result<UserCreatedResponse, AgentClientError> {
        let mut rpc = self.connect().await?;
        let username_str = username.into();
        let req_id = format!(
            "ucreate-{}-{}",
            username_str,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let req = RpcRequest::new(
            req_id,
            RequestPayload::UserCreate(UserCreateRequest {
                username: username_str,
                uid,
                shell,
                home_dir,
                web_user,
            }),
        );
        rpc.send_request(&req).await?;
        let res = rpc.recv_response().await?;
        if !res.success {
            return Err(AgentClientError::AgentError(
                res.error
                    .unwrap_or_else(|| "Unknown agent error".to_string()),
            ));
        }
        match res.payload {
            ResponsePayload::UserCreated(data) => Ok(data),
            other => Err(AgentClientError::UnexpectedResponse {
                expected: "UserCreated",
                got: format!("{other:?}"),
            }),
        }
    }

    pub async fn suspend_user(
        &self,
        username: impl Into<String>,
        uid: u32,
    ) -> Result<UserSuspendedResponse, AgentClientError> {
        let mut rpc = self.connect().await?;
        let username_str = username.into();
        let req_id = format!(
            "ususpend-{}-{}",
            username_str,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let req = RpcRequest::new(
            req_id,
            RequestPayload::UserSuspend(UserSuspendRequest {
                username: username_str,
                uid,
            }),
        );
        rpc.send_request(&req).await?;
        let res = rpc.recv_response().await?;
        if !res.success {
            return Err(AgentClientError::AgentError(
                res.error
                    .unwrap_or_else(|| "Unknown agent error".to_string()),
            ));
        }
        match res.payload {
            ResponsePayload::UserSuspended(data) => Ok(data),
            other => Err(AgentClientError::UnexpectedResponse {
                expected: "UserSuspended",
                got: format!("{other:?}"),
            }),
        }
    }

    pub async fn unsuspend_user(
        &self,
        username: impl Into<String>,
        uid: u32,
        shell: Option<String>,
    ) -> Result<UserUnsuspendedResponse, AgentClientError> {
        let mut rpc = self.connect().await?;
        let username_str = username.into();
        let req_id = format!(
            "uunsuspend-{}-{}",
            username_str,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let req = RpcRequest::new(
            req_id,
            RequestPayload::UserUnsuspend(UserUnsuspendRequest {
                username: username_str,
                uid,
                shell,
            }),
        );
        rpc.send_request(&req).await?;
        let res = rpc.recv_response().await?;
        if !res.success {
            return Err(AgentClientError::AgentError(
                res.error
                    .unwrap_or_else(|| "Unknown agent error".to_string()),
            ));
        }
        match res.payload {
            ResponsePayload::UserUnsuspended(data) => Ok(data),
            other => Err(AgentClientError::UnexpectedResponse {
                expected: "UserUnsuspended",
                got: format!("{other:?}"),
            }),
        }
    }

    pub async fn delete_user(
        &self,
        username: impl Into<String>,
        uid: u32,
        purge: bool,
        trash_dir: Option<String>,
    ) -> Result<UserDeletedResponse, AgentClientError> {
        let mut rpc = self.connect().await?;
        let username_str = username.into();
        let req_id = format!(
            "udelete-{}-{}",
            username_str,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let req = RpcRequest::new(
            req_id,
            RequestPayload::UserDelete(UserDeleteRequest {
                username: username_str,
                uid,
                purge,
                trash_dir,
            }),
        );
        rpc.send_request(&req).await?;
        let res = rpc.recv_response().await?;
        if !res.success {
            return Err(AgentClientError::AgentError(
                res.error
                    .unwrap_or_else(|| "Unknown agent error".to_string()),
            ));
        }
        match res.payload {
            ResponsePayload::UserDeleted(data) => Ok(data),
            other => Err(AgentClientError::UnexpectedResponse {
                expected: "UserDeleted",
                got: format!("{other:?}"),
            }),
        }
    }
    pub async fn apply_slice(
        &self,
        req: SliceApplyRequest,
    ) -> Result<SliceAppliedResponse, AgentClientError> {
        let mut rpc = self.connect().await?;
        let req_id = format!(
            "slice-apply-{}-{}",
            req.uid,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let rpc_req = RpcRequest::new(req_id, RequestPayload::SliceApply(req));
        rpc.send_request(&rpc_req).await?;
        let res = rpc.recv_response().await?;
        if !res.success {
            return Err(AgentClientError::AgentError(
                res.error
                    .unwrap_or_else(|| "Unknown agent error".to_string()),
            ));
        }
        match res.payload {
            ResponsePayload::SliceApplied(data) => Ok(data),
            other => Err(AgentClientError::UnexpectedResponse {
                expected: "SliceApplied",
                got: format!("{other:?}"),
            }),
        }
    }

    pub async fn remove_slice(&self, uid: u32) -> Result<SliceRemovedResponse, AgentClientError> {
        let mut rpc = self.connect().await?;
        let req_id = format!(
            "slice-remove-{}-{}",
            uid,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let rpc_req = RpcRequest::new(
            req_id,
            RequestPayload::SliceRemove(SliceRemoveRequest { uid }),
        );
        rpc.send_request(&rpc_req).await?;
        let res = rpc.recv_response().await?;
        if !res.success {
            return Err(AgentClientError::AgentError(
                res.error
                    .unwrap_or_else(|| "Unknown agent error".to_string()),
            ));
        }
        match res.payload {
            ResponsePayload::SliceRemoved(data) => Ok(data),
            other => Err(AgentClientError::UnexpectedResponse {
                expected: "SliceRemoved",
                got: format!("{other:?}"),
            }),
        }
    }

    pub async fn set_quota(
        &self,
        req: QuotaSetRequest,
    ) -> Result<QuotaSetResponse, AgentClientError> {
        let mut rpc = self.connect().await?;
        let req_id = format!(
            "quota-set-{}-{}",
            req.uid,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let rpc_req = RpcRequest::new(req_id, RequestPayload::QuotaSet(req));
        rpc.send_request(&rpc_req).await?;
        let res = rpc.recv_response().await?;
        if !res.success {
            return Err(AgentClientError::AgentError(
                res.error
                    .unwrap_or_else(|| "Unknown agent error".to_string()),
            ));
        }
        match res.payload {
            ResponsePayload::QuotaSet(data) => Ok(data),
            other => Err(AgentClientError::UnexpectedResponse {
                expected: "QuotaSet",
                got: format!("{other:?}"),
            }),
        }
    }

    pub async fn get_quota(
        &self,
        req: QuotaGetRequest,
    ) -> Result<QuotaGetResponse, AgentClientError> {
        let mut rpc = self.connect().await?;
        let req_id = format!(
            "quota-get-{}-{}",
            req.uid,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let rpc_req = RpcRequest::new(req_id, RequestPayload::QuotaGet(req));
        rpc.send_request(&rpc_req).await?;
        let res = rpc.recv_response().await?;
        if !res.success {
            return Err(AgentClientError::AgentError(
                res.error
                    .unwrap_or_else(|| "Unknown agent error".to_string()),
            ));
        }
        match res.payload {
            ResponsePayload::QuotaGet(data) => Ok(data),
            other => Err(AgentClientError::UnexpectedResponse {
                expected: "QuotaGet",
                got: format!("{other:?}"),
            }),
        }
    }

    pub async fn scan_quota(
        &self,
        req: QuotaScanRequest,
    ) -> Result<QuotaScanResponse, AgentClientError> {
        let mut rpc = self.connect().await?;
        let req_id = format!(
            "quota-scan-{}-{}",
            req.uid,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let rpc_req = RpcRequest::new(req_id, RequestPayload::QuotaScan(req));
        rpc.send_request(&rpc_req).await?;
        let res = rpc.recv_response().await?;
        if !res.success {
            return Err(AgentClientError::AgentError(
                res.error
                    .unwrap_or_else(|| "Unknown agent error".to_string()),
            ));
        }
        match res.payload {
            ResponsePayload::QuotaScan(data) => Ok(data),
            other => Err(AgentClientError::UnexpectedResponse {
                expected: "QuotaScan",
                got: format!("{other:?}"),
            }),
        }
    }

    pub async fn read_slice_resources(
        &self,
        uid: u32,
    ) -> Result<SliceResourceMetrics, AgentClientError> {
        let mut rpc = self.connect().await?;
        let req_id = format!(
            "resource-read-{}-{}",
            uid,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let rpc_req = RpcRequest::new(
            req_id,
            RequestPayload::ResourceRead(ResourceReadRequest { uid }),
        );
        rpc.send_request(&rpc_req).await?;
        let res = rpc.recv_response().await?;
        if !res.success {
            return Err(AgentClientError::AgentError(
                res.error
                    .unwrap_or_else(|| "Unknown agent error".to_string()),
            ));
        }
        match res.payload {
            ResponsePayload::ResourceRead(data) => Ok(data.metrics),
            other => Err(AgentClientError::UnexpectedResponse {
                expected: "ResourceRead",
                got: format!("{other:?}"),
            }),
        }
    }

    pub async fn check_hardening(&self) -> Result<HardeningStatus, AgentClientError> {
        let mut rpc = self.connect().await?;
        let req_id = format!(
            "hardening-check-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let rpc_req = RpcRequest::new(req_id, RequestPayload::HardeningCheck);
        rpc.send_request(&rpc_req).await?;
        let res = rpc.recv_response().await?;
        if !res.success {
            return Err(AgentClientError::AgentError(
                res.error
                    .unwrap_or_else(|| "Unknown agent error".to_string()),
            ));
        }
        match res.payload {
            ResponsePayload::HardeningStatus(data) => Ok(*data),
            other => Err(AgentClientError::UnexpectedResponse {
                expected: "HardeningStatus",
                got: format!("{other:?}"),
            }),
        }
    }

    pub async fn apply_hardening(
        &self,
        req: HardeningApplyRequest,
    ) -> Result<HardeningApplyResponse, AgentClientError> {
        let mut rpc = self.connect().await?;
        let req_id = format!(
            "hardening-apply-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let rpc_req = RpcRequest::new(req_id, RequestPayload::HardeningApply(req));
        rpc.send_request(&rpc_req).await?;
        let res = rpc.recv_response().await?;
        if !res.success {
            return Err(AgentClientError::AgentError(
                res.error
                    .unwrap_or_else(|| "Unknown agent error".to_string()),
            ));
        }
        match res.payload {
            ResponsePayload::HardeningApplied(data) => Ok(data),
            other => Err(AgentClientError::UnexpectedResponse {
                expected: "HardeningApplied",
                got: format!("{other:?}"),
            }),
        }
    }
}

pub fn plan_to_quota_set_request(
    uid: u32,
    username: impl Into<String>,
    plan: &panel_db::models::Plan,
) -> QuotaSetRequest {
    QuotaSetRequest {
        uid,
        username: username.into(),
        disk_limit_bytes: plan.disk_limit_bytes as u64,
        inode_limit: plan.inode_limit as u64,
        mount_point: None,
    }
}

pub fn plan_to_slice_apply_request(uid: u32, plan: &panel_db::models::Plan) -> SliceApplyRequest {
    SliceApplyRequest {
        uid,
        memory_max: Some(format!("{}M", plan.memory_limit_bytes / (1024 * 1024))),
        memory_high: plan
            .memory_high_bytes
            .map(|b| format!("{}M", b / (1024 * 1024))),
        cpu_quota: Some(format!("{}%", plan.cpu_quota_percent)),
        tasks_max: Some(plan.tasks_max as u64),
        io_weight: Some(plan.io_weight as u32),
    }
}
