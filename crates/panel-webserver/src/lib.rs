//! `panel-webserver`: Nginx template renderer, atomic apply, and rollback mechanisms.

pub mod baseline;
pub mod error;

pub use baseline::NginxBaselineConfig;
pub use error::WebserverError;
