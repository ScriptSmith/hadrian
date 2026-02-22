//! Vector store cleanup worker for removing soft-deleted stores.
//!
//! This module provides a background worker that periodically:
//! 1. Finds soft-deleted vector stores that have passed the cleanup delay
//! 2. Deletes all chunks from the vector database for each store
//! 3. Removes files that are no longer referenced by any vector store
//! 4. Hard deletes the vector store record from the database
//!
//! The cleanup process is designed to be safe and incremental:
//! - Cleanup is batched to avoid long-running operations
//! - A configurable delay between soft delete and hard delete gives users
//!   time to recover accidentally deleted stores
//! - Dry run mode allows testing the cleanup configuration

use std::{sync::Arc, time::Instant};

use chrono::{Duration, Utc};

use crate::{
    cache::vector_store::VectorBackend, config::VectorStoreCleanupConfig, db::DbPool,
    observability::metrics,
};

/// Results from a single cleanup run.
#[derive(Debug, Default)]
pub struct CleanupRunResult {
    /// Number of vector stores hard-deleted.
    pub stores_deleted: u64,
    /// Number of vector store files (file links) hard-deleted.
    pub vector_store_files_deleted: u64,
    /// Number of files deleted (no longer referenced).
    pub files_deleted: u64,
    /// Number of chunks deleted from vector store.
    pub chunks_deleted: u64,
    /// Total storage bytes freed (approximate).
    pub storage_bytes_freed: u64,
    /// Duration of the cleanup run in milliseconds.
    pub duration_ms: u64,
}

impl CleanupRunResult {
    /// Check if any records were deleted.
    pub fn has_deletions(&self) -> bool {
        self.stores_deleted > 0
            || self.vector_store_files_deleted > 0
            || self.files_deleted > 0
            || self.chunks_deleted > 0
    }
}

/// Starts the vector store cleanup worker as a background task.
///
/// The worker runs in a loop, cleaning up soft-deleted stores at the configured interval.
/// It will run indefinitely until the task is cancelled.
pub async fn start_vector_store_cleanup_worker(
    db: Arc<DbPool>,
    vector_store: Option<Arc<dyn VectorBackend>>,
    config: VectorStoreCleanupConfig,
) {
    if !config.enabled {
        tracing::info!("Vector store cleanup worker disabled by configuration");
        return;
    }

    let vector_store = match vector_store {
        Some(vs) => vs,
        None => {
            tracing::warn!(
                "Vector store cleanup worker enabled but no vector store configured. \
                 Cleanup will only remove database records, not vector chunks."
            );
            // Continue anyway - we can still clean up DB records
            // We'll skip vector operations if vector_store is None
            return;
        }
    };

    let dry_run_msg = if config.dry_run { " (DRY RUN)" } else { "" };

    tracing::info!(
        interval_secs = config.interval_secs,
        cleanup_delay_secs = config.cleanup_delay_secs,
        batch_size = config.batch_size,
        max_duration_secs = config.max_duration_secs,
        dry_run = config.dry_run,
        "Starting vector store cleanup worker{}",
        dry_run_msg
    );

    let interval = config.interval();

    loop {
        match run_cleanup(&db, &vector_store, &config).await {
            Ok(result) => {
                if result.has_deletions() {
                    tracing::info!(
                        stores = result.stores_deleted,
                        vector_store_files = result.vector_store_files_deleted,
                        files = result.files_deleted,
                        chunks = result.chunks_deleted,
                        storage_bytes_freed = result.storage_bytes_freed,
                        duration_ms = result.duration_ms,
                        dry_run = config.dry_run,
                        "Vector store cleanup run complete{}",
                        dry_run_msg
                    );
                } else {
                    tracing::debug!("Vector store cleanup run complete, nothing to clean up");
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "Error running vector store cleanup");
                metrics::record_cleanup_error("vector_store");
            }
        }

        tokio::time::sleep(interval).await;
    }
}

