//! Static website management API handlers and services.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use panel_core::domain::DomainName;
use panel_db::models::{CreateDomain, CreateSite, UpdateSite, User};
use panel_db::repositories::{
    domain_repo::SqliteDomainRepository, site_repo::SqliteSiteRepository,
    user_repo::SqliteUserRepository,
};
use panel_proto::{SiteApplyRequest, SiteProvisionRequest, SiteRemoveRequest};
use panel_webserver::template::{
    NginxTemplateRenderer, StaticVhostContext, render_placeholder_html,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::auth::middleware::AuthenticatedUser;
use crate::routes::AppState;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateStaticSiteRequest {
    pub domain: String,
    pub user_id: Option<i64>,
    pub spa_fallback: Option<bool>,
    pub docroot: Option<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ListSitesQuery {
    pub user_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct SiteDomainResponse {
    pub id: i64,
    pub name: String,
    pub unicode_name: String,
    pub is_primary: bool,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct SiteResponse {
    pub id: i64,
    pub user_id: i64,
    pub kind: String,
    pub primary_domain: String,
    pub docroot: String,
    pub status: String,
    pub spa_fallback: bool,
    pub domains: Vec<SiteDomainResponse>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct SiteMessageResponse {
    pub message: String,
    pub site_id: i64,
    pub status: String,
}

async fn resolve_target_user(
    user_repo: &SqliteUserRepository,
    auth_user: &AuthenticatedUser,
    requested_user_id: Option<i64>,
) -> Result<User, (StatusCode, &'static str)> {
    let target_user_id = if auth_user.is_admin() {
        requested_user_id.unwrap_or(auth_user.id)
    } else {
        auth_user.id
    };

    let user = user_repo
        .find_by_id(target_user_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or((StatusCode::NOT_FOUND, "Tenant user not found"))?;

    if user.status != "active" {
        return Err((
            StatusCode::BAD_REQUEST,
            "Cannot create site for suspended or inactive tenant",
        ));
    }

    Ok(user)
}

async fn provision_and_apply_vhost(
    state: &AppState,
    site_id: i64,
    target_user: &User,
    ascii_domain: &str,
    unicode_domain: &str,
    docroot: &str,
    spa_fallback: bool,
) -> Result<(), (StatusCode, String)> {
    let linux_username = target_user
        .linux_username
        .as_deref()
        .unwrap_or(target_user.username.as_str())
        .to_string();
    let linux_uid = target_user.linux_uid.unwrap_or(1000) as u32;
    let home_dir = format!("/home/{linux_username}");

    // Provision docroot and placeholder on host
    let placeholder = render_placeholder_html(unicode_domain);
    let prov_req = SiteProvisionRequest {
        site_id,
        username: linux_username.clone(),
        uid: linux_uid,
        home_dir,
        docroot: docroot.to_string(),
        web_user: Some("www-data".to_string()),
        index_content: Some(placeholder),
    };

    if let Err(e) = state.agent_client.provision_site(prov_req).await {
        tracing::warn!("Agent docroot provision warning: {e}");
    }

    // Render virtual host template
    let renderer = NginxTemplateRenderer::new();
    let vhost_ctx = StaticVhostContext {
        site_id: format!("site_{site_id}"),
        primary_domain: ascii_domain.to_string(),
        alias_domains: vec![],
        docroot: docroot.to_string(),
        log_dir: format!("/var/log/panel/{linux_username}/{ascii_domain}"),
        snippets_dir: "/etc/panel/nginx/snippets".to_string(),
        index_files: Some("index.html index.htm".to_string()),
        spa_fallback,
        has_ssl: false,
        ssl_cert: None,
        ssl_key: None,
        force_https: false,
        custom_headers: vec![],
    };

    let rendered_vhost = renderer.render_static(&vhost_ctx).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Vhost render failed: {e}"),
        )
    })?;

    // Apply virtual host atomically
    let apply_req = SiteApplyRequest {
        site_id: format!("site_{site_id}"),
        config_content: rendered_vhost,
        enable: true,
        nginx_base_dir: None,
        test_config: true,
        reload: true,
    };

    state
        .agent_client
        .apply_site(apply_req)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Host webserver apply failed: {e}"),
            )
        })?;

    Ok(())
}

