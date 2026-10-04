//! PHP runtime management endpoints.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use panel_db::models::UpdateSite;
use panel_db::repositories::SqliteSiteRepository;
use panel_runtime::php::{CURATED_PHP_EXTENSIONS, PhpVersion, find_curated_extension};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::{RequireAdminCsrf, RequireAuth, RequireCsrf};
use crate::jobs::JobItem;
use crate::routes::AppState;
use crate::sites::fetch_and_authorize_site;

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

/// Detailed information about an individual PHP extension.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PhpExtensionItem {
    pub name: String,
    pub version: String,
    pub package_name: String,
    pub installed: bool,
    pub enabled: bool,
    pub is_base: bool,
    pub category: String,
    pub description: String,
}

impl From<panel_proto::PhpExtensionDto> for PhpExtensionItem {
    fn from(dto: panel_proto::PhpExtensionDto) -> Self {
        Self {
            name: dto.name,
            version: dto.version,
            package_name: dto.package_name,
            installed: dto.installed,
            enabled: dto.enabled,
            is_base: dto.is_base,
            category: dto.category,
            description: dto.description,
        }
    }
}

/// Response containing all curated extensions for a specific PHP version.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PhpExtensionListApiResponse {
    pub version: String,
    pub extensions: Vec<PhpExtensionItem>,
}

impl From<panel_proto::PhpExtensionListResponse> for PhpExtensionListApiResponse {
    fn from(res: panel_proto::PhpExtensionListResponse) -> Self {
        Self {
            version: res.version,
            extensions: res.extensions.into_iter().map(Into::into).collect(),
        }
    }
}

/// Payload for toggling an extension's enabled/disabled state.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ToggleExtensionRequest {
    pub enabled: bool,
}

/// Response for extension actions (toggle, etc).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PhpExtensionActionApiResponse {
    pub version: String,
    pub extension: String,
    pub success: bool,
    pub message: String,
}

/// Response containing per-site PHP extensions configuration.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SitePhpExtensionsResponse {
    pub site_id: i64,
    pub php_version: String,
    pub extensions: Vec<String>,
    pub available_extensions: Vec<String>,
}

/// Input payload to update per-site PHP extension overrides.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateSitePhpExtensionsInput {
    pub extensions: Vec<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/php/versions/{version}/extensions",
    params(
        ("version" = String, Path, description = "PHP version (7.4, 8.0, 8.1, 8.2, 8.3, 8.4)")
    ),
    responses(
        (status = 200, description = "List extensions for PHP version", body = PhpExtensionListApiResponse),
        (status = 400, description = "Invalid PHP version"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Failed to communicate with agent")
    ),
    tag = "runtime"
)]
pub async fn list_php_extensions_handler(
    State(state): State<AppState>,
    RequireAuth(_user): RequireAuth,
    Path(version): Path<String>,
) -> Response {
    if let Err(e) = PhpVersion::parse(&version) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("Versi PHP tidak valid: {}", e) })),
        )
            .into_response();
    }

    match state.agent_client.list_php_extensions(&version).await {
        Ok(res) => (StatusCode::OK, Json(PhpExtensionListApiResponse::from(res))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Gagal memuat ekstensi PHP: {}", e) })),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/php/versions/{version}/extensions/{extension}/install",
    params(
        ("version" = String, Path, description = "PHP version"),
        ("extension" = String, Path, description = "Curated PHP extension name")
    ),
    responses(
        (status = 202, description = "Extension install job enqueued", body = JobItem),
        (status = 400, description = "Invalid version or extension name"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden (Admin privileges and CSRF required)"),
        (status = 500, description = "Internal error")
    ),
    tag = "runtime"
)]
pub async fn install_php_extension_handler(
    State(state): State<AppState>,
    RequireAdminCsrf(admin): RequireAdminCsrf,
    Path((version, extension)): Path<(String, String)>,
) -> Response {
    if let Err(e) = PhpVersion::parse(&version) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("Versi PHP tidak valid: {}", e) })),
        )
            .into_response();
    }
    let ext_def = match find_curated_extension(&extension) {
        Ok(def) => def,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let payload = serde_json::json!({
        "version": version,
        "extension": ext_def.name,
    });
    let (job, _) = match state
        .job_runner
        .enqueue(
            Some(admin.id),
            None,
            "php.extension.install".to_string(),
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
        "php.extension.install",
        format!("php{}:{}", version, ext_def.name),
        "success",
        None,
        Some(format!(
            "Memulai instalasi ekstensi {} untuk PHP {}",
            ext_def.name, version
        )),
    );

    (StatusCode::ACCEPTED, Json(JobItem::from(job))).into_response()
}

