//! `panel-cli` library for administrative operations and `panelctl` CLI tool.

pub mod commands;
pub mod error;

pub use commands::admin::{create_admin, list_admins, reset_password, validate_admin_username};
pub use commands::setup_token::{
    SETTING_SETUP_TOKEN, SETTING_SETUP_TOKEN_CREATED_AT, SetupStatus, generate_setup_token,
    get_setup_status, verify_setup_token,
};
pub use error::CliError;
