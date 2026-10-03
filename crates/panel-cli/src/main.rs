//! `panelctl`: Command line tool for admin, doctor, backup, and system management.

use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();
    println!("Nusa Panel Control Tool (panelctl) v0.1.0");
    println!("Use `panelctl --help` for available commands.");
    Ok(())
}