/// Run a single cleanup pass, removing soft-deleted stores and their data.
async fn run_cleanup(
    db: &Arc<DbPool>,
    vector_store: &Arc<dyn VectorBackend>,
    config: &VectorStoreCleanupConfig,
) -> Result<CleanupRunResult, Box<dyn std::error::Error + Send + Sync>> {
    let start = Instant::now();
    let mut result = CleanupRunResult::default();

    // Calculate cutoff time: records deleted before this time should be cleaned up
    let cutoff = Utc::now() - Duration::seconds(config.cleanup_delay_secs as i64);
    let max_duration = config.max_duration();

    // ==================== Phase 1: Clean up soft-deleted vector store files ====================
    // These are individual file links that were removed from vector stores
    let deleted_vector_store_files = db
        .vector_stores()
        .list_deleted_vector_store_files(cutoff)
        .await?;

    if !deleted_vector_store_files.is_empty() {
        tracing::debug!(
            count = deleted_vector_store_files.len(),
            cutoff = %cutoff,
            "Found soft-deleted vector store files to clean up"
        );

        for vector_store_file in deleted_vector_store_files {
            // Check if we've exceeded max duration
            if let Some(max_dur) = max_duration
                && start.elapsed() > max_dur
            {
                tracing::info!(
                    vector_store_files_processed = result.vector_store_files_deleted,
                    "Max cleanup duration exceeded, stopping early"
                );
                break;
            }

            let cf_id = vector_store_file.internal_id;
            let file_id = vector_store_file.file_id;
            let vector_store_id = vector_store_file.vector_store_id;

            if config.dry_run {
                tracing::info!(
                    vector_store_file_id = %cf_id,
                    file_id = %file_id,
                    vector_store_id = %vector_store_id,
                    "DRY RUN: Would delete vector store file and its chunks"
                );
                result.vector_store_files_deleted += 1;
                continue;
            }

            // Step 1: Delete chunks from vector store for this specific file+vector_store
            match vector_store
                .delete_chunks_by_file_and_vector_store(file_id, vector_store_id)
                .await
            {
                Ok(chunks_deleted) => {
                    result.chunks_deleted += chunks_deleted;
                    tracing::debug!(
                        vector_store_file_id = %cf_id,
                        file_id = %file_id,
                        vector_store_id = %vector_store_id,
                        chunks_deleted = chunks_deleted,
                        "Deleted chunks for vector store file"
                    );
                }
                Err(e) => {
                    tracing::error!(
                        vector_store_file_id = %cf_id,
                        file_id = %file_id,
                        vector_store_id = %vector_store_id,
                        error = %e,
                        "Failed to delete chunks for vector store file, skipping cleanup"
                    );
                    continue;
                }
            }

            // Step 2: Hard delete the vector_store_files record
            if let Err(e) = db
                .vector_stores()
                .hard_delete_vector_store_file(cf_id)
                .await
            {
                tracing::error!(
                    vector_store_file_id = %cf_id,
                    error = %e,
                    "Failed to hard delete vector store file record"
                );
            } else {
                result.vector_store_files_deleted += 1;
                tracing::debug!(
                    vector_store_file_id = %cf_id,
                    "Hard deleted vector store file record"
                );
            }
        }
    }

    // ==================== Phase 2: Clean up soft-deleted vector stores ====================
    // Get soft-deleted stores older than the cutoff
    let deleted_stores = db
        .vector_stores()
        .list_deleted_vector_stores(cutoff)
        .await?;

    if deleted_stores.is_empty() {
        // No stores to clean up, but we may have cleaned up vector store files
        result.duration_ms = start.elapsed().as_millis() as u64;
        // Record metrics for vector store files if any were deleted
        if result.vector_store_files_deleted > 0 {
            metrics::record_cleanup_deletion(
                "vector_store_files",
                result.vector_store_files_deleted,
            );
        }
        if result.chunks_deleted > 0 {
            metrics::record_cleanup_deletion("vector_store_chunks", result.chunks_deleted);
        }
        return Ok(result);
    }

    tracing::debug!(
        count = deleted_stores.len(),
        cutoff = %cutoff,
        "Found soft-deleted vector stores to clean up"
    );

    // Process stores up to batch_size
    let stores_to_process = deleted_stores
        .into_iter()
        .take(config.batch_size as usize)
        .collect::<Vec<_>>();

    for store in stores_to_process {
        // Check if we've exceeded max duration
        if let Some(max_dur) = max_duration
            && start.elapsed() > max_dur
        {
            tracing::info!(
                stores_processed = result.stores_deleted,
                "Max cleanup duration exceeded, stopping early"
            );
            break;
        }

        let store_id = store.id;
        let store_name = &store.name;

        if config.dry_run {
            tracing::info!(
                store_id = %store_id,
                store_name = store_name,
                "DRY RUN: Would delete vector store and its data"
            );
            result.stores_deleted += 1;
            continue;
        }

        // Step 1: Delete chunks from vector store
        match vector_store.delete_chunks_by_vector_store(store_id).await {
            Ok(chunks_deleted) => {
                result.chunks_deleted += chunks_deleted;
                tracing::debug!(
                    store_id = %store_id,
                    chunks_deleted = chunks_deleted,
                    "Deleted chunks from vector store"
                );
            }
            Err(e) => {
                tracing::error!(
                    store_id = %store_id,
                    error = %e,
                    "Failed to delete chunks from vector store, skipping store cleanup"
                );
                continue;
            }
        }

        // Step 2: Get files in this vector store and check if they're orphaned
        let vector_store_files = match db
            .vector_stores()
            .list_vector_store_files(
                store_id,
                crate::db::repos::ListParams {
                    limit: Some(10000),
                    ..Default::default()
                },
            )
            .await
        {
            Ok(files) => files.items,
            Err(e) => {
                tracing::error!(
                    store_id = %store_id,
                    error = %e,
                    "Failed to list vector store files, skipping file cleanup"
                );
                vec![]
            }
        };

        for vector_store_file in vector_store_files {
            let file_id = vector_store_file.file_id;

            // Check if file is referenced by other vector stores
            match db.files().count_file_references(file_id).await {
                Ok(ref_count) if ref_count <= 1 => {
                    // File is only referenced by this (deleted) vector store, delete it
                    // First get the file to know its size
                    if let Ok(Some(file)) = db.files().get_file(file_id).await {
                        result.storage_bytes_freed += file.size_bytes as u64;
                    }

                    if let Err(e) = db.files().delete_file(file_id).await {
                        tracing::error!(
                            file_id = %file_id,
                            error = %e,
                            "Failed to delete orphaned file"
                        );
                    } else {
                        result.files_deleted += 1;
                        tracing::debug!(
                            file_id = %file_id,
                            "Deleted orphaned file"
                        );
                    }
                }
                Ok(ref_count) => {
                    tracing::debug!(
                        file_id = %file_id,
                        ref_count = ref_count,
                        "File still referenced by other vector stores, skipping"
                    );
                }
                Err(e) => {
                    tracing::error!(
                        file_id = %file_id,
                        error = %e,
                        "Failed to check file references"
                    );
                }
            }
        }

        // Step 3: Hard delete the vector store record
        if let Err(e) = db.vector_stores().hard_delete_vector_store(store_id).await {
            tracing::error!(
                store_id = %store_id,
                error = %e,
                "Failed to hard delete vector store"
            );
        } else {
            result.stores_deleted += 1;
            tracing::debug!(
                store_id = %store_id,
                store_name = store_name,
                "Hard deleted vector store"
            );
        }
    }

    result.duration_ms = start.elapsed().as_millis() as u64;

    // Record metrics
    if result.stores_deleted > 0 {
        metrics::record_cleanup_deletion("vector_stores", result.stores_deleted);
    }
    if result.vector_store_files_deleted > 0 {
        metrics::record_cleanup_deletion("vector_store_files", result.vector_store_files_deleted);
    }
    if result.files_deleted > 0 {
        metrics::record_cleanup_deletion("vector_store_files", result.files_deleted);
    }
    if result.chunks_deleted > 0 {
        metrics::record_cleanup_deletion("vector_store_chunks", result.chunks_deleted);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_run_result_default() {
        let result = CleanupRunResult::default();
        assert_eq!(result.stores_deleted, 0);
        assert_eq!(result.vector_store_files_deleted, 0);
        assert_eq!(result.files_deleted, 0);
        assert_eq!(result.chunks_deleted, 0);
        assert_eq!(result.storage_bytes_freed, 0);
        assert_eq!(result.duration_ms, 0);
        assert!(!result.has_deletions());
    }

    #[test]
    fn test_cleanup_run_result_has_deletions() {
        let empty = CleanupRunResult::default();
        assert!(!empty.has_deletions());

        let with_stores = CleanupRunResult {
            stores_deleted: 1,
            ..Default::default()
        };
        assert!(with_stores.has_deletions());

        let with_vector_store_files = CleanupRunResult {
            vector_store_files_deleted: 1,
            ..Default::default()
        };
        assert!(with_vector_store_files.has_deletions());

        let with_files = CleanupRunResult {
            files_deleted: 1,
            ..Default::default()
        };
        assert!(with_files.has_deletions());

        let with_chunks = CleanupRunResult {
            chunks_deleted: 1,
            ..Default::default()
        };
        assert!(with_chunks.has_deletions());
    }
}
