//! `panel-runtime`: Runtime management for PHP-FPM, Node.js, Python, and Reverse Proxies.

pub mod error;
pub mod php;

pub use error::RuntimeError;
pub use php::{PhpVersion, PhpVersionInfo, is_sury_repo_installed};
