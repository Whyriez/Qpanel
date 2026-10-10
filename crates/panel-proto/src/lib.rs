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
    PhpFpmHealthCheck(PhpFpmHealthCheckRequest),
    PhpFpmPoolStatus(PhpFpmPoolStatusRequest),
    PhpLogsRead(PhpLogsReadRequest),
    FsList(FsListRequest),
    FsStat(FsStatRequest),
    FsMkdir(FsMkdirRequest),
    FsRename(FsRenameRequest),
    FsCopy(FsCopyRequest),
    FsTrash(FsTrashRequest),
    FsTrashList(FsTrashListRequest),
    FsTrashRestore(FsTrashRestoreRequest),
    FsTrashEmpty(FsTrashEmptyRequest),
    FsDelete(FsDeleteRequest),
    FsUploadInit(FsUploadInitRequest),
    FsUploadGetSession(FsUploadGetSessionRequest),
    FsUploadAppendChunk(FsUploadAppendChunkRequest),
    FsReadRange(FsReadRangeRequest),
    FsZipDirectory(FsZipDirectoryRequest),
    FsArchiveCompress(FsArchiveCompressRequest),
    FsArchiveExtract(FsArchiveExtractRequest),
    FsChmod(FsChmodRequest),
    FsChmodRecursive(FsChmodRecursiveRequest),
    FsChown(FsChownRequest),
    FsReadFile(FsReadFileRequest),
    FsWriteAtomic(FsWriteAtomicRequest),
    FsSearch(FsSearchRequest),
    PtyOpen(PtyOpenRequest),
    PtyInput(PtyInputRequest),
    PtyReadOutput(PtyReadOutputRequest),
    PtyResize(PtyResizeRequest),
    PtyClose(PtyCloseRequest),
    PtyList(PtyListRequest),
    SshKeysSync(SshKeysSyncRequest),
    SshPolicyApply(SshPolicyApplyRequest),
    SshDropinApply(SshDropinApplyRequest),
    NodeRuntimeList(NodeRuntimeListRequest),
    NodeRuntimeInstall(NodeRuntimeInstallRequest),
    NodeRuntimeRemove(NodeRuntimeRemoveRequest),
    NodeRuntimeCorepack(NodeRuntimeCorepackRequest),
    NodeSiteApply(NodeSiteApplyRequest),
    NodeSiteRemove(NodeSiteRemoveRequest),
    NodeSiteAction(NodeSiteActionRequest),
    NodeSiteStatus(NodeSiteStatusRequest),
    NodeSiteJob(NodeSiteJobRequest),
    PythonRuntimeList(PythonRuntimeListRequest),
    PythonRuntimeInstall(PythonRuntimeInstallRequest),
    PythonRuntimeRemove(PythonRuntimeRemoveRequest),
    PythonSiteApply(PythonSiteApplyRequest),
    PythonSiteRemove(PythonSiteRemoveRequest),
    PythonSiteAction(PythonSiteActionRequest),
    PythonSiteStatus(PythonSiteStatusRequest),
    PythonSiteJob(PythonSiteJobRequest),
    ProxyUpstreamProbe(ProxyUpstreamProbeRequest),
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
    PhpFpmHealthChecked(PhpFpmHealthCheckResponse),
    PhpFpmPoolStatus(PhpFpmPoolStatusResponse),
    PhpLogsRead(PhpLogsReadResponse),
    FsList(FsListResponse),
    FsStat(FsStatResponse),
    FsTrash(FsTrashResponse),
    FsTrashList(FsTrashListResponse),
    FsTrashRestore(FsTrashRestoreResponse),
    FsTrashEmpty(FsTrashEmptyResponse),
    FsAction(FsActionResponse),
    FsUploadInit(FsUploadInitResponse),
    FsUploadGetSession(FsUploadGetSessionResponse),
    FsUploadAppendChunk(FsUploadAppendChunkResponse),
    FsReadRange(FsReadRangeResponse),
    FsZipDirectory(FsZipDirectoryResponse),
    FsArchiveCompress(FsArchiveCompressResponse),
    FsArchiveExtract(FsArchiveExtractResponse),
    FsChmod(FsChmodResponse),
    FsChmodRecursive(FsChmodRecursiveResponse),
    FsChown(FsChownResponse),
    FsReadFile(FsReadFileResponse),
    FsWriteAtomic(FsWriteAtomicResponse),
    FsSearch(FsSearchResponse),
    PtyOpen(PtyOpenResponse),
    PtyInput(PtyInputResponse),
    PtyReadOutput(PtyReadOutputResponse),
    PtyResize(PtyResizeResponse),
    PtyClose(PtyCloseResponse),
    PtyList(PtyListResponse),
    SshKeysSync(SshKeysSyncResponse),
    SshPolicyApply(SshPolicyApplyResponse),
    SshDropinApply(SshDropinApplyResponse),
    NodeRuntimeList(NodeRuntimeListResponse),
    NodeRuntimeInstalled(NodeRuntimeActionResponse),
    NodeRuntimeRemoved(NodeRuntimeActionResponse),
    NodeRuntimeCorepack(NodeRuntimeActionResponse),
    NodeSiteApplied(NodeSiteApplyResponse),
    NodeSiteRemoved(NodeSiteActionResponse),
    NodeSiteAction(NodeSiteActionResponse),
    NodeSiteStatus(NodeSiteStatusResponse),
    NodeSiteJob(NodeSiteJobResponse),
    PythonRuntimeList(PythonRuntimeListResponse),
    PythonRuntimeInstalled(PythonRuntimeActionResponse),
    PythonRuntimeRemoved(PythonRuntimeActionResponse),
    PythonSiteApplied(PythonSiteApplyResponse),
    PythonSiteRemoved(PythonSiteActionResponse),
    PythonSiteAction(PythonSiteActionResponse),
    PythonSiteStatus(PythonSiteStatusResponse),
    PythonSiteJob(PythonSiteJobResponse),
    ProxyUpstreamProbed(ProxyUpstreamProbeResponse),
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsEntryDto {
    pub name: String,
    pub relative_path: String,
    pub is_dir: bool,
    pub is_file: bool,
    pub is_symlink: bool,
    pub symlink_target: Option<String>,
    pub size_bytes: u64,
    pub modified_at: Option<i64>,
    pub created_at: Option<i64>,
    pub mode: u32,
    pub mode_octal: String,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsListRequest {
    pub username: String,
    pub path: String,
    pub limit: usize,
    pub offset: usize,
    pub sort_by: String,
    pub sort_order: String,
    pub show_hidden: bool,
    pub search_query: Option<String>,
    pub extension: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsListResponse {
    pub entries: Vec<FsEntryDto>,
    pub total_count: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsStatRequest {
    pub username: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsStatResponse {
    pub entry: FsEntryDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsMkdirRequest {
    pub username: String,
    pub path: String,
    pub recursive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsRenameRequest {
    pub username: String,
    pub source: String,
    pub destination: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsCopyRequest {
    pub username: String,
    pub source: String,
    pub destination: String,
    pub recursive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrashedItemDto {
    pub trash_id: String,
    pub original_path: String,
    pub name: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub trashed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsTrashRequest {
    pub username: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsTrashResponse {
    pub item: TrashedItemDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsTrashListRequest {
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsTrashListResponse {
    pub items: Vec<TrashedItemDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsTrashRestoreRequest {
    pub username: String,
    pub trash_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsTrashRestoreResponse {
    pub item: TrashedItemDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsTrashEmptyRequest {
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsTrashEmptyResponse {
    pub purged_count: usize,
    pub bytes_freed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsDeleteRequest {
    pub username: String,
    pub path: String,
    pub recursive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsActionResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UploadMetadataDto {
    pub upload_id: String,
    pub file_name: String,
    pub target_path: String,
    pub total_size: u64,
    pub current_offset: u64,
    pub created_at: i64,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsUploadInitRequest {
    pub username: String,
    pub target_path: String,
    pub file_name: String,
    pub total_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsUploadInitResponse {
    pub metadata: UploadMetadataDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsUploadGetSessionRequest {
    pub username: String,
    pub upload_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsUploadGetSessionResponse {
    pub metadata: UploadMetadataDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsUploadAppendChunkRequest {
    pub username: String,
    pub upload_id: String,
    pub offset: u64,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsUploadAppendChunkResponse {
    pub metadata: UploadMetadataDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsReadRangeRequest {
    pub username: String,
    pub path: String,
    pub start: u64,
    pub length: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsReadRangeResponse {
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsZipDirectoryRequest {
    pub username: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsZipDirectoryResponse {
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArchiveFormatDto {
    Zip,
    TarGz,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchiveResultDto {
    pub entries_processed: usize,
    pub total_bytes: u64,
    pub target_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsArchiveCompressRequest {
    pub username: String,
    pub source_paths: Vec<String>,
    pub destination: String,
    pub format: ArchiveFormatDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsArchiveCompressResponse {
    pub result: ArchiveResultDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsArchiveExtractRequest {
    pub username: String,
    pub archive_path: String,
    pub destination_dir: String,
    pub format: Option<ArchiveFormatDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsArchiveExtractResponse {
    pub result: ArchiveResultDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsChmodRequest {
    pub username: String,
    pub path: String,
    pub mode: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsChmodResponse {
    pub path: String,
    pub mode: u32,
    pub mode_octal: String,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsChmodRecursiveRequest {
    pub username: String,
    pub path: String,
    pub file_mode: u32,
    pub dir_mode: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsChmodRecursiveResponse {
    pub path: String,
    pub file_mode: u32,
    pub file_mode_octal: String,
    pub dir_mode: u32,
    pub dir_mode_octal: String,
    pub affected_count: usize,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsChownRequest {
    pub username: String,
    pub path: String,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub caller_uid: u32,
    pub is_admin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsChownResponse {
    pub path: String,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsReadFileRequest {
    pub username: String,
    pub path: String,
    pub max_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsReadFileResponse {
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsWriteAtomicRequest {
    pub username: String,
    pub path: String,
    pub data: Vec<u8>,
    pub mode: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsWriteAtomicResponse {
    pub bytes_written: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContentMatchDto {
    pub line_number: usize,
    pub line_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsSearchItemDto {
    pub path: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub modified_at: Option<i64>,
    pub matches: Vec<ContentMatchDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsSearchRequest {
    pub username: String,
    pub path: String,
    pub query: String,
    pub search_content: bool,
    pub case_sensitive: bool,
    pub extension: Option<String>,
    pub max_results: Option<usize>,
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsSearchResponse {
    pub items: Vec<FsSearchItemDto>,
    pub total_scanned: usize,
    pub limit_reached: bool,
    pub timed_out: bool,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PtyOpenRequest {
    pub username: String,
    pub uid: u32,
    pub gid: u32,
    pub cwd: Option<String>,
    pub rows: u16,
    pub cols: u16,
    pub term: Option<String>,
    pub max_sessions: Option<usize>,
    pub idle_timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PtyOpenResponse {
    pub session_id: String,
    pub username: String,
    pub pid: u32,
    pub cwd: String,
    pub rows: u16,
    pub cols: u16,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PtyInputRequest {
    pub session_id: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PtyInputResponse {
    pub session_id: String,
    pub bytes_written: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PtyReadOutputRequest {
    pub session_id: String,
    pub max_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PtyReadOutputResponse {
    pub session_id: String,
    pub data: Vec<u8>,
    pub closed: bool,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PtyResizeRequest {
    pub session_id: String,
    pub rows: u16,
    pub cols: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PtyResizeResponse {
    pub session_id: String,
    pub rows: u16,
    pub cols: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PtyCloseRequest {
    pub session_id: String,
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PtyCloseResponse {
    pub session_id: String,
    pub closed: bool,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PtyListRequest {
    pub username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PtySessionInfoDto {
    pub session_id: String,
    pub username: String,
    pub uid: u32,
    pub pid: u32,
    pub cwd: String,
    pub rows: u16,
    pub cols: u16,
    pub created_at: i64,
    pub last_active_at: i64,
    pub is_alive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PtyListResponse {
    pub sessions: Vec<PtySessionInfoDto>,
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
    #[serde(default)]
    pub site_id: Option<String>,
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
    #[serde(default)]
    pub settings: Option<panel_core::php::PhpSettings>,
    #[serde(default)]
    pub admin_ini: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PhpFpmStatusMetrics {
    pub pool: String,
    #[serde(rename = "process manager", default)]
    pub process_manager: String,
    #[serde(rename = "start time", default)]
    pub start_time: u64,
    #[serde(rename = "start since", default)]
    pub start_since: u64,
    #[serde(rename = "accepted conn", default)]
    pub accepted_conn: u64,
    #[serde(rename = "listen queue", default)]
    pub listen_queue: u32,
    #[serde(rename = "max listen queue", default)]
    pub max_listen_queue: u32,
    #[serde(rename = "listen queue len", default)]
    pub listen_queue_len: u32,
    #[serde(rename = "idle processes", default)]
    pub idle_processes: u32,
    #[serde(rename = "active processes", default)]
    pub active_processes: u32,
    #[serde(rename = "total processes", default)]
    pub total_processes: u32,
    #[serde(rename = "max active processes", default)]
    pub max_active_processes: u32,
    #[serde(rename = "max children reached", default)]
    pub max_children_reached: u32,
    #[serde(rename = "slow requests", default)]
    pub slow_requests: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpFpmMasterStatusResponse {
    pub service_name: String,
    pub is_active: bool,
    pub substate: String,
    pub socket_exists: bool,
    pub pools_count: usize,
    pub metrics: Option<PhpFpmStatusMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpFpmPoolStatusRequest {
    pub username: String,
    pub version: String,
    pub pool_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpFpmPoolStatusResponse {
    pub pool_name: String,
    pub service_name: String,
    pub is_active: bool,
    pub socket_exists: bool,
    pub metrics: Option<PhpFpmStatusMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpLogsReadRequest {
    pub username: String,
    pub version: String,
    pub pool_name: Option<String>,
    pub log_type: String, // "error" or "slow"
    pub lines: usize,
    pub query: Option<String>,
    pub log_base_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpLogsReadResponse {
    pub lines: Vec<String>,
    pub total_matched: usize,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpFpmHealthCheckRequest {
    pub username: String,
    pub version: String,
    pub pool_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpFpmHealthCheckResponse {
    pub healthy: bool,
    pub service_active: bool,
    pub socket_exists: bool,
    pub pool_exists: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SshKeysSyncRequest {
    pub linux_username: String,
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SshKeysSyncResponse {
    pub success: bool,
    pub path: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SshPolicyApplyRequest {
    pub linux_username: String,
    pub uid: u32,
    pub policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SshPolicyApplyResponse {
    pub success: bool,
    pub shell: String,
    pub sftp_group_member: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SshDropinApplyRequest {
    pub custom_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SshDropinApplyResponse {
    pub success: bool,
    pub path: String,
    pub validated: bool,
    pub reloaded: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeInstalledVersionDto {
    pub version: String,
    pub raw_version: String,
    pub path: String,
    pub node_binary: String,
    pub npm_binary: Option<String>,
    pub npx_binary: Option<String>,
    pub corepack_binary: Option<String>,
    pub pnpm_binary: Option<String>,
    pub yarn_binary: Option<String>,
    pub corepack_enabled: bool,
    pub is_lts: bool,
    pub lts_codename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRuntimeListRequest {
    pub base_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRuntimeListResponse {
    pub base_dir: String,
    pub versions: Vec<NodeInstalledVersionDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRuntimeInstallRequest {
    pub version: String,
    pub mirror_url: Option<String>,
    pub offline_tarball_path: Option<String>,
    pub offline_checksum: Option<String>,
    pub enable_corepack: bool,
    pub base_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRuntimeRemoveRequest {
    pub version: String,
    pub base_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRuntimeCorepackRequest {
    pub version: String,
    pub base_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRuntimeActionResponse {
    pub version: String,
    pub success: bool,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum NodeSiteEntrypoint {
    NodeScript(String),
    NpmStart,
    NpmCustom(String),
}

impl Default for NodeSiteEntrypoint {
    fn default() -> Self {
        Self::NodeScript("server.js".to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeSiteApplyRequest {
    pub site_id: String,
    pub username: String,
    pub uid: u32,
    pub node_version: String,
    pub app_dir: String,
    pub port: u16,
    pub entrypoint: NodeSiteEntrypoint,
    pub env_vars: std::collections::BTreeMap<String, String>,
    pub auto_restart: bool,
    pub env_file_path: Option<String>,
    pub unit_path: Option<String>,
    pub base_runtime_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeSiteApplyResponse {
    pub site_id: String,
    pub service_name: String,
    pub unit_path: String,
    pub env_path: String,
    pub port: u16,
    pub active: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeSiteRemoveRequest {
    pub site_id: String,
    pub unit_path: Option<String>,
    pub env_file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NodeSiteActionType {
    Start,
    Stop,
    Restart,
    Reload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeSiteActionRequest {
    pub site_id: String,
    pub action: NodeSiteActionType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeSiteActionResponse {
    pub site_id: String,
    pub action: String,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeSiteStatusRequest {
    pub site_id: String,
    pub port: Option<u16>,
    pub probe_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeSiteStatusResponse {
    pub site_id: String,
    pub service_name: String,
    pub active: bool,
    pub sub_state: String,
    pub pid: Option<u32>,
    pub memory_bytes: Option<u64>,
    pub cpu_usage_usec: Option<u64>,
    pub uptime_secs: Option<u64>,
    pub healthy: bool,
    pub probe_status_code: Option<u16>,
    pub probe_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeSiteJobType {
    NpmInstall,
    NpmCi,
    NpmBuild,
    PnpmInstall,
    YarnInstall,
    CustomScript { script_name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeSiteJobRequest {
    pub site_id: String,
    pub username: String,
    pub uid: u32,
    pub node_version: String,
    pub app_dir: String,
    pub job_type: NodeSiteJobType,
    pub timeout_secs: Option<u64>,
    pub base_runtime_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeSiteJobResponse {
    pub site_id: String,
    pub command: String,
    pub exit_code: i32,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PythonInstalledVersionDto {
    pub version: String,
    pub install_path: String,
    pub bin_path: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PythonRuntimeListRequest {
    pub base_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PythonRuntimeListResponse {
    pub installed: Vec<PythonInstalledVersionDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PythonRuntimeInstallRequest {
    pub version: String,
    pub base_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PythonRuntimeRemoveRequest {
    pub version: String,
    pub base_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PythonRuntimeActionResponse {
    pub version: String,
    pub success: bool,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PythonSiteFramework {
    Django,
    Flask,
    FastApi,
    #[default]
    Generic,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PythonSiteServer {
    #[default]
    GunicornWsgi,
    UvicornAsgi,
    GunicornUvicornWorker,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum PythonSiteEntrypoint {
    Wsgi(String),
    Asgi(String),
    CustomCommand(String),
}

impl Default for PythonSiteEntrypoint {
    fn default() -> Self {
        Self::Wsgi("app:app".to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PythonSiteApplyRequest {
    pub site_id: String,
    pub username: String,
    pub uid: u32,
    pub python_version: String,
    pub app_dir: String,
    pub venv_dir: String,
    pub socket_path: String,
    pub framework: PythonSiteFramework,
    pub server_type: PythonSiteServer,
    pub entrypoint: PythonSiteEntrypoint,
    pub workers: u32,
    pub env_vars: std::collections::BTreeMap<String, String>,
    pub auto_restart: bool,
    pub env_file_path: Option<String>,
    pub unit_path: Option<String>,
    pub base_runtime_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PythonSiteApplyResponse {
    pub site_id: String,
    pub service_name: String,
    pub unit_path: String,
    pub env_path: String,
    pub socket_path: String,
    pub active: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PythonSiteRemoveRequest {
    pub site_id: String,
    pub socket_path: Option<String>,
    pub unit_path: Option<String>,
    pub env_file_path: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PythonSiteActionType {
    Start,
    Stop,
    Restart,
    Redeploy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PythonSiteActionRequest {
    pub site_id: String,
    pub action: PythonSiteActionType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PythonSiteActionResponse {
    pub site_id: String,
    pub action: String,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PythonSiteStatusRequest {
    pub site_id: String,
    pub socket_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PythonSiteStatusResponse {
    pub site_id: String,
    pub service_name: String,
    pub active: bool,
    pub sub_state: String,
    pub pid: Option<u32>,
    pub memory_bytes: Option<u64>,
    pub cpu_usage_usec: Option<u64>,
    pub uptime_secs: Option<u64>,
    pub socket_exists: bool,
    pub healthy: bool,
    pub probe_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PythonSiteJobType {
    UvVenvCreate,
    UvPipInstallRequirements { req_file: Option<String> },
    UvPipInstallPyproject,
    CustomScript { script_name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PythonSiteJobRequest {
    pub site_id: String,
    pub username: String,
    pub uid: u32,
    pub python_version: String,
    pub app_dir: String,
    pub venv_dir: String,
    pub job_type: PythonSiteJobType,
    pub timeout_secs: Option<u64>,
    pub base_runtime_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PythonSiteJobResponse {
    pub site_id: String,
    pub command: String,
    pub exit_code: i32,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProxyUpstreamProbeRequest {
    pub upstream_url: String,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProxyUpstreamProbeResponse {
    pub healthy: bool,
    pub status_code: Option<u16>,
    pub latency_ms: u64,
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
                site_id: None,
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
                settings: None,
                admin_ini: None,
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

        let req_hc = RpcRequest::new(
            "hc-req-1",
            RequestPayload::PhpFpmHealthCheck(PhpFpmHealthCheckRequest {
                username: "p_alice".to_string(),
                version: "8.3".to_string(),
                pool_name: "site-1".to_string(),
            }),
        );
        let s_hc = serde_json::to_string(&req_hc).expect("serialize PhpFpmHealthCheckRequest");
        let de_hc: RpcRequest =
            serde_json::from_str(&s_hc).expect("deserialize PhpFpmHealthCheckRequest");
        assert_eq!(req_hc, de_hc);

        let res_hc = RpcResponse::ok(
            "hc-res-1",
            ResponsePayload::PhpFpmHealthChecked(PhpFpmHealthCheckResponse {
                healthy: true,
                service_active: true,
                socket_exists: true,
                pool_exists: true,
                message: "Healthy".to_string(),
            }),
        );
        let s_res_hc = serde_json::to_string(&res_hc).expect("serialize PhpFpmHealthCheckResponse");
        let de_res_hc: RpcResponse =
            serde_json::from_str(&s_res_hc).expect("deserialize PhpFpmHealthCheckResponse");
        assert_eq!(res_hc, de_res_hc);

        // FPM status metrics parsing test with PHP-FPM json format
        let raw_fpm_json = r#"{
            "pool": "site-1",
            "process manager": "ondemand",
            "start time": 1728000000,
            "start since": 3600,
            "accepted conn": 150,
            "listen queue": 2,
            "max listen queue": 5,
            "listen queue len": 128,
            "idle processes": 3,
            "active processes": 1,
            "total processes": 4,
            "max active processes": 8,
            "max children reached": 0,
            "slow requests": 1
        }"#;
        let metrics: PhpFpmStatusMetrics =
            serde_json::from_str(raw_fpm_json).expect("parse metrics");
        assert_eq!(metrics.pool, "site-1");
        assert_eq!(metrics.process_manager, "ondemand");
        assert_eq!(metrics.listen_queue, 2);
        assert_eq!(metrics.active_processes, 1);
        assert_eq!(metrics.idle_processes, 3);
        assert_eq!(metrics.slow_requests, 1);

        // Pool status RPC
        let req_status = RpcRequest::new(
            "status-req-1",
            RequestPayload::PhpFpmPoolStatus(PhpFpmPoolStatusRequest {
                username: "p_alice".to_string(),
                version: "8.3".to_string(),
                pool_name: "site-1".to_string(),
            }),
        );
        let s_status = serde_json::to_string(&req_status).unwrap();
        let de_status: RpcRequest = serde_json::from_str(&s_status).unwrap();
        assert_eq!(req_status, de_status);

        // Log read RPC
        let req_logs = RpcRequest::new(
            "log-req-1",
            RequestPayload::PhpLogsRead(PhpLogsReadRequest {
                username: "p_alice".to_string(),
                version: "8.3".to_string(),
                pool_name: Some("site-1".to_string()),
                log_type: "error".to_string(),
                lines: 50,
                query: Some("warning".to_string()),
                log_base_dir: None,
            }),
        );
        let s_logs = serde_json::to_string(&req_logs).unwrap();
        let de_logs: RpcRequest = serde_json::from_str(&s_logs).unwrap();
        assert_eq!(req_logs, de_logs);

        // PTY Open RPC test
        let req_pty_open = RpcRequest::new(
            "pty-open-1",
            RequestPayload::PtyOpen(PtyOpenRequest {
                username: "p_alice".to_string(),
                uid: 10001,
                gid: 10001,
                cwd: Some("sites/example.com".to_string()),
                rows: 24,
                cols: 80,
                term: Some("xterm-256color".to_string()),
                max_sessions: Some(5),
                idle_timeout_secs: Some(900),
            }),
        );
        let s_pty_open = serde_json::to_string(&req_pty_open).unwrap();
        let de_pty_open: RpcRequest = serde_json::from_str(&s_pty_open).unwrap();
        assert_eq!(req_pty_open, de_pty_open);

        let res_pty_open = RpcResponse::ok(
            "pty-open-1",
            ResponsePayload::PtyOpen(PtyOpenResponse {
                session_id: "test-sess-1".to_string(),
                username: "p_alice".to_string(),
                pid: 12345,
                cwd: "/home/p_alice/sites/example.com".to_string(),
                rows: 24,
                cols: 80,
                created_at: 1728000000,
            }),
        );
        let s_res_open = serde_json::to_string(&res_pty_open).unwrap();
        let de_res_open: RpcResponse = serde_json::from_str(&s_res_open).unwrap();
        assert_eq!(res_pty_open, de_res_open);

        // PTY Input RPC test
        let req_pty_in = RpcRequest::new(
            "pty-in-1",
            RequestPayload::PtyInput(PtyInputRequest {
                session_id: "test-sess-1".to_string(),
                data: b"ls -la\n".to_vec(),
            }),
        );
        let s_pty_in = serde_json::to_string(&req_pty_in).unwrap();
        let de_pty_in: RpcRequest = serde_json::from_str(&s_pty_in).unwrap();
        assert_eq!(req_pty_in, de_pty_in);

        // PTY Resize RPC test
        let req_pty_resize = RpcRequest::new(
            "pty-res-1",
            RequestPayload::PtyResize(PtyResizeRequest {
                session_id: "test-sess-1".to_string(),
                rows: 40,
                cols: 120,
            }),
        );
        let s_pty_resize = serde_json::to_string(&req_pty_resize).unwrap();
        let de_pty_resize: RpcRequest = serde_json::from_str(&s_pty_resize).unwrap();
        assert_eq!(req_pty_resize, de_pty_resize);

        // Node.js Runtime RPC tests
        let req_node_list = RpcRequest::new(
            "node-list-1",
            RequestPayload::NodeRuntimeList(NodeRuntimeListRequest {
                base_dir: Some("/opt/panel/runtimes/node".to_string()),
            }),
        );
        let s_node_list = serde_json::to_string(&req_node_list).unwrap();
        let de_node_list: RpcRequest = serde_json::from_str(&s_node_list).unwrap();
        assert_eq!(req_node_list, de_node_list);

        let req_node_install = RpcRequest::new(
            "node-install-1",
            RequestPayload::NodeRuntimeInstall(NodeRuntimeInstallRequest {
                version: "22.14.0".to_string(),
                mirror_url: Some("https://nodejs.org/dist".to_string()),
                offline_tarball_path: None,
                offline_checksum: None,
                enable_corepack: true,
                base_dir: None,
            }),
        );
        let s_node_install = serde_json::to_string(&req_node_install).unwrap();
        let de_node_install: RpcRequest = serde_json::from_str(&s_node_install).unwrap();
        assert_eq!(req_node_install, de_node_install);

        let res_node_action = RpcResponse::ok(
            "node-install-1",
            ResponsePayload::NodeRuntimeInstalled(NodeRuntimeActionResponse {
                version: "22.14.0".to_string(),
                success: true,
                path: "/opt/panel/runtimes/node/22.14.0".to_string(),
                message: "Node.js v22.14.0 berhasil dipasang".to_string(),
            }),
        );
        let s_node_res = serde_json::to_string(&res_node_action).unwrap();
        let de_node_res: RpcResponse = serde_json::from_str(&s_node_res).unwrap();
        assert_eq!(res_node_action, de_node_res);

        // Node.js Site RPC tests
        let mut env_vars = std::collections::BTreeMap::new();
        env_vars.insert("NODE_ENV".to_string(), "production".to_string());
        env_vars.insert("DB_HOST".to_string(), "127.0.0.1".to_string());

        let req_site_apply = RpcRequest::new(
            "node-site-1",
            RequestPayload::NodeSiteApply(NodeSiteApplyRequest {
                site_id: "site-42".to_string(),
                username: "p_alice".to_string(),
                uid: 10001,
                node_version: "22.14.0".to_string(),
                app_dir: "/home/p_alice/sites/node.test/app".to_string(),
                port: 20042,
                entrypoint: NodeSiteEntrypoint::NodeScript("server.js".to_string()),
                env_vars,
                auto_restart: true,
                env_file_path: None,
                unit_path: None,
                base_runtime_dir: None,
            }),
        );
        let s_site_apply = serde_json::to_string(&req_site_apply).unwrap();
        let de_site_apply: RpcRequest = serde_json::from_str(&s_site_apply).unwrap();
        assert_eq!(req_site_apply, de_site_apply);

        let res_site_apply = RpcResponse::ok(
            "node-site-1",
            ResponsePayload::NodeSiteApplied(NodeSiteApplyResponse {
                site_id: "site-42".to_string(),
                service_name: "site-site-42.service".to_string(),
                unit_path: "/etc/systemd/system/site-site-42.service".to_string(),
                env_path: "/etc/panel/env/site-site-42.env".to_string(),
                port: 20042,
                active: true,
                message: "Service site-site-42.service started".to_string(),
            }),
        );
        let s_res_apply = serde_json::to_string(&res_site_apply).unwrap();
        let de_res_apply: RpcResponse = serde_json::from_str(&s_res_apply).unwrap();
        assert_eq!(res_site_apply, de_res_apply);

        let req_site_action = RpcRequest::new(
            "node-action-1",
            RequestPayload::NodeSiteAction(NodeSiteActionRequest {
                site_id: "site-42".to_string(),
                action: NodeSiteActionType::Restart,
            }),
        );
        let s_site_action = serde_json::to_string(&req_site_action).unwrap();
        let de_site_action: RpcRequest = serde_json::from_str(&s_site_action).unwrap();
        assert_eq!(req_site_action, de_site_action);

        let req_site_status = RpcRequest::new(
            "node-status-1",
            RequestPayload::NodeSiteStatus(NodeSiteStatusRequest {
                site_id: "site-42".to_string(),
                port: Some(20042),
                probe_path: Some("/health".to_string()),
            }),
        );
        let s_site_status = serde_json::to_string(&req_site_status).unwrap();
        let de_site_status: RpcRequest = serde_json::from_str(&s_site_status).unwrap();
        assert_eq!(req_site_status, de_site_status);

        let res_site_status = RpcResponse::ok(
            "node-status-1",
            ResponsePayload::NodeSiteStatus(NodeSiteStatusResponse {
                site_id: "site-42".to_string(),
                service_name: "site-site-42.service".to_string(),
                active: true,
                sub_state: "running".to_string(),
                pid: Some(12345),
                memory_bytes: Some(64 * 1024 * 1024),
                cpu_usage_usec: Some(100_000),
                uptime_secs: Some(3600),
                healthy: true,
                probe_status_code: Some(200),
                probe_message: Some("OK".to_string()),
            }),
        );
        let s_res_status = serde_json::to_string(&res_site_status).unwrap();
        let de_res_status: RpcResponse = serde_json::from_str(&s_res_status).unwrap();
        assert_eq!(res_site_status, de_res_status);

        let req_site_job = RpcRequest::new(
            "node-job-1",
            RequestPayload::NodeSiteJob(NodeSiteJobRequest {
                site_id: "site-42".to_string(),
                username: "p_alice".to_string(),
                uid: 10001,
                node_version: "22.14.0".to_string(),
                app_dir: "/home/p_alice/sites/node.test/app".to_string(),
                job_type: NodeSiteJobType::NpmInstall,
                timeout_secs: Some(300),
                base_runtime_dir: None,
            }),
        );
        let s_site_job = serde_json::to_string(&req_site_job).unwrap();
        let de_site_job: RpcRequest = serde_json::from_str(&s_site_job).unwrap();
        assert_eq!(req_site_job, de_site_job);

        // Python Proto Serialization Tests
        let mut py_env = std::collections::BTreeMap::new();
        py_env.insert(
            "DJANGO_SETTINGS_MODULE".to_string(),
            "mysite.settings".to_string(),
        );
        let req_py_apply = RpcRequest::new(
            "py-apply-1",
            RequestPayload::PythonSiteApply(PythonSiteApplyRequest {
                site_id: "site_py42".to_string(),
                username: "p_alice".to_string(),
                uid: 10001,
                python_version: "3.12.9".to_string(),
                app_dir: "/home/p_alice/sites/django.test/app".to_string(),
                venv_dir: "/home/p_alice/sites/django.test/.venv".to_string(),
                socket_path: "/run/panel/python/site_py42.sock".to_string(),
                framework: PythonSiteFramework::Django,
                server_type: PythonSiteServer::GunicornWsgi,
                entrypoint: PythonSiteEntrypoint::Wsgi("mysite.wsgi:application".to_string()),
                workers: 3,
                env_vars: py_env,
                auto_restart: true,
                env_file_path: None,
                unit_path: None,
                base_runtime_dir: None,
            }),
        );
        let s_py_apply = serde_json::to_string(&req_py_apply).unwrap();
        let de_py_apply: RpcRequest = serde_json::from_str(&s_py_apply).unwrap();
        assert_eq!(req_py_apply, de_py_apply);

        let res_py_status = RpcResponse::ok(
            "py-status-1",
            ResponsePayload::PythonSiteStatus(PythonSiteStatusResponse {
                site_id: "site_py42".to_string(),
                service_name: "site-site_py42.service".to_string(),
                active: true,
                sub_state: "running".to_string(),
                pid: Some(54321),
                memory_bytes: Some(128 * 1024 * 1024),
                cpu_usage_usec: Some(250_000),
                uptime_secs: Some(7200),
                socket_exists: true,
                healthy: true,
                probe_message: Some("Socket probe OK".to_string()),
            }),
        );
        let s_py_status = serde_json::to_string(&res_py_status).unwrap();
        let de_py_status: RpcResponse = serde_json::from_str(&s_py_status).unwrap();
        assert_eq!(res_py_status, de_py_status);

        let req_py_job = RpcRequest::new(
            "py-job-1",
            RequestPayload::PythonSiteJob(PythonSiteJobRequest {
                site_id: "site_py42".to_string(),
                username: "p_alice".to_string(),
                uid: 10001,
                python_version: "3.12.9".to_string(),
                app_dir: "/home/p_alice/sites/django.test/app".to_string(),
                venv_dir: "/home/p_alice/sites/django.test/.venv".to_string(),
                job_type: PythonSiteJobType::UvPipInstallRequirements { req_file: None },
                timeout_secs: Some(300),
                base_runtime_dir: None,
            }),
        );
        let s_py_job = serde_json::to_string(&req_py_job).unwrap();
        let de_py_job: RpcRequest = serde_json::from_str(&s_py_job).unwrap();
        assert_eq!(req_py_job, de_py_job);

        let req_probe = RpcRequest::new(
            "proxy-probe-1",
            RequestPayload::ProxyUpstreamProbe(ProxyUpstreamProbeRequest {
                upstream_url: "http://127.0.0.1:8080".to_string(),
                timeout_ms: Some(2000),
            }),
        );
        let s_probe = serde_json::to_string(&req_probe).unwrap();
        let de_probe: RpcRequest = serde_json::from_str(&s_probe).unwrap();
        assert_eq!(req_probe, de_probe);

        let res_probe = RpcResponse::ok(
            "proxy-probe-1",
            ResponsePayload::ProxyUpstreamProbed(ProxyUpstreamProbeResponse {
                healthy: true,
                status_code: Some(200),
                latency_ms: 15,
                message: "Upstream healthy".to_string(),
            }),
        );
        let s_res_probe = serde_json::to_string(&res_probe).unwrap();
        let de_res_probe: RpcResponse = serde_json::from_str(&s_res_probe).unwrap();
        assert_eq!(res_probe, de_res_probe);
    }
}
