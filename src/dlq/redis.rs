use async_trait::async_trait;
use chrono::{DateTime, Utc};
use redis::{AsyncCommands, RedisResult, Value, streams::StreamReadOptions};
use uuid::Uuid;

use super::{
    error::{DlqError, DlqResult},
    traits::{DeadLetterQueue, DlqCursorDirection, DlqEntry, DlqListParams, DlqListResult},
};

/// Redis Streams-based dead-letter queue implementation.
///
/// Uses Redis Streams for reliable message delivery with:
/// - Automatic message IDs with timestamps
/// - Consumer groups for distributed processing
/// - Message acknowledgment
/// - Persistence and replication support
pub struct RedisDlq {
    /// Redis connection manager.
    client: redis::Client,
    /// Stream key for the DLQ.
    stream_key: String,
    /// Consumer group name.
    group_name: String,
    /// Maximum number of entries to keep.
    max_entries: u64,
    /// TTL for entries in milliseconds (unused currently, kept for API compatibility).
    #[allow(dead_code)] // Set via constructor; reserved for TTL-based pruning
    ttl_ms: u64,
}

/// A parsed stream entry from XRANGE/XREAD.
struct StreamEntry {
    id: String,
    fields: Vec<(String, String)>,
}

impl RedisDlq {
    /// Create a new Redis Streams-based DLQ.
    pub async fn new(
        url: &str,
        key_prefix: String,
        max_entries: u64,
        ttl_secs: u64,
    ) -> DlqResult<Self> {
        let client = redis::Client::open(url)?;

        // Test connectivity
        let mut conn = client.get_multiplexed_async_connection().await?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;

        let stream_key = format!("{}stream", key_prefix);
        let group_name = "dlq_consumers".to_string();

        // Create consumer group if it doesn't exist
        // Use MKSTREAM to create the stream if needed
        let result: RedisResult<()> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(&stream_key)
            .arg(&group_name)
            .arg("0")
            .arg("MKSTREAM")
            .query_async(&mut conn)
            .await;

        // Ignore BUSYGROUP error (group already exists)
        if let Err(e) = result {
            let err_str = e.to_string();
            if !err_str.contains("BUSYGROUP") {
                return Err(DlqError::Redis(e));
            }
        }

        Ok(Self {
            client,
            stream_key,
            group_name,
            max_entries,
            ttl_ms: ttl_secs * 1000,
        })
    }

    /// Get a connection from the pool.
    async fn conn(&self) -> DlqResult<redis::aio::MultiplexedConnection> {
        Ok(self.client.get_multiplexed_async_connection().await?)
    }

    /// Trim stream to max entries.
    async fn trim_stream(&self, conn: &mut redis::aio::MultiplexedConnection) -> DlqResult<()> {
        // XTRIM with MAXLEN ~ (approximate) for efficiency
        let _: u64 = redis::cmd("XTRIM")
            .arg(&self.stream_key)
            .arg("MAXLEN")
            .arg("~")
            .arg(self.max_entries)
            .query_async(conn)
            .await?;
        Ok(())
    }

    /// Parse XRANGE response into stream entries.
    fn parse_xrange_response(value: Value) -> Vec<StreamEntry> {
        let mut entries = Vec::new();

        if let Value::Array(items) = value {
            for item in items {
                if let Value::Array(entry) = item
                    && entry.len() >= 2
                {
                    // First element is the ID
                    let id = match &entry[0] {
                        Value::BulkString(bytes) => String::from_utf8_lossy(bytes).to_string(),
                        _ => continue,
                    };

                    // Second element is the fields array
                    let mut fields = Vec::new();
                    if let Value::Array(field_values) = &entry[1] {
                        let mut iter = field_values.iter();
                        while let (Some(key), Some(val)) = (iter.next(), iter.next()) {
                            if let (Value::BulkString(k), Value::BulkString(v)) = (key, val) {
                                fields.push((
                                    String::from_utf8_lossy(k).to_string(),
                                    String::from_utf8_lossy(v).to_string(),
                                ));
                            }
                        }
                    }

                    entries.push(StreamEntry { id, fields });
                }
            }
        }

        entries
    }

