//! Worker session management, pooling, and idle timeout lifecycle.

use super::handler::FsWorkerHandler;
use super::protocol::{FsWorkerRequest, FsWorkerResponse};
use crate::error::FsError;
use crate::fs::ConfinedFs;
use crate::policy::SymlinkPolicy;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, info, warn};

/// Tenant identification for routing file worker processes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantWorkerKey {
    pub uid: u32,
    pub gid: u32,
    pub home: PathBuf,
}

impl TenantWorkerKey {
    pub fn new(uid: u32, gid: u32, home: impl Into<PathBuf>) -> Self {
        Self {
            uid,
            gid,
            home: home.into(),
        }
    }
}

/// Configuration settings for the worker pool.
#[derive(Debug, Clone)]
pub struct FsWorkerPoolConfig {
    /// Inactivity duration before an idle worker process is terminated.
    pub idle_timeout: Duration,
    /// Maximum concurrent worker instances across all tenants.
    pub max_workers: usize,
    /// Symlink containment policy to enforce within the worker.
    pub symlink_policy: SymlinkPolicy,
}

impl Default for FsWorkerPoolConfig {
    fn default() -> Self {
        Self {
            idle_timeout: Duration::from_secs(60),
            max_workers: 64,
            symlink_policy: SymlinkPolicy::DenyAll,
        }
    }
}

type WorkerSender = mpsc::Sender<(FsWorkerRequest, oneshot::Sender<FsWorkerResponse>)>;

struct ActiveWorker {
    tx: WorkerSender,
    last_used: Instant,
}

impl ActiveWorker {
    fn is_alive(&self) -> bool {
        !self.tx.is_closed()
    }
}

/// Pool managing pooled `fs-worker` sessions per tenant with automatic idle reaping.
pub struct FsWorkerPool {
    config: FsWorkerPoolConfig,
    workers: Mutex<HashMap<TenantWorkerKey, ActiveWorker>>,
}

impl FsWorkerPool {
    /// Creates a new worker pool with the specified configuration.
    pub fn new(config: FsWorkerPoolConfig) -> Self {
        Self {
            config,
            workers: Mutex::new(HashMap::new()),
        }
    }

    /// Returns the configured idle timeout duration.
    pub fn idle_timeout(&self) -> Duration {
        self.config.idle_timeout
    }

    /// Dispatches a filesystem request to a tenant's pooled worker.
    ///
    /// Spawns a new worker if none exists or if the previous worker terminated.
    pub async fn execute(
        &self,
        tenant: &TenantWorkerKey,
        request: FsWorkerRequest,
    ) -> Result<FsWorkerResponse, FsError> {
        let (tx, mut attempts) = (self.get_or_spawn_worker(tenant)?, 0);

        loop {
            let (resp_tx, resp_rx) = oneshot::channel();
            match tx.send((request.clone(), resp_tx)).await {
                Ok(_) => {
                    match resp_rx.await {
                        Ok(response) => {
                            // Update last used timestamp
                            if let Ok(mut lock) = self.workers.lock()
                                && let Some(worker) = lock.get_mut(tenant)
                            {
                                worker.last_used = Instant::now();
                            }
                            return Ok(response);
                        }
                        Err(_) => {
                            warn!(
                                "Worker for UID {} dropped response channel unexpectedly",
                                tenant.uid
                            );
                            self.evict_worker(tenant);
                            attempts += 1;
                            if attempts >= 2 {
                                return Err(FsError::Io {
                                    path: tenant.home.display().to_string(),
                                    detail: "Worker terminated during response transfer".into(),
                                });
                            }
                        }
                    }
                }
                Err(_) => {
                    warn!("Worker for UID {} is unreachable (closed)", tenant.uid);
                    self.evict_worker(tenant);
                    attempts += 1;
                    if attempts >= 2 {
                        return Err(FsError::Io {
                            path: tenant.home.display().to_string(),
                            detail: "Worker channel closed and could not be restarted".into(),
                        });
                    }
                }
            }
        }
    }

