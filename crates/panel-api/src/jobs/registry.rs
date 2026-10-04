//! Handler registry and execution context for background jobs.

use crate::jobs::events::JobProgressEvent;
use panel_db::error::DbError;
use panel_db::models::Job;
use panel_db::pool::DbPool;
use panel_db::repositories::SqliteJobRepository;
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::warn;

pub type JobFuture<'a> = Pin<Box<dyn Future<Output = Result<Option<String>, String>> + Send + 'a>>;

/// Execution context passed to each job handler.
pub struct JobContext {
    pub job_id: String,
    pub pool: DbPool,
    pub job_repo: SqliteJobRepository,
    pub event_tx: broadcast::Sender<JobProgressEvent>,
    pub cancel_token: CancellationToken,
}

impl JobContext {
    /// Reports incremental progress (0 to 100) and broadcasts it to SSE subscribers.
    pub async fn report_progress(
        &self,
        progress: i64,
        message: Option<&str>,
    ) -> Result<(), DbError> {
        let bounded = progress.clamp(0, 100);
        self.job_repo.update_progress(&self.job_id, bounded).await?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let _ = self.event_tx.send(JobProgressEvent {
            job_id: self.job_id.clone(),
            status: "running".to_string(),
            progress: bounded,
            message: message.map(ToString::to_string),
            timestamp: now,
        });

        Ok(())
    }

    /// Checks if a cancellation request has been signaled for this job.
    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }
}

/// Interface for background job handlers.
pub trait JobHandler: Send + Sync {
    fn run<'a>(&'a self, job: &'a Job, ctx: &'a JobContext) -> JobFuture<'a>;
}

/// Test sleep handler that emits progress milestones.
pub struct SleepHandler;

impl JobHandler for SleepHandler {
    fn run<'a>(&'a self, job: &'a Job, ctx: &'a JobContext) -> JobFuture<'a> {
        Box::pin(async move {
            let payload: Value = serde_json::from_str(&job.payload_json).unwrap_or(Value::Null);
            let duration_ms = payload
                .get("duration_ms")
                .and_then(Value::as_u64)
                .unwrap_or(100);
            let steps = payload
                .get("steps")
                .and_then(Value::as_u64)
                .unwrap_or(4)
                .max(1);

            let step_duration = Duration::from_millis(duration_ms / steps);

            for step in 1..=steps {
                if ctx.is_cancelled() {
                    return Err("Job was cancelled during execution".to_string());
                }
                tokio::time::sleep(step_duration).await;
                if ctx.is_cancelled() {
                    return Err("Job was cancelled during execution".to_string());
                }

                let pct = (step * 100 / steps) as i64;
                let msg = format!("Step {}/{} finished", step, steps);
                if let Err(e) = ctx.report_progress(pct, Some(&msg)).await {
                    warn!("Failed to report progress: {}", e);
                }
            }

            Ok(Some(
                serde_json::to_string(&serde_json::json!({
                    "slept_ms": duration_ms,
                    "steps": steps,
                }))
                .unwrap_or_default(),
            ))
        })
    }
}

/// Test failure handler to verify retry and backoff logic.
pub struct FailHandler;

impl JobHandler for FailHandler {
    fn run<'a>(&'a self, job: &'a Job, _ctx: &'a JobContext) -> JobFuture<'a> {
        Box::pin(async move {
            let payload: Value = serde_json::from_str(&job.payload_json).unwrap_or(Value::Null);
            let error_msg = payload
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("Simulated job failure");

            Err(error_msg.to_string())
        })
    }
}

/// Echo handler returning the payload as result.
pub struct EchoHandler;

impl JobHandler for EchoHandler {
    fn run<'a>(&'a self, job: &'a Job, _ctx: &'a JobContext) -> JobFuture<'a> {
        Box::pin(async move { Ok(Some(job.payload_json.clone())) })
    }
}

/// Quota report handler querying tenant quota usage via agent client.
pub struct QuotaReportHandler {
    agent_client: Arc<crate::agent_client::AgentClient>,
}

impl QuotaReportHandler {
    pub fn new(agent_client: Arc<crate::agent_client::AgentClient>) -> Self {
        Self { agent_client }
    }
}

