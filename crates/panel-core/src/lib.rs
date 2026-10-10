//! `panel-core`: Domain types, newtype validators, and common error definitions.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod auth;
pub mod cron;
pub mod domain;
pub mod php;
pub mod rbac;
pub mod ssh;
pub mod tenant;
pub mod totp;
pub use auth::{hash_password, hash_token, verify_password};
pub use cron::{
    ParsedCron, cron_to_human_en, cron_to_human_id, cron_to_systemd_calendar,
    validate_cron_expression,
};
pub use domain::{DomainName, DomainValidationError};
pub use rbac::{Action, Role, authorize, check_permission};
pub use ssh::{
    ParsedSshKey, SshConnectionParams, SshKeyError, escape_shell_single_quote,
    generate_sftp_command, generate_sftp_url, generate_ssh_command, generate_ssh_config_snippet,
    parse_ssh_public_key,
};
pub use tenant::{MAX_TENANT_UID, MIN_TENANT_UID, ShellPolicy, TenantUid, is_reserved_username};
pub use totp::{
    generate_recovery_codes, generate_totp_qr_base64, generate_totp_secret, generate_totp_uri,
    hash_recovery_code, normalize_recovery_code, verify_totp_code,
};

/// Common error types for domain operations.
#[derive(Debug, Error, Serialize, Deserialize)]
pub enum CoreError {
    #[error("Invalid username: {0}")]
    InvalidUsername(String),
    #[error("Invalid domain: {0}")]
    InvalidDomain(String),
    #[error("Invalid relative path: {0}")]
    InvalidPath(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Forbidden: {0}")]
    Forbidden(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
}

/// Linux tenant username (e.g. `p_alice`).
/// Must match `^p_[a-z0-9_]{1,28}$` and not collide with system users.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Username(String);

impl Username {
    pub fn parse(input: impl Into<String>) -> Result<Self, CoreError> {
        let s = input.into();
        if !s.starts_with("p_") {
            return Err(CoreError::InvalidUsername(
                "Username must start with 'p_' prefix".to_string(),
            ));
        }
        if s.len() < 3 || s.len() > 30 {
            return Err(CoreError::InvalidUsername(
                "Username length must be between 3 and 30 characters".to_string(),
            ));
        }
        let rest = &s[2..];
        if !rest
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(CoreError::InvalidUsername(
                "Username may only contain lowercase letters, numbers, and underscores".to_string(),
            ));
        }
        if is_reserved_username(&s) {
            return Err(CoreError::InvalidUsername(format!(
                "Username '{}' collides with reserved system user",
                s
            )));
        }
        Ok(Self(s))
    }

    /// Generates a valid Linux tenant username from a user slug/name.
    pub fn from_slug(slug: &str) -> Result<Self, CoreError> {
        let cleaned: String = slug
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect();
        let stripped = cleaned.trim_matches('_');
        let raw = if stripped.starts_with("p_") {
            stripped.to_string()
        } else {
            format!("p_{stripped}")
        };
        Self::parse(raw)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Username {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_username() {
        let u = Username::parse("p_alice").expect("valid username");
        assert_eq!(u.as_str(), "p_alice");
    }

    #[test]
    fn test_invalid_username_no_prefix() {
        assert!(Username::parse("alice").is_err());
    }

    #[test]
    fn test_invalid_username_characters() {
        assert!(Username::parse("p_Alice!").is_err());
    }

    #[test]
    fn test_reserved_username_rejected() {
        assert!(Username::parse("p_root").is_err());
        assert!(Username::parse("p_nginx").is_err());
        assert!(Username::parse("p_admin").is_err());
    }

    #[test]
    fn test_username_from_slug() {
        let u1 = Username::from_slug("My Client").expect("from slug");
        assert_eq!(u1.as_str(), "p_my_client");

        let u2 = Username::from_slug("p_bob").expect("from slug with prefix");
        assert_eq!(u2.as_str(), "p_bob");

        assert!(Username::from_slug("root").is_err());
    }
}
