//! `panelctl`: Command line tool for admin bootstrap, setup token, doctor, and backup operations.

use clap::{Parser, Subcommand};
use panel_cli::{
    SetupStatus, create_admin, generate_setup_token, get_setup_status, list_admins, reset_2fa,
    reset_password, verify_setup_token,
};
use panel_db::pool::{init_pool, run_migrations};
use std::error::Error;
use std::io::{self, Write};

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

    let db_url = normalize_db_url(&cli.db_path);
    let pool = init_pool(&db_url).await?;
    run_migrations(&pool).await?;

    match cli.command {
        Commands::Admin { command } => match command {
            AdminCommands::Create {
                username,
                password,
                email,
            } => {
                let pass = match password {
                    Some(p) => p,
                    None => prompt_password(&format!("Enter password for admin '{}': ", username))?,
                };
                let id = create_admin(&pool, &username, &pass, email).await?;
                println!(
                    "Admin user '{}' (ID: {}) created successfully.",
                    username, id
                );
            }
            AdminCommands::ResetPassword { username, password } => {
                let pass = match password {
                    Some(p) => p,
                    None => {
                        prompt_password(&format!("Enter new password for user '{}': ", username))?
                    }
                };
                reset_password(&pool, &username, &pass).await?;
                println!("Password for user '{}' updated successfully.", username);
            }
            AdminCommands::Reset2fa { username } => {
                reset_2fa(&pool, &username).await?;
                println!(
                    "Two-factor authentication for user '{}' has been reset successfully.",
                    username
                );
            }
            AdminCommands::List => {
                let admins = list_admins(&pool).await?;
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
        },
        Commands::SetupToken { command } => match command {
            None | Some(SetupTokenSubcommand::Generate) => {
                let status = get_setup_status(&pool).await?;
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
                        let token = generate_setup_token(&pool).await?;
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
                let status = get_setup_status(&pool).await?;
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
                let is_valid = verify_setup_token(&pool, &token).await?;
                if is_valid {
                    println!("Token is valid for first-run setup.");
                } else {
                    println!("Token is invalid or setup has already been completed.");
                    std::process::exit(1);
                }
            }
        },
    }

    Ok(())
}
