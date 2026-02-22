//! Data retention module for automatic purging of old data.
//!
//! This module provides a background worker that periodically:
//! 1. Deletes usage records older than the configured retention period
//! 2. Deletes daily spend aggregates older than the configured retention period
//! 3. Deletes audit logs older than the configured retention period
//! 4. Hard-deletes soft-deleted conversations after their grace period
//!
//! All deletion operations are batched to avoid long-running transactions
//! and support dry-run mode for testing retention policies.

mod worker;

pub use worker::start_retention_worker;
