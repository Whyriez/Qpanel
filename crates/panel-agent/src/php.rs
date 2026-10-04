//! PHP Runtime Manager agent operations: PPA repo setup, install/remove version,
//! and masking the default systemd OS services.

use crate::AgentError;
use crate::system::SystemRunner;
use panel_proto::{
    PhpExtensionActionResponse, PhpExtensionDto, PhpExtensionListResponse,
    PhpFpmMasterActionResponse, PhpFpmMasterEnsureRequest, PhpFpmMasterEnsureResponse,
    PhpFpmMasterReloadRequest, PhpFpmMasterRestartRequest, PhpFpmMasterStatusRequest,
    PhpFpmMasterStatusResponse, PhpFpmPoolApplyRequest, PhpFpmPoolApplyResponse,
    PhpFpmPoolRemoveRequest, PhpFpmPoolRemoveResponse, PhpRepoSetupResponse,
    PhpRuntimeActionResponse, PhpRuntimeListResponse, PhpVersionStatusDto,
};
use panel_runtime::fpm::{
    DEFAULT_PM_MAX_REQUESTS, DEFAULT_PM_PROCESS_IDLE_TIMEOUT_SECS, PhpFpmMasterConfig,
    PhpFpmPoolConfig,
};
use panel_runtime::php::{
    CURATED_PHP_EXTENSIONS, PhpVersion, find_curated_extension, is_sury_repo_installed,
};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Sets up the Ondřej Surý PHP PPA repository on Debian/Ubuntu systems.
pub fn setup_php_repository(runner: &dyn SystemRunner) -> Result<PhpRepoSetupResponse, AgentError> {
    info!("Setting up Ondřej Surý PHP PPA repository");

    // Check if repo already installed
    let already_installed = is_sury_repo_installed(|p| runner.path_exists(p));
    if already_installed {
        info!("Sury PHP repository already present in apt sources");
        return Ok(PhpRepoSetupResponse {
            success: true,
            ppa_configured: true,
            message: "Repositori PPA PHP Sury sudah terpasang".to_string(),
        });
    }

    // Step 1: Install prerequisite packages
    runner.run_command(
        "apt-get",
        &[
            "install",
            "-y",
            "--no-install-recommends",
            "software-properties-common",
            "ca-certificates",
            "lsb-release",
            "apt-transport-https",
        ],
    )?;

    // Step 2: Add PPA repository
    runner.run_command("add-apt-repository", &["-y", "ppa:ondrej/php"])?;

    // Step 3: Refresh apt package index
    runner.run_command("apt-get", &["update"])?;

    info!("Ondřej Surý PHP PPA repository configured successfully");
    Ok(PhpRepoSetupResponse {
        success: true,
        ppa_configured: true,
        message: "Repositori PPA PHP Ondřej Surý berhasil dikonfigurasi".to_string(),
    })
}

/// Lists all supported PHP versions along with installation and masking statuses.
pub fn list_php_runtimes(runner: &dyn SystemRunner) -> Result<PhpRuntimeListResponse, AgentError> {
    let ppa_configured = is_sury_repo_installed(|p| runner.path_exists(p));
    let mut versions = Vec::new();

    for version in PhpVersion::all_supported() {
        let fpm_binary_exists = runner.path_exists(&version.fpm_binary_path());
        let cli_binary_exists = runner.path_exists(&version.cli_binary_path());
        let installed = fpm_binary_exists && cli_binary_exists;

        let service_name = version.fpm_service_name();
        let service_masked = if installed || runner.path_exists(&version.fpm_config_dir()) {
            // Check systemctl is-enabled
            match runner.run_command("systemctl", &["is-enabled", &service_name]) {
                Ok(output) => {
                    let out_str = String::from_utf8_lossy(&output.stdout).to_lowercase();
                    out_str.contains("masked")
                }
                Err(AgentError::CommandFailed { stderr, .. }) => stderr.contains("masked"),
                Err(_) => false,
            }
        } else {
            false
        };

        let status = if installed {
            "installed".to_string()
        } else {
            "not_installed".to_string()
        };

        versions.push(PhpVersionStatusDto {
            version: version.as_str().to_string(),
            installed,
            fpm_binary_exists,
            cli_binary_exists,
            service_masked,
            status,
            fpm_binary_path: version.fpm_binary_path().to_string_lossy().to_string(),
            cli_binary_path: version.cli_binary_path().to_string_lossy().to_string(),
            service_name,
        });
    }

    Ok(PhpRuntimeListResponse {
        ppa_configured,
        versions,
    })
}

