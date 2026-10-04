//! `panel-runtime`: Runtime management for PHP-FPM, Node.js, Python, and Reverse Proxies.

pub mod error;
pub mod fpm;
pub mod php;

pub use error::RuntimeError;
pub use fpm::{
    DEFAULT_HARDENED_DISABLE_FUNCTIONS, DEFAULT_PHP_CONFIG_BASE, DEFAULT_PHP_LOG_BASE,
    DEFAULT_PHP_RUNTIME_BASE, DEFAULT_PING_PATH, DEFAULT_PM_MAX_CHILDREN, DEFAULT_PM_MAX_REQUESTS,
    DEFAULT_PM_PROCESS_IDLE_TIMEOUT_SECS, DEFAULT_REQUEST_SLOWLOG_TIMEOUT_SECS,
    DEFAULT_STATUS_PATH, PhpFpmMasterConfig, PhpFpmPoolConfig, validate_fpm_identifier,
};
pub use php::{
    CURATED_PHP_EXTENSIONS, PhpExtensionDefinition, PhpExtensionInfo, PhpVersion, PhpVersionInfo,
    find_curated_extension, is_sury_repo_installed,
};
