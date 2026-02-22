use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::{
    error::{DlqError, DlqResult},
    traits::{DeadLetterQueue, DlqCursorDirection, DlqEntry, DlqListParams, DlqListResult},
};

/// File-based dead-letter queue implementation.
///
/// Stores entries as JSON files in a directory structure.
/// Uses an in-memory index for fast lookups.
pub struct FileDlq {
    /// Base directory for DLQ files.
    path: PathBuf,
    /// Maximum file size in bytes before rotation (reserved for future use).
    #[allow(dead_code)] // Set via constructor; reserved for file rotation logic
    max_file_size: u64,
    /// Maximum number of files to keep.
    max_files: u32,
    /// In-memory index of entries (id -> entry).
    index: Arc<RwLock<HashMap<Uuid, DlqEntry>>>,
}

impl FileDlq {
    /// Create a new file-based DLQ.
    pub async fn new(
        path: impl AsRef<Path>,
        max_file_size_mb: u64,
        max_files: u32,
    ) -> DlqResult<Self> {
        let path = path.as_ref().to_path_buf();

        // Create directory if it doesn't exist
        tokio::fs::create_dir_all(&path).await?;

        let dlq = Self {
            path,
            max_file_size: max_file_size_mb * 1024 * 1024,
            max_files,
            index: Arc::new(RwLock::new(HashMap::new())),
        };

        // Load existing entries from disk
        dlq.load_from_disk().await?;

        Ok(dlq)
    }

    /// Load existing entries from disk into the index.
    async fn load_from_disk(&self) -> DlqResult<()> {
        let mut entries = tokio::fs::read_dir(&self.path).await?;
        let mut index = self.index.write().await;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                match self.load_entry_file(&path).await {
                    Ok(dlq_entry) => {
                        index.insert(dlq_entry.id, dlq_entry);
                    }
                    Err(e) => {
                        tracing::warn!(path = ?path, error = %e, "Failed to load DLQ entry file");
                    }
                }
            }
        }

        tracing::info!(
            path = ?self.path,
            entries = index.len(),
            "Loaded DLQ entries from disk"
        );

        Ok(())
    }

    /// Load a single entry from a file.
    async fn load_entry_file(&self, path: &Path) -> DlqResult<DlqEntry> {
        let contents = tokio::fs::read_to_string(path).await?;
        serde_json::from_str(&contents).map_err(|e| DlqError::Deserialization(e.to_string()))
    }

    /// Get the file path for an entry.
    fn entry_path(&self, id: Uuid) -> PathBuf {
        self.path.join(format!("{}.json", id))
    }

    /// Write an entry to disk.
    async fn write_entry(&self, entry: &DlqEntry) -> DlqResult<()> {
        let path = self.entry_path(entry.id);
        let json = serde_json::to_string_pretty(entry)
            .map_err(|e| DlqError::Serialization(e.to_string()))?;
        tokio::fs::write(&path, json).await?;
        Ok(())
    }

    /// Delete an entry file from disk.
    async fn delete_entry_file(&self, id: Uuid) -> DlqResult<bool> {
        let path = self.entry_path(id);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// Enforce the maximum number of files by deleting oldest entries.
    async fn enforce_max_files(&self) -> DlqResult<()> {
        let index = self.index.read().await;
        if index.len() <= self.max_files as usize {
            return Ok(());
        }

        // Sort entries by creation time
        let mut entries: Vec<_> = index.values().collect();
        entries.sort_by_key(|e| e.created_at);

        // Determine how many to delete
        let to_delete = entries.len() - self.max_files as usize;
        let ids_to_delete: Vec<Uuid> = entries.iter().take(to_delete).map(|e| e.id).collect();

        drop(index);

        // Delete the oldest entries
        for id in ids_to_delete {
            let _ = self.remove(id).await;
        }

        Ok(())
    }
}

#[async_trait]
impl DeadLetterQueue for FileDlq {
    async fn push(&self, entry: DlqEntry) -> DlqResult<()> {
        // Write to disk first
        self.write_entry(&entry).await?;

        // Update index
        let mut index = self.index.write().await;
        index.insert(entry.id, entry);
        drop(index);

        // Enforce max files limit
        self.enforce_max_files().await?;

        Ok(())
    }

    async fn pop(&self) -> DlqResult<Option<DlqEntry>> {
        let mut index = self.index.write().await;

        // Find oldest entry
        let oldest = index.values().min_by_key(|e| e.created_at).cloned();

        if let Some(entry) = &oldest {
            index.remove(&entry.id);
            drop(index);
            self.delete_entry_file(entry.id).await?;
        }

        Ok(oldest)
    }

