//! Job progress event types for SSE and background worker communication.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Real-time progress event emitted during background job execution.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct JobProgressEvent {
    /// ID of the job
    pub job_id: String,
    /// Current job status ('queued', 'running', 'succeeded', 'failed', 'cancelled')
    pub status: String,
    /// Completion percentage (0 - 100)
    pub progress: i64,
    /// Optional progress log or error message
    pub message: Option<String>,
    /// Unix timestamp (seconds) when this event was emitted
    pub timestamp: i64,
}
