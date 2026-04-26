use std::{sync::Arc, time::Duration as StdDuration};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, Utc};
use rand::RngCore;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    cache::Cache,
    db::{DbPool, DbResult, NewAuthorizationCode},
    models::{OAuthAuthorizationCode, OAuthKeyOptions, PkceCodeChallengeMethod},
};

/// How many failed PKCE verifications a single authorization code may suffer
/// before it is destroyed. The choice trades two attacks against each other:
/// burning on the first failure lets a network attacker who can write any
/// request DoS legitimate users; never burning lets an attacker who actually
/// stole the code keep guessing the verifier offline. Three matches the OAuth
/// security BCP guidance on "limited" retries.
const MAX_PKCE_FAILURES_PER_CODE: i64 = 3;
/// TTL for the failure counter. Authorization codes themselves live ~10 min,
/// so the counter is forced to outlive any reasonable code lifetime — that
/// way the count for a given code can't be reset by waiting it out.
const PKCE_FAILURE_TTL: StdDuration = StdDuration::from_secs(900);

/// Errors specific to the OAuth PKCE service. Mapped to HTTP status codes
/// by the route handlers.
#[derive(Debug, Error)]
pub enum OAuthPkceError {
    #[error("Authorization code is invalid, expired, or already used")]
    InvalidCode,
    #[error("PKCE verification failed")]
    PkceMismatch,
    #[error("Database error: {0}")]
    Db(#[from] crate::db::DbError),
}

/// Input bundle for issuing a new authorization code, populated from the
/// authorize request handler.
#[derive(Debug, Clone)]
pub struct IssueCodeInput {
    pub user_id: Uuid,
    pub callback_url: String,
    pub code_challenge: String,
    pub code_challenge_method: PkceCodeChallengeMethod,
    pub app_name: Option<String>,
    pub key_options: OAuthKeyOptions,
    pub ttl_seconds: u64,
}

/// Service for the OAuth-style PKCE flow.
#[derive(Clone)]
pub struct OAuthPkceService {
    db: Arc<DbPool>,
    /// Optional cache backing the per-code failure counter. When absent we
    /// fall back to the legacy "never burn on failure" behaviour because we
    /// have nowhere to track attempts; deployments that care about the
    /// limited-retry guarantee should configure a cache backend.
    cache: Option<Arc<dyn Cache>>,
}

impl OAuthPkceService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db, cache: None }
    }

    pub fn with_cache(mut self, cache: Option<Arc<dyn Cache>>) -> Self {
        self.cache = cache;
        self
    }

    /// Generate and persist a new authorization code bound to `user_id` and
    /// the supplied PKCE challenge. Returns the stored row; the caller forms
    /// the redirect URL.
    pub async fn issue_code(&self, input: IssueCodeInput) -> DbResult<OAuthAuthorizationCode> {
        let code = generate_code();
        let expires_at = Utc::now() + Duration::seconds(input.ttl_seconds as i64);

        self.db
            .oauth_authorization_codes()
            .insert(NewAuthorizationCode {
                code,
                code_challenge: input.code_challenge,
                code_challenge_method: input.code_challenge_method,
                callback_url: input.callback_url,
                user_id: input.user_id,
                app_name: input.app_name,
                key_options: input.key_options,
                expires_at,
            })
            .await
    }

    /// Verify the supplied `code_verifier` against a stored authorization
    /// code, and consume the code only if verification passes. Returns the
    /// stored row on success so the caller can issue an API key under the
    /// bound user.
    ///
    /// Per RFC 7636 §4.5, the server already knows the challenge method
    /// from the authorization request — `client_method` is an optional
    /// client-side hint we sanity-check but otherwise ignore.
    ///
    /// Order of operations matters: we look the code up *without* mutating
    /// it, run PKCE verification, and only then atomically claim it. If the
    /// verifier is wrong, the code stays usable so the legitimate caller
    /// can retry — otherwise an attacker who intercepted the code in
    /// transit could permanently burn it by submitting any wrong verifier.
    /// The consume step is still atomic, so concurrent honest redemptions
    /// can't both succeed.
    pub async fn redeem_code(
        &self,
        code: &str,
        code_verifier: &str,
        client_method: Option<PkceCodeChallengeMethod>,
    ) -> Result<OAuthAuthorizationCode, OAuthPkceError> {
        let repo = self.db.oauth_authorization_codes();

        let stored = repo
            .lookup_active(code)
            .await?
            .ok_or(OAuthPkceError::InvalidCode)?;

        // If the client supplied a method, it must match what we stored.
        // RFC 7636 doesn't require resubmission, so a missing value is fine.
        if let Some(client_method) = client_method
            && client_method != stored.code_challenge_method
        {
            return Err(OAuthPkceError::PkceMismatch);
        }

        let derived = derive_challenge(code_verifier, stored.code_challenge_method);
        if derived
            .as_bytes()
            .ct_eq(stored.code_challenge.as_bytes())
            .unwrap_u8()
            != 1
        {
            // Bump the per-code failure counter. Once the threshold is hit
            // we burn the code so an attacker who stole it can't keep
            // probing verifiers. We still hand out the same `PkceMismatch`
            // error either way so the attacker can't probe for "this code
            // is now burned" vs "still alive".
            self.record_pkce_failure(code).await;
            return Err(OAuthPkceError::PkceMismatch);
        }

        // PKCE verified — now atomically claim the code. If a concurrent
        // redemption already won, treat the code as gone (InvalidCode)
        // rather than handing out a second key.
        repo.consume(code).await?.ok_or(OAuthPkceError::InvalidCode)
    }

    /// Increment the per-code PKCE failure counter and burn the code once it
    /// exceeds `MAX_PKCE_FAILURES_PER_CODE`. Cache errors are swallowed: if
    /// the cache is unavailable we fall back to the original (no-burn)
    /// behaviour rather than blocking authentication.
    async fn record_pkce_failure(&self, code: &str) {
        let Some(cache) = &self.cache else {
            return;
        };
        let key = pkce_failure_key(code);
        match cache.incr(&key, PKCE_FAILURE_TTL).await {
            Ok(count) if count >= MAX_PKCE_FAILURES_PER_CODE => {
                // Burn the code. Failures from a network attacker or a
                // genuinely broken client both end up here; the legitimate
                // user has had `MAX_PKCE_FAILURES_PER_CODE - 1` chances to
                // retry, which is enough headroom for a transient bug.
                if let Err(e) = self.db.oauth_authorization_codes().consume(code).await {
                    tracing::warn!(error = %e, "Failed to burn PKCE code after repeated verifier failures");
                }
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(error = %e, "Failed to record PKCE failure counter; not burning code");
            }
        }
    }
}

