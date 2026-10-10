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
    DomainRedirectConfig, FastCgiCacheConfig, HeaderConfig, NginxTemplateRenderer,
    NodejsVhostContext, PhpPreset, PhpVhostContext, PythonVhostContext, ReverseProxyVhostContext,
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
    pub node_version: Option<String>,
    pub port: Option<u16>,
    pub app_dir: Option<String>,
    pub entrypoint_type: Option<String>,
    pub entrypoint_value: Option<String>,
    pub auto_restart: Option<bool>,
    pub env_vars: Option<std::collections::BTreeMap<String, String>>,
    pub python_version: Option<String>,
    pub framework: Option<String>,
    pub server_type: Option<String>,
    pub workers: Option<u32>,
    pub venv_dir: Option<String>,
    pub static_url: Option<String>,
    pub static_dir: Option<String>,
    pub media_url: Option<String>,
    pub media_dir: Option<String>,
    pub upstream_url: Option<String>,
    pub preserve_host: Option<bool>,
    pub websocket_support: Option<bool>,
    pub proxy_connect_timeout: Option<String>,
    pub proxy_send_timeout: Option<String>,
    pub proxy_read_timeout: Option<String>,
    pub proxy_buffering: Option<bool>,
    pub custom_request_headers: Option<Vec<crate::proxy_site::ProxyHeaderConfig>>,
    pub custom_response_headers: Option<Vec<crate::proxy_site::ProxyHeaderConfig>>,
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

fn build_domain_redirects_and_aliases(
    domains: &[Domain],
    primary_domain: &str,
    has_ssl: bool,
    force_https: bool,
) -> (Vec<String>, Vec<DomainRedirectConfig>) {
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

    (alias_domains, redirects)
}

struct PythonVhostRenderArgs<'a> {
    site: &'a Site,
    primary_domain: &'a str,
    linux_uname: &'a str,
    settings: &'a serde_json::Value,
    alias_domains: Vec<String>,
    redirects: Vec<DomainRedirectConfig>,
    has_ssl: bool,
    ssl_cert: Option<String>,
    ssl_key: Option<String>,
    force_https: bool,
}

