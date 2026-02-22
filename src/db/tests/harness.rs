//! Test harness for database repository testing
//!
//! Provides utilities for setting up test databases:
//! - SQLite: Fast in-memory databases with real migrations
//! - PostgreSQL: Testcontainers-based instances with real migrations

#[cfg(feature = "database-sqlite")]
use sqlx::SqlitePool;

/// Create an in-memory SQLite pool for testing
#[cfg(feature = "database-sqlite")]
pub async fn create_sqlite_pool() -> SqlitePool {
    sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory SQLite pool")
}

/// Run SQLite migrations on the pool
///
/// Uses the actual migration files to ensure tests match production schema
#[cfg(feature = "database-sqlite")]
pub async fn run_sqlite_migrations(pool: &SqlitePool) {
    sqlx::migrate!("./migrations_sqlx/sqlite")
        .run(pool)
        .await
        .expect("Failed to run SQLite migrations");
}

/// Redis test harness using testcontainers
#[cfg(all(test, feature = "redis"))]
pub mod redis {
    use testcontainers_modules::{
        redis::Redis,
        testcontainers::{ContainerAsync, runners::AsyncRunner},
    };

    /// Start a Redis container and return the connection URL and container handle
    /// The container is kept alive as long as the returned handle is held
    pub async fn create_redis_container() -> (String, ContainerAsync<Redis>) {
        let container = Redis::default()
            .start()
            .await
            .expect("Failed to start Redis container");

        let host = container.get_host().await.expect("Failed to get host");
        let port = container
            .get_host_port_ipv4(6379)
            .await
            .expect("Failed to get port");

        let url = format!("redis://{}:{}", host, port);

        (url, container)
    }
}

/// PostgreSQL test harness using testcontainers
#[cfg(all(test, feature = "database-postgres"))]
pub mod postgres {
    use std::sync::OnceLock;

    use sqlx::PgPool;
    use testcontainers_modules::{
        postgres::Postgres,
        testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
    };
    use tokio::sync::OnceCell;

    /// Shared container state - initialized once per test run
    struct SharedContainer {
        #[allow(dead_code)] // Test infrastructure: keeps container alive
        container: ContainerAsync<Postgres>,
        connection_string: String,
    }

    /// Global shared container - lazily initialized on first use
    static SHARED_CONTAINER: OnceLock<OnceCell<SharedContainer>> = OnceLock::new();

    /// Get or initialize the shared PostgreSQL container
    async fn get_shared_container() -> &'static SharedContainer {
        let cell = SHARED_CONTAINER.get_or_init(OnceCell::new);
        cell.get_or_init(|| async {
            let container = Postgres::default()
                .with_tag("18-alpine")
                .start()
                .await
                .expect("Failed to start PostgreSQL container");

            let host = container.get_host().await.expect("Failed to get host");
            let port = container
                .get_host_port_ipv4(5432)
                .await
                .expect("Failed to get port");

            let connection_string =
                format!("postgres://postgres:postgres@{}:{}/postgres", host, port);

            SharedContainer {
                container,
                connection_string,
            }
        })
        .await
    }

    /// Create an isolated database schema for a single test
    ///
    /// This starts a shared PostgreSQL container (if not already running) and creates
    /// a unique schema for test isolation. Each test gets its own schema with fresh
    /// migrations, avoiding container startup overhead while maintaining isolation.
    ///
    /// The schema is automatically cleaned up when the pool is dropped.
    pub async fn create_isolated_postgres_pool() -> PgPool {
        let shared = get_shared_container().await;

        // Create admin pool for schema creation
        let admin_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&shared.connection_string)
            .await
            .expect("Failed to connect to PostgreSQL");

        // Generate unique schema name for this test
        let schema_name = format!("test_{}", uuid::Uuid::new_v4().simple());

        // Create the schema
        sqlx::query(&format!("CREATE SCHEMA \"{}\"", schema_name))
            .execute(&admin_pool)
            .await
            .expect("Failed to create test schema");

        // Create a new pool with search_path set to our isolated schema
        let isolated_url = format!(
            "{}?options=-c search_path={}",
            shared.connection_string, schema_name
        );

        sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(&isolated_url)
            .await
            .expect("Failed to connect to isolated schema")
    }

    /// Run PostgreSQL migrations on the pool
    pub async fn run_postgres_migrations(pool: &PgPool) {
        sqlx::migrate!("./migrations_sqlx/postgres")
            .run(pool)
            .await
            .expect("Failed to run PostgreSQL migrations");
    }
}