    /// Parse a stream entry into a DlqEntry.
    fn parse_stream_entry(entry: &StreamEntry) -> DlqResult<Option<DlqEntry>> {
        // Find the "data" field which contains JSON
        for (key, value) in &entry.fields {
            if key == "data" {
                let mut dlq_entry: DlqEntry = serde_json::from_str(value)
                    .map_err(|e| DlqError::Deserialization(e.to_string()))?;
                // Store the stream ID for later operations
                dlq_entry
                    .metadata
                    .insert("stream_id".to_string(), entry.id.clone());
                return Ok(Some(dlq_entry));
            }
        }
        Ok(None)
    }

    /// Get entry UUID from stream entry fields.
    fn get_entry_id(entry: &StreamEntry) -> Option<Uuid> {
        for (key, value) in &entry.fields {
            if key == "id" {
                return Uuid::parse_str(value).ok();
            }
        }
        None
    }
}

#[async_trait]
impl DeadLetterQueue for RedisDlq {
    async fn push(&self, entry: DlqEntry) -> DlqResult<()> {
        let mut conn = self.conn().await?;

        let json =
            serde_json::to_string(&entry).map_err(|e| DlqError::Serialization(e.to_string()))?;

        // XADD with entry ID and JSON data
        // Use * for auto-generated ID (timestamp-based)
        let _: String = redis::cmd("XADD")
            .arg(&self.stream_key)
            .arg("*")
            .arg("id")
            .arg(entry.id.to_string())
            .arg("type")
            .arg(&entry.entry_type)
            .arg("data")
            .arg(&json)
            .query_async(&mut conn)
            .await?;

        // Trim to max entries
        self.trim_stream(&mut conn).await?;

        Ok(())
    }