/// Installs a specific PHP runtime version and masks the default OS FPM service.
pub fn install_php_runtime(
    runner: &dyn SystemRunner,
    version_str: &str,
) -> Result<PhpRuntimeActionResponse, AgentError> {
    let version =
        PhpVersion::parse(version_str).map_err(|e| AgentError::InvalidInput(e.to_string()))?;

    info!("Installing PHP version {}", version.as_str());

    // Step 1: Install packages via apt-get
    let mut install_args = vec!["install", "-y", "--no-install-recommends"];
    let pkgs = version.base_packages();
    let pkg_strs: Vec<&str> = pkgs.iter().map(String::as_str).collect();
    install_args.extend(pkg_strs);

    runner.run_command("apt-get", &install_args)?;

    // Step 2: Stop OS-level service if running
    let service_name = version.fpm_service_name();
    let _ = runner.run_command("systemctl", &["stop", &service_name]);

    // Step 3: Mask OS-level service so it NEVER runs globally (panel creates per-tenant masters)
    runner.run_command("systemctl", &["mask", &service_name])?;

    // Step 4: Verify masked status
    let mask_verified = match runner.run_command("systemctl", &["is-enabled", &service_name]) {
        Ok(out) => {
            let s = String::from_utf8_lossy(&out.stdout).to_lowercase();
            s.contains("masked")
        }
        Err(AgentError::CommandFailed { stderr, .. }) => stderr.contains("masked"),
        Err(_) => false,
    };

    if !mask_verified {
        warn!("Warning: Failed to verify masked status for {service_name}, but install completed.");
    }

    info!(
        "PHP {} installed successfully and {} masked",
        version.as_str(),
        service_name
    );

    Ok(PhpRuntimeActionResponse {
        version: version.as_str().to_string(),
        success: true,
        masked: true,
        message: format!(
            "PHP {} berhasil dipasang dan service {} telah di-mask",
            version.as_str(),
            service_name
        ),
    })
}

/// Removes a specific PHP runtime version and cleans up.
pub fn remove_php_runtime(
    runner: &dyn SystemRunner,
    version_str: &str,
) -> Result<PhpRuntimeActionResponse, AgentError> {
    let version =
        PhpVersion::parse(version_str).map_err(|e| AgentError::InvalidInput(e.to_string()))?;

    info!("Removing PHP version {}", version.as_str());

    // Step 1: Unmask service
    let service_name = version.fpm_service_name();
    let _ = runner.run_command("systemctl", &["unmask", &service_name]);

    // Step 2: Purge packages
    let mut remove_args = vec!["remove", "-y", "--purge"];
    let pkgs = version.base_packages();
    let pkg_strs: Vec<&str> = pkgs.iter().map(String::as_str).collect();
    remove_args.extend(pkg_strs);

    runner.run_command("apt-get", &remove_args)?;

    // Step 3: Autoremove unused dependencies
    let _ = runner.run_command("apt-get", &["autoremove", "-y"]);

    info!("PHP {} removed successfully", version.as_str());

    Ok(PhpRuntimeActionResponse {
        version: version.as_str().to_string(),
        success: true,
        masked: false,
        message: format!("PHP {} berhasil dihapus dari sistem", version.as_str()),
    })
}

/// Lists curated PHP extensions and their installation and enabled statuses for a given PHP version.
pub fn list_php_extensions(
    runner: &dyn SystemRunner,
    version_str: &str,
) -> Result<PhpExtensionListResponse, AgentError> {
    let version =
        PhpVersion::parse(version_str).map_err(|e| AgentError::InvalidInput(e.to_string()))?;

    let mut extensions = Vec::with_capacity(CURATED_PHP_EXTENSIONS.len());

    for def in CURATED_PHP_EXTENSIONS {
        let pkg_name = def.package_name(version);

        // Check if extension is installed:
        // 1) /etc/php/{ver}/mods-available/{ini}.ini exists
        // 2) or dpkg-query status contains "install ok installed"
        let mods_exist = def.ini_names.iter().any(|ini| {
            let path_str = format!("/etc/php/{}/mods-available/{}.ini", version.as_str(), ini);
            runner.path_exists(Path::new(&path_str))
        });

        let installed = if mods_exist {
            true
        } else {
            match runner.run_command("dpkg-query", &["-W", "-f=${Status}", &pkg_name]) {
                Ok(out) => {
                    let s = String::from_utf8_lossy(&out.stdout).to_lowercase();
                    s.contains("install ok installed")
                }
                _ => false,
            }
        };

        // Check if extension is enabled in FPM or CLI conf.d
        let enabled = if !installed {
            false
        } else {
            let prefixes = ["20-", "10-", "30-", "15-", ""];
            def.ini_names.iter().any(|ini| {
                prefixes.iter().any(|prefix| {
                    let fpm_ini = format!(
                        "/etc/php/{}/fpm/conf.d/{}{}.ini",
                        version.as_str(),
                        prefix,
                        ini
                    );
                    let cli_ini = format!(
                        "/etc/php/{}/cli/conf.d/{}{}.ini",
                        version.as_str(),
                        prefix,
                        ini
                    );
                    runner.path_exists(Path::new(&fpm_ini))
                        || runner.path_exists(Path::new(&cli_ini))
                })
            })
        };

        extensions.push(PhpExtensionDto {
            name: def.name.to_string(),
            version: version.as_str().to_string(),
            package_name: pkg_name,
            installed,
            enabled,
            is_base: def.is_base,
            category: def.category.to_string(),
            description: def.description_id.to_string(),
        });
    }

    Ok(PhpExtensionListResponse {
        version: version.as_str().to_string(),
        extensions,
    })
}

