//! `panel-core`: Domain types, newtype validators, and common error definitions.

use serde::{Deserialize, Serialize};
use thiserror::Error;

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
        Ok(Self(s))
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
}
