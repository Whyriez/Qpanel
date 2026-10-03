//! Authentication and CSRF extractors and middleware.

use axum::{
    Json,
    extract::FromRequestParts,
    http::{HeaderMap, StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use panel_core::auth::hash_token;
use panel_db::repositories::{SqliteSessionRepository, SqliteUserRepository};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use super::csrf::{CSRF_HEADER_NAME, generate_csrf_token, verify_csrf_token};
use super::session::extract_session_token;
use crate::routes::AppState;

/// Represents a verified authenticated user in a request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    pub id: i64,
    pub username: String,
    pub role: String,
    pub email: Option<String>,
    pub totp_enabled: bool,
    pub session_id: String,
    pub csrf_token: String,
}

#[derive(Debug, Clone)]
pub struct RequireAuth(pub AuthenticatedUser);

#[derive(Debug, Clone)]
pub struct RequireCsrf(pub AuthenticatedUser);

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = match extract_session_token(&parts.headers) {
            Some(t) => t,
            None => {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({
                        "error": "Authentication required: missing session cookie"
                    })),
                )
                    .into_response());
            }
        };

        let token_hash = hash_token(&token);
        let session_repo = SqliteSessionRepository::new(state.pool.clone());
        let session = match session_repo.find_by_token_hash(&token_hash).await {
            Ok(Some(s)) => s,
            _ => {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({
                        "error": "Invalid or expired session"
                    })),
                )
                    .into_response());
            }
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        if session.expires_at <= now {
            let _ = session_repo.delete(&session.id).await;
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "Session has expired"
                })),
            )
                .into_response());
        }

        let user_repo = SqliteUserRepository::new(state.pool.clone());
        let user = match user_repo.find_by_id(session.user_id).await {
            Ok(Some(u)) => u,
            _ => {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({
                        "error": "User not found"
                    })),
                )
                    .into_response());
            }
        };

        if user.status != "active" {
            return Err((
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "Account is inactive or suspended"
                })),
            )
                .into_response());
        }

        let csrf_token = generate_csrf_token(&session.id);

        Ok(AuthenticatedUser {
            id: user.id,
            username: user.username,
            role: user.role,
            email: user.email,
            totp_enabled: user.totp_enabled,
            session_id: session.id,
            csrf_token,
        })
    }
}

impl FromRequestParts<AppState> for RequireAuth {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthenticatedUser::from_request_parts(parts, state).await?;
        Ok(RequireAuth(user))
    }
}

impl FromRequestParts<AppState> for RequireCsrf {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthenticatedUser::from_request_parts(parts, state).await?;

        let candidate_csrf = parts
            .headers
            .get(CSRF_HEADER_NAME)
            .and_then(|h| h.to_str().ok());

        match candidate_csrf {
            Some(candidate) if verify_csrf_token(&user.session_id, candidate) => {
                Ok(RequireCsrf(user))
            }
            _ => Err((
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "CSRF token missing or invalid"
                })),
            )
                .into_response()),
        }
    }
}

/// Helper function to validate CSRF token from headers against authenticated user.
pub fn validate_csrf_header(headers: &HeaderMap, user: &AuthenticatedUser) -> bool {
    let candidate_csrf = headers.get(CSRF_HEADER_NAME).and_then(|h| h.to_str().ok());

    match candidate_csrf {
        Some(candidate) => verify_csrf_token(&user.session_id, candidate),
        None => false,
    }
}
