//! Audit logging utilities for guardrails events.
//!
//! This module provides utilities for guardrails audit logging.
//!
//! The main entry points for audit logging are in `src/routes/api.rs`:
//! - `log_guardrails_evaluation()` - Logs input guardrails evaluations
//! - `log_output_guardrails_evaluation()` - Logs output guardrails evaluations
//!
//! These functions are fire-and-forget: they spawn background tasks to log events
//! asynchronously to avoid impacting request latency.
//!
//! # Event Types Logged
//!
//! - `guardrails.block` - Blocked requests/responses
//! - `guardrails.warn` - Warnings (violations allowed)
//! - `guardrails.log` - Logged violations (no action taken)
//! - `guardrails.redact` - Redaction events (with content hashes, not actual content)
//! - `guardrails.allow` - Passed evaluations (when `log_all_evaluations` is true)
//!
//! # Configuration
//!
//! Audit logging is configured via `[features.guardrails.audit]`:
//!
//! ```toml
//! [features.guardrails.audit]
//! enabled = true
//! log_all_evaluations = false  # Only log violations, not all evaluations
//! log_blocked = true           # Log blocked requests/responses
//! log_violations = true        # Log policy violations
//! log_redacted = true          # Log redaction events (hashes, not content)
//! ```

use sha2::{Digest, Sha256};

/// Computes a SHA-256 hash of content for audit trail.
///
/// This allows tracking content changes without storing actual content
/// (important for PII and sensitive data). The hash can be used to:
/// - Verify content was redacted correctly
/// - Correlate redaction events across the audit log
/// - Prove content was seen without storing the actual content
///
/// # Example
///
/// ```rust,ignore
/// let original_hash = hash_content("Hello, my SSN is 123-45-6789");
/// let redacted_hash = hash_content("Hello, my SSN is [REDACTED]");
/// // Store both hashes in the audit log for verification
/// ```
pub fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_content() {
        let hash1 = hash_content("hello world");
        let hash2 = hash_content("hello world");
        let hash3 = hash_content("different content");

        // Same content produces same hash
        assert_eq!(hash1, hash2);

        // Different content produces different hash
        assert_ne!(hash1, hash3);

        // Hash is 64 hex characters (256 bits)
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_hash_content_empty() {
        let hash = hash_content("");
        // SHA-256 of empty string is a known value
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_hash_content_unicode() {
        let hash = hash_content("Hello, 世界! 🌍");
        // Should produce a valid 64-char hex hash
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
