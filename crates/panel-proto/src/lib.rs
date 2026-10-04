//! `panel-proto`: RPC protocol types, versioning, and length-delimited framing.

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum ProtoError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Protocol version mismatch: expected {expected}, got {got}")]
    VersionMismatch { expected: u32, got: u32 },
    #[error("Connection closed prematurely")]
    ConnectionClosed,
    #[error("Agent error: {0}")]
    AgentError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcRequest {
    pub proto_version: u32,
    pub request_id: String,
    pub payload: RequestPayload,
}

impl RpcRequest {
    pub fn new(request_id: impl Into<String>, payload: RequestPayload) -> Self {
        Self {
            proto_version: PROTOCOL_VERSION,
            request_id: request_id.into(),
            payload,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "cmd", content = "data")]
pub enum RequestPayload {
    Ping,
    Version,
    UserCreate(UserCreateRequest),
    UserSuspend(UserSuspendRequest),
    UserUnsuspend(UserUnsuspendRequest),
    UserDelete(UserDeleteRequest),
    SliceApply(SliceApplyRequest),
    SliceRemove(SliceRemoveRequest),
    QuotaSet(QuotaSetRequest),
    QuotaGet(QuotaGetRequest),
    QuotaScan(QuotaScanRequest),
    ResourceRead(ResourceReadRequest),
    HardeningCheck,
    HardeningApply(HardeningApplyRequest),
    NginxBaselineApply(NginxBaselineApplyRequest),
    SiteApply(SiteApplyRequest),
    SiteRemove(SiteRemoveRequest),
    SiteProvision(SiteProvisionRequest),
    AcmeChallengeDeploy(AcmeChallengeDeployRequest),
    AcmeChallengeCleanup(AcmeChallengeCleanupRequest),
    SslCertDeploy(SslCertDeployRequest),
    SiteLogsRead(SiteLogsReadRequest),
    PhpRepoSetup(PhpRepoSetupRequest),
    PhpRuntimeList(PhpRuntimeListRequest),
    PhpRuntimeInstall(PhpRuntimeInstallRequest),
    PhpRuntimeRemove(PhpRuntimeRemoveRequest),
    PhpExtensionList(PhpExtensionListRequest),
    PhpExtensionInstall(PhpExtensionInstallRequest),
    PhpExtensionToggle(PhpExtensionToggleRequest),
    PhpExtensionRemove(PhpExtensionRemoveRequest),
    PhpFpmMasterEnsure(PhpFpmMasterEnsureRequest),
    PhpFpmPoolApply(PhpFpmPoolApplyRequest),
    PhpFpmPoolRemove(PhpFpmPoolRemoveRequest),
    PhpFpmMasterStatus(PhpFpmMasterStatusRequest),
    PhpFpmMasterReload(PhpFpmMasterReloadRequest),
    PhpFpmMasterRestart(PhpFpmMasterRestartRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SliceApplyRequest {
    pub uid: u32,
    pub memory_max: Option<String>,
    pub memory_high: Option<String>,
    pub cpu_quota: Option<String>,
    pub tasks_max: Option<u64>,
    pub io_weight: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SliceRemoveRequest {
    pub uid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuotaSetRequest {
    pub uid: u32,
    pub username: String,
    pub disk_limit_bytes: u64,
    pub inode_limit: u64,
    pub mount_point: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuotaGetRequest {
    pub uid: u32,
    pub username: String,
    pub mount_point: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuotaScanRequest {
    pub uid: u32,
    pub username: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceReadRequest {
    pub uid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserCreateRequest {
    pub username: String,
    pub uid: u32,
    pub shell: Option<String>,
    pub home_dir: Option<String>,
    pub web_user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserSuspendRequest {
    pub username: String,
    pub uid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserUnsuspendRequest {
    pub username: String,
    pub uid: u32,
    pub shell: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserDeleteRequest {
    pub username: String,
    pub uid: u32,
    pub purge: bool,
    pub trash_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcResponse {
    pub proto_version: u32,
    pub request_id: String,
    pub success: bool,
    pub payload: ResponsePayload,
    pub error: Option<String>,
}

impl RpcResponse {
    pub fn ok(request_id: impl Into<String>, payload: ResponsePayload) -> Self {
        Self {
            proto_version: PROTOCOL_VERSION,
            request_id: request_id.into(),
            success: true,
            payload,
            error: None,
        }
    }

    pub fn err(request_id: impl Into<String>, message: impl Into<String>) -> Self {
        let msg = message.into();
        Self {
            proto_version: PROTOCOL_VERSION,
            request_id: request_id.into(),
            success: false,
            payload: ResponsePayload::Error(msg.clone()),
            error: Some(msg),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum ResponsePayload {
    Pong,
    Version(VersionInfo),
    UserCreated(UserCreatedResponse),
    UserSuspended(UserSuspendedResponse),
    UserUnsuspended(UserUnsuspendedResponse),
    UserDeleted(UserDeletedResponse),
    SliceApplied(SliceAppliedResponse),
    SliceRemoved(SliceRemovedResponse),
    QuotaSet(QuotaSetResponse),
    QuotaGet(QuotaGetResponse),
    QuotaScan(QuotaScanResponse),
    ResourceRead(ResourceReadResponse),
    HardeningStatus(Box<HardeningStatus>),
    HardeningApplied(HardeningApplyResponse),
    NginxBaselineApplied(NginxBaselineAppliedResponse),
    SiteApplied(SiteAppliedResponse),
    SiteRemoved(SiteRemovedResponse),
    SiteProvisioned(SiteProvisionResponse),
    AcmeChallengeDeployed(AcmeChallengeDeployedResponse),
    AcmeChallengeCleaned(AcmeChallengeCleanedResponse),
    SslCertDeployed(SslCertDeployedResponse),
    SiteLogsRead(SiteLogsReadResponse),
    PhpRepoSetup(PhpRepoSetupResponse),
    PhpRuntimeList(PhpRuntimeListResponse),
    PhpRuntimeInstalled(PhpRuntimeActionResponse),
    PhpRuntimeRemoved(PhpRuntimeActionResponse),
    PhpExtensionList(PhpExtensionListResponse),
    PhpExtensionAction(PhpExtensionActionResponse),
    PhpFpmMasterEnsure(PhpFpmMasterEnsureResponse),
    PhpFpmPoolApply(PhpFpmPoolApplyResponse),
    PhpFpmPoolRemove(PhpFpmPoolRemoveResponse),
    PhpFpmMasterStatus(PhpFpmMasterStatusResponse),
    PhpFpmMasterAction(PhpFpmMasterActionResponse),
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserCreatedResponse {
    pub username: String,
    pub uid: u32,
    pub home_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserSuspendedResponse {
    pub username: String,
    pub uid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserUnsuspendedResponse {
    pub username: String,
    pub uid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserDeletedResponse {
    pub username: String,
    pub uid: u32,
    pub purged: bool,
    pub archived_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SliceAppliedResponse {
    pub uid: u32,
    pub slice_name: String,
    pub dropin_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SliceRemovedResponse {
    pub uid: u32,
    pub slice_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuotaSetResponse {
    pub uid: u32,
    pub username: String,
    pub applied: bool,
    pub backend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuotaGetResponse {
    pub uid: u32,
    pub username: String,
    pub bytes_used: u64,
    pub bytes_limit: u64,
    pub inodes_used: u64,
    pub inodes_limit: u64,
    pub backend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuotaScanResponse {
    pub uid: u32,
    pub username: String,
    pub bytes_used: u64,
    pub inodes_used: u64,
    pub scan_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SliceResourceMetrics {
    pub uid: u32,
    pub slice_name: String,
    pub exists: bool,
    pub cpu_usage_usec: u64,
    pub cpu_user_usec: u64,
    pub cpu_system_usec: u64,
    pub cpu_nr_periods: u64,
    pub cpu_nr_throttled: u64,
    pub cpu_throttled_usec: u64,
    pub memory_current_bytes: u64,
    pub memory_peak_bytes: Option<u64>,
    pub pids_current: u64,
    pub pids_peak: Option<u64>,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
    pub io_read_ops: u64,
    pub io_write_ops: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceReadResponse {
    pub metrics: SliceResourceMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrerequisiteCheckItem {
    pub name: String,
    pub ok: bool,
    pub message: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HardeningStatus {
    pub cgroup_v2: PrerequisiteCheckItem,
    pub acl: PrerequisiteCheckItem,
    pub quota: PrerequisiteCheckItem,
    pub hidepid: PrerequisiteCheckItem,
    pub tenant_group: PrerequisiteCheckItem,
    pub overall_ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HardeningApplyRequest {
    pub enable_hidepid: bool,
    pub ensure_tenant_group: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HardeningApplyResponse {
    pub hidepid_applied: bool,
    pub tenant_group_created: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NginxBaselineApplyRequest {
    pub base_dir: Option<String>,
    pub log_dir: Option<String>,
    pub acme_dir: Option<String>,
    pub panel_api_port: Option<u16>,
    pub panel_listen_port: Option<u16>,
    pub test_config: bool,
    pub reload: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NginxBaselineAppliedResponse {
    pub base_dir: String,
    pub nginx_conf_path: String,
    pub conf_d_dir: String,
    pub snippets_dir: String,
    pub sites_available_dir: String,
    pub sites_enabled_dir: String,
    pub log_dir: String,
    pub acme_dir: String,
    pub test_passed: bool,
    pub reloaded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SiteApplyRequest {
    pub site_id: String,
    pub config_content: String,
    pub enable: bool,
    pub nginx_base_dir: Option<String>,
    pub test_config: bool,
    pub reload: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SiteAppliedResponse {
    pub site_id: String,
    pub available_path: String,
    pub enabled_path: Option<String>,
    pub enabled: bool,
    pub reloaded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SiteRemoveRequest {
    pub site_id: String,
    pub nginx_base_dir: Option<String>,
    pub reload: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SiteRemovedResponse {
    pub site_id: String,
    pub reloaded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SiteProvisionRequest {
    pub site_id: i64,
    pub username: String,
    pub uid: u32,
    pub home_dir: String,
    pub docroot: String,
    pub web_user: Option<String>,
    pub index_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SiteProvisionResponse {
    pub docroot: String,
    pub placeholder_written: bool,
    pub acls_applied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcmeChallengeDeployRequest {
    pub token: String,
    pub key_authorization: String,
    pub acme_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcmeChallengeDeployedResponse {
    pub challenge_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcmeChallengeCleanupRequest {
    pub token: String,
    pub acme_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcmeChallengeCleanedResponse {
    pub token: String,
    pub removed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SslCertDeployRequest {
    pub domain: String,
    pub fullchain_pem: String,
    pub privkey_pem: String,
    pub ssl_base_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SslCertDeployedResponse {
    pub domain: String,
    pub cert_path: String,
    pub key_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SiteLogsReadRequest {
    pub username: String,
    pub site_domain: String,
    pub log_type: String,
    pub lines: usize,
    pub query: Option<String>,
    pub log_base_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SiteLogsReadResponse {
    pub lines: Vec<String>,
    pub total_matched: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpRepoSetupRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpRepoSetupResponse {
    pub success: bool,
    pub ppa_configured: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpRuntimeListRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpVersionStatusDto {
    pub version: String,
    pub installed: bool,
    pub fpm_binary_exists: bool,
    pub cli_binary_exists: bool,
    pub service_masked: bool,
    pub status: String,
    pub fpm_binary_path: String,
    pub cli_binary_path: String,
    pub service_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpRuntimeListResponse {
    pub ppa_configured: bool,
    pub versions: Vec<PhpVersionStatusDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpRuntimeInstallRequest {
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpRuntimeRemoveRequest {
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpRuntimeActionResponse {
    pub version: String,
    pub success: bool,
    pub masked: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpExtensionDto {
    pub name: String,
    pub version: String,
    pub package_name: String,
    pub installed: bool,
    pub enabled: bool,
    pub is_base: bool,
    pub category: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpExtensionListRequest {
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpExtensionListResponse {
    pub version: String,
    pub extensions: Vec<PhpExtensionDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpExtensionInstallRequest {
    pub version: String,
    pub extension: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpExtensionToggleRequest {
    pub version: String,
    pub extension: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpExtensionRemoveRequest {
    pub version: String,
    pub extension: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpExtensionActionResponse {
    pub version: String,
    pub extension: String,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpFpmMasterEnsureRequest {
    pub username: String,
    pub uid: u32,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpFpmMasterEnsureResponse {
    pub service_name: String,
    pub config_dir: String,
    pub default_socket_path: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpFpmPoolApplyRequest {
    pub username: String,
    pub uid: u32,
    pub version: String,
    pub pool_name: String,
    pub docroot: String,
    pub socket_path: Option<String>,
    pub max_children: Option<u32>,
    pub process_idle_timeout_secs: Option<u32>,
    pub max_requests: Option<u32>,
    pub open_basedir: Option<Vec<String>>,
    pub disable_functions: Option<Vec<String>>,
    pub reload: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpFpmPoolApplyResponse {
    pub pool_name: String,
    pub pool_conf_path: String,
    pub socket_path: String,
    pub reloaded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpFpmPoolRemoveRequest {
    pub username: String,
    pub version: String,
    pub pool_name: String,
    pub reload: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpFpmPoolRemoveResponse {
    pub pool_name: String,
    pub removed: bool,
    pub remaining_pools: usize,
    pub master_stopped: bool,
    pub reloaded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpFpmMasterStatusRequest {
    pub username: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpFpmMasterStatusResponse {
    pub service_name: String,
    pub is_active: bool,
    pub substate: String,
    pub socket_exists: bool,
    pub pools_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpFpmMasterReloadRequest {
    pub username: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpFpmMasterRestartRequest {
    pub username: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpFpmMasterActionResponse {
    pub service_name: String,
    pub action: String,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VersionInfo {
    pub version: String,
    pub proto_version: u32,
    pub os: String,
    pub arch: String,
}

/// Helper to wrap any async IO stream with length-delimited JSON RPC framing.
pub struct RpcStream<S> {
    inner: Framed<S, LengthDelimitedCodec>,
}

impl<S> RpcStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(stream: S) -> Self {
        let codec = LengthDelimitedCodec::builder()
            .max_frame_length(16 * 1024 * 1024) // 16 MB max frame
            .new_codec();
        Self {
            inner: Framed::new(stream, codec),
        }
    }

    pub async fn send_request(&mut self, req: &RpcRequest) -> Result<(), ProtoError> {
        let bytes = serde_json::to_vec(req)?;
        self.inner.send(Bytes::from(bytes)).await?;
        Ok(())
    }

    pub async fn recv_request(&mut self) -> Result<RpcRequest, ProtoError> {
        match self.inner.next().await {
            Some(Ok(bytes)) => {
                let req: RpcRequest = serde_json::from_slice(&bytes)?;
                if req.proto_version != PROTOCOL_VERSION {
                    return Err(ProtoError::VersionMismatch {
                        expected: PROTOCOL_VERSION,
                        got: req.proto_version,
                    });
                }
                Ok(req)
            }
            Some(Err(e)) => Err(ProtoError::Io(e)),
            None => Err(ProtoError::ConnectionClosed),
        }
    }

    pub async fn send_response(&mut self, res: &RpcResponse) -> Result<(), ProtoError> {
        let bytes = serde_json::to_vec(res)?;
        self.inner.send(Bytes::from(bytes)).await?;
        Ok(())
    }

    pub async fn recv_response(&mut self) -> Result<RpcResponse, ProtoError> {
        match self.inner.next().await {
            Some(Ok(bytes)) => {
                let res: RpcResponse = serde_json::from_slice(&bytes)?;
                Ok(res)
            }
            Some(Err(e)) => Err(ProtoError::Io(e)),
            None => Err(ProtoError::ConnectionClosed),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rpc_serialization_ping() {
        let req = RpcRequest::new("req-1", RequestPayload::Ping);
        let serialized = serde_json::to_string(&req).expect("serialize");
        let deserialized: RpcRequest = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(req, deserialized);
    }

    #[tokio::test]
    async fn test_rpc_roundtrip_stream() {
        let (client_io, server_io) = tokio::io::duplex(1024);
        let mut client_rpc = RpcStream::new(client_io);
        let mut server_rpc = RpcStream::new(server_io);

        let client_task = tokio::spawn(async move {
            let req = RpcRequest::new("test-123", RequestPayload::Ping);
            client_rpc.send_request(&req).await.expect("send request");
            let res = client_rpc.recv_response().await.expect("recv response");
            assert_eq!(res.request_id, "test-123");
            assert_eq!(res.payload, ResponsePayload::Pong);
        });

        let server_task = tokio::spawn(async move {
            let req = server_rpc.recv_request().await.expect("recv request");
            assert_eq!(req.payload, RequestPayload::Ping);
            let res = RpcResponse::ok(req.request_id, ResponsePayload::Pong);
            server_rpc.send_response(&res).await.expect("send response");
        });

        let _ = tokio::try_join!(client_task, server_task).expect("roundtrip successful");
    }

    #[test]
    fn test_rpc_user_lifecycle_payloads() {
        let req = RpcRequest::new(
            "user-1",
            RequestPayload::UserCreate(UserCreateRequest {
                username: "p_alice".to_string(),
                uid: 10001,
                shell: Some("/usr/sbin/nologin".to_string()),
                home_dir: Some("/home/p_alice".to_string()),
                web_user: Some("www-data".to_string()),
            }),
        );
        let s = serde_json::to_string(&req).expect("serialize UserCreate");
        let de: RpcRequest = serde_json::from_str(&s).expect("deserialize UserCreate");
        assert_eq!(req, de);

        let res = RpcResponse::ok(
            "user-1",
            ResponsePayload::UserCreated(UserCreatedResponse {
                username: "p_alice".to_string(),
                uid: 10001,
                home_dir: "/home/p_alice".to_string(),
            }),
        );
        let s_res = serde_json::to_string(&res).expect("serialize UserCreated response");
        let de_res: RpcResponse = serde_json::from_str(&s_res).expect("deserialize UserCreated");
        assert_eq!(res, de_res);
    }

    #[test]
    fn test_rpc_slice_payloads() {
        let req = RpcRequest::new(
            "slice-1",
            RequestPayload::SliceApply(SliceApplyRequest {
                uid: 10001,
                memory_max: Some("1G".to_string()),
                memory_high: Some("800M".to_string()),
                cpu_quota: Some("150%".to_string()),
                tasks_max: Some(250),
                io_weight: Some(100),
            }),
        );
        let s = serde_json::to_string(&req).expect("serialize SliceApply");
        let de: RpcRequest = serde_json::from_str(&s).expect("deserialize SliceApply");
        assert_eq!(req, de);

        let res = RpcResponse::ok(
            "slice-1",
            ResponsePayload::SliceApplied(SliceAppliedResponse {
                uid: 10001,
                slice_name: "panel-u10001.slice".to_string(),
                dropin_path: "/etc/systemd/system/panel-u10001.slice.d/50-limits.conf".to_string(),
            }),
        );
        let s_res = serde_json::to_string(&res).expect("serialize SliceApplied");
        let de_res: RpcResponse = serde_json::from_str(&s_res).expect("deserialize SliceApplied");
        assert_eq!(res, de_res);

        let rem_req = RpcRequest::new(
            "slice-2",
            RequestPayload::SliceRemove(SliceRemoveRequest { uid: 10001 }),
        );
        let s_rem = serde_json::to_string(&rem_req).expect("serialize SliceRemove");
        let de_rem: RpcRequest = serde_json::from_str(&s_rem).expect("deserialize SliceRemove");
        assert_eq!(rem_req, de_rem);
    }

    #[test]
    fn test_rpc_quota_payloads() {
        let set_req = RpcRequest::new(
            "quota-1",
            RequestPayload::QuotaSet(QuotaSetRequest {
                uid: 10001,
                username: "p_alice".to_string(),
                disk_limit_bytes: 10 * 1024 * 1024 * 1024,
                inode_limit: 500_000,
                mount_point: Some("/home".to_string()),
            }),
        );
        let s_set = serde_json::to_string(&set_req).expect("serialize QuotaSet");
        let de_set: RpcRequest = serde_json::from_str(&s_set).expect("deserialize QuotaSet");
        assert_eq!(set_req, de_set);

        let get_res = RpcResponse::ok(
            "quota-2",
            ResponsePayload::QuotaGet(QuotaGetResponse {
                uid: 10001,
                username: "p_alice".to_string(),
                bytes_used: 1024 * 1024 * 50,
                bytes_limit: 10 * 1024 * 1024 * 1024,
                inodes_used: 1200,
                inodes_limit: 500_000,
                backend: "ext4".to_string(),
            }),
        );
        let s_get = serde_json::to_string(&get_res).expect("serialize QuotaGet");
        let de_get: RpcResponse = serde_json::from_str(&s_get).expect("deserialize QuotaGet");
        assert_eq!(get_res, de_get);

        let scan_res = RpcResponse::ok(
            "quota-3",
            ResponsePayload::QuotaScan(QuotaScanResponse {
                uid: 10001,
                username: "p_alice".to_string(),
                bytes_used: 2048,
                inodes_used: 10,
                scan_duration_ms: 15,
            }),
        );
        let s_scan = serde_json::to_string(&scan_res).expect("serialize QuotaScan");
        let de_scan: RpcResponse = serde_json::from_str(&s_scan).expect("deserialize QuotaScan");
        assert_eq!(scan_res, de_scan);
    }

    #[test]
    fn test_rpc_resource_read_payloads() {
        let req = RpcRequest::new(
            "res-1",
            RequestPayload::ResourceRead(ResourceReadRequest { uid: 10001 }),
        );
        let s_req = serde_json::to_string(&req).expect("serialize ResourceRead");
        let de_req: RpcRequest = serde_json::from_str(&s_req).expect("deserialize ResourceRead");
        assert_eq!(req, de_req);

        let res = RpcResponse::ok(
            "res-2",
            ResponsePayload::ResourceRead(ResourceReadResponse {
                metrics: SliceResourceMetrics {
                    uid: 10001,
                    slice_name: "panel-u10001.slice".to_string(),
                    exists: true,
                    cpu_usage_usec: 500_000,
                    cpu_user_usec: 400_000,
                    cpu_system_usec: 100_000,
                    cpu_nr_periods: 100,
                    cpu_nr_throttled: 2,
                    cpu_throttled_usec: 20_000,
                    memory_current_bytes: 64 * 1024 * 1024,
                    memory_peak_bytes: Some(128 * 1024 * 1024),
                    pids_current: 5,
                    pids_peak: Some(12),
                    io_read_bytes: 10 * 1024 * 1024,
                    io_write_bytes: 5 * 1024 * 1024,
                    io_read_ops: 256,
                    io_write_ops: 128,
                },
            }),
        );
        let s_res = serde_json::to_string(&res).expect("serialize ResourceRead response");
        let de_res: RpcResponse =
            serde_json::from_str(&s_res).expect("deserialize ResourceRead response");
        assert_eq!(res, de_res);
    }

    #[test]
    fn test_rpc_hardening_payloads() {
        let check_req = RpcRequest::new("h-1", RequestPayload::HardeningCheck);
        let s_chk = serde_json::to_string(&check_req).expect("serialize HardeningCheck");
        let de_chk: RpcRequest = serde_json::from_str(&s_chk).expect("deserialize HardeningCheck");
        assert_eq!(check_req, de_chk);

        let apply_req = RpcRequest::new(
            "h-2",
            RequestPayload::HardeningApply(HardeningApplyRequest {
                enable_hidepid: true,
                ensure_tenant_group: true,
            }),
        );
        let s_app = serde_json::to_string(&apply_req).expect("serialize HardeningApply");
        let de_app: RpcRequest = serde_json::from_str(&s_app).expect("deserialize HardeningApply");
        assert_eq!(apply_req, de_app);

        let status_res = RpcResponse::ok(
            "h-3",
            ResponsePayload::HardeningStatus(Box::new(HardeningStatus {
                cgroup_v2: PrerequisiteCheckItem {
                    name: "cgroup_v2".to_string(),
                    ok: true,
                    message: "cgroup v2 is active".to_string(),
                    details: None,
                },
                acl: PrerequisiteCheckItem {
                    name: "acl".to_string(),
                    ok: true,
                    message: "ACL is supported".to_string(),
                    details: None,
                },
                quota: PrerequisiteCheckItem {
                    name: "quota".to_string(),
                    ok: true,
                    message: "Quota is supported".to_string(),
                    details: None,
                },
                hidepid: PrerequisiteCheckItem {
                    name: "hidepid".to_string(),
                    ok: false,
                    message: "hidepid=2 not active".to_string(),
                    details: None,
                },
                tenant_group: PrerequisiteCheckItem {
                    name: "tenant_group".to_string(),
                    ok: true,
                    message: "Group exists".to_string(),
                    details: None,
                },
                overall_ok: false,
            })),
        );
        let s_stat = serde_json::to_string(&status_res).expect("serialize HardeningStatus");
        let de_stat: RpcResponse =
            serde_json::from_str(&s_stat).expect("deserialize HardeningStatus");
        assert_eq!(status_res, de_stat);
    }

    #[test]
    fn test_rpc_nginx_baseline_payloads() {
        let req = RpcRequest::new(
            "baseline-1",
            RequestPayload::NginxBaselineApply(NginxBaselineApplyRequest {
                base_dir: Some("/etc/panel/nginx".to_string()),
                log_dir: Some("/var/log/panel".to_string()),
                acme_dir: Some("/var/lib/panel/acme".to_string()),
                panel_api_port: Some(3000),
                panel_listen_port: Some(8443),
                test_config: true,
                reload: true,
            }),
        );
        let s = serde_json::to_string(&req).expect("serialize NginxBaselineApply");
        let de: RpcRequest = serde_json::from_str(&s).expect("deserialize NginxBaselineApply");
        assert_eq!(req, de);

        let res = RpcResponse::ok(
            "baseline-1",
            ResponsePayload::NginxBaselineApplied(NginxBaselineAppliedResponse {
                base_dir: "/etc/panel/nginx".to_string(),
                nginx_conf_path: "/etc/panel/nginx/nginx.conf".to_string(),
                conf_d_dir: "/etc/panel/nginx/conf.d".to_string(),
                snippets_dir: "/etc/panel/nginx/snippets".to_string(),
                sites_available_dir: "/etc/panel/nginx/sites-available".to_string(),
                sites_enabled_dir: "/etc/panel/nginx/sites-enabled".to_string(),
                log_dir: "/var/log/panel".to_string(),
                acme_dir: "/var/lib/panel/acme".to_string(),
                test_passed: true,
                reloaded: true,
            }),
        );
        let s_res = serde_json::to_string(&res).expect("serialize NginxBaselineApplied");
        let de_res: RpcResponse =
            serde_json::from_str(&s_res).expect("deserialize NginxBaselineApplied");
        assert_eq!(res, de_res);
    }

    #[test]
    fn test_rpc_site_logs_read_payloads() {
        let req = RpcRequest::new(
            "logs-1",
            RequestPayload::SiteLogsRead(SiteLogsReadRequest {
                username: "p_alice".to_string(),
                site_domain: "example.org".to_string(),
                log_type: "access".to_string(),
                lines: 50,
                query: Some("404".to_string()),
                log_base_dir: Some("/var/log/panel".to_string()),
            }),
        );
        let s = serde_json::to_string(&req).expect("serialize SiteLogsRead");
        let de: RpcRequest = serde_json::from_str(&s).expect("deserialize SiteLogsRead");
        assert_eq!(req, de);

        let res = RpcResponse::ok(
            "logs-1",
            ResponsePayload::SiteLogsRead(SiteLogsReadResponse {
                lines: vec!["{\"status\":404,\"request_uri\":\"/missing\"}".to_string()],
                total_matched: 1,
            }),
        );
        let s_res = serde_json::to_string(&res).expect("serialize SiteLogsRead response");
        let de_res: RpcResponse =
            serde_json::from_str(&s_res).expect("deserialize SiteLogsRead response");
        assert_eq!(res, de_res);
    }

    #[test]
    fn test_rpc_php_runtime_payloads() {
        let req_install = RpcRequest::new(
            "php-inst-1",
            RequestPayload::PhpRuntimeInstall(PhpRuntimeInstallRequest {
                version: "8.3".to_string(),
            }),
        );
        let s_req = serde_json::to_string(&req_install).expect("serialize PhpRuntimeInstall");
        let de_req: RpcRequest =
            serde_json::from_str(&s_req).expect("deserialize PhpRuntimeInstall");
        assert_eq!(req_install, de_req);

        let res_install = RpcResponse::ok(
            "php-inst-1",
            ResponsePayload::PhpRuntimeInstalled(PhpRuntimeActionResponse {
                version: "8.3".to_string(),
                success: true,
                masked: true,
                message: "PHP 8.3 installed and php8.3-fpm service masked".to_string(),
            }),
        );
        let s_res = serde_json::to_string(&res_install).expect("serialize PhpRuntimeInstalled");
        let de_res: RpcResponse =
            serde_json::from_str(&s_res).expect("deserialize PhpRuntimeInstalled");
        assert_eq!(res_install, de_res);

        let req_list = RpcRequest::new(
            "php-list-1",
            RequestPayload::PhpRuntimeList(PhpRuntimeListRequest {}),
        );
        let s_list = serde_json::to_string(&req_list).expect("serialize PhpRuntimeList");
        let de_list: RpcRequest =
            serde_json::from_str(&s_list).expect("deserialize PhpRuntimeList");
        assert_eq!(req_list, de_list);

        let res_list = RpcResponse::ok(
            "php-list-1",
            ResponsePayload::PhpRuntimeList(PhpRuntimeListResponse {
                ppa_configured: true,
                versions: vec![PhpVersionStatusDto {
                    version: "8.3".to_string(),
                    installed: true,
                    fpm_binary_exists: true,
                    cli_binary_exists: true,
                    service_masked: true,
                    status: "installed".to_string(),
                    fpm_binary_path: "/usr/sbin/php-fpm8.3".to_string(),
                    cli_binary_path: "/usr/bin/php8.3".to_string(),
                    service_name: "php8.3-fpm.service".to_string(),
                }],
            }),
        );
        let s_res_list =
            serde_json::to_string(&res_list).expect("serialize PhpRuntimeListResponse");
        let de_res_list: RpcResponse =
            serde_json::from_str(&s_res_list).expect("deserialize PhpRuntimeListResponse");
        assert_eq!(res_list, de_res_list);

        let req_ext = RpcRequest::new(
            "php-ext-1",
            RequestPayload::PhpExtensionToggle(PhpExtensionToggleRequest {
                version: "8.3".to_string(),
                extension: "redis".to_string(),
                enabled: true,
            }),
        );
        let s_ext = serde_json::to_string(&req_ext).expect("serialize PhpExtensionToggle");
        let de_ext: RpcRequest =
            serde_json::from_str(&s_ext).expect("deserialize PhpExtensionToggle");
        assert_eq!(req_ext, de_ext);

        let res_ext = RpcResponse::ok(
            "php-ext-1",
            ResponsePayload::PhpExtensionList(PhpExtensionListResponse {
                version: "8.3".to_string(),
                extensions: vec![PhpExtensionDto {
                    name: "redis".to_string(),
                    version: "8.3".to_string(),
                    package_name: "php8.3-redis".to_string(),
                    installed: true,
                    enabled: true,
                    is_base: false,
                    category: "Cache".to_string(),
                    description: "Driver Redis".to_string(),
                }],
            }),
        );
        let s_res_ext =
            serde_json::to_string(&res_ext).expect("serialize PhpExtensionListResponse");
        let de_res_ext: RpcResponse =
            serde_json::from_str(&s_res_ext).expect("deserialize PhpExtensionListResponse");
        assert_eq!(res_ext, de_res_ext);

        let req_fpm = RpcRequest::new(
            "fpm-req-1",
            RequestPayload::PhpFpmPoolApply(PhpFpmPoolApplyRequest {
                username: "p_alice".to_string(),
                uid: 10001,
                version: "8.3".to_string(),
                pool_name: "site-1".to_string(),
                docroot: "/home/p_alice/sites/example.com/public".to_string(),
                socket_path: None,
                max_children: Some(10),
                process_idle_timeout_secs: Some(10),
                max_requests: Some(500),
                open_basedir: None,
                disable_functions: None,
                reload: true,
            }),
        );
        let s_fpm = serde_json::to_string(&req_fpm).expect("serialize PhpFpmPoolApplyRequest");
        let de_fpm: RpcRequest =
            serde_json::from_str(&s_fpm).expect("deserialize PhpFpmPoolApplyRequest");
        assert_eq!(req_fpm, de_fpm);

        let res_fpm = RpcResponse::ok(
            "fpm-res-1",
            ResponsePayload::PhpFpmPoolApply(PhpFpmPoolApplyResponse {
                pool_name: "site-1".to_string(),
                pool_conf_path: "/etc/panel/php/p_alice-8.3/pool.d/site-1.conf".to_string(),
                socket_path: "/run/panel/php/p_alice-8.3.sock".to_string(),
                reloaded: true,
            }),
        );
        let s_res_fpm = serde_json::to_string(&res_fpm).expect("serialize PhpFpmPoolApplyResponse");
        let de_res_fpm: RpcResponse =
            serde_json::from_str(&s_res_fpm).expect("deserialize PhpFpmPoolApplyResponse");
        assert_eq!(res_fpm, de_res_fpm);
    }
}
