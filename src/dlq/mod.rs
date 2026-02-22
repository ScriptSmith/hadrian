#[cfg(any(feature = "database-sqlite", feature = "database-postgres"))]
mod database;
mod error;
mod file;
#[cfg(feature = "redis")]
mod redis;
pub mod traits;
pub mod worker;

use std::sync::Arc;

#[cfg(any(feature = "database-sqlite", feature = "database-postgres"))]
pub use database::DatabaseDlq;
pub use error::{DlqError, DlqResult};
pub use file::FileDlq;
#[cfg(feature = "redis")]
pub use redis::RedisDlq;
pub use traits::{DeadLetterQueue, DlqCursor, DlqCursorDirection, DlqEntry, DlqListParams};
pub use worker::start_dlq_worker;

use crate::{config::DeadLetterQueueConfig, db::DbPool};

/// Create a dead-letter queue from configuration.
///
/// Returns `None` if no DLQ is configured.
pub async fn create_dlq(
    config: &Option<DeadLetterQueueConfig>,
    db: Option<&Arc<DbPool>>,
) -> DlqResult<Option<Arc<dyn DeadLetterQueue>>> {
    #[cfg(not(any(feature = "database-sqlite", feature = "database-postgres")))]
    let _ = &db;
    let Some(config) = config else {
        return Ok(None);
    };

    let dlq: Arc<dyn DeadLetterQueue> = match config {
        DeadLetterQueueConfig::File {
            path,
            max_file_size_mb,
            max_files,
            ..
        } => Arc::new(FileDlq::new(path, *max_file_size_mb, *max_files).await?),

        #[cfg(feature = "redis")]
        DeadLetterQueueConfig::Redis {
            url,
            key_prefix,
            max_entries,
            ttl_secs,
            ..
        } => Arc::new(RedisDlq::new(url, key_prefix.clone(), *max_entries, *ttl_secs).await?),
        #[cfg(not(feature = "redis"))]
        DeadLetterQueueConfig::Redis { .. } => {
            return Err(DlqError::Internal(
                "Redis DLQ configured but the 'redis' feature is not enabled. \
                Rebuild with: cargo build --features redis"
                    .to_string(),
            ));
        }

        #[cfg(any(feature = "database-sqlite", feature = "database-postgres"))]
        DeadLetterQueueConfig::Database {
            table_name,
            max_entries,
            ttl_secs,
            ..
        } => {
            let db = db.ok_or_else(|| {
                DlqError::Internal(
                    "Database DLQ configured but no database connection available".to_string(),
                )
            })?;
            Arc::new(DatabaseDlq::new(
                db.clone(),
                table_name.clone(),
                *max_entries,
                *ttl_secs,
            ))
        }
        #[cfg(not(any(feature = "database-sqlite", feature = "database-postgres")))]
        DeadLetterQueueConfig::Database { .. } => {
            return Err(DlqError::Internal(
                "Database DLQ configured but no database feature is enabled. \
                Rebuild with: cargo build --features database-sqlite"
                    .to_string(),
            ));
        }
    };

    Ok(Some(dlq))
}
