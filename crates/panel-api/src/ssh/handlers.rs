//! Handlers for SSH public key management.

use super::{
    CreateSshKeyRequest, SshConnectionInfoQuery, SshConnectionInfoResponse,
    SshDropinApplyRequestDto, SshDropinApplyResponseDto, SshKeyQuery, SshKeyResponse,
    ValidateSshKeyRequest, ValidateSshKeyResponse,
};
use crate::AppState;
use crate::auth::AuthenticatedUser;
use crate::auth::middleware::{RequireAdmin, RequireAdminCsrf};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use panel_core::ssh::{
    SshConnectionParams, generate_sftp_command, generate_sftp_url, generate_ssh_command,
    generate_ssh_config_snippet, parse_ssh_public_key,
};
use panel_core::tenant::ShellPolicy;
use panel_db::models::CreateSshKey;
use panel_db::repositories::{
    SqliteDomainRepository, SqlitePlanRepository, SqliteSiteRepository, SqliteSshKeyRepository,
    SqliteUserRepository,
};
use tracing::{info, warn};

/// `GET /api/v1/ssh-keys`: List SSH public keys for the current user or target tenant (admin only).
#[utoipa::path(
    get,
    path = "/api/v1/ssh-keys",
    params(SshKeyQuery),
    responses(
        (status = 200, description = "List of registered SSH public keys", body = Vec<SshKeyResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden: Access denied to other tenant's keys")
    ),
    tag = "ssh"
)]
pub async fn list_ssh_keys_handler(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Query(query): Query<SshKeyQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let ssh_repo = SqliteSshKeyRepository::new(state.pool.clone());
    let user_repo = SqliteUserRepository::new(state.pool.clone());

    let target_user_id = if auth_user.is_admin() {
        if let Some(uid) = query.user_id {
            uid
        } else if let Some(ref tenant_slug) = query.tenant {
            // Find user by linux_username or username
            let found = if tenant_slug.starts_with("p_") {
                user_repo
                    .find_by_linux_username(tenant_slug)
                    .await
                    .map_err(|e| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({ "error": e.to_string() })),
                        )
                    })?
            } else {
                user_repo.find_by_username(tenant_slug).await.map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({ "error": e.to_string() })),
                    )
                })?
            };
            match found {
                Some(u) => u.id,
                None => {
                    return Err((
                        StatusCode::NOT_FOUND,
                        Json(serde_json::json!({ "error": "Tenant tidak ditemukan" })),
                    ));
                }
            }
        } else {
            auth_user.id
        }
    } else {
        // Tenants can strictly only list their own keys
        if let Some(uid) = query.user_id
            && uid != auth_user.id
        {
            return Err((
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({ "error": "Akses ditolak ke SSH keys tenant lain" })),
            ));
        }
        auth_user.id
    };

    let keys = ssh_repo.list_by_user(target_user_id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    let response: Vec<SshKeyResponse> = keys.into_iter().map(SshKeyResponse::from).collect();
    Ok(Json(response))
}

