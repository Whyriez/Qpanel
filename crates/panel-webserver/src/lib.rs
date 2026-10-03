//! `panel-webserver`: Nginx template renderer, atomic apply, and rollback mechanisms.

pub mod baseline;
pub mod error;
pub mod pipeline;
pub mod template;

pub use baseline::NginxBaselineConfig;
pub use error::WebserverError;
pub use pipeline::{ApplyResult, NginxApplyPipeline, ReloadDebouncer};
pub use template::{
    DomainRedirectConfig, HeaderConfig, NginxTemplateRenderer, StaticVhostContext,
    render_placeholder_html,
};
