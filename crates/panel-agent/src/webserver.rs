//! Nginx webserver manager for host configuration and lifecycle operations.

use crate::AgentError;
use crate::system::SystemRunner;
use panel_proto::{NginxBaselineAppliedResponse, NginxBaselineApplyRequest};
use panel_webserver::baseline::NginxBaselineConfig;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

pub struct NginxManager {
    runner: Arc<dyn SystemRunner>,
}

impl NginxManager {
    pub fn new(runner: Arc<dyn SystemRunner>) -> Self {
        Self { runner }
    }

    fn build_config(&self, req: &NginxBaselineApplyRequest) -> NginxBaselineConfig {
        let base_dir = req
            .base_dir
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/etc/panel/nginx"));
        let log_dir = req
            .log_dir
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/var/log/panel"));
        let acme_dir = req
            .acme_dir
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/var/lib/panel/acme"));

        NginxBaselineConfig {
            base_dir,
            log_dir,
            acme_dir,
            panel_api_port: req.panel_api_port.unwrap_or(3000),
            panel_listen_port: req.panel_listen_port.unwrap_or(8443),
            worker_processes: "auto".to_string(),
            worker_connections: 4096,
        }
    }

    fn test_nginx_syntax(&self, config: &NginxBaselineConfig) -> bool {
        let conf_path_str = config.nginx_conf_path().to_string_lossy().to_string();
        match self
            .runner
            .run_command("nginx", &["-t", "-c", &conf_path_str])
        {
            Ok(_) => {
                info!(
                    "Nginx baseline configuration test passed (-t -c {})",
                    conf_path_str
                );
                true
            }
            Err(e) => {
                warn!("Nginx test failed or binary missing: {e}");
                false
            }
        }
    }

    fn reload_nginx(&self) -> bool {
        match self.runner.run_command("systemctl", &["reload", "nginx"]) {
            Ok(_) => {
                info!("Nginx reloaded successfully via systemctl");
                true
            }
            Err(e) => {
                warn!("systemctl reload nginx failed or not supported in test environment: {e}");
                false
            }
        }
    }

    /// Applies the baseline Nginx configuration and directory structure.
    pub fn apply_baseline(
        &self,
        req: NginxBaselineApplyRequest,
    ) -> Result<NginxBaselineAppliedResponse, AgentError> {
        let config = self.build_config(&req);
        info!("Installing Nginx baseline to {}", config.base_dir.display());
        config.install_baseline().map_err(|e| {
            AgentError::InvalidInput(format!("Failed to install Nginx baseline: {e}"))
        })?;

        let test_passed = if req.test_config {
            self.test_nginx_syntax(&config)
        } else {
            true
        };

        let reloaded = if req.reload && test_passed {
            self.reload_nginx()
        } else {
            false
        };

        Ok(NginxBaselineAppliedResponse {
            base_dir: config.base_dir.to_string_lossy().to_string(),
            nginx_conf_path: config.nginx_conf_path().to_string_lossy().to_string(),
            conf_d_dir: config.conf_d_dir().to_string_lossy().to_string(),
            snippets_dir: config.snippets_dir().to_string_lossy().to_string(),
            sites_available_dir: config.sites_available_dir().to_string_lossy().to_string(),
            sites_enabled_dir: config.sites_enabled_dir().to_string_lossy().to_string(),
            log_dir: config.log_dir.to_string_lossy().to_string(),
            acme_dir: config.acme_dir.to_string_lossy().to_string(),
            test_passed,
            reloaded,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::system::MockSystemRunner;
    use tempfile::TempDir;

    #[test]
    fn test_apply_baseline_with_mock_runner() {
        let temp = TempDir::new().unwrap();
        let nginx_dir = temp.path().join("nginx");
        let log_dir = temp.path().join("logs");
        let acme_dir = temp.path().join("acme");

        let runner = Arc::new(MockSystemRunner::new());
        // Configure mock responses for nginx -t and systemctl reload
        runner.set_response("nginx", 0, b"syntax is ok", b"");
        runner.set_response("systemctl", 0, b"", b"");

        let manager = NginxManager::new(runner.clone());

        let req = NginxBaselineApplyRequest {
            base_dir: Some(nginx_dir.to_string_lossy().to_string()),
            log_dir: Some(log_dir.to_string_lossy().to_string()),
            acme_dir: Some(acme_dir.to_string_lossy().to_string()),
            panel_api_port: Some(3000),
            panel_listen_port: Some(8443),
            test_config: true,
            reload: true,
        };

        let res = manager.apply_baseline(req).expect("apply baseline");
        assert!(res.test_passed);
        assert!(res.reloaded);
        assert!(nginx_dir.join("nginx.conf").exists());
        assert!(nginx_dir.join("conf.d/00-default.conf").exists());
        assert!(nginx_dir.join("snippets/deny-sensitive.conf").exists());

        // Verify commands recorded in runner
        let actions = runner.recorded_actions();
        assert_eq!(actions.len(), 2);
    }
}
