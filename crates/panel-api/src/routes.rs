//! API route definitions and OpenAPI schemas.

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use panel_proto::PROTOCOL_VERSION;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{OpenApi, ToSchema};

use crate::agent_client::AgentClient;
use crate::auth::{
    ChangePasswordRequest, CsrfResponse, LockoutManager, LoginRequest, LoginResponse,
    MessageResponse, TwoFactorDisableRequest, TwoFactorEnableRequest, TwoFactorSetupResponse,
    TwoFactorStatusResponse, UserInfo,
};
use crate::config::AuthConfig;
use panel_db::pool::DbPool;

#[derive(OpenApi)]
#[openapi(
    paths(
        healthz_handler,
        system_info_handler,
        crate::auth::handlers::login_handler,
        crate::auth::handlers::logout_handler,
        crate::auth::handlers::me_handler,
        crate::auth::handlers::csrf_handler,
        crate::auth::handlers::change_password_handler,
        crate::auth::handlers::two_factor_setup_handler,
        crate::auth::handlers::two_factor_enable_handler,
        crate::auth::handlers::two_factor_disable_handler,
        crate::auth::handlers::two_factor_status_handler,
        crate::admin::handlers::admin_list_users_handler,
        crate::admin::handlers::admin_get_user_handler,
        crate::admin::handlers::admin_update_user_status_handler,
        crate::admin::handlers::admin_get_setting_handler,
        crate::admin::handlers::admin_update_setting_handler,
        crate::admin::handlers::user_get_profile_handler,
        crate::audit::handlers::list_audit_logs_handler,
        crate::audit::handlers::get_audit_log_handler,
        crate::jobs::handlers::create_job_handler,
        crate::jobs::handlers::list_jobs_handler,
        crate::jobs::handlers::get_job_handler,
        crate::jobs::handlers::cancel_job_handler,
        crate::jobs::handlers::job_events_handler,
        crate::ssl::get_site_ssl_handler,
        crate::ssl::issue_site_ssl_handler,
        crate::ssl::self_signed_site_ssl_handler,
        crate::ssl::custom_site_ssl_handler,
        crate::ssl::renew_site_ssl_handler,
        crate::ssl::delete_site_ssl_handler,
        crate::logs::get_site_logs_handler,
        crate::php::list_php_versions_handler,
        crate::php::setup_php_repo_handler,
        crate::php::install_php_version_handler,
        crate::php::remove_php_version_handler,
        crate::php::list_php_extensions_handler,
        crate::php::install_php_extension_handler,
        crate::php::toggle_php_extension_handler,
        crate::php::remove_php_extension_handler,
        crate::php::get_site_php_extensions_handler,
        crate::php::update_site_php_extensions_handler,
        crate::php::list_php_fpm_masters_handler,
        crate::php::get_php_fpm_master_status_handler,
        crate::php::reload_php_fpm_master_handler,
        crate::php::restart_php_fpm_master_handler,
        crate::php::get_site_php_status_handler,
        crate::php::get_site_php_logs_handler,
        crate::files::list_files_handler,
        crate::files::stat_file_handler,
        crate::files::mkdir_handler,
        crate::files::rename_handler,
        crate::files::copy_handler,
        crate::files::trash_handler,
        crate::files::list_trash_handler,
        crate::files::restore_trash_handler,
        crate::files::empty_trash_handler,
        crate::files::delete_permanent_handler,
        crate::files::upload_init_handler,
        crate::files::upload_status_handler,
        crate::files::upload_chunk_handler,
        crate::files::download_file_handler,
        crate::files::download_zip_handler,
        crate::files::archive_compress_handler,
        crate::files::archive_extract_handler,
        crate::files::chmod_handler,
        crate::files::chmod_recursive_handler,
        crate::files::chown_handler,
        crate::files::get_file_content_handler,
        crate::files::save_file_content_handler,
        crate::files::search_files_handler,
        crate::terminal::create_session_handler,
        crate::terminal::list_sessions_handler,
        crate::terminal::get_session_handler,
        crate::terminal::close_session_handler,
        crate::terminal::resize_session_handler,
        crate::terminal::get_terminal_audit_handler,
        crate::ssh::handlers::list_ssh_keys_handler,
        crate::ssh::handlers::create_ssh_key_handler,
        crate::ssh::handlers::delete_ssh_key_handler,
        crate::ssh::handlers::validate_ssh_key_handler,
        crate::ssh::handlers::apply_ssh_dropin_handler,
        crate::tenants::handlers::get_tenant_shell_policy_handler,
        crate::tenants::handlers::update_tenant_shell_policy_handler,
    ),
    components(
        schemas(
            HealthResponse,
            SystemInfoResponse,
            LoginRequest,
            LoginResponse,
            UserInfo,
            ChangePasswordRequest,
            MessageResponse,
            CsrfResponse,
            TwoFactorSetupResponse,
            TwoFactorEnableRequest,
            TwoFactorDisableRequest,
            TwoFactorStatusResponse,
            crate::admin::AdminUserItem,
            crate::admin::UpdateUserStatusRequest,
            crate::admin::UpdateSettingRequest,
            crate::admin::SettingResponse,
            crate::audit::AuditLogItem,
            crate::audit::AuditLogListResponse,
            crate::jobs::JobItem,
            crate::jobs::CreateJobRequest,
            crate::jobs::JobListResponse,
            crate::jobs::JobProgressEvent,
            crate::sites::CreateStaticSiteRequest,
            crate::sites::AddSiteDomainRequest,
            crate::sites::UpdateSiteDomainRequest,
            crate::sites::SetForceHttpsRequest,
            crate::sites::SiteResponse,
            crate::sites::SiteDomainResponse,
            crate::sites::SiteMessageResponse,
            crate::ssl::IssueSslRequest,
            crate::ssl::CustomCertRequest,
            crate::ssl::SslStatusResponse,
            crate::ssl::CertificateDetailResponse,
            crate::ssl::SslActionResponse,
            crate::logs::SiteLogsResponse,
            crate::php::PhpVersionStatusItem,
            crate::php::PhpRuntimeListApiResponse,
            crate::php::PhpExtensionItem,
            crate::php::PhpExtensionListApiResponse,
            crate::php::ToggleExtensionRequest,
            crate::php::PhpExtensionActionApiResponse,
            crate::php::SitePhpExtensionsResponse,
            crate::php::UpdateSitePhpExtensionsInput,
            crate::php::PhpFpmMasterOverviewDto,
            crate::php::PhpFpmMasterStatusDto,
            crate::php::PhpFpmActionResponseDto,
            crate::php::SwitchPhpVersionRequest,
            crate::php::SwitchPhpVersionResponse,
            crate::php::PhpFpmStatusMetricsDto,
            crate::php::SitePhpStatusResponse,
            crate::php::SitePhpLogsResponse,
            crate::files::MkdirPayload,
            crate::files::RenamePayload,
            crate::files::CopyPayload,
            crate::files::TrashPayload,
            crate::files::TrashRestorePayload,
            crate::files::DeletePayload,
            crate::files::FileActionResponse,
            crate::files::TrashEmptyApiResponse,
            crate::files::FsEntryItem,
            crate::files::FsListApiResponse,
            crate::files::TrashedItemApiResponse,
            crate::files::UploadMetadataApiResponse,
            crate::files::UploadInitPayload,
            crate::files::ArchiveCompressPayload,
            crate::files::ArchiveExtractPayload,
            crate::files::ArchiveResultApiResponse,
            crate::files::ArchiveActionResponse,
            crate::files::ChmodPayload,
            crate::files::ChmodApiResponse,
            crate::files::ChmodRecursivePayload,
            crate::files::ChmodRecursiveApiResponse,
            crate::files::ChownPayload,
            crate::files::ChownApiResponse,
            crate::files::FileContentApiResponse,
            crate::files::FileSavePayload,
            crate::files::FileSaveApiResponse,
            crate::files::FileSearchPayload,
            crate::files::ContentMatchApiDto,
            crate::files::SearchResultItemApi,
            crate::files::FileSearchApiResponse,
            crate::terminal::CreateTerminalSessionRequest,
            crate::terminal::TerminalSessionResponse,
            crate::terminal::TerminalSessionListResponse,
            crate::terminal::TerminalCloseApiResponse,
            crate::terminal::TerminalResizeRequest,
            crate::terminal::TerminalResizeResponse,
            crate::terminal::TerminalAuditItem,
            crate::terminal::TerminalAuditListResponse,
            crate::ssh::SshKeyResponse,
            crate::ssh::CreateSshKeyRequest,
            crate::ssh::ValidateSshKeyRequest,
            crate::ssh::ValidateSshKeyResponse,
            crate::ssh::SshDropinApplyRequestDto,
            crate::ssh::SshDropinApplyResponseDto,
            crate::tenants::TenantShellPolicyResponse,
            crate::tenants::UpdateTenantShellPolicyRequest,
        )
    ),
    tags(
        (name = "system", description = "System and health check endpoints"),
        (name = "auth", description = "Authentication and session management"),
        (name = "admin", description = "Administrator-only management endpoints"),
        (name = "user", description = "Tenant user resource endpoints"),
        (name = "audit", description = "Audit log query and tracking endpoints"),
        (name = "jobs", description = "Background jobs and real-time progress tracking endpoints"),
        (name = "ssl", description = "SSL/TLS and ACME certificate management"),
        (name = "runtime", description = "Runtime and PHP version management"),
        (name = "files", description = "File manager operations and trash management"),
        (name = "terminal", description = "Web terminal and PTY session endpoints"),
        (name = "ssh", description = "SSH public key management and validation"),
        (name = "tenants", description = "Tenant lifecycle and resource management")
    ),
    info(
        title = "Nusa Panel API",
        version = "0.1.0",
        description = "Lightweight, secure, tenant-isolated web hosting control panel API"
    )
)]
pub struct ApiDoc;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    #[schema(example = "ok")]
    pub status: String,
    pub timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SystemInfoResponse {
    #[schema(example = "0.1.0")]
    pub api_version: String,
    pub agent_version: Option<String>,
    pub proto_version: u32,
    pub os: String,
    pub arch: String,
    pub agent_connected: bool,
    pub environment: String,
}

