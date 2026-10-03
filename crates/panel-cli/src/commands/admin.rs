//! Admin management commands for `panelctl`.

use crate::error::CliError;
use panel_core::auth::hash_password;
use panel_db::models::CreateUser;
use panel_db::pool::DbPool;
use panel_db::repositories::{SqliteSettingRepository, SqliteUserRepository};

/// Validate that a username is suitable for a panel user.
pub fn validate_admin_username(username: &str) -> Result<(), CliError> {
    if username.len() < 3 || username.len() > 32 {
        return Err(CliError::Validation(
            "Username must be between 3 and 32 characters long".to_string(),
        ));
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(CliError::Validation(
            "Username may only contain alphanumeric characters, underscores, and hyphens"
                .to_string(),
        ));
    }
    Ok(())
}

/// Creates a new admin user.
pub async fn create_admin(
    pool: &DbPool,
    username: &str,
    password: &str,
    email: Option<String>,
) -> Result<i64, CliError> {
    validate_admin_username(username)?;

    let user_repo = SqliteUserRepository::new(pool.clone());
    let setting_repo = SqliteSettingRepository::new(pool.clone());

    if user_repo.find_by_username(username).await?.is_some() {
        return Err(CliError::Conflict(format!(
            "User '{}' already exists",
            username
        )));
    }

    let password_hash = hash_password(password)?;

    let user = user_repo
        .create(&CreateUser {
            username: username.to_string(),
            password_hash,
            role: "admin".to_string(),
            email,
            linux_username: None,
            linux_uid: None,
            plan_id: None,
        })
        .await?;

    // Invalidate any first-run setup token now that an admin user exists
    let _ = setting_repo.delete("setup.first_run_token").await;
    let _ = setting_repo
        .delete("setup.first_run_token_created_at")
        .await;

    Ok(user.id)
}

/// Resets the password for an existing user.
pub async fn reset_password(
    pool: &DbPool,
    username: &str,
    new_password: &str,
) -> Result<(), CliError> {
    let user_repo = SqliteUserRepository::new(pool.clone());

    let user = user_repo
        .find_by_username(username)
        .await?
        .ok_or_else(|| CliError::NotFound(format!("User '{}' not found", username)))?;

    let new_hash = hash_password(new_password)?;
    user_repo.update_password(user.id, &new_hash).await?;

    Ok(())
}

/// Lists all admin users.
pub async fn list_admins(pool: &DbPool) -> Result<Vec<panel_db::models::User>, CliError> {
    let user_repo = SqliteUserRepository::new(pool.clone());
    let users = user_repo.list_all().await?;
    let admins = users.into_iter().filter(|u| u.role == "admin").collect();
    Ok(admins)
}
