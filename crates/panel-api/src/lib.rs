//! `panel-api`: HTTP API server, background job runner, and web UI host.

pub mod agent_client;
pub mod auth;
pub mod config;
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
        .merge(SwaggerUi::new("/swagger-ui").url("/api/v1/openapi.json", api_doc))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Fallback to static files from web/dist for all non-API paths
    api_routes.fallback(static_files::static_handler)
}
