//! First-run setup token management commands for `panelctl`.

use crate::error::CliError;
use panel_db::pool::DbPool;
use panel_db::repositories::{SqliteSettingRepository, SqliteUserRepository};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SETTING_SETUP_TOKEN: &str = "setup.first_run_token";
pub const SETTING_SETUP_TOKEN_CREATED_AT: &str = "setup.first_run_token_created_at";

/// Result of checking setup status.
#[derive(Debug, PartialEq, Eq)]
pub enum SetupStatus {
    /// An admin account is already present; first-run setup is complete.
    Completed,
    /// No admin account exists, and an active setup token is available.
    Pending {
        token: String,
        created_at: Option<i64>,
    },
    /// No admin account exists, but no setup token has been generated yet.
    Uninitialized,
}

/// Generates a single-use setup token for first-run configuration.
///
/// Fails with `CliError::Conflict` if an admin user already exists.
pub async fn generate_setup_token(pool: &DbPool) -> Result<String, CliError> {
    let user_repo = SqliteUserRepository::new(pool.clone());
    let setting_repo = SqliteSettingRepository::new(pool.clone());

    let users = user_repo.list_all().await?;
    if users.iter().any(|u| u.role == "admin") {
        return Err(CliError::Conflict(
            "Setup token cannot be generated: an admin user already exists".to_string(),
        ));
    }

    // Generate a 64-character cryptographically secure token using two v4 UUIDs
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    setting_repo.set(SETTING_SETUP_TOKEN, &token).await?;
    setting_repo
        .set(SETTING_SETUP_TOKEN_CREATED_AT, &now.to_string())
        .await?;

    Ok(token)
}

/// Inspects the current setup status.
pub async fn get_setup_status(pool: &DbPool) -> Result<SetupStatus, CliError> {
    let user_repo = SqliteUserRepository::new(pool.clone());
    let setting_repo = SqliteSettingRepository::new(pool.clone());

    let users = user_repo.list_all().await?;
    if users.iter().any(|u| u.role == "admin") {
        return Ok(SetupStatus::Completed);
    }

    if let Some(token) = setting_repo.get(SETTING_SETUP_TOKEN).await? {
        let created_at = setting_repo
            .get(SETTING_SETUP_TOKEN_CREATED_AT)
            .await?
            .and_then(|s| s.parse::<i64>().ok());

        Ok(SetupStatus::Pending { token, created_at })
    } else {
        Ok(SetupStatus::Uninitialized)
    }
}

/// Verifies whether the provided token matches the active first-run setup token.
pub async fn verify_setup_token(pool: &DbPool, candidate: &str) -> Result<bool, CliError> {
    let user_repo = SqliteUserRepository::new(pool.clone());
    let setting_repo = SqliteSettingRepository::new(pool.clone());

    let users = user_repo.list_all().await?;
    if users.iter().any(|u| u.role == "admin") {
        return Ok(false);
    }

    if let Some(token) = setting_repo.get(SETTING_SETUP_TOKEN).await? {
        Ok(token == candidate)
    } else {
        Ok(false)
    }
}
