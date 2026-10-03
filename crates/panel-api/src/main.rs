//! `panel-api`: Binary entrypoint.

use panel_api::agent_client::AgentClient;
use panel_api::config::PanelConfig;
use panel_api::create_router;
use panel_api::routes::AppState;
use std::sync::Arc;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .json()
        .init();

    info!("Starting Nusa Panel API daemon...");

    let config = PanelConfig::load(None).unwrap_or_else(|e| {
        error!("Error loading configuration: {}, using defaults", e);
        PanelConfig::default()
    });

    info!(
        "Server listening on {}, Agent socket configured at {}",
        config.server.listen_addr,
        config.agent.socket_path.display()
    );

    let db_url = if config.database.url.starts_with("sqlite:") {
        config.database.url.clone()
    } else {
        format!("sqlite://{}", config.database.url.replace('\\', "/"))
    };
    let pool = panel_db::pool::init_pool(&db_url).await?;
    panel_db::pool::run_migrations(&pool).await?;

    let lockout_manager = Arc::new(panel_api::auth::LockoutManager::new(
        config.auth.max_failed_attempts,
        config.auth.lockout_duration_secs,
    ));

    let agent_client = Arc::new(AgentClient::new(config.agent.socket_path.clone()));
    let state = AppState {
        agent_client,
        pool,
        environment: config.server.environment.clone(),
        auth_config: config.auth.clone(),
        lockout_manager,
    };

    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind(&config.server.listen_addr).await?;
    info!(
        "Panel API listening at http://{}",
        config.server.listen_addr
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Panel API shutdown gracefully.");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sig.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, initiating graceful shutdown...");
        }
        _ = terminate => {
            info!("Received terminate signal, initiating graceful shutdown...");
        }
    }
}
