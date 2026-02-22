use std::time::Duration;

use chrono::{Datelike, TimeZone, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    api_types::{
        CreateChatCompletionPayload, CreateCompletionPayload, CreateEmbeddingPayload,
        CreateResponsesPayload, Message, MessageContent,
    },
    config::CacheKeyComponents,
    models::BudgetPeriod,
};

pub struct CacheKeys;

impl CacheKeys {
    /// API key lookup by hash: gw:apikey:{hash}
    pub fn api_key(hash: &str) -> String {
        format!("gw:apikey:{}", hash)
    }

    /// API key lookup by ID: gw:apikey:id:{id}
    /// Used for cache invalidation when we only have the ID
    pub fn api_key_by_id(id: Uuid) -> String {
        format!("gw:apikey:id:{}", id)
    }

    /// Reverse mapping: ID to hash for cache invalidation
    /// gw:apikey:reverse:{id} -> hash
    pub fn api_key_reverse(id: Uuid) -> String {
        format!("gw:apikey:reverse:{}", id)
    }

    /// Rate limiting (requests): gw:ratelimit:{api_key_id}:{window}
    ///
    /// Uses Redis hash tags `{api_key_id}` to ensure all keys for the same API key
    /// hash to the same cluster slot, enabling pipelining in cluster mode.
    pub fn rate_limit(api_key_id: Uuid, window: &str) -> String {
        format!("gw:ratelimit:{{{}}}:{}", api_key_id, window)
    }

    /// IP-based rate limiting (requests): gw:ratelimit:ip:{ip}:{window}
    pub fn rate_limit_ip(ip: &str, window: &str) -> String {
        format!("gw:ratelimit:ip:{}:{}", ip, window)
    }

    /// Rate limiting (tokens): gw:ratelimit:tokens:{api_key_id}:{window}
    ///
    /// Uses Redis hash tags `{api_key_id}` to ensure all keys for the same API key
    /// hash to the same cluster slot, enabling pipelining in cluster mode.
    pub fn rate_limit_tokens(api_key_id: Uuid, window: &str) -> String {
        format!("gw:ratelimit:tokens:{{{}}}:{}", api_key_id, window)
    }

    /// Concurrent requests: gw:concurrent:{api_key_id}
    ///
    /// Uses Redis hash tags `{api_key_id}` to ensure all keys for the same API key
    /// hash to the same cluster slot, enabling pipelining in cluster mode.
    pub fn concurrent_requests(api_key_id: Uuid) -> String {
        format!("gw:concurrent:{{{}}}", api_key_id)
    }

    /// Spend tracking: gw:spend:{api_key_id}:{period}:{date}
    ///
    /// Uses Redis hash tags `{api_key_id}` to ensure all keys for the same API key
    /// hash to the same cluster slot, enabling pipelining in cluster mode.
    pub fn spend(api_key_id: Uuid, period: BudgetPeriod) -> String {
        let now = Utc::now();
        let date_suffix = match period {
            BudgetPeriod::Daily => now.format("%Y-%m-%d").to_string(),
            BudgetPeriod::Monthly => now.format("%Y-%m").to_string(),
        };
        format!(
            "gw:spend:{{{}}}:{}:{}",
            api_key_id,
            period.as_str(),
            date_suffix
        )
    }

    /// Org membership check: gw:orgaccess:{user_id}:{org_id}
    pub fn org_access(user_id: Uuid, org_id: Uuid) -> String {
        format!("gw:orgaccess:{}:{}", user_id, org_id)
    }

    /// API key last_used_at debounce: gw:apikey:lastused:{id}
    ///
    /// Presence of this key means a `last_used_at` write was already issued
    /// within the debounce window, so we can skip another DB write.
    pub fn api_key_last_used(id: Uuid) -> String {
        format!("gw:apikey:lastused:{}", id)
    }

