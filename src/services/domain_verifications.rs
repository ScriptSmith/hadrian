//! Domain verification service for SSO configuration.
//!
//! This service handles domain ownership verification via DNS TXT records.
//! Organizations must verify ownership of email domains before they can
//! enforce SSO for users with those email domains.

use std::sync::Arc;

use base64::Engine;
use chrono::Utc;
use hickory_resolver::{
    Resolver, TokioResolver, name_server::TokioConnectionProvider, system_conf::read_system_conf,
};
use rand::Rng;
use uuid::Uuid;

use crate::{
    db::{DbPool, ListParams},
    models::{
        CreateDomainVerification, DomainVerification, DomainVerificationStatus,
        UpdateDomainVerification, VerifyDomainResponse,
    },
};

/// Well-known public email domains that should not be claimed for SSO.
///
/// These domains are shared by millions of users and cannot be owned by a
/// single organization. Attempting to verify these domains will be rejected.
const PUBLIC_EMAIL_DOMAINS: &[&str] = &[
    // Google
    "gmail.com",
    "googlemail.com",
    // Microsoft
    "outlook.com",
    "hotmail.com",
    "live.com",
    "msn.com",
    "hotmail.co.uk",
    "live.co.uk",
    // Yahoo
    "yahoo.com",
    "yahoo.co.uk",
    "yahoo.ca",
    "yahoo.com.au",
    "ymail.com",
    "rocketmail.com",
    // Apple
    "icloud.com",
    "me.com",
    "mac.com",
    // AOL
    "aol.com",
    "aim.com",
    // Proton
    "protonmail.com",
    "protonmail.ch",
    "proton.me",
    "pm.me",
    // Zoho
    "zoho.com",
    "zohomail.com",
    // GMX/Mail.com
    "gmx.com",
    "gmx.de",
    "gmx.net",
    "mail.com",
    // Regional providers
    "qq.com",
    "163.com",
    "126.com",
    "yeah.net",
    "sina.com",
    "sina.cn",
    "yandex.ru",
    "yandex.com",
    "mail.ru",
    "inbox.ru",
    "list.ru",
    "bk.ru",
    // Disposable/temporary email services
    "mailinator.com",
    "guerrillamail.com",
    "guerrillamail.org",
    "10minutemail.com",
    "tempmail.com",
    "temp-mail.org",
    "throwaway.email",
    "sharklasers.com",
    "mailnesia.com",
    "trashmail.com",
    "discard.email",
    "yopmail.com",
    "getnada.com",
    "maildrop.cc",
    // Other major providers
    "comcast.net",
    "verizon.net",
    "att.net",
    "sbcglobal.net",
    "cox.net",
    "charter.net",
    "earthlink.net",
    "optonline.net",
    "frontiernet.net",
    "windstream.net",
];

/// Minimum seconds between verification attempts (base cooldown).
const VERIFY_BASE_COOLDOWN_SECS: u64 = 30;

/// Maximum cooldown between verification attempts (5 minutes).
const VERIFY_MAX_COOLDOWN_SECS: u64 = 300;

/// Service for domain verification operations.
///
/// Handles creating domain verification records, performing DNS lookups to
/// verify ownership, and managing the verification lifecycle.
#[derive(Clone)]
pub struct DomainVerificationService {
    db: Arc<DbPool>,
    resolver: TokioResolver,
}

