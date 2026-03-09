//! Database layer for the Selean design platform.
//!
//! Provides `PostgreSQL`-backed persistence for users, workspaces, documents,
//! assets, and sessions via sqlx. Migrations are embedded in the binary and
//! run automatically on pool creation.

pub mod models;
pub mod queries;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// Configuration for connecting to the database.
#[derive(Debug, Clone)]
pub struct DbConfig {
    /// `PostgreSQL` connection URL (e.g. `postgres://user:pass@host/db`).
    pub database_url: String,
    /// Maximum number of connections in the pool.
    pub max_connections: u32,
}

impl DbConfig {
    /// Reads database configuration from environment variables.
    ///
    /// # Required
    /// - `DATABASE_URL`: `PostgreSQL` connection string
    ///
    /// # Optional
    /// - `DATABASE_MAX_CONNECTIONS`: pool size (default 10)
    ///
    /// # Errors
    ///
    /// Returns an error if `DATABASE_URL` is not set.
    pub fn from_env() -> Result<Self, DbError> {
        let database_url =
            std::env::var("DATABASE_URL").map_err(|_| DbError::MissingDatabaseUrl)?;

        let max_connections: u32 = std::env::var("DATABASE_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);

        Ok(Self {
            database_url,
            max_connections,
        })
    }
}

/// Creates a connection pool and runs all pending migrations.
///
/// # Errors
///
/// Returns an error if the connection fails or migrations fail.
pub async fn create_pool(config: &DbConfig) -> Result<PgPool, DbError> {
    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .connect(&config.database_url)
        .await
        .map_err(DbError::Connection)?;

    tracing::info!(
        "database connected (max_connections={})",
        config.max_connections
    );

    run_migrations(&pool).await?;

    Ok(pool)
}

/// Runs embedded SQL migrations in order.
///
/// # Errors
///
/// Returns an error if any migration fails.
pub async fn run_migrations(pool: &PgPool) -> Result<(), DbError> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(DbError::Migration)?;

    tracing::info!("database migrations applied");
    Ok(())
}

/// Errors from the database layer.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// `DATABASE_URL` environment variable is not set.
    #[error("DATABASE_URL environment variable not set")]
    MissingDatabaseUrl,

    /// Failed to connect to the database.
    #[error("database connection failed: {0}")]
    Connection(sqlx::Error),

    /// A migration failed.
    #[error("migration failed: {0}")]
    Migration(sqlx::migrate::MigrateError),

    /// A query failed.
    #[error("query failed: {0}")]
    Query(#[from] sqlx::Error),

    /// A row was not found.
    #[error("{0} not found")]
    NotFound(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_config_defaults() {
        // Cannot test from_env without setting env vars, but verify the struct.
        let config = DbConfig {
            database_url: "postgres://localhost/test".to_string(),
            max_connections: 5,
        };
        assert_eq!(config.max_connections, 5);
        assert!(config.database_url.contains("localhost"));
    }

    #[test]
    fn db_error_display() {
        let err = DbError::MissingDatabaseUrl;
        assert!(err.to_string().contains("DATABASE_URL"));

        let err = DbError::NotFound("user".to_string());
        assert!(err.to_string().contains("user not found"));
    }
}
