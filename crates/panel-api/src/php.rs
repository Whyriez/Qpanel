//! PHP runtime management endpoints.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use panel_db::repositories::SqliteSiteRepository;
use panel_runtime::php::PhpVersion;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::{RequireAdminCsrf, RequireAuth};
use crate::jobs::JobItem;
use crate::routes::AppState;

/// Status information of a specific PHP runtime version.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PhpVersionStatusItem {
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

/// Response containing all PHP runtime versions and PPA configuration status.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PhpRuntimeListApiResponse {
    pub ppa_configured: bool,
    pub versions: Vec<PhpVersionStatusItem>,
}

impl From<panel_proto::PhpVersionStatusDto> for PhpVersionStatusItem {
    fn from(dto: panel_proto::PhpVersionStatusDto) -> Self {
        Self {
            version: dto.version,
            installed: dto.installed,
            fpm_binary_exists: dto.fpm_binary_exists,
            cli_binary_exists: dto.cli_binary_exists,
            service_masked: dto.service_masked,
            status: dto.status,
            fpm_binary_path: dto.fpm_binary_path,
            cli_binary_path: dto.cli_binary_path,
            service_name: dto.service_name,
        }
    }
}

impl From<panel_proto::PhpRuntimeListResponse> for PhpRuntimeListApiResponse {
    fn from(res: panel_proto::PhpRuntimeListResponse) -> Self {
        Self {
            ppa_configured: res.ppa_configured,
            versions: res.versions.into_iter().map(Into::into).collect(),
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/php/versions",
    responses(
        (status = 200, description = "List available and installed PHP runtime versions", body = PhpRuntimeListApiResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Failed to communicate with agent")
    ),
    tag = "runtime"
)]
pub async fn list_php_versions_handler(
    State(state): State<AppState>,
    RequireAuth(_user): RequireAuth,
) -> Response {
    match state.agent_client.list_php_runtimes().await {
        Ok(res) => (StatusCode::OK, Json(PhpRuntimeListApiResponse::from(res))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Gagal memuat status runtime PHP: {}", e) })),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/php/repo/setup",
    responses(
        (status = 202, description = "PHP Sury/PPA repository setup job enqueued", body = JobItem),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden (Admin privileges and CSRF required)"),
        (status = 500, description = "Internal error")
    ),
    tag = "runtime"
)]
pub async fn setup_php_repo_handler(
    State(state): State<AppState>,
    RequireAdminCsrf(admin): RequireAdminCsrf,
) -> Response {
    let (job, _) = match state
        .job_runner
        .enqueue(
            Some(admin.id),
            None,
            "php.repo.setup".to_string(),
            "{}".to_string(),
            Some(3),
        )
        .await
    {
        Ok(res) => res,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Gagal membuat antrean job: {}", e) })),
            )
                .into_response();
        }
    };

    state.job_runner.notify.notify_one();

    state.audit_writer.log_async(
        Some(admin.id),
        &admin.username,
        "php.repo.setup",
        "system:apt".to_string(),
        "success",
        None,
        Some("Setup Sury PHP PPA repository".to_string()),
    );

    (StatusCode::ACCEPTED, Json(JobItem::from(job))).into_response()
}

#[utoipa::path(
    post,
    path = "/api/v1/php/versions/{version}/install",
    params(
        ("version" = String, Path, description = "PHP version (7.4, 8.0, 8.1, 8.2, 8.3, 8.4)")
    ),
    responses(
        (status = 202, description = "PHP install job enqueued", body = JobItem),
        (status = 400, description = "Invalid PHP version"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden (Admin privileges and CSRF required)"),
        (status = 500, description = "Internal error")
    ),
    tag = "runtime"
)]
pub async fn install_php_version_handler(
    State(state): State<AppState>,
    RequireAdminCsrf(admin): RequireAdminCsrf,
    Path(version): Path<String>,
) -> Response {
    if let Err(e) = PhpVersion::parse(&version) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("Versi PHP tidak valid: {}", e) })),
        )
            .into_response();
    }

    let payload = serde_json::json!({ "version": version });
    let (job, _) = match state
        .job_runner
        .enqueue(
            Some(admin.id),
            None,
            "php.install".to_string(),
            payload.to_string(),
            Some(3),
        )
        .await
    {
        Ok(res) => res,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Gagal membuat antrean job instalasi: {}", e) })),
            )
                .into_response();
        }
    };

    state.job_runner.notify.notify_one();

    state.audit_writer.log_async(
        Some(admin.id),
        &admin.username,
        "php.install",
        format!("php:{}", version),
        "success",
        None,
        Some(format!("Memulai instalasi PHP {}", version)),
    );

    (StatusCode::ACCEPTED, Json(JobItem::from(job))).into_response()
}

#[utoipa::path(
    post,
    path = "/api/v1/php/versions/{version}/remove",
    params(
        ("version" = String, Path, description = "PHP version (7.4, 8.0, 8.1, 8.2, 8.3, 8.4)")
    ),
    responses(
        (status = 202, description = "PHP remove job enqueued", body = JobItem),
        (status = 400, description = "Invalid PHP version or version currently in use by sites"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden (Admin privileges and CSRF required)"),
        (status = 500, description = "Internal error")
    ),
    tag = "runtime"
)]
pub async fn remove_php_version_handler(
    State(state): State<AppState>,
    RequireAdminCsrf(admin): RequireAdminCsrf,
    Path(version): Path<String>,
) -> Response {
    if let Err(e) = PhpVersion::parse(&version) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("Versi PHP tidak valid: {}", e) })),
        )
            .into_response();
    }

    // Pre-check: Ensure no sites are currently using this PHP version
    let site_repo = SqliteSiteRepository::new(state.pool.clone());
    match site_repo
        .count_by_runtime_and_version("php", &version)
        .await
    {
        Ok(count) if count > 0 => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Versi PHP {} sedang digunakan oleh {} situs dan tidak dapat dihapus", version, count)
                })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Gagal memeriksa database situs: {}", e) })),
            )
                .into_response();
        }
        Ok(_) => {}
    }

    let payload = serde_json::json!({ "version": version });
    let (job, _) = match state
        .job_runner
        .enqueue(
            Some(admin.id),
            None,
            "php.remove".to_string(),
            payload.to_string(),
            Some(3),
        )
        .await
    {
        Ok(res) => res,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Gagal membuat antrean job penghapusan: {}", e) })),
            )
                .into_response();
        }
    };

    state.job_runner.notify.notify_one();

    state.audit_writer.log_async(
        Some(admin.id),
        &admin.username,
        "php.remove",
        format!("php:{}", version),
        "success",
        None,
        Some(format!("Memulai penghapusan PHP {}", version)),
    );

    (StatusCode::ACCEPTED, Json(JobItem::from(job))).into_response()
}
