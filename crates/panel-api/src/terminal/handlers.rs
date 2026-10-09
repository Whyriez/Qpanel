//! HTTP REST API handlers for terminal session management.
//!
//! Complies with PRD Section 5.4 and Task P6-T2:
//! - Session creation with tenant quota and plan enforcement
//! - Directory confinement within tenant home
//! - Session listing, inspection, resizing, and shutdown
//! - Audit trail logging for session lifecycle events

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use panel_db::repositories::{SqlitePlanRepository, SqliteUserRepository};
use tracing::{info, warn};

use crate::auth::AuthenticatedUser;
use crate::routes::AppState;
use crate::terminal::{
    CreateTerminalSessionRequest, TerminalCloseApiResponse, TerminalListQuery,
    TerminalResizeRequest, TerminalResizeResponse, TerminalSessionListResponse,
    TerminalSessionResponse,
};

const DEFAULT_COLS: u16 = 80;
const DEFAULT_ROWS: u16 = 24;
const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 900; // 15 minutes

/// Create and spawn a new PTY terminal session.
#[utoipa::path(
    post,
    path = "/api/v1/terminal/sessions",
    request_body = CreateTerminalSessionRequest,
    responses(
        (status = 201, description = "Terminal session initiated", body = TerminalSessionResponse),
        (status = 400, description = "Invalid dimensions or working directory"),
        (status = 403, description = "Forbidden: Access denied to target tenant"),
        (status = 429, description = "Tenant plan maximum terminal session quota exceeded")
    ),
    tag = "terminal"
)]
pub async fn create_session_handler(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateTerminalSessionRequest>,
) -> Result<(StatusCode, Json<TerminalSessionResponse>), (StatusCode, Json<serde_json::Value>)> {
    let user_repo = SqliteUserRepository::new(state.pool.clone());
    let plan_repo = SqlitePlanRepository::new(state.pool.clone());

    // 1. Resolve target tenant username, uid, and gid
    let (target_user_id, linux_username, uid, gid, max_sessions) = if let Some(req_t) =
        payload.tenant.as_deref()
    {
        if req_t != user.username && !user.is_admin() {
            return Err((
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "Forbidden: Tenant users cannot spawn terminal sessions as another user"
                })),
            ));
        }

        let target_user = user_repo
            .find_by_username(req_t)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            })?
            .ok_or_else(|| {
                (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "error": "Target tenant user not found" })),
                )
            })?;

        let plan = if let Some(pid) = target_user.plan_id {
            plan_repo.find_by_id(pid).await.ok().flatten()
        } else {
            None
        };

        let max_terms = plan.map(|p| p.max_terminal_sessions as usize).unwrap_or(2);
        let uname = target_user
            .linux_username
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| {
                if target_user.username.starts_with("p_") {
                    target_user.username.clone()
                } else {
                    format!("p_{}", target_user.username)
                }
            });

        let target_uid = target_user.linux_uid.unwrap_or(10000 + target_user.id) as u32;
        let target_gid = target_uid;

        (target_user.id, uname, target_uid, target_gid, max_terms)
    } else {
        // Current user
        let current_db_user = user_repo
            .find_by_id(user.id)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            })?
            .ok_or_else(|| {
                (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "error": "User account not found" })),
                )
            })?;

        let plan = if let Some(pid) = current_db_user.plan_id {
            plan_repo.find_by_id(pid).await.ok().flatten()
        } else {
            None
        };

        let max_terms = if user.is_admin() {
            10 // Admin default generous quota
        } else {
            plan.map(|p| p.max_terminal_sessions as usize).unwrap_or(2)
        };

        let uname = current_db_user
            .linux_username
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| {
                if current_db_user.username.starts_with("p_") {
                    current_db_user.username.clone()
                } else {
                    format!("p_{}", current_db_user.username)
                }
            });

        let current_uid = current_db_user.linux_uid.unwrap_or(10000 + user.id) as u32;
        let current_gid = current_uid;

        (user.id, uname, current_uid, current_gid, max_terms)
    };

    // 2. Validate tenant plan concurrent session quota
    let active_count = state
        .terminal_hub
        .count_active_user_sessions(target_user_id)
        .await;

    if active_count >= max_sessions {
        warn!(
            "Tenant {} (user {}) exceeded terminal session quota ({} >= {})",
            linux_username, target_user_id, active_count, max_sessions
        );
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "error": format!(
                    "Batas kuota sesi terminal aktif tercapai ({}/{} sesi). Tutup sesi yang ada sebelum membuka sesi baru.",
                    active_count, max_sessions
                )
            })),
        ));
    }

    // 3. Normalize dimensions
    let cols = payload.cols.unwrap_or(DEFAULT_COLS).clamp(20, 500);
    let rows = payload.rows.unwrap_or(DEFAULT_ROWS).clamp(5, 200);

    // 4. Normalize and clean cwd
    let cwd = payload.cwd.as_deref().and_then(|c| {
        let trimmed = c.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });

    // 5. Spawn PTY session via hub & host agent
    let session = state
        .terminal_hub
        .create_session(
            target_user_id,
            linux_username.clone(),
            uid,
            gid,
            cwd.clone(),
            cols,
            rows,
            Some(max_sessions),
            Some(DEFAULT_IDLE_TIMEOUT_SECS),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;

    // 6. Record audit log entry
    let audit_details = serde_json::json!({
        "session_id": session.session_id,
        "linux_username": linux_username,
        "pid": session.pid,
        "cwd": session.cwd,
        "cols": cols,
        "rows": rows,
    });

    state.audit_writer.log_async(
        Some(user.id),
        &user.username,
        "terminal.session_create",
        &session.session_id,
        "success",
        None,
        Some(audit_details.to_string()),
    );

    info!(
        "Terminal session created: {} for user {} at {}",
        session.session_id, linux_username, session.cwd
    );

    Ok((StatusCode::CREATED, Json(session.to_response())))
}

