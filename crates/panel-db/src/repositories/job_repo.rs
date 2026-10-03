//! Job repository for background tasks and asynchronous operations.

use crate::error::DbError;
use crate::models::{CreateJob, Job};
use crate::pool::DbPool;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct SqliteJobRepository {
    pool: DbPool,
}

impl SqliteJobRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: &CreateJob) -> Result<Job, DbError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        sqlx::query(
            r#"
            INSERT INTO jobs (
                id, user_id, name, status, progress, payload_json,
                result_json, error_message, retry_count, max_retries, created_at, updated_at
            )
            VALUES (?, ?, ?, 'queued', 0, ?, NULL, NULL, 0, ?, ?, ?)
            "#,
        )
        .bind(&input.id)
        .bind(input.user_id)
        .bind(&input.name)
        .bind(&input.payload_json)
        .bind(input.max_retries)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(Job {
            id: input.id.clone(),
            user_id: input.user_id,
            name: input.name.clone(),
            status: "queued".to_string(),
            progress: 0,
            payload_json: input.payload_json.clone(),
            result_json: None,
            error_message: None,
            retry_count: 0,
            max_retries: input.max_retries,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Option<Job>, DbError> {
        let job = sqlx::query_as::<_, Job>(
            r#"
            SELECT
                id, user_id, name, status, progress, payload_json,
                result_json, error_message, retry_count, max_retries, created_at, updated_at
            FROM jobs
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(job)
    }

    pub async fn update_progress(&self, id: &str, progress: i64) -> Result<(), DbError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let res = sqlx::query("UPDATE jobs SET progress = ?, updated_at = ? WHERE id = ?")
            .bind(progress)
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await?;

        if res.rows_affected() == 0 {
            return Err(DbError::NotFound(format!("Job {} not found", id)));
        }
        Ok(())
    }

    pub async fn update_status(
        &self,
        id: &str,
        status: &str,
        result_json: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), DbError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let res = sqlx::query(
            r#"
            UPDATE jobs
            SET status = ?, result_json = ?, error_message = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(status)
        .bind(result_json)
        .bind(error_message)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        if res.rows_affected() == 0 {
            return Err(DbError::NotFound(format!("Job {} not found", id)));
        }
        Ok(())
    }

    pub async fn list_by_status(&self, status: &str) -> Result<Vec<Job>, DbError> {
        let jobs = sqlx::query_as::<_, Job>(
            r#"
            SELECT
                id, user_id, name, status, progress, payload_json,
                result_json, error_message, retry_count, max_retries, created_at, updated_at
            FROM jobs
            WHERE status = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(status)
        .fetch_all(&self.pool)
        .await?;

        Ok(jobs)
    }

    pub async fn list_by_user(&self, user_id: i64, limit: i64) -> Result<Vec<Job>, DbError> {
        let jobs = sqlx::query_as::<_, Job>(
            r#"
            SELECT
                id, user_id, name, status, progress, payload_json,
                result_json, error_message, retry_count, max_retries, created_at, updated_at
            FROM jobs
            WHERE user_id = ?
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(jobs)
    }

    /// Recovers jobs left in 'running' state after an unexpected server restart.
    pub async fn recover_interrupted_jobs(&self) -> Result<u64, DbError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let res = sqlx::query(
            r#"
            UPDATE jobs
            SET status = 'failed', error_message = 'Interrupted by server restart', updated_at = ?
            WHERE status = 'running'
            "#,
        )
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(res.rows_affected())
    }
}
