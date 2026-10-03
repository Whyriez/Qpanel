//! Audit log repository for recording operational and security actions.

use crate::error::DbError;
use crate::models::{AuditLog, CreateAuditLog};
use crate::pool::DbPool;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct SqliteAuditLogRepository {
    pool: DbPool,
}

impl SqliteAuditLogRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: &CreateAuditLog) -> Result<AuditLog, DbError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let res = sqlx::query(
            r#"
            INSERT INTO audit_logs (
                user_id, actor_username, action, target_resource, details_json, ip_address, status, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(input.user_id)
        .bind(&input.actor_username)
        .bind(&input.action)
        .bind(&input.target_resource)
        .bind(&input.details_json)
        .bind(&input.ip_address)
        .bind(&input.status)
        .bind(now)
        .execute(&self.pool)
        .await?;

        let id = res.last_insert_rowid();

        Ok(AuditLog {
            id,
            user_id: input.user_id,
            actor_username: input.actor_username.clone(),
            action: input.action.clone(),
            target_resource: input.target_resource.clone(),
            details_json: input.details_json.clone(),
            ip_address: input.ip_address.clone(),
            status: input.status.clone(),
            created_at: now,
        })
    }

    pub async fn list(&self, limit: i64, offset: i64) -> Result<Vec<AuditLog>, DbError> {
        let logs = sqlx::query_as::<_, AuditLog>(
            r#"
            SELECT id, user_id, actor_username, action, target_resource, details_json, ip_address, status, created_at
            FROM audit_logs
            ORDER BY created_at DESC, id DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(logs)
    }

    pub async fn list_by_user(
        &self,
        user_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditLog>, DbError> {
        let logs = sqlx::query_as::<_, AuditLog>(
            r#"
            SELECT id, user_id, actor_username, action, target_resource, details_json, ip_address, status, created_at
            FROM audit_logs
            WHERE user_id = ?
            ORDER BY created_at DESC, id DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(logs)
    }
}