/// `POST /api/v1/ssh-keys`: Register a new SSH public key for the current user.
#[utoipa::path(
    post,
    path = "/api/v1/ssh-keys",
    request_body = CreateSshKeyRequest,
    responses(
        (status = 201, description = "SSH public key registered", body = SshKeyResponse),
        (status = 400, description = "Invalid key format, algorithm or modulus size"),
        (status = 409, description = "SSH key with this fingerprint already exists"),
        (status = 401, description = "Unauthorized")
    ),
    tag = "ssh"
)]
pub async fn create_ssh_key_handler(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    headers: HeaderMap,
    Json(req): Json<CreateSshKeyRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let name = req.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                serde_json::json!({ "error": "Nama kunci SSH harus antara 1 sampai 100 karakter" }),
            ),
        ));
    }

    let parsed = parse_ssh_public_key(&req.public_key).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    let ssh_repo = SqliteSshKeyRepository::new(state.pool.clone());
    let user_repo = SqliteUserRepository::new(state.pool.clone());

    // Check for duplicate fingerprint for this user
    let existing = ssh_repo
        .find_by_user_and_fingerprint(auth_user.id, &parsed.fingerprint_sha256)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;

    if existing.is_some() {
        return Err((
            StatusCode::CONFLICT,
            Json(
                serde_json::json!({ "error": "Kunci SSH dengan fingerprint ini sudah terdaftar" }),
            ),
        ));
    }

    let created = ssh_repo
        .create(&CreateSshKey {
            user_id: auth_user.id,
            name: name.to_string(),
            public_key: parsed.public_key.clone(),
            key_type: parsed.key_type.clone(),
            fingerprint_sha256: parsed.fingerprint_sha256.clone(),
            fingerprint_md5: Some(parsed.fingerprint_md5.clone()),
        })
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;

    // Synchronize root-owned authorized_keys file via agent
    let user_record = user_repo.find_by_id(auth_user.id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    let linux_user = user_record.and_then(|u| u.linux_username);

    if let Some(ref linux_username) = linux_user {
        let all_keys = ssh_repo
            .list_by_user(auth_user.id)
            .await
            .unwrap_or_default();
        let key_strings: Vec<String> = all_keys.into_iter().map(|k| k.public_key).collect();

        if let Err(e) = state
            .agent_client
            .sync_ssh_keys(linux_username, key_strings)
            .await
        {
            warn!(
                "Failed to synchronize authorized_keys for {}: {:?}",
                linux_username, e
            );
        } else {
            info!(
                "Synchronized authorized_keys for {} after adding key id {}",
                linux_username, created.id
            );
        }
    }

    // Record audit event
    let client_ip = crate::audit::extract_client_ip(&headers);
    let details = serde_json::json!({
        "name": created.name,
        "key_type": created.key_type,
        "fingerprint": created.fingerprint_sha256,
        "linux_username": linux_user,
    });

    state.audit_writer.log_async(
        Some(auth_user.id),
        &auth_user.username,
        "ssh_key.create",
        created.id.to_string(),
        "success",
        client_ip,
        Some(details.to_string()),
    );

    Ok((StatusCode::CREATED, Json(SshKeyResponse::from(created))))
}

/// `DELETE /api/v1/ssh-keys/{id}`: Delete an existing SSH public key.
#[utoipa::path(
    delete,
    path = "/api/v1/ssh-keys/{id}",
    params(
        ("id" = i64, Path, description = "SSH key database identifier")
    ),
    responses(
        (status = 200, description = "SSH key successfully deleted"),
        (status = 403, description = "Forbidden: Access denied to other tenant's key"),
        (status = 404, description = "SSH key not found"),
        (status = 401, description = "Unauthorized")
    ),
    tag = "ssh"
)]
pub async fn delete_ssh_key_handler(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let ssh_repo = SqliteSshKeyRepository::new(state.pool.clone());
    let user_repo = SqliteUserRepository::new(state.pool.clone());

    let existing = ssh_repo.get_by_id(id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    let key = match existing {
        Some(k) => k,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Kunci SSH tidak ditemukan" })),
            ));
        }
    };

    // RBAC: Non-admin users can only delete their own keys
    if !auth_user.is_admin() && key.user_id != auth_user.id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Akses ditolak untuk menghapus kunci SSH ini" })),
        ));
    }

    let deleted = ssh_repo
        .delete(
            id,
            if auth_user.is_admin() {
                None
            } else {
                Some(auth_user.id)
            },
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;

    if !deleted {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Kunci SSH tidak ditemukan" })),
        ));
    }

    // Synchronize remaining keys to agent
    let owner = user_repo.find_by_id(key.user_id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    if let Some(user) = owner
        && let Some(linux_username) = user.linux_username
    {
        let remaining = ssh_repo.list_by_user(key.user_id).await.unwrap_or_default();
        let key_strings: Vec<String> = remaining.into_iter().map(|k| k.public_key).collect();

        if let Err(e) = state
            .agent_client
            .sync_ssh_keys(&linux_username, key_strings)
            .await
        {
            warn!(
                "Failed to synchronize authorized_keys for {} after deletion: {:?}",
                linux_username, e
            );
        } else {
            info!(
                "Synchronized authorized_keys for {} after deleting key id {}",
                linux_username, id
            );
        }
    }

    // Record audit event
    let client_ip = crate::audit::extract_client_ip(&headers);
    let details = serde_json::json!({
        "name": key.name,
        "key_type": key.key_type,
        "fingerprint": key.fingerprint_sha256,
        "owner_user_id": key.user_id,
    });

    state.audit_writer.log_async(
        Some(auth_user.id),
        &auth_user.username,
        "ssh_key.delete",
        id.to_string(),
        "success",
        client_ip,
        Some(details.to_string()),
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Kunci SSH berhasil dihapus"
    })))
}

