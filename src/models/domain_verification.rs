use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Domain verification status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum DomainVerificationStatus {
    /// Verification is pending - waiting for DNS record
    #[default]
    Pending,
    /// Domain has been successfully verified
    Verified,
    /// Verification failed (e.g., DNS record not found)
    Failed,
}

impl std::fmt::Display for DomainVerificationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DomainVerificationStatus::Pending => write!(f, "pending"),
            DomainVerificationStatus::Verified => write!(f, "verified"),
            DomainVerificationStatus::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for DomainVerificationStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(DomainVerificationStatus::Pending),
            "verified" => Ok(DomainVerificationStatus::Verified),
            "failed" => Ok(DomainVerificationStatus::Failed),
            _ => Err(format!("Invalid domain verification status: {}", s)),
        }
    }
}

/// Domain verification record for SSO.
///
/// Tracks the verification status of email domains claimed by an organization's
/// SSO configuration. Domains must be verified via DNS TXT record before SSO
/// can be enforced for users with that email domain.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DomainVerification {
    /// Unique identifier for this verification record
    pub id: Uuid,
    /// SSO config this verification belongs to
    pub org_sso_config_id: Uuid,
    /// The domain being verified (e.g., "acme.com")
    pub domain: String,
    /// Random token for DNS TXT record verification
    pub verification_token: String,
    /// Current verification status
    pub status: DomainVerificationStatus,
    /// The actual DNS TXT record found during verification (for audit)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns_txt_record: Option<String>,
    /// Number of verification attempts
    pub verification_attempts: i32,
    /// Last verification attempt timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_attempt_at: Option<DateTime<Utc>>,
    /// When the domain was successfully verified
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<DateTime<Utc>>,
    /// Optional: require re-verification after this date
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    /// When this record was created
    pub created_at: DateTime<Utc>,
    /// When this record was last updated
    pub updated_at: DateTime<Utc>,
}

impl DomainVerification {
    /// Returns the DNS TXT record name that should be created for verification.
    /// Format: `_hadrian-verify.{domain}`
    pub fn dns_record_name(&self) -> String {
        format!("_hadrian-verify.{}", self.domain)
    }

    /// Returns the expected DNS TXT record value.
    /// Format: `hadrian-verify={token}`
    pub fn expected_dns_value(&self) -> String {
        format!("hadrian-verify={}", self.verification_token)
    }

    /// Returns true if this domain is currently verified and not expired.
    pub fn is_verified(&self) -> bool {
        if self.status != DomainVerificationStatus::Verified {
            return false;
        }
        // Check if verification has expired
        if let Some(expires_at) = self.expires_at
            && expires_at < Utc::now()
        {
            return false;
        }
        true
    }
}

/// Request to initiate domain verification.
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateDomainVerification {
    /// The domain to verify (e.g., "acme.com")
    #[validate(length(min = 1, max = 255))]
    pub domain: String,
}

/// Request to update a domain verification record.
#[derive(Debug, Clone, Default, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateDomainVerification {
    /// Update verification status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<DomainVerificationStatus>,
    /// Update DNS TXT record found
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns_txt_record: Option<Option<String>>,
    /// Update verification attempts count
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_attempts: Option<i32>,
    /// Update last attempt timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_attempt_at: Option<Option<DateTime<Utc>>>,
    /// Update verified at timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<Option<DateTime<Utc>>>,
    /// Update expiration date
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<Option<DateTime<Utc>>>,
}

/// Response from a verification attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct VerifyDomainResponse {
    /// Whether the verification succeeded
    pub verified: bool,
    /// The DNS TXT record that was found (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns_record_found: Option<String>,
    /// Human-readable message explaining the result
    pub message: String,
    /// Updated verification record
    pub verification: DomainVerification,
}

/// Instructions for verifying a domain.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DomainVerificationInstructions {
    /// The domain being verified
    pub domain: String,
    /// DNS record type to create
    pub record_type: String,
    /// DNS record host/name
    pub record_host: String,
    /// DNS record value
    pub record_value: String,
    /// Human-readable instructions
    pub instructions: String,
}

impl DomainVerification {
    /// Generate verification instructions for display in the UI.
    pub fn instructions(&self) -> DomainVerificationInstructions {
        DomainVerificationInstructions {
            domain: self.domain.clone(),
            record_type: "TXT".to_string(),
            record_host: self.dns_record_name(),
            record_value: self.expected_dns_value(),
            instructions: format!(
                "To verify ownership of {}, add a DNS TXT record:\n\n\
                Host: {}\n\
                Type: TXT\n\
                Value: {}\n\n\
                DNS changes can take up to 72 hours to propagate. \
                Click \"Verify\" once the record is added.",
                self.domain,
                self.dns_record_name(),
                self.expected_dns_value()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dns_record_name() {
        let verification = DomainVerification {
            id: Uuid::new_v4(),
            org_sso_config_id: Uuid::new_v4(),
            domain: "acme.com".to_string(),
            verification_token: "abc123xyz".to_string(),
            status: DomainVerificationStatus::Pending,
            dns_txt_record: None,
            verification_attempts: 0,
            last_attempt_at: None,
            verified_at: None,
            expires_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(verification.dns_record_name(), "_hadrian-verify.acme.com");
        assert_eq!(
            verification.expected_dns_value(),
            "hadrian-verify=abc123xyz"
        );
    }

    #[test]
    fn test_is_verified() {
        let mut verification = DomainVerification {
            id: Uuid::new_v4(),
            org_sso_config_id: Uuid::new_v4(),
            domain: "acme.com".to_string(),
            verification_token: "abc123xyz".to_string(),
            status: DomainVerificationStatus::Pending,
            dns_txt_record: None,
            verification_attempts: 0,
            last_attempt_at: None,
            verified_at: None,
            expires_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Pending status - not verified
        assert!(!verification.is_verified());

        // Verified status without expiration
        verification.status = DomainVerificationStatus::Verified;
        assert!(verification.is_verified());

        // Verified but expired
        verification.expires_at = Some(Utc::now() - chrono::Duration::hours(1));
        assert!(!verification.is_verified());

        // Verified with future expiration
        verification.expires_at = Some(Utc::now() + chrono::Duration::days(90));
        assert!(verification.is_verified());
    }

    #[test]
    fn test_status_parsing() {
        assert_eq!(
            "pending".parse::<DomainVerificationStatus>().unwrap(),
            DomainVerificationStatus::Pending
        );
        assert_eq!(
            "verified".parse::<DomainVerificationStatus>().unwrap(),
            DomainVerificationStatus::Verified
        );
        assert_eq!(
            "failed".parse::<DomainVerificationStatus>().unwrap(),
            DomainVerificationStatus::Failed
        );
        assert!("invalid".parse::<DomainVerificationStatus>().is_err());
    }
}