impl JobHandler for QuotaReportHandler {
    fn run<'a>(&'a self, job: &'a Job, ctx: &'a JobContext) -> JobFuture<'a> {
        Box::pin(async move {
            let payload: Value = serde_json::from_str(&job.payload_json).unwrap_or(Value::Null);
            let uid = payload.get("uid").and_then(Value::as_u64).unwrap_or(10001) as u32;
            let username = payload
                .get("username")
                .and_then(Value::as_str)
                .unwrap_or("p_tenant")
                .to_string();

            let _ = ctx
                .report_progress(10, Some("Querying quota metrics from agent..."))
                .await;

            let quota_res = self
                .agent_client
                .get_quota(panel_proto::QuotaGetRequest {
                    uid,
                    username: username.clone(),
                    mount_point: None,
                })
                .await
                .map_err(|e| format!("Agent quota query failed: {e}"))?;

            let _ = ctx
                .report_progress(70, Some("Aggregating quota usage data..."))
                .await;

            let result = serde_json::json!({
                "uid": quota_res.uid,
                "username": quota_res.username,
                "bytes_used": quota_res.bytes_used,
                "bytes_limit": quota_res.bytes_limit,
                "inodes_used": quota_res.inodes_used,
                "inodes_limit": quota_res.inodes_limit,
                "backend": quota_res.backend,
            });

            let _ = ctx
                .report_progress(100, Some("Quota report finished successfully"))
                .await;

            Ok(Some(result.to_string()))
        })
    }
}

/// Quota scan handler executing recursive directory scan via agent client.
pub struct QuotaScanHandler {
    agent_client: Arc<crate::agent_client::AgentClient>,
}

impl QuotaScanHandler {
    pub fn new(agent_client: Arc<crate::agent_client::AgentClient>) -> Self {
        Self { agent_client }
    }
}

impl JobHandler for QuotaScanHandler {
    fn run<'a>(&'a self, job: &'a Job, ctx: &'a JobContext) -> JobFuture<'a> {
        Box::pin(async move {
            let payload: Value = serde_json::from_str(&job.payload_json).unwrap_or(Value::Null);
            let uid = payload.get("uid").and_then(Value::as_u64).unwrap_or(10001) as u32;
            let username = payload
                .get("username")
                .and_then(Value::as_str)
                .unwrap_or("p_tenant")
                .to_string();

            let _ = ctx
                .report_progress(15, Some("Scanning tenant filesystem tree..."))
                .await;

            let scan_res = self
                .agent_client
                .scan_quota(panel_proto::QuotaScanRequest {
                    uid,
                    username: username.clone(),
                    path: None,
                })
                .await
                .map_err(|e| format!("Agent quota scan failed: {e}"))?;

            let _ = ctx
                .report_progress(80, Some("Finalizing directory scan metrics..."))
                .await;

            let result = serde_json::json!({
                "uid": scan_res.uid,
                "username": scan_res.username,
                "bytes_used": scan_res.bytes_used,
                "inodes_used": scan_res.inodes_used,
                "scan_duration_ms": scan_res.scan_duration_ms,
            });

            let _ = ctx
                .report_progress(100, Some("Filesystem scan complete"))
                .await;

            Ok(Some(result.to_string()))
        })
    }
}

/// PHP Sury/PPA repository setup handler.
pub struct PhpRepoSetupHandler {
    agent_client: Arc<crate::agent_client::AgentClient>,
}

impl PhpRepoSetupHandler {
    pub fn new(agent_client: Arc<crate::agent_client::AgentClient>) -> Self {
        Self { agent_client }
    }
}

impl JobHandler for PhpRepoSetupHandler {
    fn run<'a>(&'a self, _job: &'a Job, ctx: &'a JobContext) -> JobFuture<'a> {
        Box::pin(async move {
            let _ = ctx
                .report_progress(10, Some("Menyiapkan repositori Sury PHP PPA..."))
                .await;

            if ctx.is_cancelled() {
                return Err("Job was cancelled before execution".to_string());
            }

            let res = self
                .agent_client
                .setup_php_repo()
                .await
                .map_err(|e| format!("Setup repositori PHP gagal: {e}"))?;

            let _ = ctx
                .report_progress(100, Some("Repositori PHP berhasil dikonfigurasi"))
                .await;

            let result = serde_json::json!({
                "success": res.success,
                "message": res.message,
            });

            Ok(Some(result.to_string()))
        })
    }
}

/// PHP runtime installation handler.
pub struct PhpInstallHandler {
    agent_client: Arc<crate::agent_client::AgentClient>,
}

impl PhpInstallHandler {
    pub fn new(agent_client: Arc<crate::agent_client::AgentClient>) -> Self {
        Self { agent_client }
    }
}

