//! `panel-cli` library for administrative operations and `panelctl` CLI tool.

pub mod commands;
pub mod error;

pub use commands::admin::{
    create_admin, list_admins, reset_2fa, reset_password, validate_admin_username,
};
pub use commands::benchmark::{
    BenchmarkComparison, BenchmarkOptions, BenchmarkRunResult, BenchmarkTarget, BenchmarkTool,
    LatencyDistribution, MeasuredSystemMetrics, NonFunctionalTargetsReport, TargetMetricStatus,
    TuningProfile, detect_available_tools, export_results_to_json, generate_markdown_report,
    run_builtin_benchmark, validate_non_functional_targets,
};
pub use commands::setup_token::{
    SETTING_SETUP_TOKEN, SETTING_SETUP_TOKEN_CREATED_AT, SetupStatus, generate_setup_token,
    get_setup_status, verify_setup_token,
};
pub use error::CliError;
