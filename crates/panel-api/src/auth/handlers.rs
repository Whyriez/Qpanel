//! HTTP handlers for authentication and session management.

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Response},
};
use panel_core::auth::{hash_password, hash_token, verify_password};
use panel_core::totp::{
    generate_recovery_codes, generate_totp_qr_base64, generate_totp_secret, generate_totp_uri,
    hash_recovery_code, verify_totp_code,
};
use panel_db::models::CreateSession;
use panel_db::repositories::{
    SqliteRecoveryCodeRepository, SqliteSessionRepository, SqliteUserRepository,
};
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
    #[schema(example = "123456")]
    pub code: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<UserInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub csrf_token: Option<String>,
    pub requires_2fa: bool,
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

#[derive(Debug, Serialize, ToSchema)]
pub struct TwoFactorSetupResponse {
    pub secret: String,
    pub otpauth_url: String,
    pub qr_code_base64: String,
    pub recovery_codes: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TwoFactorEnableRequest {
    #[schema(example = "123456")]
    pub code: String,
    pub secret: String,
    pub recovery_codes: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TwoFactorDisableRequest {
    #[schema(example = "123456")]
    pub code: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TwoFactorStatusResponse {
    pub enabled: bool,
    pub remaining_recovery_codes: i64,
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful or 2FA challenge required", body = LoginResponse),
        (status = 401, description = "Invalid credentials"),
        (status = 423, description = "Account locked due to brute force attempts")
    ),
    tag = "auth"
)]
pub async fn login_handler(
    State(state): State<AppState>,
    req_headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> Response {
    let client_ip = crate::audit::extract_client_ip(&req_headers);

    // 1. Check account lockout status
    if let Err(remaining_secs) = state.lockout_manager.check_lockout(&payload.username) {
        state.audit_writer.log_async(
            None,
            &payload.username,
            "auth.login",
            "auth",
            "failure",
            client_ip.clone(),
            Some(r#"{"reason":"account_locked"}"#.to_string()),
        );
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
            state.audit_writer.log_async(
                None,
                &payload.username,
                "auth.login",
                "auth",
                "failure",
                client_ip.clone(),
                Some(r#"{"reason":"user_not_found"}"#.to_string()),
            );
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
        state.audit_writer.log_async(
            Some(user.id),
            &user.username,
            "auth.login",
            "auth",
            "failure",
            client_ip.clone(),
            Some(r#"{"reason":"invalid_password"}"#.to_string()),
        );
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
        state.audit_writer.log_async(
            Some(user.id),
            &user.username,
            "auth.login",
            "auth",
            "failure",
            client_ip.clone(),
            Some(r#"{"reason":"inactive_account"}"#.to_string()),
        );
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "Account is inactive or suspended"
            })),
        )
            .into_response();
    }

    // 5. Check 2FA if enabled
    if user.totp_enabled {
        match payload.code.as_deref() {
            None => {
                return (
                    StatusCode::OK,
                    Json(LoginResponse {
                        user: None,
                        csrf_token: None,
                        requires_2fa: true,
                    }),
                )
                    .into_response();
            }
            Some(submitted_code) => {
                let code_trimmed = submitted_code.trim();
                let secret = user.totp_secret.as_deref().unwrap_or("");
                let is_totp_valid = verify_totp_code(secret, code_trimmed, &user.username);

                let is_valid_2fa = if is_totp_valid {
                    true
                } else {
                    let recovery_repo = SqliteRecoveryCodeRepository::new(state.pool.clone());
                    let code_hash = hash_recovery_code(code_trimmed);
                    recovery_repo
                        .verify_and_consume(user.id, &code_hash)
                        .await
                        .unwrap_or(false)
                };

                if !is_valid_2fa {
                    let lockout = state.lockout_manager.record_failure(&payload.username);
                    state.audit_writer.log_async(
                        Some(user.id),
                        &user.username,
                        "auth.login",
                        "auth",
                        "failure",
                        client_ip.clone(),
                        Some(r#"{"reason":"invalid_2fa"}"#.to_string()),
                    );
                    let status = if lockout.is_some() {
                        StatusCode::LOCKED
                    } else {
                        StatusCode::UNAUTHORIZED
                    };
                    return (
                        status,
                        Json(serde_json::json!({
                            "error": "Invalid two-factor authentication code or recovery code"
                        })),
                    )
                        .into_response();
                }
            }
        }
    }

    // 6. Successful login: reset lockout counter
    state.lockout_manager.record_success(&payload.username);

    // 7. Create session in database
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
            ip_address: client_ip.clone(),
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

    // Record successful login audit log
    state.audit_writer.log_async(
        Some(user.id),
        &user.username,
        "auth.login",
        "auth",
        "success",
        client_ip,
        None,
    );

    // 8. Generate CSRF token and Set-Cookie header
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
        user: Some(UserInfo {
            id: user.id,
            username: user.username,
            role: user.role,
            email: user.email,
            totp_enabled: user.totp_enabled,
        }),
        csrf_token: Some(csrf_token),
        requires_2fa: false,
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

    state.audit_writer.log_async(
        Some(user.id),
        &user.username,
        "auth.logout",
        "auth",
        "success",
        None,
        None,
    );

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
        user: Some(UserInfo {
            id: user.id,
            username: user.username,
            role: user.role,
            email: user.email,
            totp_enabled: user.totp_enabled,
        }),
        csrf_token: Some(user.csrf_token),
        requires_2fa: false,
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
        state.audit_writer.log_async(
            Some(auth_user.id),
            &auth_user.username,
            "auth.password_change",
            "auth",
            "failure",
            None,
            Some(r#"{"reason":"incorrect_current_password"}"#.to_string()),
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Current password is incorrect" })),
        )
            .into_response();
    }

    // 3. Validate new password length
    if payload.new_password.len() < 8 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "New password must be at least 8 characters long"
            })),
        )
            .into_response();
    }

    // 4. Hash new password and update in database
    let new_hash = match hash_password(&payload.new_password) {
        Ok(h) => h,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Failed to hash password: {}", e) })),
            )
                .into_response();
        }
    };

    if let Err(e) = user_repo.update_password(user.id, &new_hash).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to update password: {}", e) })),
        )
            .into_response();
    }

    // 5. Session rotation: delete old session and create a new one
    let session_repo = SqliteSessionRepository::new(state.pool.clone());
    let _ = session_repo.delete(&auth_user.session_id).await;

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

    state.audit_writer.log_async(
        Some(user.id),
        &user.username,
        "auth.password_change",
        "auth",
        "success",
        None,
        None,
    );

    let response_body = LoginResponse {
        user: Some(UserInfo {
            id: user.id,
            username: user.username,
            role: user.role,
            email: user.email,
            totp_enabled: user.totp_enabled,
        }),
        csrf_token: Some(new_csrf),
        requires_2fa: false,
    };

    (headers, Json(response_body)).into_response()
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/2fa/setup",
    responses(
        (status = 200, description = "Generated TOTP secret, QR code, and recovery codes", body = TwoFactorSetupResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Invalid CSRF token")
    ),
    tag = "auth"
)]
pub async fn two_factor_setup_handler(RequireCsrf(user): RequireCsrf) -> Response {
    let secret = generate_totp_secret();
    let otpauth_url = match generate_totp_uri(&secret, &user.username, None) {
        Ok(url) => url,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Failed to generate TOTP URI: {}", e) })),
            )
                .into_response();
        }
    };

    let qr_code_base64 = match generate_totp_qr_base64(&secret, &user.username, None) {
        Ok(qr) => qr,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Failed to generate QR code: {}", e) })),
            )
                .into_response();
        }
    };

    let recovery_codes = generate_recovery_codes(8);

    Json(TwoFactorSetupResponse {
        secret,
        otpauth_url,
        qr_code_base64,
        recovery_codes,
    })
    .into_response()
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/2fa/enable",
    request_body = TwoFactorEnableRequest,
    responses(
        (status = 200, description = "2FA enabled successfully", body = MessageResponse),
        (status = 400, description = "Invalid verification code or recovery codes"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Invalid CSRF token")
    ),
    tag = "auth"
)]
pub async fn two_factor_enable_handler(
    State(state): State<AppState>,
    RequireCsrf(user): RequireCsrf,
    Json(payload): Json<TwoFactorEnableRequest>,
) -> Response {
    // 1. Verify code against provided secret
    if !verify_totp_code(&payload.secret, &payload.code, &user.username) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Invalid verification code" })),
        )
            .into_response();
    }

    if payload.recovery_codes.len() != 8 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Must provide exactly 8 recovery codes" })),
        )
            .into_response();
    }

    // 2. Hash recovery codes and save to DB
    let recovery_hashes: Vec<String> = payload
        .recovery_codes
        .iter()
        .map(|code| hash_recovery_code(code))
        .collect();

    let recovery_repo = SqliteRecoveryCodeRepository::new(state.pool.clone());
    if let Err(e) = recovery_repo.replace_codes(user.id, &recovery_hashes).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to save recovery codes: {}", e) })),
        )
            .into_response();
    }

    // 3. Update user to enable TOTP and store secret
    let user_repo = SqliteUserRepository::new(state.pool.clone());
    if let Err(e) = user_repo
        .update_totp(user.id, Some(&payload.secret), true)
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to enable TOTP on user: {}", e) })),
        )
            .into_response();
    }

    state.audit_writer.log_async(
        Some(user.id),
        &user.username,
        "auth.2fa_enable",
        "auth",
        "success",
        None,
        None,
    );

    Json(MessageResponse {
        message: "Two-factor authentication enabled successfully".to_string(),
    })
    .into_response()
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/2fa/disable",
    request_body = TwoFactorDisableRequest,
    responses(
        (status = 200, description = "2FA disabled successfully", body = MessageResponse),
        (status = 400, description = "Invalid code or 2FA not enabled"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Invalid CSRF token")
    ),
    tag = "auth"
)]
pub async fn two_factor_disable_handler(
    State(state): State<AppState>,
    RequireCsrf(user): RequireCsrf,
    Json(payload): Json<TwoFactorDisableRequest>,
) -> Response {
    let user_repo = SqliteUserRepository::new(state.pool.clone());
    let db_user = match user_repo.find_by_id(user.id).await {
        Ok(Some(u)) => u,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "User not found" })),
            )
                .into_response();
        }
    };

    if !db_user.totp_enabled {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Two-factor authentication is not enabled" })),
        )
            .into_response();
    }

    let code_trimmed = payload.code.trim();
    let secret = db_user.totp_secret.as_deref().unwrap_or("");
    let is_totp_valid = verify_totp_code(secret, code_trimmed, &db_user.username);

    let is_valid = if is_totp_valid {
        true
    } else {
        let recovery_repo = SqliteRecoveryCodeRepository::new(state.pool.clone());
        let code_hash = hash_recovery_code(code_trimmed);
        recovery_repo
            .verify_and_consume(user.id, &code_hash)
            .await
            .unwrap_or(false)
    };

    if !is_valid {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Invalid verification code or recovery code" })),
        )
            .into_response();
    }

    // Disable TOTP on user
    if let Err(e) = user_repo.update_totp(user.id, None, false).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to disable TOTP: {}", e) })),
        )
            .into_response();
    }

    // Delete recovery codes
    let recovery_repo = SqliteRecoveryCodeRepository::new(state.pool.clone());
    let _ = recovery_repo.delete_by_user_id(user.id).await;

    state.audit_writer.log_async(
        Some(user.id),
        &user.username,
        "auth.2fa_disable",
        "auth",
        "success",
        None,
        None,
    );

    Json(MessageResponse {
        message: "Two-factor authentication disabled successfully".to_string(),
    })
    .into_response()
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/2fa/status",
    responses(
        (status = 200, description = "Two-factor authentication status", body = TwoFactorStatusResponse),
        (status = 401, description = "Not authenticated")
    ),
    tag = "auth"
)]
pub async fn two_factor_status_handler(
    State(state): State<AppState>,
    RequireAuth(user): RequireAuth,
) -> Response {
    let user_repo = SqliteUserRepository::new(state.pool.clone());
    let db_user = match user_repo.find_by_id(user.id).await {
        Ok(Some(u)) => u,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "User not found" })),
            )
                .into_response();
        }
    };

    let remaining = if db_user.totp_enabled {
        let recovery_repo = SqliteRecoveryCodeRepository::new(state.pool.clone());
        recovery_repo.count_unused(user.id).await.unwrap_or(0)
    } else {
        0
    };

    Json(TwoFactorStatusResponse {
        enabled: db_user.totp_enabled,
        remaining_recovery_codes: remaining,
    })
    .into_response()
}
