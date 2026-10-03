//! Static website and domain alias/redirect management API handlers and services.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use panel_core::domain::DomainName;
use panel_db::models::{
    CreateDomain, CreateSite, Domain, RedirectMode, Site, UpdateDomain, UpdateSite, User,
};
use panel_db::repositories::{
    domain_repo::SqliteDomainRepository, site_repo::SqliteSiteRepository,
    user_repo::SqliteUserRepository,
};
use panel_proto::{SiteApplyRequest, SiteProvisionRequest, SiteRemoveRequest};
use panel_webserver::template::{
    DomainRedirectConfig, NginxTemplateRenderer, StaticVhostContext, render_placeholder_html,
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
    pub force_https: Option<bool>,
    pub docroot: Option<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ListSitesQuery {
    pub user_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct AddSiteDomainRequest {
    pub domain: String,
    pub redirect_mode: Option<String>,
    pub redirect_target: Option<String>,
    pub is_primary: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct UpdateSiteDomainRequest {
    pub redirect_mode: Option<String>,
    pub redirect_target: Option<String>,
    pub is_primary: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct SetForceHttpsRequest {
    pub force_https: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct SiteDomainResponse {
    pub id: i64,
    pub site_id: i64,
    pub name: String,
    pub unicode_name: String,
    pub is_primary: bool,
    pub redirect_mode: String,
    pub redirect_target: Option<String>,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
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
    pub force_https: bool,
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

fn domain_to_response(d: &Domain) -> SiteDomainResponse {
    SiteDomainResponse {
        id: d.id,
        site_id: d.site_id,
        name: d.name.clone(),
        unicode_name: d.unicode_name.clone(),
        is_primary: d.is_primary,
        redirect_mode: d.redirect_mode.clone(),
        redirect_target: d.redirect_target.clone(),
        status: d.status.clone(),
        created_at: d.created_at,
        updated_at: d.updated_at,
    }
}

fn build_site_response(s: &Site, domains: &[Domain]) -> SiteResponse {
    let primary_name = domains
        .iter()
        .find(|d| d.is_primary)
        .map(|d| d.name.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let settings: serde_json::Value =
        serde_json::from_str(&s.settings_json).unwrap_or(serde_json::json!({}));
    let spa_fallback = settings
        .get("spa_fallback")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let force_https = settings
        .get("force_https")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let domain_dtos = domains.iter().map(domain_to_response).collect();

    SiteResponse {
        id: s.id,
        user_id: s.user_id,
        kind: s.kind.clone(),
        primary_domain: primary_name,
        docroot: s.docroot.clone(),
        status: s.status.clone(),
        spa_fallback,
        force_https,
        domains: domain_dtos,
        created_at: s.created_at,
        updated_at: s.updated_at,
    }
}

async fn fetch_and_authorize_site(
    site_repo: &SqliteSiteRepository,
    auth_user: &AuthenticatedUser,
    site_id: i64,
) -> Result<Site, (StatusCode, &'static str)> {
    let site = site_repo
        .find_by_id(site_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or((StatusCode::NOT_FOUND, "Site not found"))?;

    if !auth_user.can_access_user(site.user_id) {
        return Err((StatusCode::FORBIDDEN, "Access denied"));
    }

    Ok(site)
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

/// Recompiles the Nginx virtual host configuration for a site from its current DB state
/// and applies it atomically with automatic rollback via the privileged agent.
pub async fn recompile_and_apply_site_vhost(
    state: &AppState,
    site: &Site,
    user: &User,
    domains: &[Domain],
) -> Result<(), (StatusCode, String)> {
    let primary_domain = domains
        .iter()
        .find(|d| d.is_primary)
        .map(|d| d.name.as_str())
        .unwrap_or("unknown");

    let settings: serde_json::Value =
        serde_json::from_str(&site.settings_json).unwrap_or(serde_json::json!({}));
    let spa_fallback = settings
        .get("spa_fallback")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let force_https = settings
        .get("force_https")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let has_ssl = settings
        .get("has_ssl")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let ssl_cert = settings
        .get("ssl_cert")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let ssl_key = settings
        .get("ssl_key")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut alias_domains = Vec::new();
    let mut redirects = Vec::new();

    for d in domains {
        if d.is_primary {
            continue;
        }
        match d.redirect_mode.as_str() {
            "none" => {
                alias_domains.push(d.name.clone());
            }
            "to_primary" => {
                let target = if has_ssl || force_https {
                    format!("https://{primary_domain}$request_uri")
                } else {
                    format!("$scheme://{primary_domain}$request_uri")
                };
                redirects.push(DomainRedirectConfig {
                    domain: d.name.clone(),
                    target,
                    status_code: 301,
                });
            }
            "to_www" => {
                let target_host = if d.name.starts_with("www.") {
                    d.name.clone()
                } else {
                    format!("www.{}", d.name)
                };
                let target = if has_ssl || force_https {
                    format!("https://{target_host}$request_uri")
                } else {
                    format!("$scheme://{target_host}$request_uri")
                };
                redirects.push(DomainRedirectConfig {
                    domain: d.name.clone(),
                    target,
                    status_code: 301,
                });
            }
            "to_non_www" => {
                let target_host = if let Some(non_www) = d.name.strip_prefix("www.") {
                    non_www.to_string()
                } else {
                    d.name.clone()
                };
                let target = if has_ssl || force_https {
                    format!("https://{target_host}$request_uri")
                } else {
                    format!("$scheme://{target_host}$request_uri")
                };
                redirects.push(DomainRedirectConfig {
                    domain: d.name.clone(),
                    target,
                    status_code: 301,
                });
            }
            "custom" => {
                if let Some(ref cust) = d.redirect_target {
                    redirects.push(DomainRedirectConfig {
                        domain: d.name.clone(),
                        target: cust.clone(),
                        status_code: 301,
                    });
                }
            }
            _ => {}
        }
    }

    let linux_uname = user
        .linux_username
        .as_deref()
        .unwrap_or(user.username.as_str());

    let vhost_ctx = StaticVhostContext {
        site_id: format!("site_{}", site.id),
        primary_domain: primary_domain.to_string(),
        alias_domains,
        redirects,
        docroot: site.docroot.clone(),
        log_dir: format!("/var/log/panel/{linux_uname}/{primary_domain}"),
        snippets_dir: "/etc/panel/nginx/snippets".to_string(),
        index_files: Some("index.html index.htm".to_string()),
        spa_fallback,
        has_ssl,
        ssl_cert,
        ssl_key,
        force_https,
        custom_headers: vec![],
    };

    let renderer = NginxTemplateRenderer::new();
    let rendered = renderer.render_static(&vhost_ctx).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Vhost render failed: {e}"),
        )
    })?;

    let enable = site.status == "active";
    let apply_req = SiteApplyRequest {
        site_id: format!("site_{}", site.id),
        config_content: rendered,
        enable,
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
    let force_https = payload.force_https.unwrap_or(false);
    let settings = serde_json::json!({
        "spa_fallback": spa_fallback,
        "force_https": force_https
    });

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

    // 7. Provision host docroot via agent
    let placeholder = render_placeholder_html(&unicode_domain);
    let prov_req = SiteProvisionRequest {
        site_id: created_site.id,
        username: linux_uname.to_string(),
        uid: target_user.linux_uid.unwrap_or(1000) as u32,
        home_dir: format!("/home/{linux_uname}"),
        docroot: docroot.clone(),
        web_user: Some("www-data".to_string()),
        index_content: Some(placeholder),
    };
    if let Err(e) = state.agent_client.provision_site(prov_req).await {
        tracing::warn!("Agent docroot provision warning: {e}");
    }

    // 8. Recompile and apply vhost atomically
    let domains = vec![created_domain.clone()];
    if let Err((status, msg)) =
        recompile_and_apply_site_vhost(&state, &created_site, &target_user, &domains).await
    {
        return (status, Json(serde_json::json!({ "error": msg }))).into_response();
    }

    // 9. Audit log
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
        Json(build_site_response(&created_site, &domains)),
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
        response_list.push(build_site_response(&s, &domains));
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

    let site = match fetch_and_authorize_site(&site_repo, &auth_user, id).await {
        Ok(s) => s,
        Err((code, msg)) => {
            return (code, Json(serde_json::json!({ "error": msg }))).into_response();
        }
    };

    let domains = domain_repo
        .find_by_site_id(site.id)
        .await
        .unwrap_or_default();
    (StatusCode::OK, Json(build_site_response(&site, &domains))).into_response()
}

/// Enables a suspended site.
pub async fn enable_site_handler(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let site_repo = SqliteSiteRepository::new(state.pool.clone());
    let _site = match fetch_and_authorize_site(&site_repo, &auth_user, id).await {
        Ok(s) => s,
        Err((code, msg)) => {
            return (code, Json(serde_json::json!({ "error": msg }))).into_response();
        }
    };

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
    let _site = match fetch_and_authorize_site(&site_repo, &auth_user, id).await {
        Ok(s) => s,
        Err((code, msg)) => {
            return (code, Json(serde_json::json!({ "error": msg }))).into_response();
        }
    };

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
    let site = match fetch_and_authorize_site(&site_repo, &auth_user, id).await {
        Ok(s) => s,
        Err((code, msg)) => {
            return (code, Json(serde_json::json!({ "error": msg }))).into_response();
        }
    };

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

/// Lists all domains associated with a website.
pub async fn list_site_domains_handler(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(site_id): Path<i64>,
) -> impl IntoResponse {
    let site_repo = SqliteSiteRepository::new(state.pool.clone());
    let domain_repo = SqliteDomainRepository::new(state.pool.clone());

    if let Err((code, msg)) = fetch_and_authorize_site(&site_repo, &auth_user, site_id).await {
        return (code, Json(serde_json::json!({ "error": msg }))).into_response();
    }

    let domains = match domain_repo.find_by_site_id(site_id).await {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let dtos: Vec<SiteDomainResponse> = domains.iter().map(domain_to_response).collect();
    (StatusCode::OK, Json(dtos)).into_response()
}

/// Adds a new domain (alias, subdomain, or redirect) to a website.
pub async fn add_site_domain_handler(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(site_id): Path<i64>,
    Json(payload): Json<AddSiteDomainRequest>,
) -> impl IntoResponse {
    let site_repo = SqliteSiteRepository::new(state.pool.clone());
    let domain_repo = SqliteDomainRepository::new(state.pool.clone());
    let user_repo = SqliteUserRepository::new(state.pool.clone());

    let site = match fetch_and_authorize_site(&site_repo, &auth_user, site_id).await {
        Ok(s) => s,
        Err((code, msg)) => {
            return (code, Json(serde_json::json!({ "error": msg }))).into_response();
        }
    };

    let user = match user_repo.find_by_id(site.user_id).await {
        Ok(Some(u)) => u,
        _ => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Site owner user not found" })),
            )
                .into_response();
        }
    };

    // Validate domain format
    let parsed = match DomainName::parse(&payload.domain) {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": format!("Invalid domain name: {e}") })),
            )
                .into_response();
        }
    };
    let ascii_domain = parsed.as_ascii().to_string();
    let unicode_domain = parsed.as_unicode().to_string();

    // Check domain global availability
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

    // Validate redirect mode
    let redirect_mode_str = payload.redirect_mode.as_deref().unwrap_or("none");
    if RedirectMode::parse(redirect_mode_str).is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("Invalid redirect_mode '{redirect_mode_str}'") })),
        )
            .into_response();
    }

    let is_primary = payload.is_primary.unwrap_or(false);

    let create_input = CreateDomain {
        site_id,
        name: ascii_domain.clone(),
        unicode_name: unicode_domain,
        is_primary,
        redirect_mode: Some(redirect_mode_str.to_string()),
        redirect_target: payload.redirect_target,
        status: Some("active".to_string()),
    };

    let created_domain = match domain_repo.create(&create_input).await {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    // Recompile virtual host with new domain
    let all_domains = match domain_repo.find_by_site_id(site_id).await {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    if let Err((status, msg)) =
        recompile_and_apply_site_vhost(&state, &site, &user, &all_domains).await
    {
        // Rollback created domain in DB if apply fails
        let _ = domain_repo.delete(created_domain.id).await;
        return (status, Json(serde_json::json!({ "error": msg }))).into_response();
    }

    state.audit_writer.log_async(
        Some(auth_user.id),
        &auth_user.username,
        "domain.create",
        format!("domain:{}", created_domain.id),
        "success",
        None,
        Some(format!("site:{site_id},domain:{ascii_domain}")),
    );

    (
        StatusCode::CREATED,
        Json(domain_to_response(&created_domain)),
    )
        .into_response()
}

/// Updates domain configuration (redirect mode, redirect target, or primary promotion).
pub async fn update_site_domain_handler(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path((site_id, domain_id)): Path<(i64, i64)>,
    Json(payload): Json<UpdateSiteDomainRequest>,
) -> impl IntoResponse {
    let site_repo = SqliteSiteRepository::new(state.pool.clone());
    let domain_repo = SqliteDomainRepository::new(state.pool.clone());
    let user_repo = SqliteUserRepository::new(state.pool.clone());

    let site = match fetch_and_authorize_site(&site_repo, &auth_user, site_id).await {
        Ok(s) => s,
        Err((code, msg)) => {
            return (code, Json(serde_json::json!({ "error": msg }))).into_response();
        }
    };

    let user = match user_repo.find_by_id(site.user_id).await {
        Ok(Some(u)) => u,
        _ => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Site owner user not found" })),
            )
                .into_response();
        }
    };

    let existing_domain = match domain_repo.find_by_id(domain_id).await {
        Ok(Some(d)) if d.site_id == site_id => d,
        Ok(Some(_)) | Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Domain not found for this site" })),
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

    if payload
        .redirect_mode
        .as_deref()
        .is_some_and(|mode| RedirectMode::parse(mode).is_none())
    {
        let mode = payload.redirect_mode.as_deref().unwrap_or_default();
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("Invalid redirect_mode '{mode}'") })),
        )
            .into_response();
    }

    let updated_domain = match domain_repo
        .update(
            domain_id,
            &UpdateDomain {
                name: None,
                unicode_name: None,
                is_primary: payload.is_primary,
                redirect_mode: payload.redirect_mode,
                redirect_target: payload.redirect_target,
                status: None,
            },
        )
        .await
    {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let all_domains = match domain_repo.find_by_site_id(site_id).await {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    if let Err((status, msg)) =
        recompile_and_apply_site_vhost(&state, &site, &user, &all_domains).await
    {
        return (status, Json(serde_json::json!({ "error": msg }))).into_response();
    }

    state.audit_writer.log_async(
        Some(auth_user.id),
        &auth_user.username,
        "domain.update",
        format!("domain:{domain_id}"),
        "success",
        None,
        Some(format!("site:{site_id},domain:{}", existing_domain.name)),
    );

    (StatusCode::OK, Json(domain_to_response(&updated_domain))).into_response()
}

/// Deletes an auxiliary domain from a website.
pub async fn delete_site_domain_handler(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path((site_id, domain_id)): Path<(i64, i64)>,
) -> impl IntoResponse {
    let site_repo = SqliteSiteRepository::new(state.pool.clone());
    let domain_repo = SqliteDomainRepository::new(state.pool.clone());
    let user_repo = SqliteUserRepository::new(state.pool.clone());

    let site = match fetch_and_authorize_site(&site_repo, &auth_user, site_id).await {
        Ok(s) => s,
        Err((code, msg)) => {
            return (code, Json(serde_json::json!({ "error": msg }))).into_response();
        }
    };

    let user = match user_repo.find_by_id(site.user_id).await {
        Ok(Some(u)) => u,
        _ => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Site owner user not found" })),
            )
                .into_response();
        }
    };

    let existing_domain = match domain_repo.find_by_id(domain_id).await {
        Ok(Some(d)) if d.site_id == site_id => d,
        Ok(Some(_)) | Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Domain not found for this site" })),
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

    if existing_domain.is_primary {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Domain utama tidak dapat dihapus. Silakan tetapkan domain lain sebagai domain utama terlebih dahulu."
            })),
        )
            .into_response();
    }

    if let Err(e) = domain_repo.delete(domain_id).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    let all_domains = match domain_repo.find_by_site_id(site_id).await {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    if let Err((status, msg)) =
        recompile_and_apply_site_vhost(&state, &site, &user, &all_domains).await
    {
        return (status, Json(serde_json::json!({ "error": msg }))).into_response();
    }

    state.audit_writer.log_async(
        Some(auth_user.id),
        &auth_user.username,
        "domain.delete",
        format!("domain:{domain_id}"),
        "success",
        None,
        Some(format!("site:{site_id},domain:{}", existing_domain.name)),
    );

    (
        StatusCode::OK,
        Json(SiteMessageResponse {
            message: "Domain berhasil dihapus".to_string(),
            site_id,
            status: "deleted".to_string(),
        }),
    )
        .into_response()
}