    /// Dynamic provider lookup: gw:provider:{scope}:{scope_id}:{name}
    ///
    /// `scope_id` encodes the owner identity (e.g. org slug, "org:project" composite,
    /// or "org:user_id" composite) to ensure cache isolation between tenants.
    pub fn dynamic_provider(scope: &str, scope_id: &str, name: &str) -> String {
        format!("gw:provider:{}:{}:{}", scope, scope_id, name)
    }

    #[cfg(feature = "cel")]
    /// RBAC policy version for multi-node cache invalidation: gw:rbac:org:{org_id}:version
    ///
    /// Tracks the current version of RBAC policies for an organization.
    /// When policies are modified, this version is incremented to signal
    /// other nodes to refresh their local policy cache.
    pub fn rbac_policy_version(org_id: Uuid) -> String {
        format!("gw:rbac:org:{}:version", org_id)
    }

    /// Emergency access rate limiting: gw:emergency:ratelimit:{ip}
    ///
    /// Tracks failed emergency access attempts from an IP address.
    /// Counter increments on each failed attempt and resets after window expires.
    pub fn emergency_rate_limit(ip: &str) -> String {
        format!("gw:emergency:ratelimit:{}", ip)
    }

    /// Emergency access lockout: gw:emergency:lockout:{ip}
    ///
    /// Set when an IP exceeds the max failed attempts.
    /// Presence of this key blocks further emergency access attempts from the IP.
    pub fn emergency_lockout(ip: &str) -> String {
        format!("gw:emergency:lockout:{}", ip)
    }

    /// Response cache key for chat completions.
    ///
    /// Generates a deterministic cache key based on configurable components:
    /// - Model name (always included)
    /// - Messages content (always included, hashed)
    /// - Temperature (optional)
    /// - System prompt (optional, extracted and hashed separately)
    /// - Tools (optional, hashed)
    /// - Response format (if specified)
    /// - Seed (if specified, for reproducibility)
    ///
    /// Returns `gw:response:{hash}` where hash is a SHA-256 digest of the key components.
    pub fn response_cache(
        payload: &CreateChatCompletionPayload,
        model: &str,
        key_components: &CacheKeyComponents,
    ) -> String {
        let mut hasher = Sha256::new();

        // Model is always included in the cache key
        hasher.update(b"model:");
        hasher.update(model.as_bytes());
        hasher.update(b"\x00");

        // Include temperature if configured (critical for determinism)
        if key_components.temperature {
            hasher.update(b"temp:");
            let temp = payload.temperature.unwrap_or(1.0);
            hasher.update(temp.to_le_bytes());
            hasher.update(b"\x00");
        }

        // Include seed if present (for reproducibility)
        if let Some(seed) = payload.seed {
            hasher.update(b"seed:");
            hasher.update(seed.to_le_bytes());
            hasher.update(b"\x00");
        }

        // Include response_format if present
        if let Some(ref format) = payload.response_format {
            hasher.update(b"format:");
            // Serialize response format to JSON for consistent hashing
            if let Ok(json) = serde_json::to_string(format) {
                hasher.update(json.as_bytes());
            }
            hasher.update(b"\x00");
        }

        // Include tools if configured and present
        if key_components.tools
            && let Some(ref tools) = payload.tools
        {
            hasher.update(b"tools:");
            if let Ok(json) = serde_json::to_string(tools) {
                hasher.update(json.as_bytes());
            }
            hasher.update(b"\x00");
        }

        // Include system prompt separately if configured
        if key_components.system_prompt {
            hasher.update(b"system:");
            for msg in &payload.messages {
                if let Message::System { content, .. } | Message::Developer { content, .. } = msg {
                    hasher.update(Self::hash_message_content(content).as_bytes());
                    hasher.update(b"|");
                }
            }
            hasher.update(b"\x00");
        }

        // Always include all messages content (hashed)
        hasher.update(b"messages:");
        for msg in &payload.messages {
            hasher.update(Self::hash_message(msg).as_bytes());
            hasher.update(b"|");
        }

        let hash = hasher.finalize();
        format!("gw:response:{:x}", hash)
    }

