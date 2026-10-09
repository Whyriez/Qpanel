pub mod handlers;

pub use handlers::*;

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SshKeyResponse {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub key_type: String,
    pub fingerprint_sha256: String,
    pub fingerprint_md5: Option<String>,
    pub public_key: String,
    pub created_at: i64,
}

impl From<panel_db::models::SshKey> for SshKeyResponse {
    fn from(k: panel_db::models::SshKey) -> Self {
        Self {
            id: k.id,
            user_id: k.user_id,
            name: k.name,
            key_type: k.key_type,
            fingerprint_sha256: k.fingerprint_sha256,
            fingerprint_md5: k.fingerprint_md5,
            public_key: k.public_key,
            created_at: k.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateSshKeyRequest {
    pub name: String,
    pub public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ValidateSshKeyRequest {
    pub public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ValidateSshKeyResponse {
    pub valid: bool,
    pub key_type: Option<String>,
    pub fingerprint_sha256: Option<String>,
    pub fingerprint_md5: Option<String>,
    pub comment: Option<String>,
    pub bit_length: Option<usize>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct SshKeyQuery {
    pub user_id: Option<i64>,
    pub tenant: Option<String>,
}