    async fn pop(&self) -> DlqResult<Option<DlqEntry>> {
        let mut conn = self.conn().await?;

        // Use XREADGROUP to read and claim a message
        // Consumer name is generated uniquely per call
        let consumer_name = format!("consumer_{}", Uuid::new_v4());

        let opts = StreamReadOptions::default()
            .group(&self.group_name, &consumer_name)
            .count(1);

        let result: Value = conn
            .xread_options(&[&self.stream_key], &[">"], &opts)
            .await?;

        // Parse XREADGROUP response: [[stream_name, [[id, [field, value, ...]]]]]
        if let Value::Array(streams) = result {
            for stream in streams {
                if let Value::Array(stream_data) = stream
                    && stream_data.len() >= 2
                    && let Value::Array(entries) = &stream_data[1]
                {
                    for entry_value in entries {
                        if let Value::Array(entry) = entry_value
                            && entry.len() >= 2
                        {
                            let stream_id = match &entry[0] {
                                Value::BulkString(bytes) => {
                                    String::from_utf8_lossy(bytes).to_string()
                                }
                                _ => continue,
                            };

                            let mut fields = Vec::new();
                            if let Value::Array(field_values) = &entry[1] {
                                let mut iter = field_values.iter();
                                while let (Some(key), Some(val)) = (iter.next(), iter.next()) {
                                    if let (Value::BulkString(k), Value::BulkString(v)) = (key, val)
                                    {
                                        fields.push((
                                            String::from_utf8_lossy(k).to_string(),
                                            String::from_utf8_lossy(v).to_string(),
                                        ));
                                    }
                                }
                            }

                            let stream_entry = StreamEntry {
                                id: stream_id.clone(),
                                fields,
                            };

                            if let Some(dlq_entry) = Self::parse_stream_entry(&stream_entry)? {
                                // Acknowledge the message
                                let _: u64 = conn
                                    .xack(&self.stream_key, &self.group_name, &[&stream_id])
                                    .await?;

                                // Delete the message from the stream
                                let _: u64 = conn.xdel(&self.stream_key, &[&stream_id]).await?;

                                return Ok(Some(dlq_entry));
                            }
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    async fn list(&self, params: DlqListParams) -> DlqResult<DlqListResult> {
        let mut conn = self.conn().await?;
        let limit = params.limit.unwrap_or(100);
        let is_backward = params.direction == DlqCursorDirection::Backward;

        // Match repos cursor pattern: default DESC (newest first), Forward=older items, Backward=newer items
        // Use XREVRANGE for Forward (DESC) and XRANGE for Backward (ASC, then reverse)

        let value: Value = if is_backward {
            // Backward: get entries AFTER cursor (newer), sorted ASC
            let start = if let Some(ref cursor) = params.cursor {
                // Exclusive start: entries after cursor
                format!("({}-{}", cursor.created_at.timestamp_millis(), u64::MAX)
            } else {
                "-".to_string()
            };

            let end = if let Some(older_than) = params.older_than {
                format!("{}-0", older_than.timestamp_millis())
            } else {
                "+".to_string()
            };

            redis::cmd("XRANGE")
                .arg(&self.stream_key)
                .arg(&start)
                .arg(&end)
                .query_async(&mut conn)
                .await?
        } else {
            // Forward: get entries BEFORE cursor (older), sorted DESC
            let end = if let Some(ref cursor) = params.cursor {
                // Exclusive end: entries before cursor
                format!("({}-0", cursor.created_at.timestamp_millis())
            } else {
                "+".to_string()
            };

            let start = if let Some(older_than) = params.older_than {
                format!("{}-0", older_than.timestamp_millis())
            } else {
                "-".to_string()
            };

            // XREVRANGE for descending order (newest first)
            redis::cmd("XREVRANGE")
                .arg(&self.stream_key)
                .arg(&end)
                .arg(&start)
                .query_async(&mut conn)
                .await?
        };

        let entries = Self::parse_xrange_response(value);
        let mut result = Vec::new();

        for entry in entries {
            if let Some(dlq_entry) = Self::parse_stream_entry(&entry)? {
                // Apply filters
                if let Some(ref entry_type) = params.entry_type
                    && &dlq_entry.entry_type != entry_type
                {
                    continue;
                }

                if let Some(max_retries) = params.max_retries
                    && dlq_entry.retry_count >= max_retries
                {
                    continue;
                }

                // Additional cursor filtering (in case XRANGE/XREVRANGE exclusive syntax varies)
                if let Some(ref cursor) = params.cursor {
                    if is_backward {
                        // Backward: skip entries at or before cursor
                        if dlq_entry.created_at < cursor.created_at
                            || (dlq_entry.created_at == cursor.created_at
                                && dlq_entry.id <= cursor.id)
                        {
                            continue;
                        }
                    } else {
                        // Forward: skip entries at or after cursor
                        if dlq_entry.created_at > cursor.created_at
                            || (dlq_entry.created_at == cursor.created_at
                                && dlq_entry.id >= cursor.id)
                        {
                            continue;
                        }
                    }
                }

                result.push(dlq_entry);
            }
        }

        // Apply limit (fetch limit + 1 to determine has_more)
        let mut items: Vec<_> = result.into_iter().take(limit as usize + 1).collect();

        // Check if there are more entries
        let has_more = items.len() as i64 > limit;
        if has_more {
            items.pop();
        }

        // For backward pagination, reverse to maintain DESC order in response
        if is_backward {
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
        let mut conn = self.conn().await?;

        // XRANGE to search for the entry by scanning
        let value: Value = redis::cmd("XRANGE")
            .arg(&self.stream_key)
            .arg("-")
            .arg("+")
            .query_async(&mut conn)
            .await?;

        let entries = Self::parse_xrange_response(value);

        for entry in entries {
            if let Some(entry_id) = Self::get_entry_id(&entry)
                && entry_id == id
            {
                return Self::parse_stream_entry(&entry);
            }
        }

        Ok(None)
    }

    async fn remove(&self, id: Uuid) -> DlqResult<bool> {
        let mut conn = self.conn().await?;

        // Find the stream ID for this entry
        let value: Value = redis::cmd("XRANGE")
            .arg(&self.stream_key)
            .arg("-")
            .arg("+")
            .query_async(&mut conn)
            .await?;

        let entries = Self::parse_xrange_response(value);

        for entry in entries {
            if let Some(entry_id) = Self::get_entry_id(&entry)
                && entry_id == id
            {
                let deleted: u64 = conn.xdel(&self.stream_key, &[&entry.id]).await?;
                return Ok(deleted > 0);
            }
        }

        Ok(false)
    }

    async fn mark_retried(&self, id: Uuid) -> DlqResult<()> {
        // For streams, we need to remove and re-add with updated retry count
        // This is a limitation of streams - entries are immutable
        if let Some(mut entry) = self.get(id).await? {
            // Remove the old entry
            self.remove(id).await?;

            // Update retry info
            entry.retry_count += 1;
            entry.last_retry_at = Some(Utc::now());

            // Re-add with updated data
            self.push(entry).await?;
        }

        Ok(())
    }

    async fn len(&self) -> DlqResult<u64> {
        let mut conn = self.conn().await?;
        let count: u64 = conn.xlen(&self.stream_key).await?;
        Ok(count)
    }

    async fn prune(&self, older_than: DateTime<Utc>) -> DlqResult<u64> {
        let mut conn = self.conn().await?;

        // Get entries older than the cutoff
        let max_id = format!("{}-0", older_than.timestamp_millis());

        let value: Value = redis::cmd("XRANGE")
            .arg(&self.stream_key)
            .arg("-")
            .arg(&max_id)
            .query_async(&mut conn)
            .await?;

        let entries = Self::parse_xrange_response(value);
        let count = entries.len() as u64;

        // Delete each entry
        for entry in entries {
            let _: u64 = conn.xdel(&self.stream_key, &[&entry.id]).await?;
        }

        Ok(count)
    }

    async fn clear(&self) -> DlqResult<u64> {
        let mut conn = self.conn().await?;

        // Get count before clearing
        let count: u64 = conn.xlen(&self.stream_key).await?;

        // Delete the stream entirely
        let _: () = conn.del(&self.stream_key).await?;

        // Recreate the consumer group
        let _: RedisResult<()> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(&self.stream_key)
            .arg(&self.group_name)
            .arg("0")
            .arg("MKSTREAM")
            .query_async(&mut conn)
            .await;

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tests::harness::redis::create_redis_container;

    // Integration tests use testcontainers for Redis
    // Run with: cargo test -- --ignored

    #[tokio::test]
    #[ignore = "Requires Docker - run with `cargo test -- --ignored`"]
    async fn test_redis_dlq_push_pop() {
        let (url, _container) = create_redis_container().await;

        let dlq = RedisDlq::new(&url, "test_dlq:".to_string(), 1000, 3600)
            .await
            .unwrap();

        // Clear any existing data
        dlq.clear().await.unwrap();

        let entry = DlqEntry::new("test_type", r#"{"key": "value"}"#, "test error");

        // Push
        dlq.push(entry.clone()).await.unwrap();
        assert_eq!(dlq.len().await.unwrap(), 1);

        // Pop
        let popped = dlq.pop().await.unwrap();
        assert!(popped.is_some());
        let popped = popped.unwrap();
        assert_eq!(popped.id, entry.id);
        assert_eq!(popped.entry_type, "test_type");

        // Should be empty now
        assert_eq!(dlq.len().await.unwrap(), 0);
    }

    #[tokio::test]
    #[ignore = "Requires Docker - run with `cargo test -- --ignored`"]
    async fn test_redis_dlq_list() {
        let (url, _container) = create_redis_container().await;

        let dlq = RedisDlq::new(&url, "test_dlq_list:".to_string(), 1000, 3600)
            .await
            .unwrap();

        dlq.clear().await.unwrap();

        // Push multiple entries
        dlq.push(DlqEntry::new("type_a", "{}", "error"))
            .await
            .unwrap();
        dlq.push(DlqEntry::new("type_b", "{}", "error"))
            .await
            .unwrap();
        dlq.push(DlqEntry::new("type_a", "{}", "error"))
            .await
            .unwrap();

        // List all
        let all = dlq.list(DlqListParams::default()).await.unwrap();
        assert_eq!(all.items.len(), 3);

        // List by type
        let type_a = dlq
            .list(DlqListParams {
                entry_type: Some("type_a".to_string()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(type_a.items.len(), 2);

        dlq.clear().await.unwrap();
    }
}