    /// Response cache key for the Responses API (/v1/responses).
    ///
    /// Generates a deterministic cache key based on configurable components:
    /// - Model name (always included)
    /// - Input content (always included, serialized and hashed)
    /// - Instructions (optional, hashed)
    /// - Temperature (optional)
    /// - Tools (optional, hashed)
    ///
    /// Returns `gw:responses:{hash}` where hash is a SHA-256 digest of the key components.
    pub fn responses_cache(
        payload: &CreateResponsesPayload,
        model: &str,
        key_components: &CacheKeyComponents,
    ) -> String {
        let mut hasher = Sha256::new();

        // Model is always included in the cache key
        hasher.update(b"model:");
        hasher.update(model.as_bytes());
        hasher.update(b"\x00");

        // Include temperature if configured (critical for determinism)
        if key_components.temperature {
            hasher.update(b"temp:");
            let temp = payload.temperature.unwrap_or(1.0);
            hasher.update(temp.to_le_bytes());
            hasher.update(b"\x00");
        }

        // Include tools if configured and present
        if key_components.tools
            && let Some(ref tools) = payload.tools
        {
            hasher.update(b"tools:");
            if let Ok(json) = serde_json::to_string(tools) {
                hasher.update(json.as_bytes());
            }
            hasher.update(b"\x00");
        }

        // Include system prompt (instructions) if configured
        if key_components.system_prompt {
            hasher.update(b"instructions:");
            if let Some(ref instructions) = payload.instructions {
                hasher.update(instructions.as_bytes());
            }
            hasher.update(b"\x00");
        }

        // Always include input content (hashed via JSON serialization)
        hasher.update(b"input:");
        if let Some(ref input) = payload.input
            && let Ok(json) = serde_json::to_string(input)
        {
            hasher.update(json.as_bytes());
        }

        let hash = hasher.finalize();
        format!("gw:responses:{:x}", hash)
    }

    /// Response cache key for the Completions API (/v1/completions).
    ///
    /// Generates a deterministic cache key based on configurable components:
    /// - Model name (always included)
    /// - Prompt content (always included, serialized and hashed)
    /// - Temperature (optional)
    /// - Suffix (optional)
    /// - Seed (if present, for reproducibility)
    ///
    /// Returns `gw:completions:{hash}` where hash is a SHA-256 digest of the key components.
    pub fn completions_cache(
        payload: &CreateCompletionPayload,
        model: &str,
        key_components: &CacheKeyComponents,
    ) -> String {
        let mut hasher = Sha256::new();

        // Model is always included in the cache key
        hasher.update(b"model:");
        hasher.update(model.as_bytes());
        hasher.update(b"\x00");

        // Include temperature if configured (critical for determinism)
        if key_components.temperature {
            hasher.update(b"temp:");
            let temp = payload.temperature.unwrap_or(1.0);
            hasher.update(temp.to_le_bytes());
            hasher.update(b"\x00");
        }

        // Include seed if present (for reproducibility)
        if let Some(seed) = payload.seed {
            hasher.update(b"seed:");
            hasher.update(seed.to_le_bytes());
            hasher.update(b"\x00");
        }

        // Include suffix if present
        if let Some(ref suffix) = payload.suffix {
            hasher.update(b"suffix:");
            hasher.update(suffix.as_bytes());
            hasher.update(b"\x00");
        }

        // Always include prompt content (hashed via JSON serialization)
        hasher.update(b"prompt:");
        if let Ok(json) = serde_json::to_string(&payload.prompt) {
            hasher.update(json.as_bytes());
        }

        let hash = hasher.finalize();
        format!("gw:completions:{:x}", hash)
    }

