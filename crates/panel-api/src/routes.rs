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
        )
    ),
    tags(
        (name = "system", description = "System and health check endpoints"),
        (name = "auth", description = "Authentication and session management")
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