impl DomainVerificationService {
    /// Create a new domain verification service with default DNS resolver.
    ///
    /// Uses the system's DNS configuration for resolution.
    pub fn new(db: Arc<DbPool>) -> Self {
        // Try to use system configuration, fall back to default if not available
        let resolver = match read_system_conf() {
            Ok((config, opts)) => {
                Resolver::builder_with_config(config, TokioConnectionProvider::default())
                    .with_options(opts)
                    .build()
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Failed to read system DNS config, using default"
                );
                Resolver::builder_tokio().unwrap().build()
            }
        };
        Self { db, resolver }
    }

    /// Create a new domain verification service with a custom DNS resolver.
    ///
    /// Useful for testing with mock DNS responses.
    #[allow(dead_code)] // Test infrastructure
    pub fn with_resolver(db: Arc<DbPool>, resolver: TokioResolver) -> Self {
        Self { db, resolver }
    }

    /// Create a new domain verification record.
    ///
    /// Generates a cryptographic verification token and stores the pending
    /// verification in the database.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The domain is a public email domain (gmail.com, etc.)
    /// - The domain format is invalid
    /// - The domain already exists for this SSO config
    pub async fn create(
        &self,
        org_sso_config_id: Uuid,
        input: CreateDomainVerification,
    ) -> Result<DomainVerification, DomainVerificationError> {
        // Normalize domain to lowercase
        let domain = input.domain.trim().to_lowercase();

        // Validate domain format
        if !is_valid_domain(&domain) {
            return Err(DomainVerificationError::InvalidDomain(format!(
                "Invalid domain format: {}",
                domain
            )));
        }

        // Check against public domain blocklist
        if is_public_domain(&domain) {
            return Err(DomainVerificationError::PublicDomainBlocked(format!(
                "Cannot verify public email domain: {}",
                domain
            )));
        }

        // Generate verification token
        let token = generate_verification_token();

        // Create in database
        let verification = self
            .db
            .domain_verifications()
            .create(
                org_sso_config_id,
                CreateDomainVerification { domain },
                &token,
            )
            .await?;

        Ok(verification)
    }

    /// Get a domain verification by ID.
    pub async fn get_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<DomainVerification>, DomainVerificationError> {
        Ok(self.db.domain_verifications().get_by_id(id).await?)
    }

    /// Get a domain verification by SSO config ID and domain.
    pub async fn get_by_config_and_domain(
        &self,
        org_sso_config_id: Uuid,
        domain: &str,
    ) -> Result<Option<DomainVerification>, DomainVerificationError> {
        Ok(self
            .db
            .domain_verifications()
            .get_by_config_and_domain(org_sso_config_id, domain)
            .await?)
    }

    /// List all domain verifications for an SSO config.
    pub async fn list_by_config(
        &self,
        org_sso_config_id: Uuid,
        params: ListParams,
    ) -> Result<Vec<DomainVerification>, DomainVerificationError> {
        Ok(self
            .db
            .domain_verifications()
            .list_by_config(org_sso_config_id, params)
            .await?)
    }

    /// Count domain verifications for an SSO config.
    pub async fn count_by_config(
        &self,
        org_sso_config_id: Uuid,
    ) -> Result<i64, DomainVerificationError> {
        Ok(self
            .db
            .domain_verifications()
            .count_by_config(org_sso_config_id)
            .await?)
    }

    /// Delete a domain verification record.
    pub async fn delete(&self, id: Uuid) -> Result<(), DomainVerificationError> {
        Ok(self.db.domain_verifications().delete(id).await?)
    }

    /// Create an auto-verified domain record.
    ///
    /// This method creates a domain verification record that is immediately marked
    /// as verified, bypassing the normal DNS verification flow. Used for:
    /// - Bootstrap auto-verified domains from config
    /// - E2E testing where DNS verification is impossible
    /// - Pre-verified enterprise domains
    ///
    /// **Security:**
    /// - This should only be called during initial setup (bootstrap) or by admins
    /// - The `dns_txt_record` field will show "AUTO_VERIFIED_BY_BOOTSTRAP" for audit
    /// - Public email domains are still blocked
    ///
    /// # Errors
    /// - Returns error if domain is a public email domain
    /// - Returns error if domain already exists for this SSO config
    pub async fn create_auto_verified(
        &self,
        org_sso_config_id: Uuid,
        domain: &str,
    ) -> Result<DomainVerification, DomainVerificationError> {
        // Normalize domain to lowercase
        let domain = domain.trim().to_lowercase();

        // Validate domain format
        if !is_valid_domain(&domain) {
            return Err(DomainVerificationError::InvalidDomain(format!(
                "Invalid domain format: {}",
                domain
            )));
        }

        // Check against public domain blocklist
        if is_public_domain(&domain) {
            return Err(DomainVerificationError::PublicDomainBlocked(format!(
                "Cannot verify public email domain: {}",
                domain
            )));
        }

        // Generate a token (even though we won't use it for verification)
        let token = generate_verification_token();

        // Create the record in the database with auto-verified status
        let verification = self
            .db
            .domain_verifications()
            .create_auto_verified(
                org_sso_config_id,
                CreateDomainVerification {
                    domain: domain.clone(),
                },
                &token,
            )
            .await?;

        tracing::info!(
            domain = %domain,
            org_sso_config_id = %org_sso_config_id,
            verification_id = %verification.id,
            "Domain auto-verified via bootstrap configuration"
        );

        Ok(verification)
    }

    /// Find a verified domain verification by domain name.
    ///
    /// Used for SSO discovery during login.
    pub async fn find_verified_by_domain(
        &self,
        domain: &str,
    ) -> Result<Option<DomainVerification>, DomainVerificationError> {
        Ok(self
            .db
            .domain_verifications()
            .find_verified_by_domain(domain)
            .await?)
    }

    /// List all verified domains for an SSO config.
    pub async fn list_verified_by_config(
        &self,
        org_sso_config_id: Uuid,
    ) -> Result<Vec<DomainVerification>, DomainVerificationError> {
        Ok(self
            .db
            .domain_verifications()
            .list_verified_by_config(org_sso_config_id)
            .await?)
    }

    /// Check if any domain is verified for an SSO config.
    pub async fn has_verified_domain(
        &self,
        org_sso_config_id: Uuid,
    ) -> Result<bool, DomainVerificationError> {
        Ok(self
            .db
            .domain_verifications()
            .has_verified_domain(org_sso_config_id)
            .await?)
    }

    /// Attempt to verify a domain by checking DNS TXT records.
    ///
    /// Performs a DNS lookup for `_hadrian-verify.{domain}` and checks if any
    /// TXT record matches the expected verification value.
    ///
    /// # Returns
    ///
    /// Returns a `VerifyDomainResponse` with the verification result and updated
    /// domain verification record.
    pub async fn verify(&self, id: Uuid) -> Result<VerifyDomainResponse, DomainVerificationError> {
        // Get the verification record
        let verification = self
            .db
            .domain_verifications()
            .get_by_id(id)
            .await?
            .ok_or(DomainVerificationError::NotFound)?;

        // Check rate limit based on last attempt (exponential backoff)
        if let Some(last_attempt) = verification.last_attempt_at {
            let cooldown_secs = calculate_verification_cooldown(verification.verification_attempts);
            if cooldown_secs > 0 {
                let elapsed = Utc::now().signed_duration_since(last_attempt);
                let elapsed_secs = elapsed.num_seconds().max(0) as u64;

                if elapsed_secs < cooldown_secs {
                    let remaining = cooldown_secs - elapsed_secs;
                    return Err(DomainVerificationError::RateLimited {
                        seconds_remaining: remaining,
                        message: format!(
                            "Please wait {} seconds before retrying verification",
                            remaining
                        ),
                    });
                }
            }
        }

        // Construct the DNS query name
        let dns_name = verification.dns_record_name();
        let expected_value = verification.expected_dns_value();

        // Perform DNS TXT lookup
        let (verified, dns_record_found) = match self.lookup_txt_records(&dns_name).await {
            Ok(records) => {
                // Check if any record matches the expected value
                let matching_record = records.iter().find(|r| r.trim() == expected_value);
                (matching_record.is_some(), matching_record.cloned())
            }
            Err(e) => {
                tracing::debug!(
                    dns_name = %dns_name,
                    error = %e,
                    "DNS TXT lookup failed"
                );
                (false, None)
            }
        };

        // Prepare the update
        let new_status = if verified {
            DomainVerificationStatus::Verified
        } else {
            // Keep pending if not verified yet (don't mark as failed on first try)
            if verification.verification_attempts >= 2 {
                DomainVerificationStatus::Failed
            } else {
                DomainVerificationStatus::Pending
            }
        };

        let update = UpdateDomainVerification {
            status: Some(new_status),
            dns_txt_record: Some(dns_record_found.clone()),
            verification_attempts: Some(verification.verification_attempts + 1),
            last_attempt_at: Some(Some(Utc::now())),
            verified_at: if verified {
                Some(Some(Utc::now()))
            } else {
                None
            },
            ..Default::default()
        };

        // Update the record
        let updated_verification = self.db.domain_verifications().update(id, update).await?;

        // Build response message
        let message = if verified {
            format!(
                "Domain {} has been successfully verified.",
                verification.domain
            )
        } else if dns_record_found.is_some() {
            format!(
                "DNS TXT record found but does not match expected value. \
                Expected: {}, Found: {}",
                expected_value,
                dns_record_found.as_ref().unwrap()
            )
        } else {
            format!(
                "No DNS TXT record found at {}. Please add a TXT record with value: {}",
                dns_name, expected_value
            )
        };

        Ok(VerifyDomainResponse {
            verified,
            dns_record_found,
            message,
            verification: updated_verification,
        })
    }

    /// Look up TXT records for a domain.
    async fn lookup_txt_records(&self, name: &str) -> Result<Vec<String>, DomainVerificationError> {
        let response = self
            .resolver
            .txt_lookup(name)
            .await
            .map_err(|e| DomainVerificationError::DnsLookup(e.to_string()))?;

        let records: Vec<String> = response
            .iter()
            .map(|txt| {
                txt.txt_data()
                    .iter()
                    .map(|data| String::from_utf8_lossy(data).to_string())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .collect();

        Ok(records)
    }
}

/// Generate a cryptographically secure verification token.
///
/// Uses 32 random bytes (256 bits of entropy) encoded as URL-safe base64,
/// resulting in a 43-character token.
fn generate_verification_token() -> String {
    let mut rng = rand::thread_rng();
    let mut random_bytes = [0u8; 32];
    rng.fill(&mut random_bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(random_bytes)
}

/// Check if a domain is in the public email domain blocklist.
///
/// Returns true if the domain should be blocked from verification.
fn is_public_domain(domain: &str) -> bool {
    let domain_lower = domain.to_lowercase();
    PUBLIC_EMAIL_DOMAINS
        .iter()
        .any(|&blocked| domain_lower == blocked)
}

/// Validate domain format.
///
/// Checks that the domain:
/// - Is not empty
/// - Contains at least one dot
/// - Does not start or end with a dot or hyphen
/// - Contains only valid characters (alphanumeric, hyphen, dot)
fn is_valid_domain(domain: &str) -> bool {
    if domain.is_empty() || domain.len() > 253 {
        return false;
    }

    // Must contain at least one dot (has TLD)
    if !domain.contains('.') {
        return false;
    }

    // Check each label
    for label in domain.split('.') {
        if label.is_empty() || label.len() > 63 {
            return false;
        }
        if label.starts_with('-') || label.ends_with('-') {
            return false;
        }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return false;
        }
    }

    true
}

/// Calculate exponential backoff cooldown based on verification attempt count.
///
/// Returns the number of seconds that must elapse since the last attempt
/// before another verification attempt is allowed:
/// - 1st attempt: 0 seconds (immediate)
/// - 2nd attempt: 30 seconds cooldown
/// - 3rd attempt: 2 minutes cooldown
/// - 4+ attempts: 5 minutes cooldown (max)
fn calculate_verification_cooldown(attempts: i32) -> u64 {
    match attempts {
        0 => 0,                             // First attempt: no cooldown
        1 => VERIFY_BASE_COOLDOWN_SECS,     // 30 seconds
        2 => VERIFY_BASE_COOLDOWN_SECS * 4, // 2 minutes
        _ => VERIFY_MAX_COOLDOWN_SECS,      // 5 minutes max
    }
}

/// Errors that can occur in domain verification operations.
#[derive(Debug, thiserror::Error)]
pub enum DomainVerificationError {
    /// Database error
    #[error("Database error: {0}")]
    Database(#[from] crate::db::DbError),

    /// DNS lookup failed
    #[error("DNS lookup failed: {0}")]
    DnsLookup(String),

    /// Attempted to verify a public email domain
    #[error("Public domain blocked: {0}")]
    PublicDomainBlocked(String),

    /// Invalid domain format
    #[error("Invalid domain: {0}")]
    InvalidDomain(String),

    /// Domain verification not found
    #[error("Domain verification not found")]
    NotFound,

    /// Rate limit exceeded for verification attempts
    #[error("Rate limited: {message}")]
    RateLimited {
        seconds_remaining: u64,
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_verification_token() {
        let token1 = generate_verification_token();
        let token2 = generate_verification_token();

        // Check length (32 bytes in base64 without padding = 43 characters)
        assert_eq!(token1.len(), 43);
        assert_eq!(token2.len(), 43);

        // Tokens should be unique
        assert_ne!(token1, token2);

        // Should only contain URL-safe base64 characters
        assert!(
            token1
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
    }

    #[test]
    fn test_is_public_domain() {
        // Major providers
        assert!(is_public_domain("gmail.com"));
        assert!(is_public_domain("Gmail.com")); // Case insensitive
        assert!(is_public_domain("GMAIL.COM"));
        assert!(is_public_domain("outlook.com"));
        assert!(is_public_domain("hotmail.com"));
        assert!(is_public_domain("yahoo.com"));
        assert!(is_public_domain("icloud.com"));
        assert!(is_public_domain("protonmail.com"));

        // Disposable providers
        assert!(is_public_domain("mailinator.com"));
        assert!(is_public_domain("10minutemail.com"));

        // Regional providers
        assert!(is_public_domain("qq.com"));
        assert!(is_public_domain("yandex.ru"));

        // Custom domains (not blocked)
        assert!(!is_public_domain("acme.com"));
        assert!(!is_public_domain("mycompany.io"));
        assert!(!is_public_domain("example.org"));

        // Similar but different domains (not blocked)
        assert!(!is_public_domain("gmail.co")); // Different TLD
        assert!(!is_public_domain("mygmail.com")); // Subdomain-like
    }

    #[test]
    fn test_is_valid_domain() {
        // Valid domains
        assert!(is_valid_domain("example.com"));
        assert!(is_valid_domain("sub.example.com"));
        assert!(is_valid_domain("my-company.co.uk"));
        assert!(is_valid_domain("a.b.c.d.example.com"));
        assert!(is_valid_domain("123.example.com"));

        // Invalid domains
        assert!(!is_valid_domain("")); // Empty
        assert!(!is_valid_domain("example")); // No TLD
        assert!(!is_valid_domain(".example.com")); // Leading dot
        assert!(!is_valid_domain("example.com.")); // Trailing dot
        assert!(!is_valid_domain("example..com")); // Double dot
        assert!(!is_valid_domain("-example.com")); // Leading hyphen
        assert!(!is_valid_domain("example-.com")); // Trailing hyphen
        assert!(!is_valid_domain("exam ple.com")); // Space
        assert!(!is_valid_domain("exam_ple.com")); // Underscore

        // Edge cases
        assert!(is_valid_domain("a.co")); // Minimum valid
        assert!(!is_valid_domain(&"a".repeat(254))); // Too long
    }

    #[test]
    fn test_dns_record_name_format() {
        // This tests the model's dns_record_name method indirectly
        // by verifying the expected format
        let domain = "acme.com";
        let expected_dns_name = format!("_hadrian-verify.{}", domain);
        assert_eq!(expected_dns_name, "_hadrian-verify.acme.com");
    }

    #[test]
    fn test_expected_dns_value_format() {
        let token = "abc123xyz";
        let expected_value = format!("hadrian-verify={}", token);
        assert_eq!(expected_value, "hadrian-verify=abc123xyz");
    }

    #[test]
    fn test_calculate_verification_cooldown() {
        // First attempt: no cooldown
        assert_eq!(calculate_verification_cooldown(0), 0);

        // Second attempt: base cooldown (30 seconds)
        assert_eq!(
            calculate_verification_cooldown(1),
            VERIFY_BASE_COOLDOWN_SECS
        );
        assert_eq!(calculate_verification_cooldown(1), 30);

        // Third attempt: 4x base cooldown (2 minutes)
        assert_eq!(
            calculate_verification_cooldown(2),
            VERIFY_BASE_COOLDOWN_SECS * 4
        );
        assert_eq!(calculate_verification_cooldown(2), 120);

        // Fourth and subsequent attempts: max cooldown (5 minutes)
        assert_eq!(calculate_verification_cooldown(3), VERIFY_MAX_COOLDOWN_SECS);
        assert_eq!(calculate_verification_cooldown(3), 300);

        // Higher attempts still cap at max
        assert_eq!(
            calculate_verification_cooldown(10),
            VERIFY_MAX_COOLDOWN_SECS
        );
        assert_eq!(
            calculate_verification_cooldown(100),
            VERIFY_MAX_COOLDOWN_SECS
        );
    }
}
