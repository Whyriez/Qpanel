//! Core job runner managing queues, cancellation, events, and idempotency.

use crate::jobs::events::JobProgressEvent;
use crate::jobs::registry::JobRegistry;
use panel_db::error::DbError;
use panel_db::models::{CreateJob, Job};
use panel_db::pool::DbPool;
use panel_db::repositories::SqliteJobRepository;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, broadcast};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Manages enqueuing, cancellation, and event distribution for background jobs.
pub struct JobRunner {
    pub pool: DbPool,
    pub job_repo: SqliteJobRepository,
    pub event_tx: broadcast::Sender<JobProgressEvent>,
    pub notify: Arc<tokio::sync::Notify>,
    pub cancellation_tokens: Arc<RwLock<HashMap<String, CancellationToken>>>,
    pub registry: Arc<JobRegistry>,
}

impl JobRunner {
    /// Creates a new `JobRunner` with default registered handlers.
    pub fn new(pool: DbPool) -> Self {
        Self::with_registry(pool, JobRegistry::new())
    }

    /// Creates a new `JobRunner` with a custom registry.
    pub fn with_registry(pool: DbPool, registry: JobRegistry) -> Self {
        Self::with_pools(pool.clone(), pool, registry)
    }

    /// Core identity/config access stays separate from high-frequency job writes.
    pub fn with_pools(pool: DbPool, operations: DbPool, registry: JobRegistry) -> Self {
        let (event_tx, _) = broadcast::channel(1024);
        Self {
            job_repo: SqliteJobRepository::new(operations),
            pool,
            event_tx,
            notify: Arc::new(tokio::sync::Notify::new()),
            cancellation_tokens: Arc::new(RwLock::new(HashMap::new())),
            registry: Arc::new(registry),
        }
    }

    /// Enqueues a job idempotently. If `id` is provided and exists, returns the existing job.
    /// Returns `(Job, is_idempotent_duplicate)`.
    pub async fn enqueue(
        &self,
        user_id: Option<i64>,
        id: Option<String>,
        name: String,
        payload_json: String,
        max_retries: Option<i64>,
    ) -> Result<(Job, bool), DbError> {
        let job_id = id.unwrap_or_else(|| Uuid::new_v4().to_string());

        // Check idempotency if client provided specific ID
        if let Some(existing) = self.job_repo.find_by_id(&job_id).await? {
            return Ok((existing, true));
        }

        let input = CreateJob {
            id: job_id,
            user_id,
            name,
            payload_json,
            max_retries: max_retries.unwrap_or(3),
        };

        let created = self.job_repo.create(&input).await?;

        // Broadcast initial queued event
        let _ = self.event_tx.send(JobProgressEvent {
            job_id: created.id.clone(),
            status: created.status.clone(),
            progress: created.progress,
            message: Some("Job queued".to_string()),
            timestamp: created.created_at,
        });

        // Notify the worker to wake up and process immediately
        self.notify.notify_one();

        Ok((created, false))
    }

    /// Cancels a job if it is in queued or running status.
    pub async fn cancel(&self, id: &str) -> Result<Option<Job>, DbError> {
        let job = match self.job_repo.find_by_id(id).await? {
            Some(j) => j,
            None => return Ok(None),
        };

        if job.status == "succeeded" || job.status == "failed" || job.status == "cancelled" {
            return Ok(Some(job));
        }

        let cancelled = self.job_repo.cancel(id).await?;
        if cancelled {
            // Signal active cancellation token if running
            let tokens = self.cancellation_tokens.read().await;
            if let Some(token) = tokens.get(id) {
                token.cancel();
            }

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            let _ = self.event_tx.send(JobProgressEvent {
                job_id: id.to_string(),
                status: "cancelled".to_string(),
                progress: job.progress,
                message: Some("Job cancelled".to_string()),
                timestamp: now,
            });
        }

        self.job_repo.find_by_id(id).await
    }

    /// Subscribes to real-time job progress events.
    pub fn subscribe(&self) -> broadcast::Receiver<JobProgressEvent> {
        self.event_tx.subscribe()
    }

    /// Recovers interrupted jobs left in 'running' state after server restart.
    pub async fn recover_interrupted(&self) -> Result<u64, DbError> {
        self.job_repo.recover_interrupted_jobs().await
    }

    /// Calculates exponential backoff duration based on retry count.
    pub fn calculate_backoff(retry_count: i64) -> Duration {
        let exponent = retry_count.clamp(0, 8) as u32;
        let ms = 100u64.saturating_mul(1u64 << exponent);
        Duration::from_millis(ms)
    }
}
