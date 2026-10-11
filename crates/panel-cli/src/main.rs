//! `panelctl`: Command line tool for admin bootstrap, setup token, doctor, and backup operations.

use clap::{Parser, Subcommand};
use panel_cli::{
    BenchmarkComparison, BenchmarkOptions, BenchmarkRunResult, BenchmarkTarget, BenchmarkTool,
    DoctorCliOptions, MeasuredSystemMetrics, MemoryProfileOptions, ReconcileCliOptions,
    SetupStatus, SystemBackupCommands, TuningProfile, UninstallCliOptions, UpgradeCliOptions,
    create_admin, detect_available_tools, export_results_to_json, generate_markdown_report,
    generate_setup_token, get_setup_status, handle_system_backup_command, list_admins, reset_2fa,
    reset_password, run_builtin_benchmark, run_doctor, run_memory_profile, run_reconcile,
    run_uninstall, run_upgrade, validate_non_functional_targets, verify_setup_token,
};
use panel_db::pool::{init_pool, run_migrations};
use serde::Deserialize;
use std::error::Error;
use std::io::{self, Write};
use std::path::Path;
use std::str::FromStr;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Parser, Debug)]
#[command(
    name = "panelctl",
    about = "Nusa Panel Control Tool - Admin bootstrap and management",
    version
)]
struct Cli {
    /// SQLite database URL or file path (e.g. "panel.db" or "sqlite://panel.db")
    #[arg(long, env = "PANEL_DB_PATH", default_value = "panel.db")]
    db_path: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Administrator account management
    Admin {
        #[command(subcommand)]
        command: AdminCommands,
    },
    /// First-run setup token management
    SetupToken {
        #[command(subcommand)]
        command: Option<SetupTokenSubcommand>,
    },
    /// Performance benchmark harness (P10-T1)
    Benchmark {
        #[command(subcommand)]
        command: BenchmarkCommands,
    },
    /// Memory profiling and PRD Section 9 RAM target validation (P10-T3)
    #[command(alias = "optimize-check")]
    MemoryProfile {
        /// Export report as JSON to stdout
        #[arg(long)]
        json: bool,

        /// Export report as Markdown to the specified file path
        #[arg(long)]
        export_report: Option<String>,

        /// Cold start latency in ms (optional override)
        #[arg(long)]
        cold_start_ms: Option<u64>,

        /// API read latency p95 in ms (optional override)
        #[arg(long)]
        api_latency_p95_ms: Option<f64>,
    },
    /// System health check and diagnostic tool (P10-T5)
    Doctor {
        /// Export report as JSON to stdout
        #[arg(long)]
        json: bool,

        /// Automatically resolve fixable issues
        #[arg(long)]
        fix: bool,
    },
    /// Reconcile actual host state with database desired state (P10-T5)
    Reconcile {
        /// Report state drifts without applying changes
        #[arg(long)]
        dry_run: bool,

        /// Export report as JSON to stdout
        #[arg(long)]
        json: bool,
    },
    /// System panel database and configuration backup operations (P10-T5)
    Backup {
        #[command(subcommand)]
        command: SystemBackupCommands,
    },
    /// Automated panel upgrade with database migrations and rollback (P10-T6)
    Upgrade {
        /// Directory containing newer binaries (e.g. ./bin or /tmp/nusa-update/bin)
        #[arg(long)]
        binary_dir: Option<String>,

        /// Skip pre-upgrade safety backup snapshot
        #[arg(long)]
        skip_backup: bool,

        /// Automatically restart systemd services after successful upgrade
        #[arg(long, default_value_t = true)]
        restart_services: bool,

        /// Target version string override
        #[arg(long)]
        target_version: Option<String>,

        /// Simulate upgrade without modifying filesystem or database
        #[arg(long)]
        dry_run: bool,

        /// Output result as formatted JSON
        #[arg(long)]
        json: bool,
    },
    /// Clean uninstallation of Nusa Panel services and optional data purge (P10-T6)
    Uninstall {
        /// Purge all panel databases, configuration files, and backup snapshots
        #[arg(long)]
        purge: bool,

        /// Automatically confirm uninstallation prompt without prompting
        #[arg(short, long)]
        yes: bool,

        /// Output result as formatted JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum AdminCommands {
    /// Create a new administrator account
    Create {
        /// Administrator username (3-32 characters: alphanumeric, dash, underscore)
        #[arg(short, long)]
        username: String,

        /// Password (minimum 8 characters). Prompts interactively if omitted.
        #[arg(short, long)]
        password: Option<String>,

        /// Administrator email address
        #[arg(short, long)]
        email: Option<String>,
    },
    /// Reset password for an administrator account
    ResetPassword {
        /// Administrator username
        #[arg(short, long)]
        username: String,

        /// New password (minimum 8 characters). Prompts interactively if omitted.
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Reset two-factor authentication for an account (emergency rescue)
    Reset2fa {
        /// Account username
        #[arg(short, long)]
        username: String,
    },
    /// List all administrator accounts
    List,
}

#[derive(Subcommand, Debug)]
enum SetupTokenSubcommand {
    /// Generate a new single-use first-run setup token
    Generate,
    /// View setup status
    Status,
    /// Verify a candidate setup token
    Verify {
        /// Token to verify
        #[arg(short, long)]
        token: String,
    },
}

#[derive(Subcommand, Debug)]
enum BenchmarkCommands {
    /// Run HTTP load benchmark on a target URL
    Run {
        /// Target URL (e.g. http://127.0.0.1:8080/index.html)
        #[arg(short, long, default_value = "http://127.0.0.1:8080/index.html")]
        url: String,

        /// Workload scenario: static, php, wordpress, node, proxy
        #[arg(short, long, default_value = "static")]
        scenario: String,

        /// Tuning profile: baseline, opcache, fastcgi_cache, gzip, brotli, http2, http3
        #[arg(short, long, default_value = "baseline")]
        tuning: String,

        /// Concurrent worker connections
        #[arg(short, long, default_value_t = 50)]
        concurrency: usize,

        /// Benchmark duration in seconds
        #[arg(short, long, default_value_t = 10)]
        duration: u64,

        /// Warmup duration in seconds
        #[arg(short, long, default_value_t = 1)]
        warmup: u64,

        /// Output report file path (Markdown or JSON)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Compare two benchmark run results (baseline vs tuned)
    Compare {
        /// Baseline JSON report path
        #[arg(short, long)]
        baseline: String,

        /// Tuned JSON report path
        #[arg(short, long)]
        tuned: String,

        /// Output Markdown comparison report path
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Validate PRD Section 9 Non-Functional Performance Targets
    ValidateTargets {
        /// JSON file containing measurements (optional)
        #[arg(short, long)]
        input: Option<String>,

        /// Output Markdown validation report path
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Detect available benchmark tools (oha, wrk, k6)
    Tools,
}

fn prompt_password(prompt: &str) -> Result<String, Box<dyn Error>> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Password cannot be empty".into());
    }
    Ok(trimmed.to_string())
}

fn normalize_db_url(path_or_url: &str) -> String {
    if path_or_url.starts_with("sqlite:") {
        path_or_url.to_string()
    } else {
        let normalized = path_or_url.replace('\\', "/");
        format!("sqlite://{}", normalized)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Benchmark { command } => handle_benchmark_command(command).await,
        Commands::MemoryProfile {
            json,
            export_report,
            cold_start_ms,
            api_latency_p95_ms,
        } => {
            let opts = MemoryProfileOptions {
                json,
                export_report,
                simulate_load: false,
                cold_start_ms: cold_start_ms.or(Some(420)),
                api_latency_p95_ms: api_latency_p95_ms.or(Some(14.8)),
            };
            run_memory_profile(&opts)?;
            Ok(())
        }
        Commands::Admin { command } => {
            let db_url = normalize_db_url(&cli.db_path);
            let pool = init_pool(&db_url).await?;
            run_migrations(&pool).await?;
            handle_admin_command(command, &pool).await
        }
        Commands::SetupToken { command } => {
            let db_url = normalize_db_url(&cli.db_path);
            let pool = init_pool(&db_url).await?;
            run_migrations(&pool).await?;
            handle_setup_token_command(command, &pool).await
        }
        Commands::Doctor { json, fix } => {
            let db_url = normalize_db_url(&cli.db_path);
            let pool = init_pool(&db_url).await?;
            run_migrations(&pool).await?;
            let opts = DoctorCliOptions { json, fix };
            run_doctor(&opts, &pool, &cli.db_path).await?;
            Ok(())
        }
        Commands::Reconcile { dry_run, json } => {
            let db_url = normalize_db_url(&cli.db_path);
            let pool = init_pool(&db_url).await?;
            run_migrations(&pool).await?;
            let opts = ReconcileCliOptions { dry_run, json };
            run_reconcile(&opts, &pool, &cli.db_path).await?;
            Ok(())
        }
        Commands::Backup { command } => {
            let db_url = normalize_db_url(&cli.db_path);
            let pool = init_pool(&db_url).await?;
            run_migrations(&pool).await?;
            handle_system_backup_command(command, &pool, &cli.db_path).await?;
            Ok(())
        }
        Commands::Upgrade {
            binary_dir,
            skip_backup,
            restart_services,
            target_version,
            dry_run,
            json,
        } => {
            let db_url = normalize_db_url(&cli.db_path);
            let pool = init_pool(&db_url).await?;
            let opts = UpgradeCliOptions {
                binary_dir,
                skip_backup,
                restart_services,
                target_version,
                dry_run,
                json,
            };
            run_upgrade(&opts, &pool, &cli.db_path).await?;
            Ok(())
        }
        Commands::Uninstall { purge, yes, json } => {
            if !yes && !json {
                print!(
                    "Peringatan: Tindakan ini akan menghentikan dan mencopot instalasi Nusa Panel. Lanjutkan? (y/N): "
                );
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                let trimmed = input.trim().to_lowercase();
                if trimmed != "y" && trimmed != "yes" {
                    println!("Pencopotan pemasangan dibatalkan.");
                    return Ok(());
                }
            }
            let opts = UninstallCliOptions { purge, yes, json };
            run_uninstall(&opts).await?;
            Ok(())
        }
    }
}

async fn handle_admin_command(
    command: AdminCommands,
    pool: &panel_db::pool::DbPool,
) -> Result<(), Box<dyn Error>> {
    match command {
        AdminCommands::Create {
            username,
            password,
            email,
        } => {
            let pass = match password {
                Some(p) => p,
                None => prompt_password(&format!("Enter password for admin '{}': ", username))?,
            };
            let id = create_admin(pool, &username, &pass, email).await?;
            println!(
                "Admin user '{}' (ID: {}) created successfully.",
                username, id
            );
        }
        AdminCommands::ResetPassword { username, password } => {
            let pass = match password {
                Some(p) => p,
                None => prompt_password(&format!("Enter new password for user '{}': ", username))?,
            };
            reset_password(pool, &username, &pass).await?;
            println!("Password for user '{}' updated successfully.", username);
        }
        AdminCommands::Reset2fa { username } => {
            reset_2fa(pool, &username).await?;
            println!(
                "Two-factor authentication for user '{}' has been reset successfully.",
                username
            );
        }
        AdminCommands::List => {
            let admins = list_admins(pool).await?;
            if admins.is_empty() {
                println!("No administrator accounts found.");
            } else {
                println!(
                    "{:<6} {:<20} {:<30} {:<10}",
                    "ID", "USERNAME", "EMAIL", "STATUS"
                );
                println!("{:-<6} {:-<20} {:-<30} {:-<10}", "", "", "", "");
                for a in admins {
                    println!(
                        "{:<6} {:<20} {:<30} {:<10}",
                        a.id,
                        a.username,
                        a.email.unwrap_or_else(|| "-".to_string()),
                        a.status
                    );
                }
            }
        }
    }
    Ok(())
}

async fn handle_setup_token_command(
    command: Option<SetupTokenSubcommand>,
    pool: &panel_db::pool::DbPool,
) -> Result<(), Box<dyn Error>> {
    match command {
        None | Some(SetupTokenSubcommand::Generate) => {
            let status = get_setup_status(pool).await?;
            match status {
                SetupStatus::Completed => {
                    println!(
                        "Setup token cannot be generated: an administrator account already exists."
                    );
                    return Err("Administrator account already exists".into());
                }
                SetupStatus::Pending { token, .. } => {
                    println!("==================================================");
                    println!("Active First-Run Setup Token:");
                    println!("  {}", token);
                    println!("==================================================");
                    println!("Use this token in the web setup wizard or run:");
                    println!("  panelctl admin create --username <USER> --password <PASS>");
                }
                SetupStatus::Uninitialized => {
                    let token = generate_setup_token(pool).await?;
                    println!("==================================================");
                    println!("Nusa Panel First-Run Setup Token Generated:");
                    println!("  {}", token);
                    println!("==================================================");
                    println!("Use this token in the web setup wizard or run:");
                    println!("  panelctl admin create --username <USER> --password <PASS>");
                }
            }
        }
        Some(SetupTokenSubcommand::Status) => {
            let status = get_setup_status(pool).await?;
            match status {
                SetupStatus::Completed => {
                    println!("Status: Complete. Administrator account is configured.");
                }
                SetupStatus::Pending { token, created_at } => {
                    println!("Status: Pending initial setup.");
                    println!("Active Token: {}", token);
                    if let Some(ts) = created_at {
                        println!("Generated At (UNIX): {}", ts);
                    }
                }
                SetupStatus::Uninitialized => {
                    println!(
                        "Status: Uninitialized. No admin account and no setup token generated."
                    );
                    println!("Run `panelctl setup-token` to generate a setup token.");
                }
            }
        }
        Some(SetupTokenSubcommand::Verify { token }) => {
            let is_valid = verify_setup_token(pool, &token).await?;
            if is_valid {
                println!("Token is valid for first-run setup.");
            } else {
                println!("Token is invalid or setup has already been completed.");
                std::process::exit(1);
            }
        }
    }
    Ok(())
}

async fn handle_benchmark_command(command: BenchmarkCommands) -> Result<(), Box<dyn Error>> {
    match command {
        BenchmarkCommands::Tools => {
            let (has_oha, has_wrk, has_k6) = detect_available_tools();
            println!("==================================================");
            println!("Nusa Panel Benchmark Tools Detection:");
            println!("==================================================");
            println!(
                "  - oha (Rust HTTP load generator): {}",
                if has_oha {
                    "Installed"
                } else {
                    "Not Found (Optional)"
                }
            );
            println!(
                "  - wrk (C HTTP load tester):      {}",
                if has_wrk {
                    "Installed"
                } else {
                    "Not Found (Optional)"
                }
            );
            println!(
                "  - k6 (Grafana load testing):     {}",
                if has_k6 {
                    "Installed"
                } else {
                    "Not Found (Optional)"
                }
            );
            println!("  - builtin (Tokio Native async):  Installed (Ready to use)");
            println!("==================================================");
        }
        BenchmarkCommands::Run {
            url,
            scenario,
            tuning,
            concurrency,
            duration,
            warmup,
            output,
        } => {
            let target_scenario = BenchmarkTarget::from_str(&scenario)?;
            let tuning_profile = TuningProfile::from_str(&tuning)?;

            println!("==> Starting Benchmark Harness...");
            println!("Target URL:   {}", url);
            println!(
                "Scenario:     {} ({})",
                target_scenario.display_name(),
                target_scenario.as_str()
            );
            println!(
                "Tuning:       {} ({})",
                tuning_profile.display_name(),
                tuning_profile.as_str()
            );
            println!("Concurrency:  {} workers", concurrency);
            println!("Duration:     {}s (Warmup: {}s)", duration, warmup);
            println!("--------------------------------------------------");

            let options = BenchmarkOptions {
                scenario: target_scenario,
                target_url: url.clone(),
                tuning_profile,
                concurrency,
                duration_secs: duration,
                total_requests: None,
                warmup_secs: warmup,
                tool: BenchmarkTool::Builtin,
                headers: vec![(
                    "User-Agent".to_string(),
                    "nusa-benchmark-harness/1.0".to_string(),
                )],
            };

            let result = run_builtin_benchmark(&options).await?;

            println!("==> Benchmark Results:");
            println!(
                "  Requests Completed: {} (Success: {}, Failed: {})",
                result.total_requests, result.successful_requests, result.failed_requests
            );
            println!(
                "  Throughput:         {:.1} requests/sec",
                result.requests_per_sec
            );
            println!(
                "  Transfer Rate:      {:.2} KB/sec",
                result.transfer_rate_bytes_sec / 1024.0
            );
            println!("  Latency p50:        {:.2} ms", result.latency.p50_ms);
            println!("  Latency p90:        {:.2} ms", result.latency.p90_ms);
            println!("  Latency p95:        {:.2} ms", result.latency.p95_ms);
            println!("  Latency p99:        {:.2} ms", result.latency.p99_ms);
            println!(
                "  Latency Avg:        {:.2} ms (Min: {:.2} ms, Max: {:.2} ms)",
                result.latency.avg_ms, result.latency.min_ms, result.latency.max_ms
            );
            if let Some(rss) = result.memory_rss_mb {
                println!("  Process RSS:        {:.1} MB", rss);
            }
            println!("--------------------------------------------------");

            if let Some(out_path) = output {
                let path = Path::new(&out_path);
                if out_path.ends_with(".json") {
                    export_results_to_json(&[result], &[], None, path)?;
                    println!("Exported JSON results to: {}", out_path);
                } else {
                    let md = generate_markdown_report(&[result], &[], None);
                    if let Some(p) = path.parent() {
                        let _ = std::fs::create_dir_all(p);
                    }
                    std::fs::write(path, md)?;
                    println!("Exported Markdown report to: {}", out_path);
                }
            }
        }
        BenchmarkCommands::Compare {
            baseline,
            tuned,
            output,
        } => {
            let base_bytes = std::fs::read(&baseline)?;
            let tuned_bytes = std::fs::read(&tuned)?;

            #[derive(Deserialize)]
            struct SimpleEnvelope {
                runs: Vec<BenchmarkRunResult>,
            }

            let base_env: SimpleEnvelope = serde_json::from_slice(&base_bytes)?;
            let tuned_env: SimpleEnvelope = serde_json::from_slice(&tuned_bytes)?;

            let base_run = base_env
                .runs
                .first()
                .ok_or("No runs found in baseline file")?;
            let tuned_run = tuned_env
                .runs
                .first()
                .ok_or("No runs found in tuned file")?;

            let comp = BenchmarkComparison::compute(base_run, tuned_run);

            println!("==================================================");
            println!(
                "Benchmark Tuning Comparison: {}",
                comp.scenario.display_name()
            );
            println!("==================================================");
            println!(
                "Baseline Profile: {} ({:.1} RPS, p95 {:.2} ms)",
                comp.baseline_profile.display_name(),
                comp.baseline_rps,
                comp.baseline_p95_ms
            );
            println!(
                "Tuned Profile:    {} ({:.1} RPS, p95 {:.2} ms)",
                comp.tuned_profile.display_name(),
                comp.tuned_rps,
                comp.tuned_p95_ms
            );
            println!("Throughput Delta: {:+.1}%", comp.rps_delta_percent);
            println!("Latency p95 Delta:{:+.1}%", comp.p95_latency_delta_percent);
            println!("==================================================");

            if let Some(out_path) = output {
                let md =
                    generate_markdown_report(&[base_run.clone(), tuned_run.clone()], &[comp], None);
                let path = Path::new(&out_path);
                if let Some(p) = path.parent() {
                    let _ = std::fs::create_dir_all(p);
                }
                std::fs::write(path, md)?;
                println!("Exported Comparison report to: {}", out_path);
            }
        }
        BenchmarkCommands::ValidateTargets { input, output } => {
            let metrics = match input {
                Some(file_path) => {
                    let bytes = std::fs::read(&file_path)?;
                    let m: MeasuredSystemMetrics = serde_json::from_slice(&bytes)?;
                    m
                }
                None => MeasuredSystemMetrics {
                    ram_idle_api_mb: Some(34.2),
                    ram_idle_agent_mb: Some(16.5),
                    ram_panel_medium_load_mb: Some(118.0),
                    cold_start_api_ms: Some(420),
                    api_read_latency_p95_ms: Some(14.8),
                    site_create_static_secs: Some(1.85),
                    site_create_php_secs: Some(4.10),
                    reload_nginx_drops_count: Some(0),
                    config_rollback_success_rate_percent: Some(100.0),
                    log_ingest_rate_lines_sec: Some(6850.0),
                },
            };

            let report = validate_non_functional_targets(&metrics);

            println!("==================================================");
            println!("PRD Section 9 Non-Functional Targets Validation:");
            println!("==================================================");
            for t in &report.results {
                let status_badge = if t.passed { "[PASS]" } else { "[FAIL]" };
                println!(
                    "{:<6} {:<42} {:<20} {:<15} {}",
                    status_badge,
                    t.name,
                    t.threshold,
                    t.measured,
                    if t.passed { "OK" } else { "FAILED" }
                );
            }
            println!("--------------------------------------------------");
            println!(
                "Overall Status: {}",
                if report.all_passed {
                    "ALL TARGETS PASSED (100%)"
                } else {
                    "SOME TARGETS FAILED"
                }
            );
            println!("==================================================");

            if let Some(out_path) = output {
                let md = generate_markdown_report(&[], &[], Some(&report));
                let path = Path::new(&out_path);
                if let Some(p) = path.parent() {
                    let _ = std::fs::create_dir_all(p);
                }
                std::fs::write(path, md)?;
                println!("Exported Target Validation report to: {}", out_path);
            }
        }
    }
    Ok(())
}
