//! HTTP handlers for authentication and session management.

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Response},
};
use panel_core::auth::{hash_password, hash_token, verify_password};
use panel_db::models::CreateSession;
use panel_db::repositories::{SqliteSessionRepository, SqliteUserRepository};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use utoipa::ToSchema;

use super::csrf::generate_csrf_token;
use super::middleware::{RequireAuth, RequireCsrf};
use super::session::{build_clear_session_cookie, build_session_cookie};
use crate::routes::AppState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    #[schema(example = "admin")]
    pub username: String,
    #[schema(example = "Secret123!")]
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserInfo {
    pub id: i64,
    pub username: String,
    pub role: String,
    pub email: Option<String>,
    pub totp_enabled: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoginResponse {
    pub user: UserInfo,
    pub csrf_token: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangePasswordRequest {
    #[schema(example = "OldPassword123!")]
    pub current_password: String,
    #[schema(example = "NewPassword456!")]
    pub new_password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MessageResponse {
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CsrfResponse {
    pub csrf_token: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = LoginResponse),
        (status = 401, description = "Invalid credentials"),
        (status = 423, description = "Account locked due to brute force attempts")
    ),
    tag = "auth"
)]
pub async fn login_handler(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Response {
    // 1. Check account lockout status
    if let Err(remaining_secs) = state.lockout_manager.check_lockout(&payload.username) {
        return (
            StatusCode::LOCKED,
            Json(serde_json::json!({
                "error": "Account is temporarily locked due to too many failed login attempts",
                "remaining_seconds": remaining_secs
            })),
        )
            .into_response();
    }

    // 2. Fetch user from database
    let user_repo = SqliteUserRepository::new(state.pool.clone());
    let user = match user_repo.find_by_username(&payload.username).await {
        Ok(Some(u)) => u,
        _ => {
            let lockout = state.lockout_manager.record_failure(&payload.username);
            let status = if lockout.is_some() {
                StatusCode::LOCKED
            } else {
                StatusCode::UNAUTHORIZED
            };
            return (
                status,
                Json(serde_json::json!({
                    "error": "Invalid username or password"
                })),
            )
                .into_response();
        }
    };

    // 3. Verify password hash using Argon2id
    let is_valid = verify_password(&payload.password, &user.password_hash).unwrap_or(false);
    if !is_valid {
        let lockout = state.lockout_manager.record_failure(&payload.username);
        let status = if lockout.is_some() {
            StatusCode::LOCKED
        } else {
            StatusCode::UNAUTHORIZED
        };
        return (
            status,
            Json(serde_json::json!({
                "error": "Invalid username or password"
            })),
        )
            .into_response();
    }

    // 4. Check account status
    if user.status != "active" {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "Account is inactive or suspended"
            })),
        )
            .into_response();
    }

    // 5. Successful login: reset lockout counter
    state.lockout_manager.record_success(&payload.username);

    // 6. Create session in database
    let session_id = uuid::Uuid::new_v4().to_string();
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let token_hash = hash_token(&token);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let expires_at = now + state.auth_config.session_duration_secs as i64;

    let session_repo = SqliteSessionRepository::new(state.pool.clone());
    if let Err(e) = session_repo
        .create(&CreateSession {
            id: session_id.clone(),
            user_id: user.id,
            token_hash,
            ip_address: None,
            user_agent: None,
            expires_at,
        })
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to create session: {}", e) })),
        )
            .into_response();
    }

    // 7. Generate CSRF token and Set-Cookie header
    let csrf_token = generate_csrf_token(&session_id);
    let is_secure = state
        .auth_config
        .cookie_secure
        .unwrap_or(state.environment != "development");
    let cookie_val =
        build_session_cookie(&token, state.auth_config.session_duration_secs, is_secure);

    let mut headers = HeaderMap::new();
    if let Ok(cookie_header) = cookie_val.parse() {
        headers.insert(SET_COOKIE, cookie_header);
    }

    let response_body = LoginResponse {
        user: UserInfo {
            id: user.id,
            username: user.username,
            role: user.role,
            email: user.email,
            totp_enabled: user.totp_enabled,
        },
        csrf_token,
    };

    (headers, Json(response_body)).into_response()
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    responses(
        (status = 200, description = "Logged out successfully", body = MessageResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Invalid CSRF token")
    ),
    tag = "auth"
)]
pub async fn logout_handler(
    State(state): State<AppState>,
    RequireCsrf(user): RequireCsrf,
) -> Response {
    let session_repo = SqliteSessionRepository::new(state.pool.clone());
    let _ = session_repo.delete(&user.session_id).await;

    let is_secure = state
        .auth_config
        .cookie_secure
        .unwrap_or(state.environment != "development");
    let cookie_val = build_clear_session_cookie(is_secure);

    let mut headers = HeaderMap::new();
    if let Ok(cookie_header) = cookie_val.parse() {
        headers.insert(SET_COOKIE, cookie_header);
    }

    (
        headers,
        Json(MessageResponse {
            message: "Logged out successfully".to_string(),
        }),
    )
        .into_response()
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    responses(
        (status = 200, description = "Current authenticated user profile", body = LoginResponse),
        (status = 401, description = "Not authenticated")
    ),
    tag = "auth"
)]
pub async fn me_handler(RequireAuth(user): RequireAuth) -> impl IntoResponse {
    Json(LoginResponse {
        user: UserInfo {
            id: user.id,
            username: user.username,
            role: user.role,
            email: user.email,
            totp_enabled: user.totp_enabled,
        },
        csrf_token: user.csrf_token,
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/csrf",
    responses(
        (status = 200, description = "Current session CSRF token", body = CsrfResponse)
    ),
    tag = "auth"
)]
pub async fn csrf_handler(RequireAuth(user): RequireAuth) -> impl IntoResponse {
    Json(CsrfResponse {
        csrf_token: user.csrf_token,
    })
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/password",
    request_body = ChangePasswordRequest,
    responses(
        (status = 200, description = "Password updated successfully, session rotated", body = LoginResponse),
        (status = 400, description = "Incorrect current password or invalid new password"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Invalid CSRF token")
    ),
    tag = "auth"
)]
pub async fn change_password_handler(
    State(state): State<AppState>,
    RequireCsrf(auth_user): RequireCsrf,
    Json(payload): Json<ChangePasswordRequest>,
) -> Response {
    // 1. Fetch user from database
    let user_repo = SqliteUserRepository::new(state.pool.clone());
    let user = match user_repo.find_by_id(auth_user.id).await {
        Ok(Some(u)) => u,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "User not found" })),
            )
                .into_response();
        }
    };

    // 2. Verify current password
    let is_valid = verify_password(&payload.current_password, &user.password_hash).unwrap_or(false);
    if !is_valid {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Current password is incorrect" })),
        )
            .into_response();
    }

    // 3. Hash new password
    let new_hash = match hash_password(&payload.new_password) {
        Ok(h) => h,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    // 4. Update password in database
    if let Err(e) = user_repo.update_password(user.id, &new_hash).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to update password: {}", e) })),
        )
            .into_response();
    }

    // 5. Session rotation: delete all existing sessions for this user
    let session_repo = SqliteSessionRepository::new(state.pool.clone());
    let _ = session_repo.delete_by_user_id(user.id).await;

    // 6. Create a fresh session for the user
    let new_session_id = uuid::Uuid::new_v4().to_string();
    let new_token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let token_hash = hash_token(&new_token);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let expires_at = now + state.auth_config.session_duration_secs as i64;

    if let Err(e) = session_repo
        .create(&CreateSession {
            id: new_session_id.clone(),
            user_id: user.id,
            token_hash,
            ip_address: None,
            user_agent: None,
            expires_at,
        })
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to rotate session: {}", e) })),
        )
            .into_response();
    }

    let new_csrf = generate_csrf_token(&new_session_id);
    let is_secure = state
        .auth_config
        .cookie_secure
        .unwrap_or(state.environment != "development");
    let cookie_val = build_session_cookie(
        &new_token,
        state.auth_config.session_duration_secs,
        is_secure,
    );

    let mut headers = HeaderMap::new();
    if let Ok(cookie_header) = cookie_val.parse() {
        headers.insert(SET_COOKIE, cookie_header);
    }

    let response_body = LoginResponse {
        user: UserInfo {
            id: user.id,
            username: user.username,
            role: user.role,
            email: user.email,
            totp_enabled: user.totp_enabled,
        },
        csrf_token: new_csrf,
    };

    (headers, Json(response_body)).into_response()
}