/// Cache key for the per-code PKCE failure counter. The code itself is
/// hashed so we never persist a raw authorization code in the cache.
fn pkce_failure_key(code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code.as_bytes());
    let digest = hasher.finalize();
    format!("gw:oauth:pkce:fails:{:x}", digest)
}

/// Generate a 256-bit URL-safe base64 random code (~43 chars).
fn generate_code() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Compute the PKCE challenge from a verifier per RFC 7636.
fn derive_challenge(verifier: &str, method: PkceCodeChallengeMethod) -> String {
    match method {
        PkceCodeChallengeMethod::Plain => verifier.to_string(),
        PkceCodeChallengeMethod::S256 => {
            let mut hasher = Sha256::new();
            hasher.update(verifier.as_bytes());
            let digest = hasher.finalize();
            URL_SAFE_NO_PAD.encode(digest)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s256_challenge_matches_rfc7636_example() {
        // Example verifier from RFC 7636 §4.2: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let expected = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert_eq!(
            derive_challenge(verifier, PkceCodeChallengeMethod::S256),
            expected
        );
    }

    #[test]
    fn plain_challenge_returns_verifier() {
        assert_eq!(
            derive_challenge("abc", PkceCodeChallengeMethod::Plain),
            "abc"
        );
    }

    #[test]
    fn generated_codes_are_unique_and_url_safe() {
        let a = generate_code();
        let b = generate_code();
        assert_ne!(a, b);
        assert!(!a.contains('+') && !a.contains('/') && !a.contains('='));
        assert!(a.len() >= 40);
    }
}