impl JobHandler for PhpInstallHandler {
    fn run<'a>(&'a self, job: &'a Job, ctx: &'a JobContext) -> JobFuture<'a> {
        Box::pin(async move {
            let payload: Value = serde_json::from_str(&job.payload_json).unwrap_or(Value::Null);
            let version = payload
                .get("version")
                .and_then(Value::as_str)
                .ok_or_else(|| "Field 'version' wajib disertakan dalam payload".to_string())?;

            panel_runtime::php::PhpVersion::parse(version)
                .map_err(|e| format!("Versi PHP tidak valid: {e}"))?;

            let _ = ctx
                .report_progress(15, Some(&format!("Memulai instalasi PHP {}...", version)))
                .await;

            if ctx.is_cancelled() {
                return Err("Job was cancelled before installation".to_string());
            }

            let _ = ctx
                .report_progress(
                    35,
                    Some(&format!(
                        "Memasang paket PHP {} dan melakukan masking service FPM...",
                        version
                    )),
                )
                .await;

            let res = self
                .agent_client
                .install_php_runtime(version)
                .await
                .map_err(|e| format!("Instalasi PHP {version} gagal: {e}"))?;

            let _ = ctx
                .report_progress(
                    100,
                    Some(&format!(
                        "PHP {} berhasil dipasang dan service FPM dimask",
                        version
                    )),
                )
                .await;

            let result = serde_json::json!({
                "version": res.version,
                "success": res.success,
                "masked": res.masked,
                "message": res.message,
            });

            Ok(Some(result.to_string()))
        })
    }
}

/// PHP runtime removal handler.
pub struct PhpRemoveHandler {
    agent_client: Arc<crate::agent_client::AgentClient>,
}

impl PhpRemoveHandler {
    pub fn new(agent_client: Arc<crate::agent_client::AgentClient>) -> Self {
        Self { agent_client }
    }
}

impl JobHandler for PhpRemoveHandler {
    fn run<'a>(&'a self, job: &'a Job, ctx: &'a JobContext) -> JobFuture<'a> {
        Box::pin(async move {
            let payload: Value = serde_json::from_str(&job.payload_json).unwrap_or(Value::Null);
            let version = payload
                .get("version")
                .and_then(Value::as_str)
                .ok_or_else(|| "Field 'version' wajib disertakan dalam payload".to_string())?;

            panel_runtime::php::PhpVersion::parse(version)
                .map_err(|e| format!("Versi PHP tidak valid: {e}"))?;

            let _ = ctx
                .report_progress(
                    10,
                    Some(&format!(
                        "Memeriksa penggunaan PHP {} pada situs aktif...",
                        version
                    )),
                )
                .await;

            // Check if any sites are using this PHP version
            let site_repo = panel_db::repositories::SqliteSiteRepository::new(ctx.pool.clone());
            let count = site_repo
                .count_by_runtime_and_version("php", version)
                .await
                .map_err(|e| format!("Gagal memeriksa penggunaan PHP di database: {e}"))?;

            if count > 0 {
                return Err(format!(
                    "Tidak dapat menghapus PHP {}: sedang digunakan oleh {} situs.",
                    version, count
                ));
            }

            if ctx.is_cancelled() {
                return Err("Job was cancelled before removal".to_string());
            }

            let _ = ctx
                .report_progress(40, Some(&format!("Menghapus paket PHP {}...", version)))
                .await;

            let res = self
                .agent_client
                .remove_php_runtime(version)
                .await
                .map_err(|e| format!("Penghapusan PHP {version} gagal: {e}"))?;

            let _ = ctx
                .report_progress(100, Some(&format!("PHP {} berhasil dihapus", version)))
                .await;

            let result = serde_json::json!({
                "version": res.version,
                "success": res.success,
                "masked": res.masked,
                "message": res.message,
            });

            Ok(Some(result.to_string()))
        })
    }
}

pub struct PhpExtensionInstallHandler {
    agent_client: Arc<crate::agent_client::AgentClient>,
}

impl PhpExtensionInstallHandler {
    pub fn new(agent_client: Arc<crate::agent_client::AgentClient>) -> Self {
        Self { agent_client }
    }
}