/// Installs a curated PHP extension package and enables it.
pub fn install_php_extension(
    runner: &dyn SystemRunner,
    version_str: &str,
    ext_name: &str,
) -> Result<PhpExtensionActionResponse, AgentError> {
    let version =
        PhpVersion::parse(version_str).map_err(|e| AgentError::InvalidInput(e.to_string()))?;
    let def =
        find_curated_extension(ext_name).map_err(|e| AgentError::InvalidInput(e.to_string()))?;

    let pkg_name = def.package_name(version);
    info!(
        "Installing PHP extension '{}' (package: {}) for PHP {}",
        def.name,
        pkg_name,
        version.as_str()
    );

    runner.run_command(
        "apt-get",
        &["install", "-y", "--no-install-recommends", &pkg_name],
    )?;

    // Enable for all SAPI targets
    let _ = runner.run_command("phpenmod", &["-v", version.as_str(), "-s", "ALL", def.name]);

    Ok(PhpExtensionActionResponse {
        version: version.as_str().to_string(),
        extension: def.name.to_string(),
        success: true,
        message: format!(
            "Ekstensi '{}' berhasil dipasang untuk PHP {}",
            def.name,
            version.as_str()
        ),
    })
}

/// Enables or disables an installed PHP extension across SAPIs.
pub fn toggle_php_extension(
    runner: &dyn SystemRunner,
    version_str: &str,
    ext_name: &str,
    enabled: bool,
) -> Result<PhpExtensionActionResponse, AgentError> {
    let version =
        PhpVersion::parse(version_str).map_err(|e| AgentError::InvalidInput(e.to_string()))?;
    let def =
        find_curated_extension(ext_name).map_err(|e| AgentError::InvalidInput(e.to_string()))?;

    if enabled {
        info!(
            "Enabling PHP extension '{}' for PHP {}",
            def.name,
            version.as_str()
        );
        runner.run_command("phpenmod", &["-v", version.as_str(), "-s", "ALL", def.name])?;
    } else {
        info!(
            "Disabling PHP extension '{}' for PHP {}",
            def.name,
            version.as_str()
        );
        runner.run_command(
            "phpdismod",
            &["-v", version.as_str(), "-s", "ALL", def.name],
        )?;
    }

    Ok(PhpExtensionActionResponse {
        version: version.as_str().to_string(),
        extension: def.name.to_string(),
        success: true,
        message: format!(
            "Ekstensi '{}' berhasil {}",
            def.name,
            if enabled {
                "diaktifkan"
            } else {
                "dinonaktifkan"
            }
        ),
    })
}

/// Removes a PHP extension package from the host.
pub fn remove_php_extension(
    runner: &dyn SystemRunner,
    version_str: &str,
    ext_name: &str,
) -> Result<PhpExtensionActionResponse, AgentError> {
    let version =
        PhpVersion::parse(version_str).map_err(|e| AgentError::InvalidInput(e.to_string()))?;
    let def =
        find_curated_extension(ext_name).map_err(|e| AgentError::InvalidInput(e.to_string()))?;

    if def.is_base {
        return Err(AgentError::InvalidInput(format!(
            "Ekstensi dasar '{}' tidak dapat dihapus karena merupakan modul standar PHP",
            def.name
        )));
    }

    let pkg_name = def.package_name(version);
    info!(
        "Removing PHP extension '{}' (package: {}) for PHP {}",
        def.name,
        pkg_name,
        version.as_str()
    );

    // Disable first to clean symlinks
    let _ = runner.run_command(
        "phpdismod",
        &["-v", version.as_str(), "-s", "ALL", def.name],
    );

    // Purge package
    runner.run_command("apt-get", &["remove", "-y", "--purge", &pkg_name])?;

    // Autoremove unused deps
    let _ = runner.run_command("apt-get", &["autoremove", "-y"]);

    Ok(PhpExtensionActionResponse {
        version: version.as_str().to_string(),
        extension: def.name.to_string(),
        success: true,
        message: format!(
            "Ekstensi '{}' berhasil dihapus untuk PHP {}",
            def.name,
            version.as_str()
        ),
    })
}

