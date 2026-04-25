use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::{
    config::sovereignty::SovereigntyRequirements,
    models::{ApiKeyOwner, BudgetPeriod},
};

/// PKCE code challenge method supplied by the external app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub enum PkceCodeChallengeMethod {
    /// SHA-256 of the verifier, base64url-encoded. Recommended.
    S256,
    /// Plain text verifier. Only accepted when explicitly enabled in config.
    #[serde(rename = "plain")]
    Plain,
}

impl PkceCodeChallengeMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            PkceCodeChallengeMethod::S256 => "S256",
            PkceCodeChallengeMethod::Plain => "plain",
        }
    }
}

impl std::fmt::Display for PkceCodeChallengeMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for PkceCodeChallengeMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "S256" => Ok(Self::S256),
            "plain" => Ok(Self::Plain),
            other => Err(format!(
                "Invalid code_challenge_method '{other}'. Expected 'S256' or 'plain'."
            )),
        }
    }
}

/// User-chosen options for the API key that will be issued when the external
/// app redeems the code. Mirrors the fields the self-service "Create API
/// Key" modal exposes; `owner` is implied from the consenting user.
///
/// All fields are optional — the consent page populates whichever ones the
/// user filled in, and the token endpoint applies them to the issued key.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct OAuthKeyOptions {
    /// Owner the issued key should belong to. Defaults to the consenting
    /// user. The authorize endpoint validates that the user has permission
    /// to create keys for the chosen owner using the same RBAC rules as the
    /// admin "Create API Key" endpoint.
    pub owner: Option<ApiKeyOwner>,
    /// Friendly label for the issued key. Falls back to `app_name` then
    /// `"OAuth app"` when empty.
    pub name: Option<String>,
    /// Spending cap in cents. `None` or `0` means unlimited.
    pub budget_limit_cents: Option<i64>,
    /// Period the budget resets over. Required when `budget_limit_cents` is set.
    pub budget_period: Option<BudgetPeriod>,
    /// Optional expiration timestamp for the issued API key.
    pub expires_at: Option<DateTime<Utc>>,
    /// Permission scopes (e.g. `chat`, `embeddings`). `None` = full access.
    pub scopes: Option<Vec<String>>,
    /// Allowed model patterns. `None` = all models.
    pub allowed_models: Option<Vec<String>>,
    /// IP/CIDR allowlist. `None` = all IPs.
    pub ip_allowlist: Option<Vec<String>>,
    /// Per-key requests-per-minute override.
    pub rate_limit_rpm: Option<i32>,
    /// Per-key tokens-per-minute override.
    pub rate_limit_tpm: Option<i32>,
    /// Sovereignty requirements for model access.
    pub sovereignty_requirements: Option<SovereigntyRequirements>,
}

/// A pending PKCE authorization code, bound to a user and PKCE challenge.
#[derive(Debug, Clone)]
pub struct OAuthAuthorizationCode {
    pub id: Uuid,
    pub code: String,
    pub code_challenge: String,
    pub code_challenge_method: PkceCodeChallengeMethod,
    pub callback_url: String,
    pub user_id: Uuid,
    pub app_name: Option<String>,
    /// User's choices for the API key to be issued on exchange.
    pub key_options: OAuthKeyOptions,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Request body for `POST /admin/v1/oauth/authorize` — issued by the
/// authenticated user from the consent page after they click "Allow".
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateAuthorizationCode {
    /// Where the external app expects the user to land after consent.
    /// The exchange endpoint requires this exact same value when redeeming.
    #[validate(length(min = 1, max = 2048))]
    pub callback_url: String,
    /// PKCE challenge supplied by the external app.
    #[validate(length(min = 32, max = 255))]
    pub code_challenge: String,
    /// Method used to derive the challenge. `S256` unless `auth.oauth_pkce.allow_plain_method` is set.
    pub code_challenge_method: PkceCodeChallengeMethod,
    /// Optional human-readable name of the requesting app, shown on the consent screen.
    #[validate(length(max = 255))]
    pub app_name: Option<String>,
    /// User's choices for the API key that will be issued on exchange.
    /// Mirrors the fields shown in the self-service "Create API Key" modal.
    #[serde(default)]
    pub key_options: OAuthKeyOptions,
}

/// Response from `POST /admin/v1/oauth/authorize` after consent.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AuthorizationCodeResponse {
    /// Authorization code to forward to the external app via `callback_url?code=...`.
    pub code: String,
    /// Fully-formed redirect URL (callback_url with the `code` query parameter).
    pub redirect_url: String,
    /// When the code expires; the exchange must complete before this time.
    pub expires_at: DateTime<Utc>,
}

/// Request body for the public `POST /oauth/token` endpoint.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ExchangeCodeForKey {
    /// The authorization code received via the callback URL.
    pub code: String,
    /// The original PKCE verifier the external app generated.
    pub code_verifier: String,
    /// Method used to derive the challenge from the verifier. Defaults to `S256`.
    #[serde(default = "default_method")]
    pub code_challenge_method: PkceCodeChallengeMethod,
}

fn default_method() -> PkceCodeChallengeMethod {
    PkceCodeChallengeMethod::S256
}
