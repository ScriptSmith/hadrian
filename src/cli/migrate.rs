use super::resolve_config_path;
use crate::{config, db, observability};

/// Run database migrations and exit.
///
/// This is useful for:
/// - Kubernetes init containers (run migrations before main container starts)
/// - CI/CD pipelines (run migrations as a separate step)
/// - Manual migration runs
///
/// Exits with code 0 on success, 1 on failure.
pub(crate) async fn run_migrate(explicit_config_path: Option<&str>) {
    // Resolve config path
    let (config_path, _) = match resolve_config_path(explicit_config_path) {
        Ok((path, is_new)) => (path, is_new),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let config = match config::GatewayConfig::from_file(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "Failed to load config from {}: {}",
                config_path.display(),
                e
            );
            std::process::exit(1);
        }
    };

    // Initialize minimal observability for migration logging
    let _tracing_guard =
        observability::init_tracing(&config.observability).expect("Failed to initialize tracing");

    tracing::info!(
        config_file = %config_path.display(),
        "Running database migrations"
    );

    // Validate database is configured
    if config.database.is_none() {
        eprintln!("Error: Database is not configured. Nothing to migrate.");
        std::process::exit(1);
    }

    // Connect to database and run migrations
    match db::DbPool::from_config(&config.database).await {
        Ok(pool) => match pool.run_migrations().await {
            Ok(()) => {
                tracing::info!("Database migrations completed successfully");
                std::process::exit(0);
            }
            Err(e) => {
                tracing::error!(error = %e, "Database migrations failed");
                eprintln!("Error: Database migrations failed: {}", e);
                std::process::exit(1);
            }
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to connect to database");
            eprintln!("Error: Failed to connect to database: {}", e);
            std::process::exit(1);
        }
    }
}