/// Creates a new static site with atomic Nginx vhost apply and docroot provisioning.
pub async fn create_static_site_handler(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(payload): Json<CreateStaticSiteRequest>,
) -> impl IntoResponse {
    let site_repo = SqliteSiteRepository::new(state.pool.clone());
    let domain_repo = SqliteDomainRepository::new(state.pool.clone());
    let user_repo = SqliteUserRepository::new(state.pool.clone());

    // 1. Resolve target tenant
    let target_user = match resolve_target_user(&user_repo, &auth_user, payload.user_id).await {
        Ok(u) => u,
        Err((status, msg)) => {
            return (status, Json(serde_json::json!({ "error": msg }))).into_response();
        }
    };

    // 2. Validate domain
    let parsed_domain = match DomainName::parse(&payload.domain) {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": format!("Invalid domain name: {e}") })),
            )
                .into_response();
        }
    };
    let ascii_domain = parsed_domain.as_ascii().to_string();
    let unicode_domain = parsed_domain.as_unicode().to_string();

    // 3. Check domain global availability
    match domain_repo.is_domain_available(&ascii_domain).await {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({ "error": format!("Domain '{ascii_domain}' is already in use") })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    }

    // 4. Resolve docroot & settings
    let linux_uname = target_user
        .linux_username
        .as_deref()
        .unwrap_or(target_user.username.as_str());
    let docroot = payload
        .docroot
        .unwrap_or_else(|| format!("/home/{linux_uname}/sites/{ascii_domain}/public"));
    let spa_fallback = payload.spa_fallback.unwrap_or(false);
    let settings = serde_json::json!({ "spa_fallback": spa_fallback });

    // 5. Create database record for Site
    let site_input = CreateSite {
        user_id: target_user.id,
        kind: "static".to_string(),
        docroot: docroot.clone(),
        runtime: Some("none".to_string()),
        version: Some("none".to_string()),
        status: Some("active".to_string()),
        settings_json: Some(settings.to_string()),
    };

    let created_site = match site_repo.create(&site_input).await {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    // 6. Create primary domain record
    let domain_input = CreateDomain {
        site_id: created_site.id,
        name: ascii_domain.clone(),
        unicode_name: unicode_domain.clone(),
        is_primary: true,
        redirect_mode: Some("none".to_string()),
        redirect_target: None,
        status: Some("active".to_string()),
    };

    let created_domain = match domain_repo.create(&domain_input).await {
        Ok(d) => d,
        Err(e) => {
            let _ = site_repo.delete(created_site.id).await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    // 7. Provision host docroot and apply vhost atomically
    if let Err((status, msg)) = provision_and_apply_vhost(
        &state,
        created_site.id,
        &target_user,
        &ascii_domain,
        &unicode_domain,
        &docroot,
        spa_fallback,
    )
    .await
    {
        return (status, Json(serde_json::json!({ "error": msg }))).into_response();
    }

    // 8. Audit log
    state.audit_writer.log_async(
        Some(auth_user.id),
        &auth_user.username,
        "site.create",
        format!("site:{}", created_site.id),
        "success",
        None,
        Some(format!("domain:{ascii_domain},kind:static")),
    );

    (
        StatusCode::CREATED,
        Json(SiteResponse {
            id: created_site.id,
            user_id: created_site.user_id,
            kind: created_site.kind,
            primary_domain: ascii_domain,
            docroot: created_site.docroot,
            status: created_site.status,
            spa_fallback,
            domains: vec![SiteDomainResponse {
                id: created_domain.id,
                name: created_domain.name,
                unicode_name: created_domain.unicode_name,
                is_primary: created_domain.is_primary,
                status: created_domain.status,
            }],
            created_at: created_site.created_at,
            updated_at: created_site.updated_at,
        }),
    )
        .into_response()
}

/// Lists websites accessible to the authenticated user.
pub async fn list_sites_handler(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Query(query): Query<ListSitesQuery>,
) -> impl IntoResponse {
    let site_repo = SqliteSiteRepository::new(state.pool.clone());
    let domain_repo = SqliteDomainRepository::new(state.pool.clone());

    let sites_res = if auth_user.is_admin() {
        if let Some(uid) = query.user_id {
            site_repo.find_by_user_id(uid).await
        } else {
            site_repo.list_all().await
        }
    } else {
        site_repo.find_by_user_id(auth_user.id).await
    };

    let sites = match sites_res {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let mut response_list = Vec::with_capacity(sites.len());
    for s in sites {
        let domains = domain_repo.find_by_site_id(s.id).await.unwrap_or_default();

        let primary_name = domains
            .iter()
            .find(|d| d.is_primary)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let spa_fallback = serde_json::from_str::<serde_json::Value>(&s.settings_json)
            .ok()
            .and_then(|v| v.get("spa_fallback")?.as_bool())
            .unwrap_or(false);

        let domain_dtos = domains
            .into_iter()
            .map(|d| SiteDomainResponse {
                id: d.id,
                name: d.name,
                unicode_name: d.unicode_name,
                is_primary: d.is_primary,
                status: d.status,
            })
            .collect();

        response_list.push(SiteResponse {
            id: s.id,
            user_id: s.user_id,
            kind: s.kind,
            primary_domain: primary_name,
            docroot: s.docroot,
            status: s.status,
            spa_fallback,
            domains: domain_dtos,
            created_at: s.created_at,
            updated_at: s.updated_at,
        });
    }

    (StatusCode::OK, Json(response_list)).into_response()
}

/// Retrieves details for a specific site.
pub async fn get_site_handler(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let site_repo = SqliteSiteRepository::new(state.pool.clone());
    let domain_repo = SqliteDomainRepository::new(state.pool.clone());

    let site = match site_repo.find_by_id(id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Site not found" })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    if !auth_user.can_access_user(site.user_id) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Access denied" })),
        )
            .into_response();
    }

    let domains = domain_repo
        .find_by_site_id(site.id)
        .await
        .unwrap_or_default();

    let primary_name = domains
        .iter()
        .find(|d| d.is_primary)
        .map(|d| d.name.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let spa_fallback = serde_json::from_str::<serde_json::Value>(&site.settings_json)
        .ok()
        .and_then(|v| v.get("spa_fallback")?.as_bool())
        .unwrap_or(false);

    let domain_dtos = domains
        .into_iter()
        .map(|d| SiteDomainResponse {
            id: d.id,
            name: d.name,
            unicode_name: d.unicode_name,
            is_primary: d.is_primary,
            status: d.status,
        })
        .collect();

    (
        StatusCode::OK,
        Json(SiteResponse {
            id: site.id,
            user_id: site.user_id,
            kind: site.kind,
            primary_domain: primary_name,
            docroot: site.docroot,
            status: site.status,
            spa_fallback,
            domains: domain_dtos,
            created_at: site.created_at,
            updated_at: site.updated_at,
        }),
    )
        .into_response()
}

