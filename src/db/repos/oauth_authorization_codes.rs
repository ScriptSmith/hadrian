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

    /// Look up an active (unused, unexpired) code without mutating it.
    ///
    /// The token endpoint calls this first so PKCE verification can happen
    /// before the code is consumed — otherwise an attacker who intercepts
    /// the code (e.g. via referrer leakage) could permanently burn it by
    /// submitting a wrong verifier, denying the legitimate caller.
    async fn lookup_active(&self, code: &str) -> DbResult<Option<OAuthAuthorizationCode>>;

    /// Atomically claim a code by setting `used_at`. Returns the row only if
    /// the code exists, has not been claimed yet, and has not expired.
    /// Callers must verify PKCE *before* calling this so a bad verifier
    /// doesn't burn the code.
    async fn consume(&self, code: &str) -> DbResult<Option<OAuthAuthorizationCode>>;

    /// Delete codes that have expired before `before`, plus any code that
    /// has already been consumed (regardless of when). Used by the periodic
    /// cleanup job — consumed codes have nothing left to do, so the cutoff
    /// only applies to the "expired but never used" arm.
    async fn delete_stale(&self, before: DateTime<Utc>) -> DbResult<u64>;
}