fn render_python_vhost_for_site(
    renderer: &NginxTemplateRenderer,
    args: PythonVhostRenderArgs<'_>,
) -> Result<String, (StatusCode, String)> {
    let socket_path = args
        .settings
        .get("socket_path")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("/run/panel/python/site_{}.sock", args.site.id));

    let static_url = args
        .settings
        .get("static_url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let static_dir = args
        .settings
        .get("static_dir")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let media_url = args
        .settings
        .get("media_url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let media_dir = args
        .settings
        .get("media_dir")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let python_ctx = PythonVhostContext {
        site_id: format!("site_{}", args.site.id),
        primary_domain: args.primary_domain.to_string(),
        alias_domains: args.alias_domains,
        redirects: args.redirects,
        unix_socket: socket_path,
        log_dir: format!(
            "/var/log/panel/{}/{}",
            args.linux_uname, args.primary_domain
        ),
        snippets_dir: "/etc/panel/nginx/snippets".to_string(),
        has_ssl: args.has_ssl,
        ssl_cert: args.ssl_cert,
        ssl_key: args.ssl_key,
        force_https: args.force_https,
        ocsp_stapling: args
            .settings
            .get("ocsp_stapling")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        custom_headers: vec![],
        static_url,
        static_dir,
        media_url,
        media_dir,
        upload_max_size: args
            .settings
            .get("upload_max_size")
            .or_else(|| args.settings.get("client_max_body_size"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        proxy_connect_timeout: args
            .settings
            .get("proxy_connect_timeout")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        proxy_send_timeout: None,
        proxy_read_timeout: args
            .settings
            .get("proxy_read_timeout")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        proxy_buffering: None,
    };

    renderer.render_python(&python_ctx).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Python vhost render failed: {e}"),
        )
    })
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

    let (alias_domains, redirects) =
        build_domain_redirects_and_aliases(domains, primary_domain, has_ssl, force_https);

    let linux_uname = user
        .linux_username
        .as_deref()
        .unwrap_or(user.username.as_str());

    let renderer = NginxTemplateRenderer::new();
    let is_php = site.kind == "php" || site.runtime.as_deref() == Some("php");
    let is_nodejs = site.kind == "nodejs" || site.runtime.as_deref() == Some("nodejs");
    let is_python = site.kind == "python" || site.runtime.as_deref() == Some("python");
    let is_proxy = site.kind == "proxy" || site.runtime.as_deref() == Some("proxy");

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
    } else if is_nodejs {
        let port = settings
            .get("port")
            .and_then(|v| v.as_u64())
            .map(|p| p as u16)
            .unwrap_or(20000);
        let upstream_url = settings
            .get("upstream_url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("http://127.0.0.1:{}", port));

        let nodejs_ctx = NodejsVhostContext {
            site_id: format!("site_{}", site.id),
            primary_domain: primary_domain.to_string(),
            alias_domains,
            redirects,
            docroot: Some(site.docroot.clone()),
            log_dir: format!("/var/log/panel/{linux_uname}/{primary_domain}"),
            snippets_dir: "/etc/panel/nginx/snippets".to_string(),
            has_ssl,
            ssl_cert,
            ssl_key,
            force_https,
            ocsp_stapling: settings
                .get("ocsp_stapling")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            custom_headers: vec![],
            upstream_url,
            upload_max_size: settings
                .get("upload_max_size")
                .or_else(|| settings.get("client_max_body_size"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            static_assets_enabled: false,
            proxy_connect_timeout: settings
                .get("proxy_connect_timeout")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            proxy_send_timeout: None,
            proxy_read_timeout: settings
                .get("proxy_read_timeout")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            proxy_buffering: None,
        };

        renderer.render_nodejs(&nodejs_ctx).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Node.js vhost render failed: {e}"),
            )
        })?
    } else if is_python {
        render_python_vhost_for_site(
            &renderer,
            PythonVhostRenderArgs {
                site,
                primary_domain,
                linux_uname,
                settings: &settings,
                alias_domains,
                redirects,
                has_ssl,
                ssl_cert,
                ssl_key,
                force_https,
            },
        )?
    } else if is_proxy {
        let upstream_url = settings
            .get("upstream_url")
            .and_then(|v| v.as_str())
            .unwrap_or("http://127.0.0.1:8080")
            .to_string();
        let preserve_host = settings
            .get("preserve_host")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let websocket_support = settings
            .get("websocket_support")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let proxy_connect_timeout = settings
            .get("proxy_connect_timeout")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let proxy_send_timeout = settings
            .get("proxy_send_timeout")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let proxy_read_timeout = settings
            .get("proxy_read_timeout")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let proxy_buffering = settings.get("proxy_buffering").and_then(|v| v.as_bool());
        let proxy_buffering_str = proxy_buffering.map(|b| {
            if b {
                "on".to_string()
            } else {
                "off".to_string()
            }
        });
        let custom_request_headers: Vec<HeaderConfig> = settings
            .get("custom_request_headers")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let custom_headers: Vec<HeaderConfig> = settings
            .get("custom_response_headers")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let proxy_ssl_server_name = upstream_url.starts_with("https://");

        let proxy_ctx = ReverseProxyVhostContext {
            site_id: format!("site_{}", site.id),
            primary_domain: primary_domain.to_string(),
            alias_domains,
            redirects,
            log_dir: format!("/var/log/panel/{linux_uname}/{primary_domain}"),
            snippets_dir: "/etc/panel/nginx/snippets".to_string(),
            has_ssl,
            ssl_cert,
            ssl_key,
            force_https,
            ocsp_stapling: settings
                .get("ocsp_stapling")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            upstream_url,
            preserve_host,
            custom_host_header: None,
            websocket_support,
            upload_max_size: None,
            proxy_connect_timeout,
            proxy_send_timeout,
            proxy_read_timeout,
            proxy_buffering: proxy_buffering_str,
            custom_headers,
            custom_request_headers,
            proxy_ssl_server_name,
            allow_external_upstream: true,
        };

        renderer.render_proxy(&proxy_ctx).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Proxy vhost render failed: {e}"),
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

    // If Node.js site is active, also ensure systemd unit and env file are applied on host
    if is_nodejs && site.status == "active" {
        let node_ver = site
            .version
            .clone()
            .or_else(|| {
                settings
                    .get("node_version")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "22.14.0".to_string());
        let port = settings
            .get("port")
            .and_then(|v| v.as_u64())
            .map(|p| p as u16)
            .unwrap_or(20000);
        let entrypoint = match settings.get("entrypoint_type").and_then(|v| v.as_str()) {
            Some("npm_start") => panel_proto::NodeSiteEntrypoint::NpmStart,
            Some("npm_custom") => {
                let subcmd = settings
                    .get("entrypoint_value")
                    .and_then(|v| v.as_str())
                    .unwrap_or("start")
                    .to_string();
                panel_proto::NodeSiteEntrypoint::NpmCustom(subcmd)
            }
            _ => {
                let script = settings
                    .get("entrypoint_value")
                    .or_else(|| settings.get("entrypoint"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("server.js")
                    .to_string();
                panel_proto::NodeSiteEntrypoint::NodeScript(script)
            }
        };

        let mut env_vars = std::collections::BTreeMap::new();
        if let Some(env_obj) = settings.get("env_vars").and_then(|v| v.as_object()) {
            for (k, v) in env_obj {
                if let Some(val_str) = v.as_str() {
                    env_vars.insert(k.clone(), val_str.to_string());
                }
            }
        }

        let auto_restart = settings
            .get("auto_restart")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let app_dir = settings
            .get("app_dir")
            .and_then(|v| v.as_str())
            .unwrap_or(&site.docroot)
            .to_string();

        let node_apply_req = panel_proto::NodeSiteApplyRequest {
            site_id: format!("site_{}", site.id),
            username: linux_uname.to_string(),
            uid: user.linux_uid.unwrap_or(user.id) as u32,
            node_version: node_ver,
            app_dir,
            port,
            entrypoint,
            env_vars,
            auto_restart,
            env_file_path: None,
            unit_path: None,
            base_runtime_dir: None,
        };

        let _ = state.agent_client.apply_node_site(node_apply_req).await;
    }

    // If Python site is active, also ensure systemd unit and env file are applied on host
    if is_python && site.status == "active" {
        let py_ver = site
            .version
            .clone()
            .or_else(|| {
                settings
                    .get("python_version")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "3.12.9".to_string());

        let app_dir = settings
            .get("app_dir")
            .and_then(|v| v.as_str())
            .unwrap_or(&site.docroot)
            .to_string();

        let venv_dir = settings
            .get("venv_dir")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{}/.venv", app_dir));

        let socket_path = settings
            .get("socket_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("/run/panel/python/site_{}.sock", site.id));

        let fw_str = settings
            .get("framework_preset")
            .or_else(|| settings.get("framework"))
            .and_then(|v| v.as_str())
            .unwrap_or("generic");
        let framework = match fw_str {
            "django" => panel_proto::PythonSiteFramework::Django,
            "flask" => panel_proto::PythonSiteFramework::Flask,
            "fastapi" => panel_proto::PythonSiteFramework::FastApi,
            _ => panel_proto::PythonSiteFramework::Generic,
        };

        let srv_str = settings
            .get("server_type")
            .and_then(|v| v.as_str())
            .unwrap_or("gunicorn_wsgi");
        let server_type = match srv_str {
            "uvicorn_asgi" => panel_proto::PythonSiteServer::UvicornAsgi,
            "gunicorn_uvicorn_worker" => panel_proto::PythonSiteServer::GunicornUvicornWorker,
            _ => panel_proto::PythonSiteServer::GunicornWsgi,
        };

        let ep_type = settings
            .get("entrypoint_type")
            .and_then(|v| v.as_str())
            .unwrap_or("wsgi");
        let ep_val = settings
            .get("entrypoint_value")
            .or_else(|| settings.get("entrypoint"))
            .and_then(|v| v.as_str())
            .unwrap_or("app:app")
            .to_string();

        let entrypoint = match ep_type {
            "asgi" => panel_proto::PythonSiteEntrypoint::Asgi(ep_val),
            "custom" => panel_proto::PythonSiteEntrypoint::CustomCommand(ep_val),
            _ => panel_proto::PythonSiteEntrypoint::Wsgi(ep_val),
        };

        let workers = settings
            .get("workers")
            .and_then(|v| v.as_u64())
            .map(|w| w as u32)
            .unwrap_or(3);

        let mut env_vars = std::collections::BTreeMap::new();
        if let Some(env_obj) = settings.get("env_vars").and_then(|v| v.as_object()) {
            for (k, v) in env_obj {
                if let Some(val_str) = v.as_str() {
                    env_vars.insert(k.clone(), val_str.to_string());
                }
            }
        }

        let auto_restart = settings
            .get("auto_restart")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let py_apply_req = panel_proto::PythonSiteApplyRequest {
            site_id: format!("site_{}", site.id),
            username: linux_uname.to_string(),
            uid: user.linux_uid.unwrap_or(user.id) as u32,
            python_version: py_ver,
            app_dir,
            venv_dir,
            socket_path,
            framework,
            server_type,
            entrypoint,
            workers,
            env_vars,
            auto_restart,
            env_file_path: None,
            unit_path: None,
            base_runtime_dir: None,
        };

        let _ = state.agent_client.apply_python_site(py_apply_req).await;
    }

    Ok(())
}

/// Helper to recompile and apply a site configuration by fetching user and domains from DB.
pub async fn apply_site_config(state: &AppState, site: &Site) -> Result<(), (StatusCode, String)> {
    let domain_repo =
        panel_db::repositories::domain_repo::SqliteDomainRepository::new(state.pool.clone());
    let user_repo =
        panel_db::repositories::user_repo::SqliteUserRepository::new(state.pool.clone());

    let domains = domain_repo
        .find_by_site_id(site.id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let user = user_repo
        .find_by_id(site.user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "Pemilik situs tidak ditemukan".to_string(),
            )
        })?;

    recompile_and_apply_site_vhost(state, site, &user, &domains).await
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
    let is_nodejs = payload.kind.as_deref() == Some("nodejs");
    let is_python = payload.kind.as_deref() == Some("python");
    let is_proxy = payload.kind.as_deref() == Some("proxy");
    let preset_str = payload.preset.as_deref().unwrap_or("generic");
    let preset = PhpPreset::parse(preset_str).unwrap_or(PhpPreset::Generic);

    let docroot = payload.docroot.unwrap_or_else(|| {
        if is_nodejs || is_python {
            format!("/home/{linux_uname}/sites/{ascii_domain}/app")
        } else {
            format!("/home/{linux_uname}/sites/{ascii_domain}/public")
        }
    });
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
    } else if is_nodejs {
        let port = match payload.port {
            Some(p) => p,
            None => match crate::node_site::allocate_node_site_port(&state.pool).await {
                Ok(p) => p,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({ "error": e })),
                    )
                        .into_response();
                }
            },
        };
        let node_ver = payload
            .node_version
            .clone()
            .unwrap_or_else(|| "22.14.0".to_string());
        let app_dir = payload.app_dir.clone().unwrap_or_else(|| docroot.clone());
        let entrypoint_type = payload
            .entrypoint_type
            .clone()
            .unwrap_or_else(|| "script".to_string());
        let entrypoint_value = payload
            .entrypoint_value
            .clone()
            .unwrap_or_else(|| "server.js".to_string());
        let auto_restart = payload.auto_restart.unwrap_or(true);
        let env_vars = payload.env_vars.clone().unwrap_or_default();

        settings_map.insert("port".to_string(), serde_json::json!(port));
        settings_map.insert(
            "upstream_url".to_string(),
            serde_json::json!(format!("http://127.0.0.1:{}", port)),
        );
        settings_map.insert("node_version".to_string(), serde_json::json!(node_ver));
        settings_map.insert("app_dir".to_string(), serde_json::json!(app_dir));
        settings_map.insert(
            "entrypoint_type".to_string(),
            serde_json::json!(entrypoint_type),
        );
        settings_map.insert(
            "entrypoint_value".to_string(),
            serde_json::json!(entrypoint_value),
        );
        settings_map.insert("auto_restart".to_string(), serde_json::json!(auto_restart));
        settings_map.insert("env_vars".to_string(), serde_json::json!(env_vars));
        if let Some(ref fw) = payload.framework {
            settings_map.insert("framework_preset".to_string(), serde_json::json!(fw));
        }
    } else if is_python {
        let py_ver = payload
            .python_version
            .clone()
            .unwrap_or_else(|| "3.12.9".to_string());
        let fw_preset = payload
            .framework
            .clone()
            .unwrap_or_else(|| "generic".to_string());
        let srv_type = payload.server_type.clone().unwrap_or_else(|| {
            if fw_preset == "fastapi" {
                "uvicorn_asgi".to_string()
            } else {
                "gunicorn_wsgi".to_string()
            }
        });
        let ep_type = payload.entrypoint_type.clone().unwrap_or_else(|| {
            if srv_type.contains("asgi") {
                "asgi".to_string()
            } else {
                "wsgi".to_string()
            }
        });
        let ep_val = payload
            .entrypoint_value
            .clone()
            .unwrap_or_else(|| match fw_preset.as_str() {
                "django" => "wsgi:application".to_string(),
                "fastapi" => "main:app".to_string(),
                _ => "app:app".to_string(),
            });
        let app_dir = payload.app_dir.clone().unwrap_or_else(|| docroot.clone());
        let venv_dir = payload
            .venv_dir
            .clone()
            .unwrap_or_else(|| format!("{}/.venv", app_dir));
        let workers = payload.workers.unwrap_or(3);
        let auto_restart = payload.auto_restart.unwrap_or(true);
        let env_vars = payload.env_vars.clone().unwrap_or_default();

        settings_map.insert("python_version".to_string(), serde_json::json!(py_ver));
        settings_map.insert("framework_preset".to_string(), serde_json::json!(fw_preset));
        settings_map.insert("server_type".to_string(), serde_json::json!(srv_type));
        settings_map.insert("entrypoint_type".to_string(), serde_json::json!(ep_type));
        settings_map.insert("entrypoint_value".to_string(), serde_json::json!(ep_val));
        settings_map.insert("workers".to_string(), serde_json::json!(workers));
        settings_map.insert("app_dir".to_string(), serde_json::json!(app_dir));
        settings_map.insert("venv_dir".to_string(), serde_json::json!(venv_dir));
        settings_map.insert("auto_restart".to_string(), serde_json::json!(auto_restart));
        settings_map.insert("env_vars".to_string(), serde_json::json!(env_vars));

        if fw_preset == "django" {
            settings_map.insert("static_url".to_string(), serde_json::json!("/static/"));
            settings_map.insert(
                "static_dir".to_string(),
                serde_json::json!(format!("{}/static", app_dir)),
            );
            settings_map.insert("media_url".to_string(), serde_json::json!("/media/"));
            settings_map.insert(
                "media_dir".to_string(),
                serde_json::json!(format!("{}/media", app_dir)),
            );
        } else if fw_preset == "flask" {
            settings_map.insert("static_url".to_string(), serde_json::json!("/static/"));
            settings_map.insert(
                "static_dir".to_string(),
                serde_json::json!(format!("{}/static", app_dir)),
            );
        }
    } else if is_proxy {
        let upstream = payload
            .upstream_url
            .clone()
            .unwrap_or_else(|| "http://127.0.0.1:8080".to_string());

        let allow_external = crate::proxy_site::check_user_allow_external_upstream(
            &state.pool,
            target_user.id,
            auth_user.is_admin(),
        )
        .await;

        if let Err(e) = panel_webserver::template::validate_reverse_proxy_upstream_url(
            &upstream,
            allow_external,
        ) {
            let err_str = e.to_string();
            let status = if err_str.contains("not permit") {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::BAD_REQUEST
            };
            return (status, Json(serde_json::json!({ "error": err_str }))).into_response();
        }

        let preserve_host = payload.preserve_host.unwrap_or(true);
        let websocket_support = payload.websocket_support.unwrap_or(true);

        settings_map.insert("upstream_url".to_string(), serde_json::json!(upstream));
        settings_map.insert(
            "preserve_host".to_string(),
            serde_json::json!(preserve_host),
        );
        settings_map.insert(
            "websocket_support".to_string(),
            serde_json::json!(websocket_support),
        );
        if let Some(t) = &payload.proxy_connect_timeout {
            settings_map.insert("proxy_connect_timeout".to_string(), serde_json::json!(t));
        }
        if let Some(t) = &payload.proxy_send_timeout {
            settings_map.insert("proxy_send_timeout".to_string(), serde_json::json!(t));
        }
        if let Some(t) = &payload.proxy_read_timeout {
            settings_map.insert("proxy_read_timeout".to_string(), serde_json::json!(t));
        }
        if let Some(b) = payload.proxy_buffering {
            settings_map.insert("proxy_buffering".to_string(), serde_json::json!(b));
        }
        if let Some(req_headers) = &payload.custom_request_headers {
            settings_map.insert(
                "custom_request_headers".to_string(),
                serde_json::json!(req_headers),
            );
        }
        if let Some(resp_headers) = &payload.custom_response_headers {
            settings_map.insert(
                "custom_response_headers".to_string(),
                serde_json::json!(resp_headers),
            );
        }
    }
    let settings = serde_json::Value::Object(settings_map);

    // 5. Create database record for Site
    let site_input = CreateSite {
        user_id: target_user.id,
        kind: if is_php {
            "php".to_string()
        } else if is_nodejs {
            "nodejs".to_string()
        } else if is_python {
            "python".to_string()
        } else if is_proxy {
            "proxy".to_string()
        } else {
            "static".to_string()
        },
        docroot: docroot.clone(),
        runtime: if is_php {
            Some("php".to_string())
        } else if is_nodejs {
            Some("nodejs".to_string())
        } else if is_python {
            Some("python".to_string())
        } else if is_proxy {
            Some("proxy".to_string())
        } else {
            Some("none".to_string())
        },
        version: if is_php {
            Some(php_version.clone())
        } else if is_nodejs {
            Some(
                payload
                    .node_version
                    .clone()
                    .unwrap_or_else(|| "22.14.0".to_string()),
            )
        } else if is_python {
            Some(
                payload
                    .python_version
                    .clone()
                    .unwrap_or_else(|| "3.12.9".to_string()),
            )
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
    } else if is_nodejs {
        render_nodejs_placeholder(&unicode_domain)
    } else if is_python {
        render_python_placeholder(&unicode_domain)
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

    // If Node.js site, start service
    if updated.kind == "nodejs" || updated.runtime.as_deref() == Some("nodejs") {
        let _ = state
            .agent_client
            .action_node_site(panel_proto::NodeSiteActionRequest {
                site_id: format!("site_{id}"),
                action: panel_proto::NodeSiteActionType::Start,
            })
            .await;
    }

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

    // If Node.js site, stop service
    if updated.kind == "nodejs" || updated.runtime.as_deref() == Some("nodejs") {
        let _ = state
            .agent_client
            .action_node_site(panel_proto::NodeSiteActionRequest {
                site_id: format!("site_{id}"),
                action: panel_proto::NodeSiteActionType::Stop,
            })
            .await;
    }

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

    // 1c. If Node.js site, remove systemd unit and env file via agent
    if site.kind == "nodejs" || site.runtime.as_deref() == Some("nodejs") {
        let _ = state
            .agent_client
            .remove_node_site(panel_proto::NodeSiteRemoveRequest {
                site_id: format!("site_{id}"),
                unit_path: None,
                env_file_path: None,
            })
            .await;
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

fn render_nodejs_placeholder(domain: &str) -> String {
    format!(
        r#"const http = require('http');

const port = process.env.PORT || 3000;
const server = http.createServer((req, res) => {{
    res.writeHead(200, {{ 'Content-Type': 'text/html; charset=utf-8' }});
    res.end(`<!DOCTYPE html>
<html lang="id">
<head>
    <meta charset="UTF-8">
    <title>Situs Node.js Aktif - {domain}</title>
    <style>
        body {{ font-family: system-ui, sans-serif; background: #0b0f19; color: #f1f5f9; display: flex; align-items: center; justify-content: center; height: 100vh; margin: 0; }}
        .card {{ background: rgba(30, 41, 59, 0.7); border: 1px solid rgba(148, 163, 184, 0.1); border-radius: 1rem; padding: 2.5rem; text-align: center; max-width: 32rem; }}
        .badge {{ display: inline-flex; align-items: center; gap: 0.5rem; padding: 0.35rem 0.85rem; border-radius: 9999px; background: rgba(16, 185, 129, 0.15); color: #10b981; font-size: 0.875rem; margin-bottom: 1rem; }}
        h1 {{ font-size: 1.75rem; margin: 0 0 0.5rem 0; }}
        p {{ color: #94a3b8; font-size: 0.95rem; line-height: 1.5; }}
    </style>
</head>
<body>
    <div class="card">
        <div class="badge"><span>●</span> Node.js ${{process.version}} Berjalan</div>
        <h1>{domain}</h1>
        <p>Aplikasi Node.js Anda aktif berjalan di Nusa Panel pada port ${{port}}.</p>
    </div>
</body>
</html>`);
}});

server.listen(port, '127.0.0.1', () => {{
    console.log(`Server listening on port ${{port}}`);
}});
"#
    )
}

fn render_python_placeholder(domain: &str) -> String {
    format!(
        r#"def app(environ, start_response):
    status = '200 OK'
    response_headers = [('Content-Type', 'text/html; charset=utf-8')]
    start_response(status, response_headers)
    return [b"""<!DOCTYPE html>
<html lang="id">
<head>
    <meta charset="UTF-8">
    <title>Situs Python Aktif - {domain}</title>
    <style>
        body {{ font-family: system-ui, sans-serif; background: #0b0f19; color: #f1f5f9; display: flex; align-items: center; justify-content: center; height: 100vh; margin: 0; }}
        .card {{ background: rgba(30, 41, 59, 0.7); border: 1px solid rgba(148, 163, 184, 0.1); border-radius: 1rem; padding: 2.5rem; text-align: center; max-width: 32rem; }}
        .badge {{ display: inline-flex; align-items: center; gap: 0.5rem; padding: 0.35rem 0.85rem; border-radius: 9999px; background: rgba(16, 185, 129, 0.15); color: #10b981; font-size: 0.875rem; margin-bottom: 1rem; }}
        h1 {{ font-size: 1.75rem; margin: 0 0 0.5rem 0; }}
        p {{ color: #94a3b8; font-size: 0.95rem; line-height: 1.5; }}
    </style>
</head>
<body>
    <div class="card">
        <div class="badge"><span>●</span> Python WSGI Berjalan</div>
        <h1>{domain}</h1>
        <p>Aplikasi Python Anda aktif berjalan di Nusa Panel via Unix domain socket.</p>
    </div>
</body>
</html>"""]
"#
    )
}