/// Enables a suspended site.
pub async fn enable_site_handler(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let site_repo = SqliteSiteRepository::new(state.pool.clone());
    let site = match site_repo.find_by_id(id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Site not found" })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    if !auth_user.can_access_user(site.user_id) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Access denied" })),
        )
            .into_response();
    }

    let updated = match site_repo
        .update(
            id,
            &UpdateSite {
                kind: None,
                docroot: None,
                runtime: None,
                version: None,
                status: Some("active".to_string()),
                settings_json: None,
            },
        )
        .await
    {
        Ok(u) => u,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    // Enable Nginx symlink
    let apply_req = SiteApplyRequest {
        site_id: format!("site_{id}"),
        config_content: String::new(),
        enable: true,
        nginx_base_dir: None,
        test_config: false,
        reload: true,
    };
    let _ = state.agent_client.apply_site(apply_req).await;

    state.audit_writer.log_async(
        Some(auth_user.id),
        &auth_user.username,
        "site.enable",
        format!("site:{id}"),
        "success",
        None,
        None,
    );

    (
        StatusCode::OK,
        Json(SiteMessageResponse {
            message: "Situs berhasil diaktifkan".to_string(),
            site_id: updated.id,
            status: updated.status,
        }),
    )
        .into_response()
}

/// Suspends/disables an active site.
pub async fn disable_site_handler(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let site_repo = SqliteSiteRepository::new(state.pool.clone());
    let site = match site_repo.find_by_id(id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Site not found" })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    if !auth_user.can_access_user(site.user_id) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Access denied" })),
        )
            .into_response();
    }

    let updated = match site_repo
        .update(
            id,
            &UpdateSite {
                kind: None,
                docroot: None,
                runtime: None,
                version: None,
                status: Some("suspended".to_string()),
                settings_json: None,
            },
        )
        .await
    {
        Ok(u) => u,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    // Disable Nginx symlink
    let apply_req = SiteApplyRequest {
        site_id: format!("site_{id}"),
        config_content: String::new(),
        enable: false,
        nginx_base_dir: None,
        test_config: false,
        reload: true,
    };
    let _ = state.agent_client.apply_site(apply_req).await;

    state.audit_writer.log_async(
        Some(auth_user.id),
        &auth_user.username,
        "site.disable",
        format!("site:{id}"),
        "success",
        None,
        None,
    );

    (
        StatusCode::OK,
        Json(SiteMessageResponse {
            message: "Situs berhasil dinonaktifkan".to_string(),
            site_id: updated.id,
            status: updated.status,
        }),
    )
        .into_response()
}

/// Deletes a website and its associated virtual host configurations.
pub async fn delete_site_handler(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let site_repo = SqliteSiteRepository::new(state.pool.clone());
    let site = match site_repo.find_by_id(id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Site not found" })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    if !auth_user.can_access_user(site.user_id) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Access denied" })),
        )
            .into_response();
    }

    // 1. Remove Nginx configuration on host via agent
    let rm_req = SiteRemoveRequest {
        site_id: format!("site_{id}"),
        nginx_base_dir: None,
        reload: true,
    };
    let _ = state.agent_client.remove_site(rm_req).await;

    // 2. Delete database record (cascading deletes domains)
    if let Err(e) = site_repo.delete(id).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    // 3. Audit log
    state.audit_writer.log_async(
        Some(auth_user.id),
        &auth_user.username,
        "site.delete",
        format!("site:{id}"),
        "success",
        None,
        Some(format!("docroot:{}", site.docroot)),
    );

    (
        StatusCode::OK,
        Json(SiteMessageResponse {
            message: "Situs berhasil dihapus".to_string(),
            site_id: id,
            status: "deleted".to_string(),
        }),
    )
        .into_response()
}