/// `POST /api/v1/ssh-keys/validate`: Live validation utility endpoint.
#[utoipa::path(
    post,
    path = "/api/v1/ssh-keys/validate",
    request_body = ValidateSshKeyRequest,
    responses(
        (status = 200, description = "Key validation results with parsed metadata", body = ValidateSshKeyResponse),
        (status = 401, description = "Unauthorized")
    ),
    tag = "ssh"
)]
pub async fn validate_ssh_key_handler(
    _auth_user: AuthenticatedUser,
    Json(req): Json<ValidateSshKeyRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match parse_ssh_public_key(&req.public_key) {
        Ok(parsed) => Ok(Json(ValidateSshKeyResponse {
            valid: true,
            key_type: Some(parsed.key_type),
            fingerprint_sha256: Some(parsed.fingerprint_sha256),
            fingerprint_md5: Some(parsed.fingerprint_md5),
            comment: parsed.comment,
            bit_length: parsed.bit_length,
            error: None,
        })),
        Err(e) => Ok(Json(ValidateSshKeyResponse {
            valid: false,
            key_type: None,
            fingerprint_sha256: None,
            fingerprint_md5: None,
            comment: None,
            bit_length: None,
            error: Some(e.to_string()),
        })),
    }
}

/// `POST /api/v1/ssh/dropin/apply`: Apply and validate OpenSSH drop-in configuration (admin only).
#[utoipa::path(
    post,
    path = "/api/v1/ssh/dropin/apply",
    request_body = SshDropinApplyRequestDto,
    responses(
        (status = 200, description = "Drop-in configuration validated and applied", body = SshDropinApplyResponseDto),
        (status = 400, description = "sshd -t syntax validation failed"),
        (status = 403, description = "Forbidden - Admin only")
    ),
    tag = "ssh"
)]
pub async fn apply_ssh_dropin_handler(
    State(state): State<AppState>,
    admin: RequireAdmin,
    _csrf: RequireAdminCsrf,
    headers: HeaderMap,
    Json(req): Json<SshDropinApplyRequestDto>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let client_ip = crate::audit::extract_client_ip(&headers);

    match state
        .agent_client
        .apply_ssh_dropin(req.custom_content)
        .await
    {
        Ok(res) => {
            state.audit_writer.log_async(
                Some(admin.0.id),
                &admin.0.username,
                "ssh.dropin_apply",
                &res.path,
                "success",
                client_ip,
                Some(
                    serde_json::json!({
                        "validated": res.validated,
                        "reloaded": res.reloaded
                    })
                    .to_string(),
                ),
            );

            Ok(Json(SshDropinApplyResponseDto {
                success: res.success,
                path: res.path,
                validated: res.validated,
                reloaded: res.reloaded,
                message: res.message,
            }))
        }
        Err(e) => {
            state.audit_writer.log_async(
                Some(admin.0.id),
                &admin.0.username,
                "ssh.dropin_apply",
                "/etc/ssh/sshd_config.d/50-panel.conf",
                "failure",
                client_ip,
                Some(serde_json::json!({ "error": e.to_string() }).to_string()),
            );

            Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            ))
        }
    }
}