    /// Response cache key for the Embeddings API (/v1/embeddings).
    ///
    /// Generates a deterministic cache key based on:
    /// - Model name (always included)
    /// - Input content (always included, serialized and hashed)
    /// - Encoding format (if specified)
    /// - Dimensions (if specified)
    ///
    /// Note: Embeddings are fully deterministic (no temperature/seed),
    /// making them excellent candidates for caching.
    ///
    /// Returns `gw:embeddings:{hash}` where hash is a SHA-256 digest of the key components.
    pub fn embeddings_cache(payload: &CreateEmbeddingPayload, model: &str) -> String {
        let mut hasher = Sha256::new();

        // Model is always included in the cache key
        hasher.update(b"model:");
        hasher.update(model.as_bytes());
        hasher.update(b"\x00");

        // Include encoding_format if present
        if let Some(ref format) = payload.encoding_format {
            hasher.update(b"encoding:");
            if let Ok(json) = serde_json::to_string(format) {
                hasher.update(json.as_bytes());
            }
            hasher.update(b"\x00");
        }

        // Include dimensions if present
        if let Some(dimensions) = payload.dimensions {
            hasher.update(b"dimensions:");
            hasher.update(dimensions.to_le_bytes());
            hasher.update(b"\x00");
        }

        // Always include input content (hashed via JSON serialization)
        hasher.update(b"input:");
        if let Ok(json) = serde_json::to_string(&payload.input) {
            hasher.update(json.as_bytes());
        }

        let hash = hasher.finalize();
        format!("gw:embeddings:{:x}", hash)
    }

    /// Hash a single message for cache key generation.
    fn hash_message(msg: &Message) -> String {
        match msg {
            Message::System { content, name } => {
                format!(
                    "S:{}:{}",
                    name.as_deref().unwrap_or(""),
                    Self::hash_message_content(content)
                )
            }
            Message::User { content, name } => {
                format!(
                    "U:{}:{}",
                    name.as_deref().unwrap_or(""),
                    Self::hash_message_content(content)
                )
            }
            Message::Assistant {
                content,
                tool_calls,
                reasoning,
                ..
            } => {
                let content_hash = content
                    .as_ref()
                    .map(Self::hash_message_content)
                    .unwrap_or_default();
                let tool_calls_hash = tool_calls
                    .as_ref()
                    .map(|tc| serde_json::to_string(tc).unwrap_or_default())
                    .unwrap_or_default();
                let reasoning_hash = reasoning.as_deref().unwrap_or("");
                format!("A:{}:{}:{}", content_hash, tool_calls_hash, reasoning_hash)
            }
            Message::Tool {
                content,
                tool_call_id,
            } => {
                format!("T:{}:{}", tool_call_id, Self::hash_message_content(content))
            }
            Message::Developer { content, name } => {
                format!(
                    "D:{}:{}",
                    name.as_deref().unwrap_or(""),
                    Self::hash_message_content(content)
                )
            }
        }
    }

    /// Hash message content (handles both text and multimodal content).
    fn hash_message_content(content: &MessageContent) -> String {
        match content {
            MessageContent::Text(text) => text.clone(),
            MessageContent::Parts(parts) => {
                // For multimodal content, serialize to JSON for consistent hashing
                serde_json::to_string(parts).unwrap_or_default()
            }
        }
    }

    /// Fixed TTL for budget tracking cache entries.
    ///
    /// Uses a full period duration (24h for daily, 31d for monthly) rather than
    /// time-until-period-end. This prevents a race condition where:
    ///
    /// 1. A request starts near period boundary with short TTL (e.g., 5 min before midnight)
    /// 2. A long streaming request takes longer than the TTL
    /// 3. The cache entry expires, losing track of spend for that period
    /// 4. Cost adjustments arrive after expiry, creating a new entry instead of adjusting
    ///
    /// Since spend keys include the date (e.g., `gw:spend:abc:daily:2025-12-13`),
    /// a fixed TTL won't cause spend from one period to count against another.
    /// The key naturally becomes obsolete when the next period starts, and requests
    /// in the new period use a new date-stamped key.
    pub fn budget_ttl(period: BudgetPeriod) -> Duration {
        match period {
            BudgetPeriod::Daily => Duration::from_secs(86400), // 24 hours
            BudgetPeriod::Monthly => Duration::from_secs(2678400), // 31 days
        }
    }

