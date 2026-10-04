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
    DomainRedirectConfig, FastCgiCacheConfig, NginxTemplateRenderer, PhpPreset, PhpVhostContext,
    StaticVhostContext, render_placeholder_html,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::auth::middleware::AuthenticatedUser;
use crate::routes::AppState;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema, Default)]
pub struct CreateStaticSiteRequest {
    pub domain: String,
    pub user_id: Option<i64>,
    pub spa_fallback: Option<bool>,
    pub force_https: Option<bool>,
    pub docroot: Option<String>,
    pub kind: Option<String>,
    pub php_version: Option<String>,
    pub preset: Option<String>,
    pub custom_entry_file: Option<String>,
    pub upload_max_size: Option<String>,
    pub fastcgi_cache: Option<bool>,
}

pub type CreateSiteRequest = CreateStaticSiteRequest;

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
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub preset: Option<String>,
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
    let preset = settings
        .get("preset")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

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
        runtime: s.runtime.clone(),
        version: s.version.clone(),
        preset,
        created_at: s.created_at,
        updated_at: s.updated_at,
    }
}

pub async fn fetch_and_authorize_site(
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

    let renderer = NginxTemplateRenderer::new();
    let is_php = site.kind == "php" || site.runtime.as_deref() == Some("php");

    let rendered = if is_php {
        let preset_str = settings
            .get("preset")
            .and_then(|v| v.as_str())
            .unwrap_or("generic");
        let preset = PhpPreset::parse(preset_str).unwrap_or(PhpPreset::Generic);
        let custom_entry_file = settings
            .get("custom_entry_file")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let upload_max_size = settings
            .get("upload_max_size")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let php_ver = site.version.clone().unwrap_or_else(|| "8.3".to_string());
        let fpm_socket = format!("unix:/run/panel/php/{linux_uname}-{php_ver}.sock");

        let fastcgi_cache = settings.get("fastcgi_cache").and_then(|v| {
            if let Some(b) = v.as_bool() {
                if b {
                    Some(FastCgiCacheConfig::enabled_default())
                } else {
                    None
                }
            } else if let Ok(cfg) = serde_json::from_value::<FastCgiCacheConfig>(v.clone()) {
                if cfg.enabled { Some(cfg) } else { None }
            } else {
                None
            }
        });

        let php_vhost_ctx = PhpVhostContext {
            site_id: format!("site_{}", site.id),
            primary_domain: primary_domain.to_string(),
            alias_domains,
            redirects,
            docroot: site.docroot.clone(),
            log_dir: format!("/var/log/panel/{linux_uname}/{primary_domain}"),
            snippets_dir: "/etc/panel/nginx/snippets".to_string(),
            index_files: Some("index.php index.html index.htm".to_string()),
            has_ssl,
            ssl_cert,
            ssl_key,
            force_https,
            ocsp_stapling: settings
                .get("ocsp_stapling")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            custom_headers: vec![],
            php_version: php_ver,
            fpm_socket,
            preset,
            custom_entry_file,
            upload_max_size,
            fastcgi_cache,
        };

        renderer.render_php(&php_vhost_ctx).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("PHP vhost render failed: {e}"),
            )
        })?
    } else {
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
            ocsp_stapling: settings
                .get("ocsp_stapling")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            custom_headers: vec![],
        };

        renderer.render_static(&vhost_ctx).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Vhost render failed: {e}"),
            )
        })?
    };

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
#[allow(clippy::cognitive_complexity)]
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
    let is_php = payload.kind.as_deref() == Some("php");
    let preset_str = payload.preset.as_deref().unwrap_or("generic");
    let preset = PhpPreset::parse(preset_str).unwrap_or(PhpPreset::Generic);

    let docroot = payload
        .docroot
        .unwrap_or_else(|| format!("/home/{linux_uname}/sites/{ascii_domain}/public"));
    let spa_fallback = payload.spa_fallback.unwrap_or(false);
    let force_https = payload.force_https.unwrap_or(false);
    let php_version = payload.php_version.unwrap_or_else(|| "8.3".to_string());
    let upload_max_size = payload.upload_max_size.unwrap_or_else(|| "64M".to_string());
    let fastcgi_cache = payload.fastcgi_cache.unwrap_or(false);

    let mut settings_map = serde_json::Map::new();
    settings_map.insert("spa_fallback".to_string(), serde_json::json!(spa_fallback));
    settings_map.insert("force_https".to_string(), serde_json::json!(force_https));
    if is_php {
        settings_map.insert("preset".to_string(), serde_json::json!(preset.as_str()));
        settings_map.insert(
            "upload_max_size".to_string(),
            serde_json::json!(upload_max_size),
        );
        settings_map.insert(
            "fastcgi_cache".to_string(),
            serde_json::json!(fastcgi_cache),
        );
        if let Some(ref entry) = payload.custom_entry_file {
            settings_map.insert("custom_entry_file".to_string(), serde_json::json!(entry));
        }
    }
    let settings = serde_json::Value::Object(settings_map);

    // 5. Create database record for Site
    let site_input = CreateSite {
        user_id: target_user.id,
        kind: if is_php {
            "php".to_string()
        } else {
            "static".to_string()
        },
        docroot: docroot.clone(),
        runtime: if is_php {
            Some("php".to_string())
        } else {
            Some("none".to_string())
        },
        version: if is_php {
            Some(php_version.clone())
        } else {
            Some("none".to_string())
        },
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
    let placeholder = if is_php {
        render_php_placeholder(&unicode_domain, &php_version, preset.as_str())
    } else {
        render_placeholder_html(&unicode_domain)
    };
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

    // 7b. If PHP site, ensure PHP-FPM master and pool exist
    if is_php {
        let uid = target_user.linux_uid.unwrap_or(1000) as u32;
        if let Err(e) = state
            .agent_client
            .ensure_php_fpm_master(linux_uname, uid, &php_version)
            .await
        {
            tracing::warn!("Ensure PHP-FPM master warning: {e}");
        }

        let pool_req = panel_proto::PhpFpmPoolApplyRequest {
            username: linux_uname.to_string(),
            uid,
            version: php_version.clone(),
            pool_name: format!("site_{}", created_site.id),
            docroot: docroot.clone(),
            socket_path: None,
            max_children: None,
            process_idle_timeout_secs: None,
            max_requests: None,
            open_basedir: None,
            disable_functions: None,
            settings: None,
            admin_ini: None,
            reload: true,
        };
        if let Err(e) = state.agent_client.apply_php_fpm_pool(pool_req).await {
            tracing::warn!("Apply PHP-FPM pool warning: {e}");
        }
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
        Some(format!(
            "domain:{ascii_domain},kind:{}",
            if is_php { "php" } else { "static" }
        )),
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

    // 1b. If PHP site, remove PHP-FPM pool via agent
    if site.kind == "php" || site.runtime.as_deref() == Some("php") {
        let user_repo = SqliteUserRepository::new(state.pool.clone());
        if let Ok(Some(owner)) = user_repo.find_by_id(site.user_id).await {
            let uname = owner
                .linux_username
                .as_deref()
                .unwrap_or(owner.username.as_str());
            let ver = site.version.as_deref().unwrap_or("8.3");
            let _ = state
                .agent_client
                .remove_php_fpm_pool(uname, ver, format!("site_{id}"), true)
                .await;
        }
    }

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

fn render_php_placeholder(domain: &str, php_version: &str, preset: &str) -> String {
    format!(
        r#"<?php
// Nusa Panel - Welcome Page
// Domain: {domain}
// PHP Version: {php_version}
// Preset: {preset}
?>
<!DOCTYPE html>
<html lang="id">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Selamat Datang di <?= htmlspecialchars("{domain}") ?> - Nusa Panel</title>
    <style>
        :root {{
            --bg-gradient: linear-gradient(135deg, #0f172a 0%, #1e1b4b 50%, #0f172a 100%);
            --card-bg: rgba(30, 41, 59, 0.7);
            --border: rgba(255, 255, 255, 0.1);
            --text-main: #f8fafc;
            --text-muted: #94a3b8;
            --accent: #38bdf8;
            --success: #10b981;
        }}
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: var(--bg-gradient);
            color: var(--text-main);
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
            padding: 1.5rem;
        }}
        .card {{
            background: var(--card-bg);
            backdrop-filter: blur(12px);
            border: 1px solid var(--border);
            border-radius: 1rem;
            padding: 2.5rem;
            max-width: 32rem;
            width: 100%;
            text-align: center;
            box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.5);
        }}
        .badge {{
            display: inline-flex;
            align-items: center;
            gap: 0.5rem;
            padding: 0.35rem 0.85rem;
            border-radius: 9999px;
            background: rgba(16, 185, 129, 0.15);
            color: var(--success);
            font-size: 0.875rem;
            font-weight: 500;
            margin-bottom: 1.5rem;
            border: 1px solid rgba(16, 185, 129, 0.2);
        }}
        h1 {{ font-size: 1.75rem; font-weight: 700; margin-bottom: 0.75rem; letter-spacing: -0.025em; }}
        p {{ color: var(--text-muted); font-size: 0.95rem; line-height: 1.5; margin-bottom: 1.75rem; }}
        .meta {{
            background: rgba(15, 23, 42, 0.6);
            border: 1px solid var(--border);
            border-radius: 0.5rem;
            padding: 0.75rem 1rem;
            font-size: 0.85rem;
            color: var(--accent);
            font-family: ui-monospace, monospace;
            display: flex;
            justify-content: space-around;
        }}
    </style>
</head>
<body>
    <div class="card">
        <div class="badge">
            <span>●</span> PHP <?= htmlspecialchars(PHP_VERSION) ?> Berjalan
        </div>
        <h1><?= htmlspecialchars("{domain}") ?></h1>
        <p>Situs PHP Anda berhasil diprovisi dan berjalan aktif di bawah Nusa Panel dengan preset <strong><?= htmlspecialchars("{preset}") ?></strong>.</p>
        <div class="meta">
            <div>PHP: <?= htmlspecialchars(PHP_VERSION) ?></div>
            <div>Preset: <?= htmlspecialchars("{preset}") ?></div>
        </div>
    </div>
</body>
</html>
"#
    )
}
