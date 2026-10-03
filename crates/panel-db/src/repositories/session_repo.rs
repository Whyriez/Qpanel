//! Session repository for authentication session tokens.

use crate::error::DbError;
use crate::models::{CreateSession, Session};
use crate::pool::DbPool;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct SqliteSessionRepository {
    pool: DbPool,
}

impl SqliteSessionRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: &CreateSession) -> Result<Session, DbError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        sqlx::query(
            r#"
            INSERT INTO sessions (id, user_id, token_hash, ip_address, user_agent, expires_at, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&input.id)
        .bind(input.user_id)
        .bind(&input.token_hash)
        .bind(&input.ip_address)
        .bind(&input.user_agent)
        .bind(input.expires_at)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(Session {
            id: input.id.clone(),
            user_id: input.user_id,
            token_hash: input.token_hash.clone(),
            ip_address: input.ip_address.clone(),
            user_agent: input.user_agent.clone(),
            expires_at: input.expires_at,
            created_at: now,
        })
    }

    pub async fn find_by_token_hash(&self, token_hash: &str) -> Result<Option<Session>, DbError> {
        let session = sqlx::query_as::<_, Session>(
            r#"
            SELECT id, user_id, token_hash, ip_address, user_agent, expires_at, created_at
            FROM sessions
            WHERE token_hash = ?
            "#,
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;

        Ok(session)
    }

    pub async fn list_by_user_id(&self, user_id: i64) -> Result<Vec<Session>, DbError> {
        let sessions = sqlx::query_as::<_, Session>(
            r#"
            SELECT id, user_id, token_hash, ip_address, user_agent, expires_at, created_at
            FROM sessions
            WHERE user_id = ?
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(sessions)
    }

    pub async fn delete(&self, id: &str) -> Result<(), DbError> {
        let res = sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if res.rows_affected() == 0 {
            return Err(DbError::NotFound(format!("Session {} not found", id)));
        }
        Ok(())
    }

    pub async fn delete_by_user_id(&self, user_id: i64) -> Result<(), DbError> {
        sqlx::query("DELETE FROM sessions WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_expired(&self, now: i64) -> Result<u64, DbError> {
        let res = sqlx::query("DELETE FROM sessions WHERE expires_at <= ?")
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }
}
