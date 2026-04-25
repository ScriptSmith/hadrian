use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    db::error::DbResult,
    models::{OAuthAuthorizationCode, OAuthKeyOptions, PkceCodeChallengeMethod},
};

/// Input for inserting a new authorization code.
#[derive(Debug, Clone)]
pub struct NewAuthorizationCode {
    pub code: String,
    pub code_challenge: String,
    pub code_challenge_method: PkceCodeChallengeMethod,
    pub callback_url: String,
    pub user_id: Uuid,
    pub app_name: Option<String>,
    pub key_options: OAuthKeyOptions,
    pub expires_at: DateTime<Utc>,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait OAuthAuthorizationCodeRepo: Send + Sync {
    /// Insert a new pending authorization code.
    async fn insert(&self, input: NewAuthorizationCode) -> DbResult<OAuthAuthorizationCode>;

    /// Atomically claim a code by setting `used_at`. Returns the row only if
    /// the code exists, has not been claimed yet, and has not expired.
    async fn consume(&self, code: &str) -> DbResult<Option<OAuthAuthorizationCode>>;

    /// Delete codes that expired or were consumed before `before`. Used by
    /// the periodic cleanup job.
    async fn delete_stale(&self, before: DateTime<Utc>) -> DbResult<u64>;
}
