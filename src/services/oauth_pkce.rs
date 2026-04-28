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

    // ====================================================================
    // Integration tests against an in-memory SQLite DbPool. These cover the
    // full PKCE redeem path: code reuse, expiry, verifier mismatch, the
    // 3-strikes burn rule, and the plain-method client/server gate.
    // ====================================================================

    #[cfg(feature = "database-sqlite")]
    mod integration {
        use super::*;
        use crate::{
            cache::MemoryCache,
            config::MemoryCacheConfig,
            db::{DbPool, tests::harness::create_sqlite_pool},
            models::CreateUser,
        };

        async fn setup() -> (Arc<DbPool>, Uuid) {
            let pool = create_sqlite_pool().await;
            sqlx::migrate!("./migrations_sqlx/sqlite")
                .run(&pool)
                .await
                .expect("Failed to run SQLite migrations");
            let db = Arc::new(DbPool::from_sqlite(pool));
            // Insert a real user via the repo so the auth-code FK is
            // satisfied without us reaching into raw SQL.
            let user = db
                .users()
                .create(CreateUser {
                    external_id: format!("test-{}", Uuid::new_v4()),
                    email: Some(format!("user-{}@example.test", Uuid::new_v4())),
                    name: Some("Test User".to_string()),
                })
                .await
                .expect("create test user");
            (db, user.id)
        }

        fn issue_input(user_id: Uuid, challenge: &str, ttl_seconds: u64) -> IssueCodeInput {
            IssueCodeInput {
                user_id,
                callback_url: "https://example.test/cb".to_string(),
                code_challenge: challenge.to_string(),
                code_challenge_method: PkceCodeChallengeMethod::S256,
                app_name: Some("test app".to_string()),
                key_options: OAuthKeyOptions::default(),
                ttl_seconds,
            }
        }

        fn s256(verifier: &str) -> String {
            derive_challenge(verifier, PkceCodeChallengeMethod::S256)
        }

        #[tokio::test]
        async fn redeem_succeeds_then_reuse_fails() {
            let (db, user_id) = setup().await;
            let svc = OAuthPkceService::new(db.clone());
            let verifier = "verifier-12345678901234567890123456789012345678901234";
            let issued = svc
                .issue_code(issue_input(user_id, &s256(verifier), 600))
                .await
                .expect("issue code");

            // First redeem succeeds.
            svc.redeem_code(&issued.code, verifier, None)
                .await
                .expect("first redeem");

            // Second redeem fails — code was consumed.
            let err = svc
                .redeem_code(&issued.code, verifier, None)
                .await
                .expect_err("second redeem must fail");
            assert!(matches!(err, OAuthPkceError::InvalidCode));
        }

        #[tokio::test]
        async fn expired_code_rejected_as_invalid() {
            let (db, user_id) = setup().await;
            let svc = OAuthPkceService::new(db.clone());
            let verifier = "verifier-abcdefghijklmnopqrstuvwxyz0123456789ABCDEF01";
            // TTL of zero means the row is immediately past expires_at.
            let issued = svc
                .issue_code(issue_input(user_id, &s256(verifier), 0))
                .await
                .expect("issue code");

            // Sleep a hair so `expires_at < now` deterministically.
            tokio::time::sleep(StdDuration::from_millis(50)).await;

            let err = svc
                .redeem_code(&issued.code, verifier, None)
                .await
                .expect_err("expired code must not redeem");
            assert!(matches!(err, OAuthPkceError::InvalidCode));
        }

        #[tokio::test]
        async fn verifier_mismatch_keeps_code_alive_without_cache() {
            let (db, user_id) = setup().await;
            let svc = OAuthPkceService::new(db.clone());
            let verifier = "verifier-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
            let issued = svc
                .issue_code(issue_input(user_id, &s256(verifier), 600))
                .await
                .expect("issue code");

            // Without a cache, repeated wrong verifiers must NOT burn the code
            // (legitimate clients still need to be able to retry).
            for _ in 0..5 {
                let err = svc
                    .redeem_code(&issued.code, "wrong-verifier", None)
                    .await
                    .expect_err("wrong verifier must fail");
                assert!(matches!(err, OAuthPkceError::PkceMismatch));
            }

            // The original verifier still works.
            svc.redeem_code(&issued.code, verifier, None)
                .await
                .expect("legitimate redeem after retries");
        }

        #[tokio::test]
        async fn three_verifier_failures_burn_code_with_cache() {
            let (db, user_id) = setup().await;
            let cache: Arc<dyn Cache> = Arc::new(MemoryCache::new(&MemoryCacheConfig::default()));
            let svc = OAuthPkceService::new(db.clone()).with_cache(Some(cache));
            let verifier = "verifier-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
            let issued = svc
                .issue_code(issue_input(user_id, &s256(verifier), 600))
                .await
                .expect("issue code");

            // First two failures: PkceMismatch, code stays usable.
            for _ in 0..2 {
                let err = svc
                    .redeem_code(&issued.code, "wrong", None)
                    .await
                    .expect_err("wrong verifier #1/#2 must fail with mismatch");
                assert!(matches!(err, OAuthPkceError::PkceMismatch));
            }

            // Third failure: still PkceMismatch *to the caller* (so an
            // attacker can't probe for the burn boundary), but the code is
            // burned server-side.
            let err = svc
                .redeem_code(&issued.code, "wrong", None)
                .await
                .expect_err("wrong verifier #3 must fail with mismatch");
            assert!(matches!(err, OAuthPkceError::PkceMismatch));

            // After burn, the legitimate verifier no longer succeeds.
            let err = svc
                .redeem_code(&issued.code, verifier, None)
                .await
                .expect_err("legitimate redeem after burn must fail");
            assert!(matches!(err, OAuthPkceError::InvalidCode));
        }

        #[tokio::test]
        async fn client_method_must_match_stored() {
            let (db, user_id) = setup().await;
            let svc = OAuthPkceService::new(db.clone());
            let verifier = "verifier-ccccccccccccccccccccccccccccccccccccccccccc";
            let issued = svc
                .issue_code(issue_input(user_id, &s256(verifier), 600))
                .await
                .expect("issue code (S256)");

            // Client claims `plain` but server stored `S256` — reject before
            // even running the SHA-256 comparison.
            let err = svc
                .redeem_code(&issued.code, verifier, Some(PkceCodeChallengeMethod::Plain))
                .await
                .expect_err("method mismatch must reject");
            assert!(matches!(err, OAuthPkceError::PkceMismatch));
        }

        #[tokio::test]
        async fn plain_method_works_when_explicitly_chosen() {
            let (db, user_id) = setup().await;
            let svc = OAuthPkceService::new(db.clone());
            // Plain mode: challenge == verifier.
            let verifier = "plain-verifier-9999999999999999999999999999999999999";
            let mut input = issue_input(user_id, verifier, 600);
            input.code_challenge_method = PkceCodeChallengeMethod::Plain;
            let issued = svc.issue_code(input).await.expect("issue plain code");

            svc.redeem_code(&issued.code, verifier, Some(PkceCodeChallengeMethod::Plain))
                .await
                .expect("plain redeem succeeds");
        }
    }
}