    async fn list(&self, params: DlqListParams) -> DlqResult<DlqListResult> {
        let index = self.index.read().await;
        let limit = params.limit.unwrap_or(100);

        let mut entries: Vec<_> = index
            .values()
            .filter(|e| {
                // Filter by entry type
                if let Some(ref entry_type) = params.entry_type
                    && &e.entry_type != entry_type
                {
                    return false;
                }

                // Filter by older_than
                if let Some(older_than) = params.older_than
                    && e.created_at >= older_than
                {
                    return false;
                }

                // Filter by max_retries
                if let Some(max_retries) = params.max_retries
                    && e.retry_count >= max_retries
                {
                    return false;
                }

                // Filter by cursor (for cursor-based pagination)
                // Match repos cursor pattern: Forward=older items, Backward=newer items
                if let Some(ref cursor) = params.cursor {
                    match params.direction {
                        DlqCursorDirection::Forward => {
                            // Forward: entries BEFORE the cursor (older)
                            if e.created_at > cursor.created_at
                                || (e.created_at == cursor.created_at && e.id >= cursor.id)
                            {
                                return false;
                            }
                        }
                        DlqCursorDirection::Backward => {
                            // Backward: entries AFTER the cursor (newer)
                            if e.created_at < cursor.created_at
                                || (e.created_at == cursor.created_at && e.id <= cursor.id)
                            {
                                return false;
                            }
                        }
                    }
                }

                true
            })
            .cloned()
            .collect();

        // Sort by creation time
        // Match repos cursor pattern: default DESC, Backward uses ASC (then reverse)
        match params.direction {
            DlqCursorDirection::Forward => {
                // DESC order (newest first) - no cursor means first page
                entries.sort_by_key(|e| (std::cmp::Reverse(e.created_at), std::cmp::Reverse(e.id)))
            }
            DlqCursorDirection::Backward => {
                // ASC order, then reverse after taking limit
                entries.sort_by_key(|e| (e.created_at, e.id))
            }
        }

        // Apply limit (fetch limit + 1 to determine has_more)
        let mut items: Vec<_> = entries.into_iter().take(limit as usize + 1).collect();

        // Check if there are more entries
        let has_more = items.len() as i64 > limit;
        if has_more {
            items.pop();
        }

        // For backward pagination, reverse to maintain chronological order in response
        if params.direction == DlqCursorDirection::Backward {
            items.reverse();
        }

        Ok(DlqListResult::new(
            items,
            has_more,
            params.direction,
            params.cursor.as_ref(),
        ))
    }

    async fn get(&self, id: Uuid) -> DlqResult<Option<DlqEntry>> {
        let index = self.index.read().await;
        Ok(index.get(&id).cloned())
    }

    async fn remove(&self, id: Uuid) -> DlqResult<bool> {
        let mut index = self.index.write().await;

        if index.remove(&id).is_some() {
            drop(index);
            self.delete_entry_file(id).await
        } else {
            Ok(false)
        }
    }

    async fn mark_retried(&self, id: Uuid) -> DlqResult<()> {
        let mut index = self.index.write().await;

        if let Some(entry) = index.get_mut(&id) {
            entry.retry_count += 1;
            entry.last_retry_at = Some(Utc::now());

            // Update on disk
            let entry_clone = entry.clone();
            drop(index);
            self.write_entry(&entry_clone).await?;
        }

        Ok(())
    }

    async fn len(&self) -> DlqResult<u64> {
        let index = self.index.read().await;
        Ok(index.len() as u64)
    }

    async fn prune(&self, older_than: DateTime<Utc>) -> DlqResult<u64> {
        let index = self.index.read().await;

        let ids_to_delete: Vec<Uuid> = index
            .values()
            .filter(|e| e.created_at < older_than)
            .map(|e| e.id)
            .collect();

        drop(index);

        let mut deleted = 0;
        for id in ids_to_delete {
            if self.remove(id).await? {
                deleted += 1;
            }
        }

        Ok(deleted)
    }

    async fn clear(&self) -> DlqResult<u64> {
        let mut index = self.index.write().await;
        let count = index.len() as u64;

        for id in index.keys().copied().collect::<Vec<_>>() {
            let _ = self.delete_entry_file(id).await;
        }

        index.clear();
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    async fn create_test_dlq() -> (FileDlq, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let dlq: FileDlq = FileDlq::new(temp_dir.path(), 100, 100).await.unwrap();
        (dlq, temp_dir)
    }

    #[tokio::test]
    async fn test_push_and_pop() {
        let (dlq, _temp): (FileDlq, TempDir) = create_test_dlq().await;

        let entry = DlqEntry::new("test", r#"{"foo":"bar"}"#, "test error");
        DeadLetterQueue::push(&dlq, entry.clone()).await.unwrap();

        assert_eq!(DeadLetterQueue::len(&dlq).await.unwrap(), 1);

        let popped = DeadLetterQueue::pop(&dlq).await.unwrap();
        assert!(popped.is_some());
        assert_eq!(popped.unwrap().id, entry.id);
        assert_eq!(DeadLetterQueue::len(&dlq).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_list_filtering() {
        let (dlq, _temp): (FileDlq, TempDir) = create_test_dlq().await;

        DeadLetterQueue::push(&dlq, DlqEntry::new("type_a", "{}", "error"))
            .await
            .unwrap();
        DeadLetterQueue::push(&dlq, DlqEntry::new("type_b", "{}", "error"))
            .await
            .unwrap();
        DeadLetterQueue::push(&dlq, DlqEntry::new("type_a", "{}", "error"))
            .await
            .unwrap();

        let all = DeadLetterQueue::list(&dlq, DlqListParams::default())
            .await
            .unwrap();
        assert_eq!(all.items.len(), 3);

        let type_a = DeadLetterQueue::list(
            &dlq,
            DlqListParams {
                entry_type: Some("type_a".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(type_a.items.len(), 2);
    }

    #[tokio::test]
    async fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();

        // Create DLQ and add entry
        let dlq: FileDlq = FileDlq::new(temp_dir.path(), 100, 100).await.unwrap();
        let entry = DlqEntry::new("test", "{}", "error");
        let entry_id = entry.id;
        DeadLetterQueue::push(&dlq, entry).await.unwrap();
        drop(dlq);

        // Create new DLQ instance, should load from disk
        let dlq2: FileDlq = FileDlq::new(temp_dir.path(), 100, 100).await.unwrap();
        assert_eq!(DeadLetterQueue::len(&dlq2).await.unwrap(), 1);

        let loaded = DeadLetterQueue::get(&dlq2, entry_id).await.unwrap();
        assert!(loaded.is_some());
    }
}