#[derive(Clone)]
pub struct AppState {
    pub agent_client: Arc<AgentClient>,
    pub pool: DbPool,
    pub environment: String,
    pub auth_config: AuthConfig,
    pub lockout_manager: Arc<LockoutManager>,
    pub audit_writer: Arc<crate::audit::AuditWriter>,
    pub job_runner: Arc<crate::jobs::JobRunner>,
    pub terminal_hub: Arc<crate::terminal::TerminalHub>,
}

impl AppState {
    pub fn new(
        agent_client: Arc<AgentClient>,
        pool: DbPool,
        environment: String,
        auth_config: AuthConfig,
        lockout_manager: Arc<LockoutManager>,
    ) -> Self {
        let audit_writer = Arc::new(crate::audit::AuditWriter::new(pool.clone()));
        let job_registry = crate::jobs::JobRegistry::new_with_agent(Arc::clone(&agent_client));
        let job_runner = Arc::new(crate::jobs::JobRunner::with_registry(
            pool.clone(),
            job_registry,
        ));
        let terminal_hub = Arc::new(
            crate::terminal::TerminalHub::new(Arc::clone(&agent_client))
                .with_audit_writer(Arc::clone(&audit_writer)),
        );
        Self {
            agent_client,
            pool,
            environment,
            auth_config,
            lockout_manager,
            audit_writer,
            job_runner,
            terminal_hub,
        }
    }
}

#[utoipa::path(
    get,
    path = "/healthz",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse)
    ),
    tag = "system"
)]
pub async fn healthz_handler() -> impl IntoResponse {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "ok".to_string(),
            timestamp: now,
        }),
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/system/info",
    responses(
        (status = 200, description = "System information and agent status", body = SystemInfoResponse)
    ),
    tag = "system"
)]
pub async fn system_info_handler(State(state): State<AppState>) -> impl IntoResponse {
    let agent_ver = match state.agent_client.version().await {
        Ok(v) => (Some(v.version), true),
        Err(_) => (None, false),
    };

    (
        StatusCode::OK,
        Json(SystemInfoResponse {
            api_version: env!("CARGO_PKG_VERSION").to_string(),
            agent_version: agent_ver.0,
            proto_version: PROTOCOL_VERSION,
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            agent_connected: agent_ver.1,
            environment: state.environment.clone(),
        }),
    )
}
