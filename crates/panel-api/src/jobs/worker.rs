//! Background worker loop for claiming and processing queued jobs.

use crate::jobs::events::JobProgressEvent;
use crate::jobs::registry::JobContext;
use crate::jobs::runner::JobRunner;
use panel_db::error::DbError;
use panel_db::models::Job;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

async fn handle_cancellation(
    runner: &Arc<JobRunner>,
    job_id: &str,
    progress: i64,
) -> Result<(), DbError> {
    runner
        .job_repo
        .update_status(
            job_id,
            "cancelled",
            None,
            Some("Job was cancelled during execution"),
        )
        .await?;

    let _ = runner.event_tx.send(JobProgressEvent {
        job_id: job_id.to_string(),
        status: "cancelled".to_string(),
        progress,
        message: Some("Job was cancelled during execution".to_string()),
        timestamp: current_timestamp(),
    });

    Ok(())
}

async fn handle_success(
    runner: &Arc<JobRunner>,
    job_id: &str,
    result_json: Option<String>,
) -> Result<(), DbError> {
    runner
        .job_repo
        .update_status(job_id, "succeeded", result_json.as_deref(), None)
        .await?;
    runner.job_repo.update_progress(job_id, 100).await?;

    let _ = runner.event_tx.send(JobProgressEvent {
        job_id: job_id.to_string(),
        status: "succeeded".to_string(),
        progress: 100,
        message: Some("Job completed successfully".to_string()),
        timestamp: current_timestamp(),
    });

    Ok(())
}

async fn handle_failure(runner: &Arc<JobRunner>, job: &Job, err: &str) -> Result<(), DbError> {
    warn!("Job {} ('{}') failed: {}", job.id, job.name, err);
    let updated = runner
        .job_repo
        .increment_retry_or_fail(&job.id, err)
        .await?;

    if updated.status == "queued" {
        let backoff = JobRunner::calculate_backoff(job.retry_count);
        let _ = runner.event_tx.send(JobProgressEvent {
            job_id: job.id.clone(),
            status: "queued".to_string(),
            progress: job.progress,
            message: Some(format!(
                "Attempt failed, retrying ({}/{}): {}",
                updated.retry_count, updated.max_retries, err
            )),
            timestamp: current_timestamp(),
        });
        tokio::time::sleep(backoff).await;
    } else {
        let _ = runner.event_tx.send(JobProgressEvent {
            job_id: job.id.clone(),
            status: "failed".to_string(),
            progress: job.progress,
            message: Some(format!("Job failed permanently: {}", err)),
            timestamp: current_timestamp(),
        });
    }

    Ok(())
}

/// Processes a single queued job if available.
/// Returns `Ok(true)` if a job was claimed and executed, or `Ok(false)` if the queue was empty.
pub async fn process_one_job(runner: &Arc<JobRunner>) -> Result<bool, DbError> {
    let job = match runner.job_repo.claim_next_queued().await? {
        Some(j) => j,
        None => return Ok(false),
    };

    let cancel_token = CancellationToken::new();
    runner
        .cancellation_tokens
        .write()
        .await
        .insert(job.id.clone(), cancel_token.clone());

    let _ = runner.event_tx.send(JobProgressEvent {
        job_id: job.id.clone(),
        status: "running".to_string(),
        progress: job.progress,
        message: Some("Job execution started".to_string()),
        timestamp: current_timestamp(),
    });

    let ctx = JobContext {
        job_id: job.id.clone(),
        pool: runner.pool.clone(),
        job_repo: runner.job_repo.clone(),
        event_tx: runner.event_tx.clone(),
        cancel_token: cancel_token.clone(),
    };

    let execution_result = match runner.registry.get(&job.name) {
        Some(handler) => handler.run(&job, &ctx).await,
        None => Err(format!("Unknown job handler: '{}'", job.name)),
    };

    if cancel_token.is_cancelled() {
        let _ = handle_cancellation(runner, &job.id, job.progress).await;
    } else {
        match execution_result {
            Ok(result_json) => {
                handle_success(runner, &job.id, result_json).await?;
            }
            Err(err) => {
                handle_failure(runner, &job, &err).await?;
            }
        }
    }

    runner.cancellation_tokens.write().await.remove(&job.id);
    Ok(true)
}

async fn wait_for_job_event(
    runner: &Arc<JobRunner>,
    shutdown_rx: &mut tokio::sync::watch::Receiver<bool>,
) -> bool {
    tokio::select! {
        _ = shutdown_rx.changed() => {
            *shutdown_rx.borrow()
        }
        _ = runner.notify.notified() => false,
        _ = tokio::time::sleep(Duration::from_millis(500)) => false,
    }
}

async fn worker_iteration(
    runner: &Arc<JobRunner>,
    shutdown_rx: &mut tokio::sync::watch::Receiver<bool>,
) -> bool {
    match process_one_job(runner).await {
        Ok(true) => false,
        Ok(false) => wait_for_job_event(runner, shutdown_rx).await,
        Err(e) => {
            error!("Error in job worker execution: {}", e);
            tokio::time::sleep(Duration::from_millis(500)).await;
            false
        }
    }
}

/// Runs the background worker continuously until receiving a shutdown signal.
pub async fn run_worker(
    runner: Arc<JobRunner>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) {
    info!("Background job worker started");
    while !*shutdown_rx.borrow() {
        if worker_iteration(&runner, &mut shutdown_rx).await {
            break;
        }
    }
    info!("Background job worker stopped");
}
