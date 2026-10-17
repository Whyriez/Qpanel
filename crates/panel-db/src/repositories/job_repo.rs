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
    /// SQLite serializes this INSERT/SELECT transaction across API workers.
    pub async fn create_or_find_active(&self, input: &CreateJob) -> Result<(Job, bool), DbError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            "INSERT INTO jobs (id,user_id,name,status,progress,payload_json,retry_count,max_retries,created_at,updated_at) SELECT ?,?,?,'queued',0,?,0,?,?,? WHERE NOT EXISTS (SELECT 1 FROM jobs WHERE name=? AND payload_json=? AND status IN ('queued','running'))"
        ).bind(&input.id).bind(input.user_id).bind(&input.name).bind(&input.payload_json)
            .bind(input.max_retries).bind(now).bind(now).bind(&input.name).bind(&input.payload_json)
            .execute(&mut *tx).await?.rows_affected() > 0;
        let job = sqlx::query_as::<_, Job>("SELECT * FROM jobs WHERE name=? AND payload_json=? AND status IN ('queued','running') ORDER BY created_at,id LIMIT 1")
            .bind(&input.name).bind(&input.payload_json).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok((job, !inserted))
    }

    pub async fn append_event(&self, event: &crate::models::job::JobEvent) -> Result<i64, DbError> {
        let mut tx = self.pool.begin().await?;
        let sequence = sqlx::query(
            "INSERT INTO job_events(job_id,status,progress,message,timestamp) VALUES(?,?,?,?,?)",
        )
        .bind(&event.job_id)
        .bind(&event.status)
        .bind(event.progress)
        .bind(&event.message)
        .bind(event.timestamp)
        .execute(&mut *tx)
        .await?
        .last_insert_rowid();
        // Bound retained history independently per job; live delivery continues.
        sqlx::query("DELETE FROM job_events WHERE job_id=? AND sequence <= (SELECT sequence FROM job_events WHERE job_id=? ORDER BY sequence DESC LIMIT 1 OFFSET 5000)")
            .bind(&event.job_id).bind(&event.job_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(sequence)
    }

    pub async fn events_after(
        &self,
        job_id: &str,
        after: i64,
    ) -> Result<Vec<crate::models::job::JobEvent>, DbError> {
        Ok(sqlx::query_as("SELECT sequence,job_id,status,progress,message,timestamp FROM job_events WHERE job_id=? AND sequence>? ORDER BY sequence LIMIT 5000")
            .bind(job_id).bind(after).fetch_all(&self.pool).await?)
    }
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
            SET status = ?, result_json = ?, error_message = ?, updated_at = ?,
                progress = CASE WHEN ? = 'succeeded' THEN 100 ELSE progress END
            WHERE id = ?
            "#,
        )
        .bind(status)
        .bind(result_json)
        .bind(error_message)
        .bind(now)
        .bind(status)
        .bind(id)
        .execute(&self.pool)
        .await?;

        if res.rows_affected() == 0 {
            return Err(DbError::NotFound(format!("Job {} not found", id)));
        }
        Ok(())
    }

    /// Atomically claims the oldest queued job and clears the previous attempt's error.
    pub async fn claim_next_queued(&self) -> Result<Option<Job>, DbError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let job = sqlx::query_as::<_, Job>(
            r#"
            UPDATE jobs
            SET status = 'running', error_message = NULL, updated_at = ?
            WHERE id = (
                SELECT id FROM jobs
                WHERE status = 'queued'
                ORDER BY created_at ASC, id ASC
                LIMIT 1
            )
            RETURNING id, user_id, name, status, progress, payload_json,
                      result_json, error_message, retry_count, max_retries, created_at, updated_at
            "#,
        )
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;

        Ok(job)
    }

    /// Cancels a job if it is currently 'queued' or 'running'.
    pub async fn cancel(&self, id: &str) -> Result<bool, DbError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let res = sqlx::query(
            r#"
            UPDATE jobs
            SET status = 'cancelled', updated_at = ?
            WHERE id = ? AND status IN ('queued', 'running')
              AND NOT (status = 'running' AND name IN ('php.install','php.repo.setup','database.server.install','cloudflare.install','cloudflare.change','email.install','email.mailbox','email.send','email.deliver'))
            "#,
        )
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(res.rows_affected() > 0)
    }

    /// Increments retry count. If retries remain, requeues the job; otherwise marks it failed.
    pub async fn increment_retry_or_fail(
        &self,
        id: &str,
        error_message: &str,
    ) -> Result<Job, DbError> {
        let job = match self.find_by_id(id).await? {
            Some(j) => j,
            None => return Err(DbError::NotFound(format!("Job {} not found", id))),
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let new_retry = job.retry_count + 1;
        let new_status = if new_retry < job.max_retries {
            "queued"
        } else {
            "failed"
        };

        sqlx::query(
            r#"
            UPDATE jobs
            SET status = ?, retry_count = ?, error_message = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(new_status)
        .bind(new_retry)
        .bind(error_message)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        let mut updated = job;
        updated.status = new_status.to_string();
        updated.retry_count = new_retry;
        updated.error_message = Some(error_message.to_string());
        updated.updated_at = now;
        Ok(updated)
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
        self.list_paginated(Some(user_id), None, limit, 0).await
    }

    pub async fn list_paginated(
        &self,
        user_id: Option<i64>,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Job>, DbError> {
        let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "SELECT id, user_id, name, status, progress, payload_json, result_json, error_message, retry_count, max_retries, created_at, updated_at FROM jobs WHERE 1=1",
        );

        if let Some(uid) = user_id {
            builder.push(" AND user_id = ");
            builder.push_bind(uid);
        }
        if let Some(st) = status {
            builder.push(" AND status = ");
            builder.push_bind(st);
        }

        builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let query = builder.build_query_as::<Job>();
        let jobs = query.fetch_all(&self.pool).await?;
        Ok(jobs)
    }

    pub async fn count_filtered(
        &self,
        user_id: Option<i64>,
        status: Option<&str>,
    ) -> Result<i64, DbError> {
        let mut builder =
            sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT COUNT(*) FROM jobs WHERE 1=1");

        if let Some(uid) = user_id {
            builder.push(" AND user_id = ");
            builder.push_bind(uid);
        }
        if let Some(st) = status {
            builder.push(" AND status = ");
            builder.push_bind(st);
        }

        let query = builder.build_query_scalar::<i64>();
        let count = query.fetch_one(&self.pool).await?;
        Ok(count)
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