#[utoipa::path(
    post,
    path = "/api/v1/php/versions/{version}/extensions/{extension}/toggle",
    params(
        ("version" = String, Path, description = "PHP version"),
        ("extension" = String, Path, description = "Curated PHP extension name")
    ),
    request_body = ToggleExtensionRequest,
    responses(
        (status = 200, description = "Extension toggled successfully", body = PhpExtensionActionApiResponse),
        (status = 400, description = "Invalid version or extension name"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden (Admin privileges and CSRF required)"),
        (status = 500, description = "Internal error")
    ),
    tag = "runtime"
)]
pub async fn toggle_php_extension_handler(
    State(state): State<AppState>,
    RequireAdminCsrf(admin): RequireAdminCsrf,
    Path((version, extension)): Path<(String, String)>,
    Json(body): Json<ToggleExtensionRequest>,
) -> Response {
    if let Err(e) = PhpVersion::parse(&version) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("Versi PHP tidak valid: {}", e) })),
        )
            .into_response();
    }
    let ext_def = match find_curated_extension(&extension) {
        Ok(def) => def,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    match state
        .agent_client
        .toggle_php_extension(&version, ext_def.name, body.enabled)
        .await
    {
        Ok(res) => {
            state.audit_writer.log_async(
                Some(admin.id),
                &admin.username,
                "php.extension.toggle",
                format!("php{}:{}", version, ext_def.name),
                "success",
                None,
                Some(format!(
                    "Mengubah status ekstensi {} PHP {} menjadi {}",
                    ext_def.name,
                    version,
                    if body.enabled { "aktif" } else { "nonaktif" }
                )),
            );

            (
                StatusCode::OK,
                Json(PhpExtensionActionApiResponse {
                    version: res.version,
                    extension: res.extension,
                    success: res.success,
                    message: res.message,
                }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Gagal mengubah status ekstensi: {}", e) })),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/php/versions/{version}/extensions/{extension}/remove",
    params(
        ("version" = String, Path, description = "PHP version"),
        ("extension" = String, Path, description = "Curated PHP extension name")
    ),
    responses(
        (status = 202, description = "Extension remove job enqueued", body = JobItem),
        (status = 400, description = "Invalid version or base extension cannot be removed"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden (Admin privileges and CSRF required)"),
        (status = 500, description = "Internal error")
    ),
    tag = "runtime"
)]
pub async fn remove_php_extension_handler(
    State(state): State<AppState>,
    RequireAdminCsrf(admin): RequireAdminCsrf,
    Path((version, extension)): Path<(String, String)>,
) -> Response {
    if let Err(e) = PhpVersion::parse(&version) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("Versi PHP tidak valid: {}", e) })),
        )
            .into_response();
    }
    let ext_def = match find_curated_extension(&extension) {
        Ok(def) => def,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    if ext_def.is_base {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Ekstensi dasar '{}' tidak dapat dihapus karena merupakan modul inti PHP", ext_def.name)
            })),
        )
            .into_response();
    }

    let payload = serde_json::json!({
        "version": version,
        "extension": ext_def.name,
    });
    let (job, _) = match state
        .job_runner
        .enqueue(
            Some(admin.id),
            None,
            "php.extension.remove".to_string(),
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
        "php.extension.remove",
        format!("php{}:{}", version, ext_def.name),
        "success",
        None,
        Some(format!(
            "Memulai penghapusan ekstensi {} untuk PHP {}",
            ext_def.name, version
        )),
    );

    (StatusCode::ACCEPTED, Json(JobItem::from(job))).into_response()
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{id}/php/extensions",
    params(
        ("id" = i64, Path, description = "Site ID")
    ),
    responses(
        (status = 200, description = "Per-site PHP extensions preferences", body = SitePhpExtensionsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Site not found")
    ),
    tag = "runtime"
)]
pub async fn get_site_php_extensions_handler(
    State(state): State<AppState>,
    RequireAuth(auth_user): RequireAuth,
    Path(id): Path<i64>,
) -> Response {
    let site_repo = SqliteSiteRepository::new(state.pool.clone());
    let site = match fetch_and_authorize_site(&site_repo, &auth_user, id).await {
        Ok(s) => s,
        Err((code, msg)) => {
            return (code, Json(serde_json::json!({ "error": msg }))).into_response();
        }
    };

    let extensions = serde_json::from_str::<serde_json::Value>(&site.settings_json)
        .ok()
        .and_then(|val| {
            val.get("php")
                .and_then(|p| p.get("extensions"))
                .and_then(|arr| arr.as_array().cloned())
        })
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default();

    let available_extensions = CURATED_PHP_EXTENSIONS
        .iter()
        .map(|e| e.name.to_string())
        .collect();

    (
        StatusCode::OK,
        Json(SitePhpExtensionsResponse {
            site_id: site.id,
            php_version: site.version.unwrap_or_else(|| "8.3".to_string()),
            extensions,
            available_extensions,
        }),
    )
        .into_response()
}