    /// Returns an existing healthy worker or spawns a new one.
    fn get_or_spawn_worker(&self, tenant: &TenantWorkerKey) -> Result<WorkerSender, FsError> {
        let mut lock = self.workers.lock().map_err(|_| FsError::Io {
            path: tenant.home.display().to_string(),
            detail: "Worker pool mutex poisoned".into(),
        })?;

        if let Some(worker) = lock.get(tenant)
            && worker.is_alive()
        {
            return Ok(worker.tx.clone());
        }

        // Check max workers limit
        if lock.len() >= self.config.max_workers {
            // Force reap idle workers first
            let mut to_remove = Vec::new();
            for (k, w) in lock.iter() {
                if w.last_used.elapsed() > self.config.idle_timeout || !w.is_alive() {
                    to_remove.push(k.clone());
                }
            }
            for k in to_remove {
                lock.remove(&k);
            }

            if lock.len() >= self.config.max_workers {
                return Err(FsError::Io {
                    path: tenant.home.display().to_string(),
                    detail: format!(
                        "Maximum worker pool capacity reached ({} workers)",
                        self.config.max_workers
                    ),
                });
            }
        }

        let tx = Self::spawn_worker_instance(&tenant.home, self.config.symlink_policy)?;
        lock.insert(
            tenant.clone(),
            ActiveWorker {
                tx: tx.clone(),
                last_used: Instant::now(),
            },
        );

        debug!(
            "Spawned new fs-worker for UID {} (jail: {})",
            tenant.uid,
            tenant.home.display()
        );

        Ok(tx)
    }

    /// Spawns an isolated worker task operating strictly on the confined root.
    fn spawn_worker_instance(
        jail_path: &Path,
        policy: SymlinkPolicy,
    ) -> Result<WorkerSender, FsError> {
        let confined_fs = ConfinedFs::new(jail_path, policy)?;
        let handler = Arc::new(FsWorkerHandler::new(confined_fs));
        let (tx, mut rx) =
            mpsc::channel::<(FsWorkerRequest, oneshot::Sender<FsWorkerResponse>)>(32);

        tokio::spawn(async move {
            while let Some((req, resp_tx)) = rx.recv().await {
                let is_shutdown = matches!(req, FsWorkerRequest::Shutdown);
                let resp = handler.handle(req);
                let _ = resp_tx.send(resp);
                if is_shutdown {
                    break;
                }
            }
        });

        Ok(tx)
    }

    /// Evicts a terminated or broken worker from the pool.
    pub fn evict_worker(&self, tenant: &TenantWorkerKey) {
        if let Ok(mut lock) = self.workers.lock() {
            lock.remove(tenant);
        }
    }

    /// Reaps all workers that have exceeded the idle timeout.
    ///
    /// Returns the number of idle workers shut down.
    pub fn cleanup_idle(&self) -> usize {
        let mut to_reap = Vec::new();
        let timeout = self.config.idle_timeout;

        if let Ok(mut lock) = self.workers.lock() {
            for (key, worker) in lock.iter() {
                if worker.last_used.elapsed() >= timeout || !worker.is_alive() {
                    to_reap.push((key.clone(), worker.tx.clone()));
                }
            }

            for (key, tx) in to_reap.iter() {
                lock.remove(key);
                let (resp_tx, _) = oneshot::channel();
                let _ = tx.try_send((FsWorkerRequest::Shutdown, resp_tx));
            }
        }

        let reaped_count = to_reap.len();
        if reaped_count > 0 {
            info!("Reaped {reaped_count} idle fs-workers from pool");
        }
        reaped_count
    }

    /// Returns the number of currently active workers in the pool.
    pub fn active_worker_count(&self) -> usize {
        self.workers.lock().map(|l| l.len()).unwrap_or(0)
    }

    /// Gracefully shuts down all active workers in the pool.
    pub async fn shutdown_all(&self) {
        let mut workers_to_close = Vec::new();
        if let Ok(mut lock) = self.workers.lock() {
            for (_, worker) in lock.drain() {
                workers_to_close.push(worker.tx);
            }
        }

        for tx in workers_to_close {
            let (resp_tx, _) = oneshot::channel();
            let _ = tx.send((FsWorkerRequest::Shutdown, resp_tx)).await;
        }
    }
}
