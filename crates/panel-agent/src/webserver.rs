//! Nginx webserver manager for host configuration and lifecycle operations.

use crate::AgentError;
use crate::system::SystemRunner;
use panel_proto::{
    AcmeChallengeCleanedResponse, AcmeChallengeCleanupRequest, AcmeChallengeDeployRequest,
    AcmeChallengeDeployedResponse, NginxBaselineAppliedResponse, NginxBaselineApplyRequest,
    SiteAppliedResponse, SiteApplyRequest, SiteProvisionRequest, SiteProvisionResponse,
    SiteRemoveRequest, SiteRemovedResponse, SslCertDeployRequest, SslCertDeployedResponse,
};
use panel_webserver::baseline::NginxBaselineConfig;
use panel_webserver::pipeline::{NginxApplyPipeline, ReloadDebouncer};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

pub struct NginxManager {
    runner: Arc<dyn SystemRunner>,
    debouncer: ReloadDebouncer,
}

impl NginxManager {
    pub fn new(runner: Arc<dyn SystemRunner>) -> Self {
        Self {
            runner,
            debouncer: ReloadDebouncer::default(),
        }
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
        if !self.debouncer.should_reload() {
            info!("Nginx reload debounced (coalesced within 500ms)");
            return true;
        }
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

    /// Applies a virtual host configuration atomically with syntax verification and automatic rollback.
    pub fn apply_site(&self, req: SiteApplyRequest) -> Result<SiteAppliedResponse, AgentError> {
        let base_dir = req
            .nginx_base_dir
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/etc/panel/nginx"));
        let pipeline = NginxApplyPipeline::new(&base_dir);
        let nginx_conf_path = base_dir.join("nginx.conf");
        let runner = Arc::clone(&self.runner);

        let conf_path_str = nginx_conf_path.to_string_lossy().to_string();
        let syntax_check = move || -> Result<(), String> {
            match runner.run_command("nginx", &["-t", "-c", &conf_path_str]) {
                Ok(out) => {
                    if out.status.success() {
                        Ok(())
                    } else {
                        Err(String::from_utf8_lossy(&out.stderr).to_string())
                    }
                }
                Err(e) => Err(e.to_string()),
            }
        };

        let result = if req.config_content.is_empty() {
            if req.enable {
                if req.test_config {
                    pipeline.enable_site(&req.site_id, syntax_check)
                } else {
                    pipeline.enable_site(&req.site_id, || Ok(()))
                }
            } else if req.test_config {
                pipeline.disable_site(&req.site_id, syntax_check)
            } else {
                pipeline.disable_site(&req.site_id, || Ok(()))
            }
        } else if req.test_config {
            pipeline.apply_site_config(&req.site_id, &req.config_content, req.enable, syntax_check)
        } else {
            pipeline.apply_site_config(&req.site_id, &req.config_content, req.enable, || Ok(()))
        };

        let apply_res = result.map_err(|e| AgentError::InvalidInput(e.to_string()))?;

        let reloaded = if req.reload {
            self.reload_nginx()
        } else {
            false
        };

        Ok(SiteAppliedResponse {
            site_id: apply_res.site_id,
            available_path: apply_res.available_path.to_string_lossy().to_string(),
            enabled_path: apply_res
                .enabled_path
                .map(|p| p.to_string_lossy().to_string()),
            enabled: apply_res.enabled,
            reloaded,
        })
    }

    /// Removes a virtual host configuration with automatic rollback on syntax failure.
    pub fn remove_site(&self, req: SiteRemoveRequest) -> Result<SiteRemovedResponse, AgentError> {
        let base_dir = req
            .nginx_base_dir
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/etc/panel/nginx"));
        let pipeline = NginxApplyPipeline::new(&base_dir);
        let nginx_conf_path = base_dir.join("nginx.conf");
        let runner = Arc::clone(&self.runner);

        let conf_path_str = nginx_conf_path.to_string_lossy().to_string();
        let syntax_check = move || -> Result<(), String> {
            match runner.run_command("nginx", &["-t", "-c", &conf_path_str]) {
                Ok(out) => {
                    if out.status.success() {
                        Ok(())
                    } else {
                        Err(String::from_utf8_lossy(&out.stderr).to_string())
                    }
                }
                Err(e) => Err(e.to_string()),
            }
        };

        pipeline
            .remove_site(&req.site_id, syntax_check)
            .map_err(|e| AgentError::InvalidInput(e.to_string()))?;

        let reloaded = if req.reload {
            self.reload_nginx()
        } else {
            false
        };

        Ok(SiteRemovedResponse {
            site_id: req.site_id,
            reloaded,
        })
    }

    /// Provisions docroot directory, sets permissions, and writes modern placeholder index.html.
    pub fn provision_site(
        &self,
        req: SiteProvisionRequest,
    ) -> Result<SiteProvisionResponse, AgentError> {
        let username = panel_core::Username::parse(&req.username)
            .map_err(|e| AgentError::InvalidInput(e.to_string()))?;
        let uid = panel_core::tenant::TenantUid::parse(req.uid)
            .map_err(|e| AgentError::InvalidInput(e.to_string()))?;
        let uid_val = uid.as_u32();

        let docroot_path = PathBuf::from(&req.docroot);
        let home_path = PathBuf::from(&req.home_dir);
        let web_user = req.web_user.as_deref().unwrap_or("www-data");

        // 1. Create docroot directory with mode 0750 owned by tenant user
        self.runner
            .create_dir_all(&docroot_path, 0o750, uid_val, uid_val)?;

        // 2. Set ACL on home directory (u:<web_user>:--x) and docroot (u:<web_user>:r-x)
        let _ = self.runner.set_facl(&home_path, web_user, "--x");
        let _ = self.runner.set_facl(&docroot_path, web_user, "r-x");

        // 3. Write index.html placeholder if not exists
        let index_path = docroot_path.join("index.html");
        let mut placeholder_written = false;
        if !self.runner.path_exists(&index_path) {
            let placeholder_content = req.index_content.unwrap_or_else(|| {
                panel_webserver::template::render_placeholder_html(&req.username)
            });
            self.runner
                .write_file(&index_path, placeholder_content.as_bytes(), 0o644)?;
            let _ = self.runner.set_ownership(&index_path, uid_val, uid_val);
            placeholder_written = true;
        }

        info!(
            "Site docroot provisioned at {} for tenant {} (UID {})",
            docroot_path.display(),
            username.as_str(),
            uid_val
        );

        Ok(SiteProvisionResponse {
            docroot: req.docroot,
            placeholder_written,
            acls_applied: true,
        })
    }

    /// Deploys an ACME HTTP-01 challenge response file.
    pub fn deploy_acme_challenge(
        &self,
        req: AcmeChallengeDeployRequest,
    ) -> Result<AcmeChallengeDeployedResponse, AgentError> {
        if req.token.is_empty()
            || !req
                .token
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(AgentError::SecurityViolation(format!(
                "Invalid ACME challenge token: '{}'",
                req.token
            )));
        }

        let acme_dir = req
            .acme_dir
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/var/www/acme-challenge"));

        let challenge_dir = acme_dir.join(".well-known").join("acme-challenge");
        self.runner.create_dir_all(&challenge_dir, 0o755, 0, 0)?;

        let challenge_path = challenge_dir.join(&req.token);
        self.runner
            .write_file(&challenge_path, req.key_authorization.as_bytes(), 0o644)?;

        info!(
            "ACME HTTP-01 challenge deployed at {}",
            challenge_path.display()
        );

        Ok(AcmeChallengeDeployedResponse {
            challenge_path: challenge_path.to_string_lossy().to_string(),
        })
    }

