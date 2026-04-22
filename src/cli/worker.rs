use std::sync::Arc;

use super::resolve_config_path;
use crate::{config, db, init::init_worker_embedding_service, observability, services};

/// Run the file processing worker.
///
/// This worker consumes jobs from a message queue (Redis Streams) and processes
/// files by chunking them and generating embeddings for vector search.
///
/// # Requirements
/// - Queue mode must be configured: `[features.file_processing] mode = "queue"`
/// - Queue backend must be configured: `[features.file_processing.queue]`
/// - Database must be configured for file metadata and chunk storage
pub(crate) async fn run_worker(
    explicit_config_path: Option<&str>,
    consumer_name: Option<String>,
    batch_size: usize,
    block_timeout_ms: u64,
    claim_pending: bool,
    pending_timeout_ms: u64,
) {
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

    // Initialize observability
    let _tracing_guard =
        observability::init_tracing(&config.observability).expect("Failed to initialize tracing");

    tracing::info!(
        config_file = %config_path.display(),
        "Starting File Processing Worker"
    );

    // Validate that queue mode is configured
    if config.features.file_processing.mode != config::FileProcessingMode::Queue {
        eprintln!(
            "Error: File processing must be configured in queue mode.\n\
             Set [features.file_processing] mode = \"queue\" in your config file."
        );
        std::process::exit(1);
    }

    if config.features.file_processing.queue.is_none() {
        eprintln!(
            "Error: Queue backend not configured.\n\
             Add [features.file_processing.queue] section to your config file."
        );
        std::process::exit(1);
    }

    // Initialize database
    #[allow(unreachable_patterns, unreachable_code)]
    let db = match &config.database {
        config::DatabaseConfig::None => {
            eprintln!("Error: Database is required for file processing worker.");
            std::process::exit(1);
        }
        _ => {
            let pool = db::DbPool::from_config(&config.database)
                .await
                .expect("Failed to connect to database");
            pool.run_migrations()
                .await
                .expect("Failed to run migrations");
            Arc::new(pool)
        }
    };

    // Create file storage backend
    let file_storage = services::create_file_storage(&config.storage.files, db.clone())
        .await
        .expect("Failed to initialize file storage");

    // Create services
    let services = services::Services::new(
        db.clone(),
        file_storage,
        config.auth.rbac.max_expression_length,
        config.limits.resource_limits.max_skill_bytes,
    );
    let vector_stores_service = Arc::new(services.vector_stores.clone());

    // Initialize embedding service and vector store for document processing
    let (embedding_service, vector_store) =
        init_worker_embedding_service(&config, db.clone()).await;

    // Build document processor config
    let processor_config: services::DocumentProcessorConfig =
        (&config.features.file_processing).into();

    // Create document processor
    let processor = match services::DocumentProcessor::new(
        db,
        vector_stores_service,
        embedding_service,
        vector_store,
        processor_config,
    ) {
        Ok(p) => Arc::new(p),
        Err(e) => {
            eprintln!("Failed to initialize document processor: {}", e);
            std::process::exit(1);
        }
    };

    // Build worker config
    let worker_config = services::WorkerConfig {
        consumer_name: consumer_name.unwrap_or_else(|| format!("worker-{}", uuid::Uuid::new_v4())),
        batch_size,
        block_timeout_ms,
        idle_interval_secs: 1,
        claim_pending,
        pending_timeout_ms,
    };

    tracing::info!(
        consumer_name = %worker_config.consumer_name,
        batch_size = worker_config.batch_size,
        block_timeout_ms = worker_config.block_timeout_ms,
        claim_pending = worker_config.claim_pending,
        "Worker configuration"
    );

    // Run the worker (blocks until shutdown)
    services::start_file_processing_worker(processor, worker_config).await;
}
