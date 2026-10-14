//! Token-authorized first-run setup. User creation and token consumption commit together.
use crate::routes::AppState;
use axum::{Json, extract::State, http::StatusCode};
use panel_core::{constant_time_compare, hash_password, hash_token, validate_email_address};
use serde::{Deserialize, Serialize};
use sqlx::Row;

type ApiError = (StatusCode, Json<serde_json::Value>);
fn error(status: StatusCode, message: &str) -> ApiError {
    (status, Json(serde_json::json!({"error": message})))
}
fn db_error(err: sqlx::Error) -> ApiError {
    tracing::error!(error = %err, "First-run database operation failed");
    error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Gagal menyimpan administrator. Token belum digunakan; silakan coba lagi.",
    )
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetupTokenRequest {
    pub token: String,
}
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetupRequest {
    pub token: String,
    pub username: String,
    pub email: Option<String>,
    pub password: String,
}
#[derive(Serialize, utoipa::ToSchema)]
pub struct SetupResponse {
    pub user_id: i64,
    pub username: String,
    pub created: bool,
}

async fn check_token(state: &AppState, candidate: &str) -> Result<(), ApiError> {
    let admins: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role='admin'")
        .fetch_one(&state.pool)
        .await
        .map_err(db_error)?;
    if admins != 0 {
        return Err(error(
            StatusCode::CONFLICT,
            "Pengaturan administrator sudah selesai.",
        ));
    }
    let token: Option<String> =
        sqlx::query_scalar("SELECT value FROM settings WHERE key='setup.first_run_token'")
            .fetch_optional(&state.pool)
            .await
            .map_err(db_error)?;
    if !token.as_ref().is_some_and(|t| {
        !t.is_empty()
            && constant_time_compare(hash_token(t).as_bytes(), hash_token(candidate).as_bytes())
    }) {
        return Err(error(
            StatusCode::UNAUTHORIZED,
            "Token setup tidak valid atau sudah digunakan.",
        ));
    }
    Ok(())
}

#[utoipa::path(post, path="/api/v1/auth/setup/validate", request_body=SetupTokenRequest, responses((status=200,description="Setup token valid"),(status=401,description="Invalid token"),(status=409,description="Setup completed")), tag="auth")]
pub async fn validate_token(
    State(state): State<AppState>,
    Json(payload): Json<SetupTokenRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    check_token(&state, &payload.token).await?;
    Ok(Json(serde_json::json!({"valid": true})))
}

#[utoipa::path(post, path="/api/v1/auth/setup", request_body=SetupRequest, responses((status=201,description="Administrator committed and token consumed",body=SetupResponse),(status=400,description="Invalid account data"),(status=401,description="Invalid token"),(status=409,description="Setup completed or duplicate username"),(status=500,description="Transaction rolled back")), tag="auth")]
pub async fn create_administrator(
    State(state): State<AppState>,
    Json(payload): Json<SetupRequest>,
) -> Result<(StatusCode, Json<SetupResponse>), ApiError> {
    check_token(&state, &payload.token).await?;
    let username = payload.username.trim();
    if !(3..=32).contains(&username.len())
        || !username
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "Nama pengguna harus 3–32 karakter: huruf, angka, _ atau -.",
        ));
    }
    let email = payload
        .email
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(email) = email {
        validate_email_address(email)
            .map_err(|_| error(StatusCode::BAD_REQUEST, "Alamat email tidak valid."))?;
    }
    if !(8..=1024).contains(&payload.password.len()) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "Kata sandi harus 8–1024 karakter.",
        ));
    }
    let password_hash = tokio::task::spawn_blocking(move || hash_password(&payload.password))
        .await
        .map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Gagal mengamankan kata sandi.",
            )
        })?
        .map_err(|_| error(StatusCode::BAD_REQUEST, "Kata sandi tidak valid."))?;
    let mut tx = state
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(db_error)?;
    let admins: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role='admin'")
        .fetch_one(&mut *tx)
        .await
        .map_err(db_error)?;
    let stored: Option<String> =
        sqlx::query_scalar("SELECT value FROM settings WHERE key='setup.first_run_token'")
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_error)?;
    if admins != 0
        || !stored.is_some_and(|t| {
            !t.is_empty()
                && constant_time_compare(
                    hash_token(&t).as_bytes(),
                    hash_token(&payload.token).as_bytes(),
                )
        })
    {
        return Err(error(
            StatusCode::CONFLICT,
            "Setup sudah selesai atau token sudah digunakan.",
        ));
    }
    let now = chrono::Utc::now().timestamp();
    let insert = sqlx::query("INSERT INTO users (username,password_hash,role,email,status,created_at,updated_at) VALUES (?,?,'admin',?,'active',?,?)")
        .bind(username).bind(&password_hash).bind(email).bind(now).bind(now).execute(&mut *tx).await
        .map_err(|err| {
            if err.as_database_error().is_some_and(|e| e.is_unique_violation()) {
                error(StatusCode::CONFLICT, "Nama pengguna sudah digunakan. Pilih nama lain.")
            } else { db_error(err) }
        })?;
    let id = insert.last_insert_rowid();
    let persisted =
        sqlx::query("SELECT id FROM users WHERE id=? AND role='admin' AND status='active'")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_error)?;
    if persisted
        .as_ref()
        .and_then(|r| r.try_get::<i64, _>("id").ok())
        != Some(id)
    {
        return Err(error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Administrator gagal diverifikasi; setup dibatalkan.",
        ));
    }
    sqlx::query("DELETE FROM settings WHERE key IN ('setup.first_run_token','setup.first_run_token_created_at')")
        .execute(&mut *tx).await.map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    Ok((
        StatusCode::CREATED,
        Json(SetupResponse {
            user_id: id,
            username: username.to_owned(),
            created: true,
        }),
    ))
}