/// `GET /api/v1/ssh/connection-info`: Generate formatted SSH/SFTP commands and connection details.
#[utoipa::path(
    get,
    path = "/api/v1/ssh/connection-info",
    params(SshConnectionInfoQuery),
    responses(
        (status = 200, description = "SSH and SFTP connection commands and details", body = SshConnectionInfoResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden: Cannot access other tenant's details"),
        (status = 404, description = "Site or tenant not found")
    ),
    tag = "ssh"
)]
pub async fn get_ssh_connection_info_handler(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    req_headers: HeaderMap,
    Query(query): Query<SshConnectionInfoQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let plan_repo = SqlitePlanRepository::new(state.pool.clone());
    let ssh_repo = SqliteSshKeyRepository::new(state.pool.clone());
    let site_repo = SqliteSiteRepository::new(state.pool.clone());
    let domain_repo = SqliteDomainRepository::new(state.pool.clone());

    let context = crate::tenant_context::resolve_tenant_context(
        &state,
        &auth_user,
        query.tenant.as_deref(),
        query.site_id,
    )
    .await?;
    let target_user = context.user;
    let linux_username = context.linux_username;

    let home_dir = format!("/home/{}", linux_username);

    // 2. Resolve site context if site_id is provided
    let mut site_docroot: Option<String> = None;
    let mut site_domain: Option<String> = None;

    if let Some(site_id) = query.site_id {
        let site = site_repo
            .find_by_id(site_id)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            })?
            .ok_or((
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Situs tidak ditemukan" })),
            ))?;

        if !auth_user.can_access_user(site.user_id) {
            return Err((
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({ "error": "Akses ditolak ke situs ini" })),
            ));
        }

        site_docroot = Some(site.docroot);

        // Fetch primary domain
        if let Ok(domains) = domain_repo.find_by_site_id(site_id).await {
            site_domain = domains.into_iter().find(|d| d.is_primary).map(|d| d.name);
        }
    }

    // 3. Resolve path (active_path and relative_path)
    let raw_path = query
        .path
        .as_deref()
        .or(site_docroot.as_deref())
        .unwrap_or("");
    let raw_path_clean = raw_path.trim();

    let (active_path, relative_path) = if raw_path_clean.is_empty() {
        (home_dir.clone(), String::new())
    } else if let Some(stripped) = raw_path_clean.strip_prefix(&home_dir) {
        let rel = stripped.trim_start_matches('/').to_string();
        (raw_path_clean.to_string(), rel)
    } else if raw_path_clean.starts_with('/') {
        let rel = raw_path_clean.trim_start_matches('/').to_string();
        (format!("{}/{}", home_dir, rel), rel)
    } else {
        (
            format!("{}/{}", home_dir, raw_path_clean),
            raw_path_clean.to_string(),
        )
    };

    // 4. Resolve effective shell policy
    let plan = if let Some(plan_id) = target_user.plan_id {
        plan_repo.find_by_id(plan_id).await.map_err(|err| {
            tracing::error!(error=%err,"SSH tenant plan lookup failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error":"Gagal memuat kebijakan akses tenant."})),
            )
        })?
    } else {
        None
    };

    let policy_str = target_user
        .shell_policy
        .as_deref()
        .or_else(|| plan.as_ref().map(|p| p.shell_policy.as_str()))
        .unwrap_or("none");

    let shell_policy = ShellPolicy::parse(policy_str).unwrap_or(ShellPolicy::None);

    // 5. Check registered SSH public keys
    let keys = ssh_repo
        .list_by_user(target_user.id)
        .await
        .unwrap_or_default();
    let has_ssh_keys = !keys.is_empty();
    let ssh_keys_count = keys.len();

    // 6. Resolve host and port
    let host = if let Some(h) = query.host.filter(|h| !h.trim().is_empty()) {
        h.trim().to_string()
    } else if let Some(ref dom) = site_domain {
        dom.clone()
    } else if let Some(host_hdr) = req_headers.get("host").and_then(|h| h.to_str().ok()) {
        host_hdr
            .split(':')
            .next()
            .unwrap_or("127.0.0.1")
            .to_string()
    } else {
        "127.0.0.1".to_string()
    };

    let port = query.port.unwrap_or(22);

    // 7. Format commands via panel_core::ssh
    let conn_params = SshConnectionParams {
        username: &linux_username,
        host: &host,
        port,
        folder: Some(&active_path),
        policy: shell_policy,
    };

    let ssh_command = generate_ssh_command(&conn_params);
    let sftp_command = generate_sftp_command(&conn_params);
    let sftp_url = generate_sftp_url(&conn_params);
    let alias = site_domain
        .clone()
        .unwrap_or_else(|| format!("{}-vps", linux_username));
    let ssh_config_snippet =
        generate_ssh_config_snippet(&alias, &host, &linux_username, port, None);

    let can_shell = shell_policy == ShellPolicy::Full;
    let can_sftp = shell_policy != ShellPolicy::None;

    Ok(Json(SshConnectionInfoResponse {
        host,
        port,
        username: linux_username,
        user_id: target_user.id,
        home_dir,
        active_path,
        relative_path,
        shell_policy: shell_policy.to_string(),
        can_shell,
        can_sftp,
        has_ssh_keys,
        ssh_keys_count,
        ssh_command,
        sftp_command,
        sftp_url,
        ssh_config_snippet,
        site_domain,
    }))
}
