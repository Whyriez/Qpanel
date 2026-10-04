pub mod admin;
pub mod agent_client;
pub mod audit;
pub mod auth;
pub mod config;
pub mod jobs;
pub mod logs;
pub mod php;
pub mod plans;
pub mod routes;
pub mod sites;
pub mod ssl;
pub mod static_files;
pub mod tenants;

use axum::{
    Router,
    routing::{get, post, put},
};
use routes::{ApiDoc, AppState, healthz_handler, system_info_handler};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub fn create_router(state: AppState) -> Router {
    let api_doc = ApiDoc::openapi();

    let api_routes = Router::new()
        .route("/healthz", get(healthz_handler))
        .route("/api/v1/system/info", get(system_info_handler))
        .route("/api/v1/auth/login", post(auth::login_handler))
        .route("/api/v1/auth/logout", post(auth::logout_handler))
        .route("/api/v1/auth/me", get(auth::me_handler))
        .route("/api/v1/auth/csrf", get(auth::csrf_handler))
        .route("/api/v1/auth/password", post(auth::change_password_handler))
        .route(
            "/api/v1/auth/2fa/setup",
            post(auth::two_factor_setup_handler),
        )
        .route(
            "/api/v1/auth/2fa/enable",
            post(auth::two_factor_enable_handler),
        )
        .route(
            "/api/v1/auth/2fa/disable",
            post(auth::two_factor_disable_handler),
        )
        .route(
            "/api/v1/auth/2fa/status",
            get(auth::two_factor_status_handler),
        )
        .route("/api/v1/admin/users", get(admin::admin_list_users_handler))
        .route(
            "/api/v1/admin/users/{id}",
            get(admin::admin_get_user_handler),
        )
        .route(
            "/api/v1/admin/users/{id}/status",
            post(admin::admin_update_user_status_handler),
        )
        .route(
            "/api/v1/admin/settings",
            get(admin::admin_get_setting_handler),
        )
        .route(
            "/api/v1/admin/settings",
            post(admin::admin_update_setting_handler),
        )
        .route(
            "/api/v1/user/{id}/profile",
            get(admin::user_get_profile_handler),
        )
        .route("/api/v1/audit/logs", get(audit::list_audit_logs_handler))
        .route("/api/v1/audit/logs/{id}", get(audit::get_audit_log_handler))
        .route(
            "/api/v1/jobs",
            post(jobs::create_job_handler).get(jobs::list_jobs_handler),
        )
        .route("/api/v1/jobs/{id}", get(jobs::get_job_handler))
        .route("/api/v1/jobs/{id}/cancel", post(jobs::cancel_job_handler))
        .route("/api/v1/jobs/{id}/events", get(jobs::job_events_handler))
        // Plans endpoints
        .route(
            "/api/v1/plans",
            get(plans::list_plans_handler).post(plans::create_plan_handler),
        )
        .route(
            "/api/v1/plans/{id}",
            get(plans::get_plan_handler)
                .put(plans::update_plan_handler)
                .delete(plans::delete_plan_handler),
        )
        // Tenants endpoints
        .route(
            "/api/v1/tenants",
            get(tenants::list_tenants_handler).post(tenants::create_tenant_handler),
        )
        .route(
            "/api/v1/tenants/{id}",
            get(tenants::get_tenant_handler).delete(tenants::delete_tenant_handler),
        )
        .route(
            "/api/v1/tenants/{id}/resources",
            get(tenants::get_tenant_resources_handler),
        )
        .route(
            "/api/v1/tenants/{id}/suspend",
            post(tenants::suspend_tenant_handler),
        )
        .route(
            "/api/v1/tenants/{id}/unsuspend",
            post(tenants::unsuspend_tenant_handler),
        )
        .route(
            "/api/v1/tenants/{id}/impersonate",
            post(tenants::impersonate_tenant_handler),
        )
        .route(
            "/api/v1/auth/stop-impersonating",
            post(tenants::stop_impersonating_handler),
        )
        // Hardening & Prerequisites endpoints
        .route(
            "/api/v1/system/hardening",
            get(tenants::get_hardening_status_handler),
        )
        .route(
            "/api/v1/system/hardening/apply",
            post(tenants::apply_hardening_handler),
        )
        // Sites endpoints
        .route(
            "/api/v1/sites",
            post(sites::create_static_site_handler).get(sites::list_sites_handler),
        )
        .route(
            "/api/v1/sites/{id}",
            get(sites::get_site_handler).delete(sites::delete_site_handler),
        )
        .route(
            "/api/v1/sites/{id}/enable",
            post(sites::enable_site_handler),
        )
        .route(
            "/api/v1/sites/{id}/disable",
            post(sites::disable_site_handler),
        )
        .route(
            "/api/v1/sites/{id}/force-https",
            post(sites::set_force_https_handler),
        )
        .route(
            "/api/v1/sites/{id}/domains",
            get(sites::list_site_domains_handler).post(sites::add_site_domain_handler),
        )
        .route(
            "/api/v1/sites/{id}/domains/{domain_id}",
            put(sites::update_site_domain_handler).delete(sites::delete_site_domain_handler),
        )
        // SSL & ACME endpoints
        .route(
            "/api/v1/sites/{id}/ssl",
            get(ssl::get_site_ssl_handler).delete(ssl::delete_site_ssl_handler),
        )
        .route(
            "/api/v1/sites/{id}/ssl/issue",
            post(ssl::issue_site_ssl_handler),
        )
        .route(
            "/api/v1/sites/{id}/ssl/self-signed",
            post(ssl::self_signed_site_ssl_handler),
        )
        .route(
            "/api/v1/sites/{id}/ssl/custom",
            post(ssl::custom_site_ssl_handler),
        )
        .route(
            "/api/v1/sites/{id}/ssl/renew",
            post(ssl::renew_site_ssl_handler),
        )
        // Site Logs & WebSocket Live Tail
        .route("/api/v1/sites/{id}/logs", get(logs::get_site_logs_handler))
        .route("/ws/logs/{id}", get(logs::site_logs_ws_handler))
        // PHP Runtime management endpoints
        .route("/api/v1/php/versions", get(php::list_php_versions_handler))
        .route("/api/v1/php/repo/setup", post(php::setup_php_repo_handler))
        .route(
            "/api/v1/php/versions/{version}/install",
            post(php::install_php_version_handler),
        )
        .route(
            "/api/v1/php/versions/{version}/remove",
            post(php::remove_php_version_handler),
        )
        .route(
            "/api/v1/php/versions/{version}/extensions",
            get(php::list_php_extensions_handler),
        )
        .route(
            "/api/v1/php/versions/{version}/extensions/{extension}/install",
            post(php::install_php_extension_handler),
        )
        .route(
            "/api/v1/php/versions/{version}/extensions/{extension}/toggle",
            post(php::toggle_php_extension_handler),
        )
        .route(
            "/api/v1/php/versions/{version}/extensions/{extension}/remove",
            post(php::remove_php_extension_handler),
        )
        .route(
            "/api/v1/sites/{id}/php/extensions",
            get(php::get_site_php_extensions_handler).put(php::update_site_php_extensions_handler),
        )
        .merge(SwaggerUi::new("/swagger-ui").url("/api/v1/openapi.json", api_doc))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Fallback to static files from web/dist for all non-API paths
    api_routes.fallback(static_files::static_handler)
}
