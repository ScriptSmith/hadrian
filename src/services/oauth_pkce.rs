use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, Utc};
use rand::RngCore;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    db::{DbPool, DbResult, NewAuthorizationCode},
    models::{OAuthAuthorizationCode, OAuthKeyOptions, PkceCodeChallengeMethod},
};

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
}

impl OAuthPkceService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
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

    /// Atomically consume an authorization code and verify the supplied
    /// `code_verifier` matches the stored challenge. Returns the row on
    /// success so the caller can issue an API key under the bound user.
    pub async fn redeem_code(
        &self,
        code: &str,
        code_verifier: &str,
        code_challenge_method: PkceCodeChallengeMethod,
    ) -> Result<OAuthAuthorizationCode, OAuthPkceError> {
        let stored = self
            .db
            .oauth_authorization_codes()
            .consume(code)
            .await?
            .ok_or(OAuthPkceError::InvalidCode)?;

        if stored.code_challenge_method != code_challenge_method {
            return Err(OAuthPkceError::PkceMismatch);
        }

        let derived = derive_challenge(code_verifier, code_challenge_method);
        if derived
            .as_bytes()
            .ct_eq(stored.code_challenge.as_bytes())
            .unwrap_u8()
            != 1
        {
            return Err(OAuthPkceError::PkceMismatch);
        }

        Ok(stored)
    }
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
