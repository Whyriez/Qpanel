//! `panel-agent`: Binary entrypoint.

use panel_agent::{AgentConfig, AgentServer};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .json()
        .init();

    info!("Starting Nusa Panel Agent daemon...");

    let socket_path = std::env::var("PANEL_AGENT_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/run/panel/agent.sock"));

    let config = AgentConfig {
        socket_path,
        allowed_uid: None,
    };

    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    let server = Arc::new(AgentServer::new(config));

    tokio::select! {
        res = server.run(shutdown_rx) => {
            if let Err(e) = res {
                eprintln!("Agent server error: {e}");
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, terminating agent...");
            let _ = shutdown_tx.send(());
        }
    }

    Ok(())
}