impl JobHandler for PhpExtensionInstallHandler {
    fn run<'a>(&'a self, job: &'a Job, ctx: &'a JobContext) -> JobFuture<'a> {
        Box::pin(async move {
            let payload: Value = serde_json::from_str(&job.payload_json).unwrap_or(Value::Null);
            let version = payload
                .get("version")
                .and_then(Value::as_str)
                .ok_or_else(|| "Field 'version' wajib disertakan dalam payload".to_string())?;
            let extension = payload
                .get("extension")
                .and_then(Value::as_str)
                .ok_or_else(|| "Field 'extension' wajib disertakan dalam payload".to_string())?;

            panel_runtime::php::PhpVersion::parse(version)
                .map_err(|e| format!("Versi PHP tidak valid: {e}"))?;
            panel_runtime::php::find_curated_extension(extension)
                .map_err(|e| format!("Ekstensi tidak valid: {e}"))?;

            let _ = ctx
                .report_progress(
                    20,
                    Some(&format!(
                        "Memulai pemasangan ekstensi {} untuk PHP {}...",
                        extension, version
                    )),
                )
                .await;

            let res = self
                .agent_client
                .install_php_extension(version, extension)
                .await
                .map_err(|e| format!("Pemasangan ekstensi {extension} gagal: {e}"))?;

            let _ = ctx
                .report_progress(
                    100,
                    Some(&format!("Ekstensi {} berhasil dipasang", extension)),
                )
                .await;

            let result = serde_json::json!({
                "version": res.version,
                "extension": res.extension,
                "success": res.success,
                "message": res.message,
            });

            Ok(Some(result.to_string()))
        })
    }
}

pub struct PhpExtensionRemoveHandler {
    agent_client: Arc<crate::agent_client::AgentClient>,
}

impl PhpExtensionRemoveHandler {
    pub fn new(agent_client: Arc<crate::agent_client::AgentClient>) -> Self {
        Self { agent_client }
    }
}

impl JobHandler for PhpExtensionRemoveHandler {
    fn run<'a>(&'a self, job: &'a Job, ctx: &'a JobContext) -> JobFuture<'a> {
        Box::pin(async move {
            let payload: Value = serde_json::from_str(&job.payload_json).unwrap_or(Value::Null);
            let version = payload
                .get("version")
                .and_then(Value::as_str)
                .ok_or_else(|| "Field 'version' wajib disertakan dalam payload".to_string())?;
            let extension = payload
                .get("extension")
                .and_then(Value::as_str)
                .ok_or_else(|| "Field 'extension' wajib disertakan dalam payload".to_string())?;

            panel_runtime::php::PhpVersion::parse(version)
                .map_err(|e| format!("Versi PHP tidak valid: {e}"))?;
            let def = panel_runtime::php::find_curated_extension(extension)
                .map_err(|e| format!("Ekstensi tidak valid: {e}"))?;

            if def.is_base {
                return Err(format!("Ekstensi dasar '{extension}' tidak dapat dihapus"));
            }

            let _ = ctx
                .report_progress(
                    20,
                    Some(&format!(
                        "Menghapus ekstensi {} dari PHP {}...",
                        extension, version
                    )),
                )
                .await;

            let res = self
                .agent_client
                .remove_php_extension(version, extension)
                .await
                .map_err(|e| format!("Penghapusan ekstensi {extension} gagal: {e}"))?;

            let _ = ctx
                .report_progress(
                    100,
                    Some(&format!("Ekstensi {} berhasil dihapus", extension)),
                )
                .await;

            let result = serde_json::json!({
                "version": res.version,
                "extension": res.extension,
                "success": res.success,
                "message": res.message,
            });

            Ok(Some(result.to_string()))
        })
    }
}

/// Central registry mapping job names to their respective handlers.
pub struct JobRegistry {
    handlers: HashMap<String, Arc<dyn JobHandler>>,
}

impl Default for JobRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl JobRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            handlers: HashMap::new(),
        };
        reg.register("test.sleep", SleepHandler);
        reg.register("test.fail", FailHandler);
        reg.register("system.echo", EchoHandler);
        reg
    }

    pub fn new_with_agent(agent_client: Arc<crate::agent_client::AgentClient>) -> Self {
        let mut reg = Self::new();
        reg.register(
            "quota.report",
            QuotaReportHandler::new(Arc::clone(&agent_client)),
        );
        reg.register(
            "quota.scan",
            QuotaScanHandler::new(Arc::clone(&agent_client)),
        );
        reg.register(
            "php.repo.setup",
            PhpRepoSetupHandler::new(Arc::clone(&agent_client)),
        );
        reg.register(
            "php.install",
            PhpInstallHandler::new(Arc::clone(&agent_client)),
        );
        reg.register(
            "php.remove",
            PhpRemoveHandler::new(Arc::clone(&agent_client)),
        );
        reg.register(
            "php.extension.install",
            PhpExtensionInstallHandler::new(Arc::clone(&agent_client)),
        );
        reg.register(
            "php.extension.remove",
            PhpExtensionRemoveHandler::new(agent_client),
        );
        reg
    }

    pub fn register<H: JobHandler + 'static>(&mut self, name: &str, handler: H) {
        self.handlers.insert(name.to_string(), Arc::new(handler));
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn JobHandler>> {
        self.handlers.get(name).cloned()
    }
}