/// Ensures the PHP-FPM master instance for a specific (tenant, version) is configured,
/// sandboxed systemd service written, directories created, and ready to receive pools.
pub fn ensure_php_fpm_master(
    runner: &dyn SystemRunner,
    req: PhpFpmMasterEnsureRequest,
) -> Result<PhpFpmMasterEnsureResponse, AgentError> {
    let version =
        PhpVersion::parse(&req.version).map_err(|e| AgentError::InvalidInput(e.to_string()))?;
    let master = PhpFpmMasterConfig::new(&req.username, req.uid, version)
        .map_err(|e| AgentError::InvalidInput(e.to_string()))?;

    info!(
        "Ensuring PHP-FPM master for tenant '{}' on PHP {}",
        master.username,
        master.version.as_str()
    );

    // 1. Ensure runtime, config, pool, and log directories exist
    runner.create_dir_all(Path::new("/run/panel/php"), 0o755, 0, 0)?;
    runner.create_dir_all(&master.config_dir(), 0o755, 0, 0)?;
    runner.create_dir_all(&master.pool_dir(), 0o755, 0, 0)?;
    runner.create_dir_all(&master.tenant_log_dir(), 0o750, 0, 0)?;

    // Give tenant read ACL on their log dir so they can inspect logs without root
    let _ = runner.set_facl(&master.tenant_log_dir(), &master.username, "r-x");

    // Ensure tenant tmp dir exists for upload_tmp_dir and session.save_path
    let user_tmp = PathBuf::from(format!("/home/{}/tmp", master.username));
    runner.create_dir_all(&user_tmp, 0o750, master.uid, master.uid)?;

    // 2. Write master configuration (php-fpm.conf)
    runner.write_file(
        &master.master_conf_path(),
        master.render_master_conf().as_bytes(),
        0o644,
    )?;

    // 3. Write sandboxed systemd service unit
    let service_unit_path = PathBuf::from("/etc/systemd/system").join(master.service_name());
    runner.write_file(
        &service_unit_path,
        master.render_systemd_unit().as_bytes(),
        0o644,
    )?;

    // 4. Reload systemd daemon
    runner.run_command("systemctl", &["daemon-reload"])?;

    // 5. Query active status
    let is_active = runner
        .run_command(
            "systemctl",
            &["is-active", "--quiet", &master.service_name()],
        )
        .is_ok();

    Ok(PhpFpmMasterEnsureResponse {
        service_name: master.service_name(),
        config_dir: master.config_dir().to_string_lossy().replace('\\', "/"),
        default_socket_path: master
            .default_socket_path()
            .to_string_lossy()
            .replace('\\', "/"),
        is_active,
    })
}

fn build_pool_config(
    master: &PhpFpmMasterConfig,
    req: &PhpFpmPoolApplyRequest,
) -> Result<PhpFpmPoolConfig, AgentError> {
    let mut pool = PhpFpmPoolConfig::new(master, &req.pool_name, &req.docroot)
        .map_err(|e| AgentError::InvalidInput(e.to_string()))?;

    if let Some(sock) = req.socket_path.as_deref() {
        pool = pool.with_socket_path(sock);
    }
    if let (Some(max_ch), Some(idle_to), Some(max_req)) = (
        req.max_children,
        req.process_idle_timeout_secs,
        req.max_requests,
    ) {
        pool = pool.with_pm_limits(max_ch, idle_to, max_req);
    } else if let Some(max_ch) = req.max_children {
        pool = pool.with_pm_limits(
            max_ch,
            DEFAULT_PM_PROCESS_IDLE_TIMEOUT_SECS,
            DEFAULT_PM_MAX_REQUESTS,
        );
    }
    if let Some(open_bd) = &req.open_basedir {
        pool.open_basedir = open_bd.iter().map(PathBuf::from).collect();
    }
    if let Some(dis_fn) = &req.disable_functions {
        pool.disable_functions = dis_fn.clone();
    }

    Ok(pool)
}

fn validate_and_swap_staging_pool(
    runner: &dyn SystemRunner,
    master: &PhpFpmMasterConfig,
    version: PhpVersion,
    pool: &PhpFpmPoolConfig,
    staging_path: &Path,
    target_path: &Path,
) -> Result<(), AgentError> {
    runner.write_file(staging_path, pool.render_pool_conf().as_bytes(), 0o644)?;

    let fpm_bin = format!("php-fpm{}", version.as_str());
    let master_conf_str = master.master_conf_path().to_string_lossy().to_string();

    let check_failure = runner
        .run_command(&fpm_bin, &["-t", "-y", &master_conf_str])
        .ok()
        .filter(|out| !out.status.success());

    if let Some(check_out) = check_failure {
        let err_msg = String::from_utf8_lossy(&check_out.stderr).to_string();
        let _ = runner.remove_file(staging_path);
        warn!(
            "PHP-FPM syntax check failed for pool {}: {}",
            pool.pool_name, err_msg
        );
        return Err(AgentError::InvalidInput(format!(
            "Validasi sintaks PHP-FPM gagal: {err_msg}"
        )));
    }

    runner.move_path(staging_path, target_path)
}

fn activate_master_service(
    runner: &dyn SystemRunner,
    master: &PhpFpmMasterConfig,
) -> Result<(), AgentError> {
    let is_active = runner
        .run_command(
            "systemctl",
            &["is-active", "--quiet", &master.service_name()],
        )
        .is_ok();

    if is_active {
        info!(
            "Reloading active PHP-FPM service: {}",
            master.service_name()
        );
        runner.run_command("systemctl", &["reload", &master.service_name()])?;
    } else {
        info!(
            "Starting PHP-FPM service on-demand: {}",
            master.service_name()
        );
        runner.run_command("systemctl", &["enable", "--now", &master.service_name()])?;
    }
    Ok(())
}

