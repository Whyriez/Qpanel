pub mod admin;
pub mod agent_client;
pub mod audit;
pub mod auth;
pub mod config;
pub mod jobs;
pub mod routes;
pub mod static_files;

use axum::{
    Router,
    routing::{get, post},
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
        .merge(SwaggerUi::new("/swagger-ui").url("/api/v1/openapi.json", api_doc))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Fallback to static files from web/dist for all non-API paths
    api_routes.fallback(static_files::static_handler)
}