    /// Calculate TTL until the end of the current budget period.
    ///
    /// **Note:** For budget spend tracking, prefer `budget_ttl()` which uses a fixed
    /// full-period duration. This function is useful for flags or warnings that should
    /// reset at period boundaries (e.g., "budget warning logged" flags).
    ///
    /// Uses a minimum TTL of 60 seconds to handle edge cases at period boundaries.
    pub fn ttl_until_period_end(period: BudgetPeriod) -> Duration {
        let now = Utc::now();
        let seconds_until_end = match period {
            BudgetPeriod::Daily => {
                // Seconds until midnight UTC
                let tomorrow = Utc
                    .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
                    .single()
                    .expect("valid date")
                    + chrono::Duration::days(1);
                (tomorrow - now).num_seconds()
            }
            BudgetPeriod::Monthly => {
                // Seconds until first of next month at midnight UTC
                let (year, month) = if now.month() == 12 {
                    (now.year() + 1, 1)
                } else {
                    (now.year(), now.month() + 1)
                };
                let next_month = Utc
                    .with_ymd_and_hms(year, month, 1, 0, 0, 0)
                    .single()
                    .expect("valid date");
                (next_month - now).num_seconds()
            }
        };

        // Minimum TTL of 60 seconds to handle edge cases at period boundaries
        // and ensure the cache entry doesn't expire before the request completes
        Duration::from_secs(seconds_until_end.max(60) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_ttl_daily() {
        let ttl = CacheKeys::budget_ttl(BudgetPeriod::Daily);
        // Fixed 24 hours
        assert_eq!(ttl, Duration::from_secs(86400));
    }

    #[test]
    fn test_budget_ttl_monthly() {
        let ttl = CacheKeys::budget_ttl(BudgetPeriod::Monthly);
        // Fixed 31 days
        assert_eq!(ttl, Duration::from_secs(2678400));
    }

    #[test]
    fn test_ttl_until_period_end_daily() {
        let ttl = CacheKeys::ttl_until_period_end(BudgetPeriod::Daily);

        // TTL should be at least 60 seconds (minimum)
        assert!(ttl >= Duration::from_secs(60));
        // TTL should be at most 24 hours
        assert!(ttl <= Duration::from_secs(86400));
    }

    #[test]
    fn test_ttl_until_period_end_monthly() {
        let ttl = CacheKeys::ttl_until_period_end(BudgetPeriod::Monthly);

        // TTL should be at least 60 seconds (minimum)
        assert!(ttl >= Duration::from_secs(60));
        // TTL should be at most 31 days
        assert!(ttl <= Duration::from_secs(86400 * 31));
    }

    #[test]
    fn test_spend_key_format() {
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let key = CacheKeys::spend(id, BudgetPeriod::Daily);

        // Key should contain the expected components with hash tags for cluster support
        assert!(key.starts_with("gw:spend:{550e8400-e29b-41d4-a716-446655440000}:daily:"));
    }

    #[test]
    fn test_api_key_format() {
        let key = CacheKeys::api_key("abc123hash");
        assert_eq!(key, "gw:apikey:abc123hash");
    }

    #[test]
    fn test_rate_limit_key_format() {
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let key = CacheKeys::rate_limit(id, "minute");
        // Key should contain hash tags for cluster support
        assert_eq!(
            key,
            "gw:ratelimit:{550e8400-e29b-41d4-a716-446655440000}:minute"
        );
    }

    #[test]
    fn test_response_cache_key_deterministic() {
        let payload = CreateChatCompletionPayload {
            messages: vec![Message::User {
                content: MessageContent::Text("Hello, world!".to_string()),
                name: None,
            }],
            model: Some("gpt-4".to_string()),
            models: None,
            temperature: Some(0.0),
            seed: Some(42),
            response_format: None,
            tools: None,
            tool_choice: None,
            frequency_penalty: None,
            logit_bias: None,
            logprobs: None,
            top_logprobs: None,
            max_completion_tokens: None,
            max_tokens: None,
            metadata: None,
            presence_penalty: None,
            reasoning: None,
            stop: None,
            stream: false,
            stream_options: None,
            top_p: None,
            user: None,
        };

        let key_components = CacheKeyComponents::default();

        let key1 = CacheKeys::response_cache(&payload, "gpt-4", &key_components);
        let key2 = CacheKeys::response_cache(&payload, "gpt-4", &key_components);

        // Same input should produce same key
        assert_eq!(key1, key2);
        assert!(key1.starts_with("gw:response:"));
    }

    #[test]
    fn test_response_cache_key_different_messages() {
        let key_components = CacheKeyComponents::default();

        let payload1 = CreateChatCompletionPayload {
            messages: vec![Message::User {
                content: MessageContent::Text("Hello".to_string()),
                name: None,
            }],
            model: Some("gpt-4".to_string()),
            models: None,
            temperature: Some(0.0),
            seed: None,
            response_format: None,
            tools: None,
            tool_choice: None,
            frequency_penalty: None,
            logit_bias: None,
            logprobs: None,
            top_logprobs: None,
            max_completion_tokens: None,
            max_tokens: None,
            metadata: None,
            presence_penalty: None,
            reasoning: None,
            stop: None,
            stream: false,
            stream_options: None,
            top_p: None,
            user: None,
        };

        let payload2 = CreateChatCompletionPayload {
            messages: vec![Message::User {
                content: MessageContent::Text("Goodbye".to_string()),
                name: None,
            }],
            ..payload1.clone()
        };

        let key1 = CacheKeys::response_cache(&payload1, "gpt-4", &key_components);
        let key2 = CacheKeys::response_cache(&payload2, "gpt-4", &key_components);

        // Different messages should produce different keys
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_response_cache_key_different_temperature() {
        let key_components = CacheKeyComponents {
            model: true,
            temperature: true,
            system_prompt: true,
            tools: true,
        };

        let payload1 = CreateChatCompletionPayload {
            messages: vec![Message::User {
                content: MessageContent::Text("Hello".to_string()),
                name: None,
            }],
            model: Some("gpt-4".to_string()),
            models: None,
            temperature: Some(0.0),
            seed: None,
            response_format: None,
            tools: None,
            tool_choice: None,
            frequency_penalty: None,
            logit_bias: None,
            logprobs: None,
            top_logprobs: None,
            max_completion_tokens: None,
            max_tokens: None,
            metadata: None,
            presence_penalty: None,
            reasoning: None,
            stop: None,
            stream: false,
            stream_options: None,
            top_p: None,
            user: None,
        };

        let payload2 = CreateChatCompletionPayload {
            temperature: Some(0.7),
            ..payload1.clone()
        };

        let key1 = CacheKeys::response_cache(&payload1, "gpt-4", &key_components);
        let key2 = CacheKeys::response_cache(&payload2, "gpt-4", &key_components);

        // Different temperatures should produce different keys when temperature is in key_components
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_response_cache_key_different_model() {
        let key_components = CacheKeyComponents::default();

        let payload = CreateChatCompletionPayload {
            messages: vec![Message::User {
                content: MessageContent::Text("Hello".to_string()),
                name: None,
            }],
            model: Some("gpt-4".to_string()),
            models: None,
            temperature: Some(0.0),
            seed: None,
            response_format: None,
            tools: None,
            tool_choice: None,
            frequency_penalty: None,
            logit_bias: None,
            logprobs: None,
            top_logprobs: None,
            max_completion_tokens: None,
            max_tokens: None,
            metadata: None,
            presence_penalty: None,
            reasoning: None,
            stop: None,
            stream: false,
            stream_options: None,
            top_p: None,
            user: None,
        };

        let key1 = CacheKeys::response_cache(&payload, "gpt-4", &key_components);
        let key2 = CacheKeys::response_cache(&payload, "claude-3", &key_components);

        // Different models should produce different keys
        assert_ne!(key1, key2);
    }
}