/// Applies an isolated PHP-FPM pool for a website under the tenant's master instance.
/// Employs staging and atomic swap to prevent invalid configuration from breaking the master.
pub fn apply_php_fpm_pool(
    runner: &dyn SystemRunner,
    req: PhpFpmPoolApplyRequest,
) -> Result<PhpFpmPoolApplyResponse, AgentError> {
    let version =
        PhpVersion::parse(&req.version).map_err(|e| AgentError::InvalidInput(e.to_string()))?;
    let master = PhpFpmMasterConfig::new(&req.username, req.uid, version)
        .map_err(|e| AgentError::InvalidInput(e.to_string()))?;

    // Step 1: Ensure master instance and systemd service exist
    ensure_php_fpm_master(
        runner,
        PhpFpmMasterEnsureRequest {
            username: req.username.clone(),
            uid: req.uid,
            version: req.version.clone(),
        },
    )?;

    // Step 2: Build pool configuration
    let pool = build_pool_config(&master, &req)?;
    let target_path = pool.pool_conf_path(&master);
    let staging_path = master
        .pool_dir()
        .join(format!("{}.conf.staging", pool.pool_name));

    info!(
        "Applying PHP-FPM pool '{}' at {}",
        pool.pool_name,
        target_path.display()
    );

    // Step 3-5: Staging, syntax validation, and atomic swap
    validate_and_swap_staging_pool(runner, &master, version, &pool, &staging_path, &target_path)?;

    // Step 6: Reload or start master on demand
    let reloaded = if req.reload {
        activate_master_service(runner, &master)?;
        true
    } else {
        false
    };

    Ok(PhpFpmPoolApplyResponse {
        pool_name: pool.pool_name,
        pool_conf_path: target_path.to_string_lossy().replace('\\', "/"),
        socket_path: pool.socket_path.to_string_lossy().replace('\\', "/"),
        reloaded,
    })
}

fn count_remaining_pools(runner: &dyn SystemRunner, master: &PhpFpmMasterConfig) -> usize {
    let entries = runner.read_dir(&master.pool_dir()).unwrap_or_default();
    entries
        .iter()
        .filter(|p| {
            p.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext == "conf")
        })
        .count()
}

fn handle_service_after_pool_removal(
    runner: &dyn SystemRunner,
    master: &PhpFpmMasterConfig,
    remaining_pools: usize,
    reload: bool,
) -> Result<(bool, bool), AgentError> {
    if remaining_pools == 0 {
        info!(
            "No remaining pools for master {}. Stopping and disabling service to conserve RAM.",
            master.service_name()
        );
        let _ = runner.run_command("systemctl", &["stop", &master.service_name()]);
        let _ = runner.run_command("systemctl", &["disable", &master.service_name()]);
        Ok((true, false))
    } else if reload {
        let is_active = runner
            .run_command(
                "systemctl",
                &["is-active", "--quiet", &master.service_name()],
            )
            .is_ok();
        if is_active {
            info!("Reloading PHP-FPM service: {}", master.service_name());
            runner.run_command("systemctl", &["reload", &master.service_name()])?;
            Ok((false, true))
        } else {
            Ok((false, false))
        }
    } else {
        Ok((false, false))
    }
}

/// Removes a PHP-FPM pool for a website.
/// If no pools remain for the master, shuts down and disables the service to free idle RAM.
pub fn remove_php_fpm_pool(
    runner: &dyn SystemRunner,
    req: PhpFpmPoolRemoveRequest,
) -> Result<PhpFpmPoolRemoveResponse, AgentError> {
    let version =
        PhpVersion::parse(&req.version).map_err(|e| AgentError::InvalidInput(e.to_string()))?;
    let master = PhpFpmMasterConfig::new(&req.username, 10000, version)
        .map_err(|e| AgentError::InvalidInput(e.to_string()))?;

    let pool_file = master.pool_dir().join(format!("{}.conf", req.pool_name));
    let mut removed = false;

    if runner.path_exists(&pool_file) {
        info!(
            "Removing PHP-FPM pool configuration: {}",
            pool_file.display()
        );
        runner.remove_file(&pool_file)?;
        removed = true;
    }

    let remaining_pools = count_remaining_pools(runner, &master);
    let (master_stopped, reloaded) =
        handle_service_after_pool_removal(runner, &master, remaining_pools, req.reload)?;

    Ok(PhpFpmPoolRemoveResponse {
        pool_name: req.pool_name,
        removed,
        remaining_pools,
        master_stopped,
        reloaded,
    })
}

/// Retrieves the runtime and process status of a tenant's PHP-FPM master instance.
pub fn get_php_fpm_master_status(
    runner: &dyn SystemRunner,
    req: PhpFpmMasterStatusRequest,
) -> Result<PhpFpmMasterStatusResponse, AgentError> {
    let version =
        PhpVersion::parse(&req.version).map_err(|e| AgentError::InvalidInput(e.to_string()))?;
    let master = PhpFpmMasterConfig::new(&req.username, 10000, version)
        .map_err(|e| AgentError::InvalidInput(e.to_string()))?;

    let is_active = runner
        .run_command(
            "systemctl",
            &["is-active", "--quiet", &master.service_name()],
        )
        .is_ok();

    let substate = runner
        .run_command(
            "systemctl",
            &["show", "-p", "SubState", "--value", &master.service_name()],
        )
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_else(|_| {
            if is_active {
                "running".to_string()
            } else {
                "inactive".to_string()
            }
        });

    let socket_exists = runner.path_exists(&master.default_socket_path());

    let entries = runner.read_dir(&master.pool_dir()).unwrap_or_default();
    let pools_count = entries
        .iter()
        .filter(|p| {
            p.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext == "conf")
        })
        .count();

    Ok(PhpFpmMasterStatusResponse {
        service_name: master.service_name(),
        is_active,
        substate,
        socket_exists,
        pools_count,
    })
}