    /// Cleans up an ACME HTTP-01 challenge file.
    pub fn cleanup_acme_challenge(
        &self,
        req: AcmeChallengeCleanupRequest,
    ) -> Result<AcmeChallengeCleanedResponse, AgentError> {
        if req.token.is_empty()
            || !req
                .token
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(AgentError::SecurityViolation(format!(
                "Invalid ACME challenge token: '{}'",
                req.token
            )));
        }

        let acme_dir = req
            .acme_dir
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/var/www/acme-challenge"));

        let challenge_path = acme_dir
            .join(".well-known")
            .join("acme-challenge")
            .join(&req.token);

        let removed = if self.runner.path_exists(&challenge_path) {
            self.runner.remove_file(&challenge_path)?;
            true
        } else {
            false
        };

        info!("ACME HTTP-01 challenge cleaned for token {}", req.token);

        Ok(AcmeChallengeCleanedResponse {
            token: req.token,
            removed,
        })
    }

    /// Deploys SSL certificate and private key with strict 0600 mode for the private key.
    pub fn deploy_ssl_cert(
        &self,
        req: SslCertDeployRequest,
    ) -> Result<SslCertDeployedResponse, AgentError> {
        let domain = panel_core::domain::DomainName::parse(&req.domain)
            .map_err(|e| AgentError::InvalidInput(e.to_string()))?;

        let ssl_base_dir = req
            .ssl_base_dir
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/etc/panel/ssl"));

        let cert_dir = ssl_base_dir.join(domain.as_ascii());
        // Directory 0700 for root
        self.runner.create_dir_all(&cert_dir, 0o700, 0, 0)?;

        let cert_path = cert_dir.join("fullchain.pem");
        let key_path = cert_dir.join("privkey.pem");

        // Write fullchain (mode 0644)
        self.runner
            .write_file(&cert_path, req.fullchain_pem.as_bytes(), 0o644)?;

        // Write private key with strict 0600 mode as required by PRD!
        self.runner
            .write_file(&key_path, req.privkey_pem.as_bytes(), 0o600)?;

        info!(
            "SSL certificate deployed for domain {} at {}",
            domain.as_unicode(),
            cert_dir.display()
        );

        Ok(SslCertDeployedResponse {
            domain: domain.as_unicode().to_string(),
            cert_path: cert_path.to_string_lossy().to_string(),
            key_path: key_path.to_string_lossy().to_string(),
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::clone_on_ref_ptr)]
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

        let manager = NginxManager::new(Arc::clone(&runner) as Arc<dyn SystemRunner>);

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

    #[test]
    fn test_deploy_and_cleanup_acme_challenge() {
        let runner = Arc::new(MockSystemRunner::new());
        let manager = NginxManager::new(Arc::clone(&runner) as Arc<dyn SystemRunner>);

        // 1. Deploy challenge
        let deploy_req = AcmeChallengeDeployRequest {
            token: "valid-token-123_abc".to_string(),
            key_authorization: "valid-token-123_abc.account-thumbprint-xyz".to_string(),
            acme_dir: Some("/var/www/acme-challenge".to_string()),
        };
        let deploy_res = manager
            .deploy_acme_challenge(deploy_req)
            .expect("deploy challenge");
        assert!(deploy_res.challenge_path.contains("valid-token-123_abc"));

        // Verify write action
        let actions = runner.recorded_actions();
        let write_action = actions.iter().find(|a| match a {
            crate::system::MockAction::WriteFile {
                path,
                content,
                mode,
            } => {
                path.to_string_lossy().contains("valid-token-123_abc")
                    && content == b"valid-token-123_abc.account-thumbprint-xyz"
                    && *mode == 0o644
            }
            _ => false,
        });
        assert!(
            write_action.is_some(),
            "Challenge file must be written with mode 0644"
        );

        // 2. Reject malicious traversal token
        let bad_req = AcmeChallengeDeployRequest {
            token: "../../../etc/passwd".to_string(),
            key_authorization: "evil".to_string(),
            acme_dir: Some("/var/www/acme-challenge".to_string()),
        };
        assert!(manager.deploy_acme_challenge(bad_req).is_err());
    }

    #[test]
    fn test_deploy_ssl_cert_enforces_0600_key() {
        let runner = Arc::new(MockSystemRunner::new());
        let manager = NginxManager::new(Arc::clone(&runner) as Arc<dyn SystemRunner>);

        let req = SslCertDeployRequest {
            domain: "contoh.id".to_string(),
            fullchain_pem: "-----BEGIN CERTIFICATE-----\nMIIB...\n-----END CERTIFICATE-----\n"
                .to_string(),
            privkey_pem: "-----BEGIN PRIVATE KEY-----\nMIIE...\n-----END PRIVATE KEY-----\n"
                .to_string(),
            ssl_base_dir: Some("/etc/panel/ssl".to_string()),
        };

        let res = manager.deploy_ssl_cert(req).expect("deploy ssl cert");
        assert_eq!(res.domain, "contoh.id");
        assert!(res.cert_path.ends_with("fullchain.pem"));
        assert!(res.key_path.ends_with("privkey.pem"));

        // Verify actions: fullchain written with 0644, privkey written with 0600
        let actions = runner.recorded_actions();
        let cert_action = actions.iter().find(|a| match a {
            crate::system::MockAction::WriteFile {
                path,
                content,
                mode,
            } => {
                path.to_string_lossy().contains("fullchain.pem")
                    && content.starts_with(b"-----BEGIN CERTIFICATE-----")
                    && *mode == 0o644
            }
            _ => false,
        });
        assert!(cert_action.is_some(), "Cert must be written with mode 0644");

        let key_action = actions.iter().find(|a| match a {
            crate::system::MockAction::WriteFile {
                path,
                content,
                mode,
            } => {
                path.to_string_lossy().contains("privkey.pem")
                    && content.starts_with(b"-----BEGIN PRIVATE KEY-----")
                    && *mode == 0o600
            }
            _ => false,
        });
        assert!(
            key_action.is_some(),
            "Private key MUST be written with strict mode 0600"
        );
    }
}