#[utoipa::path(
    put,
    path = "/api/v1/sites/{id}/php/extensions",
    params(
        ("id" = i64, Path, description = "Site ID")
    ),
    request_body = UpdateSitePhpExtensionsInput,
    responses(
        (status = 200, description = "Per-site PHP extensions updated", body = SitePhpExtensionsResponse),
        (status = 400, description = "Invalid extension in list"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Site not found")
    ),
    tag = "runtime"
)]
pub async fn update_site_php_extensions_handler(
    State(state): State<AppState>,
    RequireCsrf(auth_user): RequireCsrf,
    Path(id): Path<i64>,
    Json(input): Json<UpdateSitePhpExtensionsInput>,
) -> Response {
    let site_repo = SqliteSiteRepository::new(state.pool.clone());
    let site = match fetch_and_authorize_site(&site_repo, &auth_user, id).await {
        Ok(s) => s,
        Err((code, msg)) => {
            return (code, Json(serde_json::json!({ "error": msg }))).into_response();
        }
    };

    // Validate that every requested extension is in CURATED_PHP_EXTENSIONS
    let mut normalized_extensions = Vec::with_capacity(input.extensions.len());
    for ext_name in &input.extensions {
        match find_curated_extension(ext_name) {
            Ok(def) => {
                if !normalized_extensions.contains(&def.name.to_string()) {
                    normalized_extensions.push(def.name.to_string());
                }
            }
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
                    .into_response();
            }
        }
    }

    // Merge into settings_json
    let mut root: serde_json::Value =
        serde_json::from_str(&site.settings_json).unwrap_or_else(|_| serde_json::json!({}));
    if !root.is_object() {
        root = serde_json::json!({});
    }
    if let Some(map) = root.as_object_mut() {
        let php_obj = map.entry("php").or_insert_with(|| serde_json::json!({}));
        if !php_obj.is_object() {
            *php_obj = serde_json::json!({});
        }
        if let Some(php_map) = php_obj.as_object_mut() {
            php_map.insert(
                "extensions".to_string(),
                serde_json::json!(normalized_extensions),
            );
        }
    }

    let updated_json = root.to_string();
    if let Err(e) = site_repo
        .update(
            site.id,
            &UpdateSite {
                settings_json: Some(updated_json),
                ..Default::default()
            },
        )
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                serde_json::json!({ "error": format!("Gagal menyimpan pengaturan situs: {}", e) }),
            ),
        )
            .into_response();
    }

    state.audit_writer.log_async(
        Some(auth_user.id),
        &auth_user.username,
        "site.php.extensions_update",
        format!("site:{}", site.id),
        "success",
        None,
        Some(format!(
            "Memperbarui ekstensi PHP situs: {:?}",
            normalized_extensions
        )),
    );

    let available_extensions = CURATED_PHP_EXTENSIONS
        .iter()
        .map(|e| e.name.to_string())
        .collect();

    (
        StatusCode::OK,
        Json(SitePhpExtensionsResponse {
            site_id: site.id,
            php_version: site.version.unwrap_or_else(|| "8.3".to_string()),
            extensions: normalized_extensions,
            available_extensions,
        }),
    )
        .into_response()
}
