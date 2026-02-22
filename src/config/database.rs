use serde::{Deserialize, Serialize};

use super::ConfigError;

/// Database configuration.
///
/// The database stores persistent data: API keys, usage logs, budgets,
/// org/project configurations, and dynamic provider credentials.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum DatabaseConfig {
    /// No database. The gateway runs in stateless/local mode.
    /// Only static providers from config are available.
    /// No authentication, tracking, or UI.
    #[default]
    None,

    /// SQLite database. Good for single-node deployments.
    #[cfg(feature = "database-sqlite")]
    Sqlite(SqliteConfig),

    /// PostgreSQL database. Required for multi-node deployments.
    #[cfg(feature = "database-postgres")]
    Postgres(PostgresConfig),
}

impl DatabaseConfig {
    pub fn is_none(&self) -> bool {
        matches!(self, DatabaseConfig::None)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        match self {
            DatabaseConfig::None => Ok(()),
            #[cfg(feature = "database-sqlite")]
            DatabaseConfig::Sqlite(c) => c.validate(),
            #[cfg(feature = "database-postgres")]
            DatabaseConfig::Postgres(c) => c.validate(),
        }
    }
}

/// SQLite configuration.
#[cfg(feature = "database-sqlite")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SqliteConfig {
    /// Path to the SQLite database file.
    /// Use `:memory:` for an in-memory database (testing only).
    pub path: String,

    /// Create the database file if it doesn't exist.
    #[serde(default = "default_true")]
    pub create_if_missing: bool,

    /// Run migrations on startup.
    #[serde(default = "default_true")]
    pub run_migrations: bool,

    /// Enable WAL mode for better concurrency.
    #[serde(default = "default_true")]
    pub wal_mode: bool,

    /// Busy timeout in milliseconds.
    #[serde(default = "default_busy_timeout")]
    pub busy_timeout_ms: u64,

    /// Maximum number of connections in the pool.
    #[serde(default = "default_sqlite_max_connections")]
    pub max_connections: u32,
}

#[cfg(feature = "database-sqlite")]
impl SqliteConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.path.is_empty() {
            return Err(ConfigError::Validation(
                "SQLite path cannot be empty".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(feature = "database-sqlite")]
fn default_busy_timeout() -> u64 {
    5000 // 5 seconds
}

#[cfg(feature = "database-sqlite")]
fn default_sqlite_max_connections() -> u32 {
    5
}

/// PostgreSQL configuration.
#[cfg(feature = "database-postgres")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PostgresConfig {
    /// PostgreSQL connection URL for the primary (write) database.
    /// Format: postgres://user:password@host:port/database
    pub url: String,

    /// Optional read replica URL for read-heavy queries.
    /// When configured, read operations will be routed to this pool.
    /// This can point to a single replica or a load balancer (e.g., PgBouncer)
    /// that distributes across multiple replicas.
    #[serde(default)]
    pub read_url: Option<String>,

    /// Minimum number of connections in each pool.
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,

    /// Maximum number of connections in each pool.
    ///
    /// Default: 20. For high-concurrency deployments, use the formula:
    /// `(cpu_cores × 2) + max_background_jobs`. Background jobs include
    /// document processing, health checks, and usage flushing.
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Connection timeout in seconds.
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_secs: u64,

    /// Idle connection timeout in seconds.
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,

    /// Run migrations on startup.
    #[serde(default = "default_true")]
    pub run_migrations: bool,

    /// SSL mode.
    #[serde(default)]
    pub ssl_mode: PostgresSslMode,
}

#[cfg(feature = "database-postgres")]
impl PostgresConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.url.is_empty() {
            return Err(ConfigError::Validation(
                "PostgreSQL URL cannot be empty".into(),
            ));
        }
        if self.min_connections > self.max_connections {
            return Err(ConfigError::Validation(
                "min_connections cannot exceed max_connections".into(),
            ));
        }
        Ok(())
    }
}

/// PostgreSQL SSL mode.
#[cfg(feature = "database-postgres")]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PostgresSslMode {
    /// No SSL.
    Disable,
    /// Try SSL, fall back to non-SSL.
    #[default]
    Prefer,
    /// Require SSL.
    Require,
    /// Require SSL and verify server certificate.
    VerifyCa,
    /// Require SSL and verify server certificate and hostname.
    VerifyFull,
}

#[cfg(any(feature = "database-sqlite", feature = "database-postgres"))]
fn default_true() -> bool {
    true
}

#[cfg(feature = "database-postgres")]
fn default_min_connections() -> u32 {
    1
}

#[cfg(feature = "database-postgres")]
fn default_max_connections() -> u32 {
    20
}

#[cfg(feature = "database-postgres")]
fn default_connect_timeout() -> u64 {
    10
}

#[cfg(feature = "database-postgres")]
fn default_idle_timeout() -> u64 {
    300
}
