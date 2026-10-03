//! Database connection pool initialization and migrations.

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use std::str::FromStr;
use std::time::Duration;
use tracing::info;

use crate::error::DbError;

pub type DbPool = SqlitePool;

/// Initializes SQLite connection pool with WAL mode, foreign keys, and 5-second busy timeout.
pub async fn init_pool(database_url: &str) -> Result<DbPool, DbError> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(16)
        .connect_with(options)
        .await?;

    info!("SQLite database connection pool initialized successfully");
    Ok(pool)
}

/// Runs embedded database migrations.
pub async fn run_migrations(pool: &DbPool) -> Result<(), DbError> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(DbError::Migrate)?;
    info!("Database migrations applied successfully");
    Ok(())
}