/// Configures force-HTTPS redirection for a website.
pub async fn set_force_https_handler(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(site_id): Path<i64>,
    Json(payload): Json<SetForceHttpsRequest>,
) -> impl IntoResponse {
    let site_repo = SqliteSiteRepository::new(state.pool.clone());
    let domain_repo = SqliteDomainRepository::new(state.pool.clone());
    let user_repo = SqliteUserRepository::new(state.pool.clone());

    let site = match fetch_and_authorize_site(&site_repo, &auth_user, site_id).await {
        Ok(s) => s,
        Err((code, msg)) => {
            return (code, Json(serde_json::json!({ "error": msg }))).into_response();
        }
    };

    let user = match user_repo.find_by_id(site.user_id).await {
        Ok(Some(u)) => u,
        _ => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Site owner user not found" })),
            )
                .into_response();
        }
    };

    let mut settings: serde_json::Value =
        serde_json::from_str(&site.settings_json).unwrap_or(serde_json::json!({}));
    settings["force_https"] = serde_json::Value::Bool(payload.force_https);

    let updated_site = match site_repo
        .update(
            site_id,
            &UpdateSite {
                kind: None,
                docroot: None,
                runtime: None,
                version: None,
                status: None,
                settings_json: Some(settings.to_string()),
            },
        )
        .await
    {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let all_domains = match domain_repo.find_by_site_id(site_id).await {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    if let Err((status, msg)) =
        recompile_and_apply_site_vhost(&state, &updated_site, &user, &all_domains).await
    {
        return (status, Json(serde_json::json!({ "error": msg }))).into_response();
    }

    state.audit_writer.log_async(
        Some(auth_user.id),
        &auth_user.username,
        "site.force_https",
        format!("site:{site_id}"),
        "success",
        None,
        Some(format!("force_https:{}", payload.force_https)),
    );

    (
        StatusCode::OK,
        Json(build_site_response(&updated_site, &all_domains)),
    )
        .into_response()
}