/// List active terminal sessions belonging to the current user (or all if admin).
#[utoipa::path(
    get,
    path = "/api/v1/terminal/sessions",
    params(TerminalListQuery),
    responses(
        (status = 200, description = "List of active terminal sessions", body = TerminalSessionListResponse)
    ),
    tag = "terminal"
)]
pub async fn list_sessions_handler(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Query(query): Query<TerminalListQuery>,
) -> Result<Json<TerminalSessionListResponse>, (StatusCode, Json<serde_json::Value>)> {
    let raw_sessions = state
        .terminal_hub
        .list_sessions_for_user(user.id, user.is_admin())
        .await;

    let sessions = raw_sessions
        .into_iter()
        .filter(|s| {
            if let Some(tenant) = query.tenant.as_deref()
                && !tenant.is_empty()
            {
                return s.username == tenant || s.username == format!("p_{}", tenant);
            }
            true
        })
        .map(|s| s.to_response())
        .collect();

    Ok(Json(TerminalSessionListResponse { sessions }))
}

/// Get detailed status of a specific terminal session.
#[utoipa::path(
    get,
    path = "/api/v1/terminal/sessions/{id}",
    responses(
        (status = 200, description = "Terminal session metadata", body = TerminalSessionResponse),
        (status = 403, description = "Forbidden: Access denied"),
        (status = 404, description = "Session not found")
    ),
    tag = "terminal"
)]
pub async fn get_session_handler(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(session_id): Path<String>,
) -> Result<Json<TerminalSessionResponse>, (StatusCode, Json<serde_json::Value>)> {
    let session = state
        .terminal_hub
        .get_session(&session_id)
        .await
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Terminal session not found" })),
            )
        })?;

    if !user.is_admin() && session.user_id != user.id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Access denied to terminal session" })),
        ));
    }

    Ok(Json(session.to_response()))
}

/// Terminate and close a terminal session.
#[utoipa::path(
    delete,
    path = "/api/v1/terminal/sessions/{id}",
    responses(
        (status = 200, description = "Session terminated", body = TerminalCloseApiResponse),
        (status = 403, description = "Forbidden: Access denied"),
        (status = 404, description = "Session not found")
    ),
    tag = "terminal"
)]
pub async fn close_session_handler(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(session_id): Path<String>,
) -> Result<Json<TerminalCloseApiResponse>, (StatusCode, Json<serde_json::Value>)> {
    let session = state
        .terminal_hub
        .get_session(&session_id)
        .await
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Terminal session not found" })),
            )
        })?;

    if !user.is_admin() && session.user_id != user.id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Access denied to terminal session" })),
        ));
    }

    let closed = state
        .terminal_hub
        .close_session(&session_id, false)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;

    // Record audit event
    state.audit_writer.log_async(
        Some(user.id),
        &user.username,
        "terminal.session_close",
        &session_id,
        "success",
        None,
        Some(
            serde_json::json!({
                "session_id": session_id,
                "closed": closed,
            })
            .to_string(),
        ),
    );

    Ok(Json(TerminalCloseApiResponse {
        session_id,
        closed,
        message: "Sesi terminal berhasil ditutup".to_string(),
    }))
}

/// Resize the terminal window for a session.
#[utoipa::path(
    post,
    path = "/api/v1/terminal/sessions/{id}/resize",
    request_body = TerminalResizeRequest,
    responses(
        (status = 200, description = "Window dimensions resized", body = TerminalResizeResponse),
        (status = 403, description = "Forbidden: Access denied"),
        (status = 404, description = "Session not found")
    ),
    tag = "terminal"
)]
pub async fn resize_session_handler(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(session_id): Path<String>,
    Json(payload): Json<TerminalResizeRequest>,
) -> Result<Json<TerminalResizeResponse>, (StatusCode, Json<serde_json::Value>)> {
    let session = state
        .terminal_hub
        .get_session(&session_id)
        .await
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Terminal session not found" })),
            )
        })?;

    if !user.is_admin() && session.user_id != user.id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Access denied to terminal session" })),
        ));
    }

    let cols = payload.cols.clamp(20, 500);
    let rows = payload.rows.clamp(5, 200);

    state
        .terminal_hub
        .resize_session(&session_id, cols, rows)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;

    Ok(Json(TerminalResizeResponse {
        session_id,
        cols,
        rows,
    }))
}