/// Gracefully reloads a tenant's PHP-FPM master instance without dropping in-flight requests.
pub fn reload_php_fpm_master(
    runner: &dyn SystemRunner,
    req: PhpFpmMasterReloadRequest,
) -> Result<PhpFpmMasterActionResponse, AgentError> {
    let version =
        PhpVersion::parse(&req.version).map_err(|e| AgentError::InvalidInput(e.to_string()))?;
    let master = PhpFpmMasterConfig::new(&req.username, 10000, version)
        .map_err(|e| AgentError::InvalidInput(e.to_string()))?;

    info!(
        "Gracefully reloading PHP-FPM master: {}",
        master.service_name()
    );
    runner.run_command("systemctl", &["reload", &master.service_name()])?;

    Ok(PhpFpmMasterActionResponse {
        service_name: master.service_name(),
        action: "reload".to_string(),
        success: true,
        message: format!(
            "PHP-FPM master '{}' berhasil di-reload",
            master.service_name()
        ),
    })
}

/// Restarts a tenant's PHP-FPM master instance.
pub fn restart_php_fpm_master(
    runner: &dyn SystemRunner,
    req: PhpFpmMasterRestartRequest,
) -> Result<PhpFpmMasterActionResponse, AgentError> {
    let version =
        PhpVersion::parse(&req.version).map_err(|e| AgentError::InvalidInput(e.to_string()))?;
    let master = PhpFpmMasterConfig::new(&req.username, 10000, version)
        .map_err(|e| AgentError::InvalidInput(e.to_string()))?;

    info!("Restarting PHP-FPM master: {}", master.service_name());
    runner.run_command("systemctl", &["restart", &master.service_name()])?;

    Ok(PhpFpmMasterActionResponse {
        service_name: master.service_name(),
        action: "restart".to_string(),
        success: true,
        message: format!(
            "PHP-FPM master '{}' berhasil di-restart",
            master.service_name()
        ),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::system::MockSystemRunner;
    use std::path::PathBuf;

    #[test]
    fn test_setup_php_repository_installs_and_updates() {
        let runner = MockSystemRunner::new();
        let res = setup_php_repository(&runner).unwrap();

        assert!(res.success);
        assert!(res.ppa_configured);

        let commands = runner.get_commands();
        assert!(
            commands
                .iter()
                .any(|cmd| cmd.contains("software-properties-common"))
        );
        assert!(
            commands
                .iter()
                .any(|cmd| cmd.contains("add-apt-repository -y ppa:ondrej/php"))
        );
        assert!(commands.iter().any(|cmd| cmd.contains("apt-get update")));
    }

    #[test]
    fn test_install_php_runtime_installs_and_masks_service() {
        let runner = MockSystemRunner::new();
        let res = install_php_runtime(&runner, "8.3").unwrap();

        assert!(res.success);
        assert!(res.masked);
        assert_eq!(res.version, "8.3");

        let commands = runner.get_commands();
        // Checked that apt-get install was called for php8.3
        assert!(
            commands
                .iter()
                .any(|cmd| cmd.contains("apt-get install -y --no-install-recommends php8.3-fpm"))
        );
        // Checked that systemctl stop was called
        assert!(
            commands
                .iter()
                .any(|cmd| cmd.contains("systemctl stop php8.3-fpm.service"))
        );
        // Checked that systemctl mask was called (requirement PRD P4-T1)
        assert!(
            commands
                .iter()
                .any(|cmd| cmd.contains("systemctl mask php8.3-fpm.service"))
        );
    }

    #[test]
    fn test_remove_php_runtime_unmasks_and_purges() {
        let runner = MockSystemRunner::new();
        let res = remove_php_runtime(&runner, "8.3").unwrap();

        assert!(res.success);
        assert_eq!(res.version, "8.3");

        let commands = runner.get_commands();
        assert!(
            commands
                .iter()
                .any(|cmd| cmd.contains("systemctl unmask php8.3-fpm.service"))
        );
        assert!(
            commands
                .iter()
                .any(|cmd| cmd.contains("apt-get remove -y --purge php8.3-fpm"))
        );
    }

    #[test]
    fn test_list_php_runtimes_reports_status() {
        let runner = MockSystemRunner::new();
        // Simulate PHP 8.2 installed
        runner.add_existing_path(PathBuf::from("/usr/sbin/php-fpm8.2"));
        runner.add_existing_path(PathBuf::from("/usr/bin/php8.2"));

        let res = list_php_runtimes(&runner).unwrap();
        assert_eq!(res.versions.len(), 6);

        let v82 = res.versions.iter().find(|v| v.version == "8.2").unwrap();
        assert!(v82.installed);
        assert_eq!(v82.status, "installed");

        let v84 = res.versions.iter().find(|v| v.version == "8.4").unwrap();
        assert!(!v84.installed);
        assert_eq!(v84.status, "not_installed");
    }

    #[test]
    fn test_list_php_extensions() {
        let runner = MockSystemRunner::new();
        // Simulate redis installed and enabled
        runner.add_existing_path(PathBuf::from("/etc/php/8.3/mods-available/redis.ini"));
        runner.add_existing_path(PathBuf::from("/etc/php/8.3/fpm/conf.d/20-redis.ini"));

        let res = list_php_extensions(&runner, "8.3").unwrap();
        assert_eq!(res.version, "8.3");
        assert_eq!(res.extensions.len(), 16);

        let redis = res.extensions.iter().find(|e| e.name == "redis").unwrap();
        assert!(redis.installed);
        assert!(redis.enabled);
        assert_eq!(redis.package_name, "php8.3-redis");
        assert!(!redis.is_base);

        let bz2 = res.extensions.iter().find(|e| e.name == "bz2").unwrap();
        assert!(!bz2.installed);
        assert!(!bz2.enabled);
    }

    #[test]
    fn test_install_and_toggle_and_remove_extension() {
        let runner = MockSystemRunner::new();

        // 1. Install extension
        let inst = install_php_extension(&runner, "8.3", "redis").unwrap();
        assert!(inst.success);
        assert_eq!(inst.extension, "redis");
        let commands = runner.get_commands();
        assert!(
            commands
                .iter()
                .any(|cmd| cmd.contains("apt-get install -y --no-install-recommends php8.3-redis"))
        );
        assert!(
            commands
                .iter()
                .any(|cmd| cmd.contains("phpenmod -v 8.3 -s ALL redis"))
        );

        // 2. Toggle extension (disable)
        let tog_off = toggle_php_extension(&runner, "8.3", "redis", false).unwrap();
        assert!(tog_off.success);
        let commands = runner.get_commands();
        assert!(
            commands
                .iter()
                .any(|cmd| cmd.contains("phpdismod -v 8.3 -s ALL redis"))
        );

        // 3. Toggle extension (enable)
        let tog_on = toggle_php_extension(&runner, "8.3", "redis", true).unwrap();
        assert!(tog_on.success);
        let commands = runner.get_commands();
        assert!(
            commands
                .iter()
                .any(|cmd| cmd.contains("phpenmod -v 8.3 -s ALL redis"))
        );

        // 4. Remove base extension should fail
        let rem_base = remove_php_extension(&runner, "8.3", "curl");
        assert!(rem_base.is_err());

        // 5. Remove non-base extension
        let rem = remove_php_extension(&runner, "8.3", "redis").unwrap();
        assert!(rem.success);
        let commands = runner.get_commands();
        assert!(
            commands
                .iter()
                .any(|cmd| cmd.contains("apt-get remove -y --purge php8.3-redis"))
        );
    }

    #[test]
    fn test_ensure_php_fpm_master_creates_structure_and_service() {
        let runner = MockSystemRunner::new();
        let res = ensure_php_fpm_master(
            &runner,
            PhpFpmMasterEnsureRequest {
                username: "p_alice".to_string(),
                uid: 10001,
                version: "8.3".to_string(),
            },
        )
        .unwrap();

        assert_eq!(res.service_name, "panel-fpm@p_alice-8.3.service");
        assert_eq!(res.config_dir, "/etc/panel/php/p_alice-8.3");
        assert_eq!(res.default_socket_path, "/run/panel/php/p_alice-8.3.sock");

        let files = runner.files.lock().unwrap();
        let master_conf_path = PathBuf::from("/etc/panel/php/p_alice-8.3/php-fpm.conf");
        assert!(files.contains_key(&master_conf_path));
        let conf_content =
            String::from_utf8(files.get(&master_conf_path).unwrap().clone()).unwrap();
        assert!(conf_content.contains("include = /etc/panel/php/p_alice-8.3/pool.d/*.conf"));

        let unit_path = PathBuf::from("/etc/systemd/system/panel-fpm@p_alice-8.3.service");
        assert!(files.contains_key(&unit_path));
        let unit_content = String::from_utf8(files.get(&unit_path).unwrap().clone()).unwrap();
        assert!(unit_content.contains("User=p_alice"));
        assert!(unit_content.contains("Slice=panel-u10001.slice"));
        assert!(unit_content.contains("NoNewPrivileges=true"));

        let commands = runner.get_commands();
        assert!(
            commands
                .iter()
                .any(|c| c.contains("systemctl daemon-reload"))
        );
    }

    #[test]
    fn test_apply_php_fpm_pool_and_validation() {
        let runner = MockSystemRunner::new();
        let res = apply_php_fpm_pool(
            &runner,
            PhpFpmPoolApplyRequest {
                username: "p_alice".to_string(),
                uid: 10001,
                version: "8.3".to_string(),
                pool_name: "site-1".to_string(),
                docroot: "/home/p_alice/sites/example.com/public".to_string(),
                socket_path: None,
                max_children: Some(15),
                process_idle_timeout_secs: Some(15),
                max_requests: Some(1000),
                open_basedir: None,
                disable_functions: None,
                reload: true,
            },
        )
        .unwrap();

        assert_eq!(res.pool_name, "site-1");
        assert!(res.reloaded);
        assert_eq!(
            res.pool_conf_path,
            PathBuf::from("/etc/panel/php/p_alice-8.3/pool.d/site-1.conf")
                .to_string_lossy()
                .to_string()
        );

        let files = runner.files.lock().unwrap();
        let pool_path = PathBuf::from("/etc/panel/php/p_alice-8.3/pool.d/site-1.conf");
        assert!(files.contains_key(&pool_path));
        let pool_content = String::from_utf8(files.get(&pool_path).unwrap().clone()).unwrap();
        assert!(pool_content.contains("[site-1]"));
        assert!(pool_content.contains("listen = /run/panel/php/p_alice-8.3.sock"));
        assert!(pool_content.contains("pm.max_children = 15"));
        assert!(pool_content.contains("pm = ondemand"));

        let commands = runner.get_commands();
        assert!(commands.iter().any(|c| {
            c.contains("systemctl reload panel-fpm@p_alice-8.3.service")
                || c.contains("systemctl enable --now panel-fpm@p_alice-8.3.service")
        }));
    }

    #[test]
    fn test_apply_php_fpm_pool_syntax_error_rollback() {
        let runner = MockSystemRunner::new();
        runner.set_response(
            "php-fpm8.3",
            1,
            b"",
            b"Fatal error: pool configuration syntax invalid at line 10",
        );

        let res = apply_php_fpm_pool(
            &runner,
            PhpFpmPoolApplyRequest {
                username: "p_alice".to_string(),
                uid: 10001,
                version: "8.3".to_string(),
                pool_name: "broken-pool".to_string(),
                docroot: "/home/p_alice/sites/broken.com/public".to_string(),
                socket_path: None,
                max_children: None,
                process_idle_timeout_secs: None,
                max_requests: None,
                open_basedir: None,
                disable_functions: None,
                reload: true,
            },
        );

        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(err.to_string().contains("Validasi sintaks PHP-FPM gagal"));

        let files = runner.files.lock().unwrap();
        let target_path = PathBuf::from("/etc/panel/php/p_alice-8.3/pool.d/broken-pool.conf");
        let staging_path =
            PathBuf::from("/etc/panel/php/p_alice-8.3/pool.d/broken-pool.conf.staging");
        assert!(!files.contains_key(&target_path));
        assert!(!files.contains_key(&staging_path));
    }

    #[test]
    fn test_remove_php_fpm_pool_and_shutdown_idle_master() {
        let runner = MockSystemRunner::new();
        apply_php_fpm_pool(
            &runner,
            PhpFpmPoolApplyRequest {
                username: "p_alice".to_string(),
                uid: 10001,
                version: "8.3".to_string(),
                pool_name: "site-1".to_string(),
                docroot: "/home/p_alice/sites/example.com/public".to_string(),
                socket_path: None,
                max_children: None,
                process_idle_timeout_secs: None,
                max_requests: None,
                open_basedir: None,
                disable_functions: None,
                reload: false,
            },
        )
        .unwrap();

        let rem_res = remove_php_fpm_pool(
            &runner,
            PhpFpmPoolRemoveRequest {
                username: "p_alice".to_string(),
                version: "8.3".to_string(),
                pool_name: "site-1".to_string(),
                reload: true,
            },
        )
        .unwrap();

        assert!(rem_res.removed);
        assert_eq!(rem_res.remaining_pools, 0);
        assert!(rem_res.master_stopped);

        let commands = runner.get_commands();
        assert!(
            commands
                .iter()
                .any(|c| c.contains("systemctl stop panel-fpm@p_alice-8.3.service"))
        );
        assert!(
            commands
                .iter()
                .any(|c| c.contains("systemctl disable panel-fpm@p_alice-8.3.service"))
        );
    }

    #[test]
    fn test_get_php_fpm_master_status_and_actions() {
        let runner = MockSystemRunner::new();
        runner.set_response("systemctl", 0, b"active\n", b"");

        let status = get_php_fpm_master_status(
            &runner,
            PhpFpmMasterStatusRequest {
                username: "p_alice".to_string(),
                version: "8.3".to_string(),
            },
        )
        .unwrap();

        assert_eq!(status.service_name, "panel-fpm@p_alice-8.3.service");
        assert!(status.is_active);

        let reload_res = reload_php_fpm_master(
            &runner,
            PhpFpmMasterReloadRequest {
                username: "p_alice".to_string(),
                version: "8.3".to_string(),
            },
        )
        .unwrap();
        assert!(reload_res.success);

        let restart_res = restart_php_fpm_master(
            &runner,
            PhpFpmMasterRestartRequest {
                username: "p_alice".to_string(),
                version: "8.3".to_string(),
            },
        )
        .unwrap();
        assert!(restart_res.success);

        let commands = runner.get_commands();
        assert!(
            commands
                .iter()
                .any(|c| c.contains("systemctl reload panel-fpm@p_alice-8.3.service"))
        );
        assert!(
            commands
                .iter()
                .any(|c| c.contains("systemctl restart panel-fpm@p_alice-8.3.service"))
        );
    }
}
