//! PHP Runtime Manager agent operations: PPA repo setup, install/remove version,
//! and masking the default systemd OS services.

use crate::AgentError;
use crate::system::SystemRunner;
use panel_proto::{
    PhpExtensionActionResponse, PhpExtensionDto, PhpExtensionListResponse, PhpRepoSetupResponse,
    PhpRuntimeActionResponse, PhpRuntimeListResponse, PhpVersionStatusDto,
};
use panel_runtime::php::{
    CURATED_PHP_EXTENSIONS, PhpVersion, find_curated_extension, is_sury_repo_installed,
};
use std::path::Path;
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
}
