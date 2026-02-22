use async_trait::async_trait;
use uuid::Uuid;

use super::ListParams;
use crate::{
    db::error::DbResult,
    models::{CreateDomainVerification, DomainVerification, UpdateDomainVerification},
};

/// Repository for domain verification records.
///
/// Domain verifications track the ownership verification status of email domains
/// claimed by an organization's SSO configuration. Domains must be verified via
/// DNS TXT record before SSO can be enforced for users with that email domain.
#[async_trait]
pub trait DomainVerificationRepo: Send + Sync {
    /// Create a new domain verification record.
    ///
    /// # Arguments
    /// * `org_sso_config_id` - The SSO config this verification belongs to
    /// * `input` - The domain verification details
    /// * `verification_token` - The random token for DNS verification
    ///
    /// # Errors
    /// Returns an error if the domain already exists for this SSO config.
    async fn create(
        &self,
        org_sso_config_id: Uuid,
        input: CreateDomainVerification,
        verification_token: &str,
    ) -> DbResult<DomainVerification>;

    /// Get a domain verification by its ID.
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<DomainVerification>>;

    /// Get a domain verification by SSO config ID and domain.
    async fn get_by_config_and_domain(
        &self,
        org_sso_config_id: Uuid,
        domain: &str,
    ) -> DbResult<Option<DomainVerification>>;

    /// List all domain verifications for an SSO config.
    async fn list_by_config(
        &self,
        org_sso_config_id: Uuid,
        params: ListParams,
    ) -> DbResult<Vec<DomainVerification>>;

    /// Count domain verifications for an SSO config.
    async fn count_by_config(&self, org_sso_config_id: Uuid) -> DbResult<i64>;

    /// Update a domain verification record.
    async fn update(
        &self,
        id: Uuid,
        input: UpdateDomainVerification,
    ) -> DbResult<DomainVerification>;

    /// Delete a domain verification record (hard delete).
    async fn delete(&self, id: Uuid) -> DbResult<()>;

    /// Find a verified domain verification by domain name.
    ///
    /// Used for SSO discovery: checks if a domain is verified by any SSO config.
    /// Only returns verifications where status = 'verified'.
    async fn find_verified_by_domain(&self, domain: &str) -> DbResult<Option<DomainVerification>>;

    /// List all verified domains for an SSO config.
    ///
    /// Only returns domains where status = 'verified' and not expired.
    async fn list_verified_by_config(
        &self,
        org_sso_config_id: Uuid,
    ) -> DbResult<Vec<DomainVerification>>;

    /// Check if any domain is verified for an SSO config.
    async fn has_verified_domain(&self, org_sso_config_id: Uuid) -> DbResult<bool>;

    /// Create an auto-verified domain verification record.
    ///
    /// This creates a domain verification that is immediately in 'verified' status,
    /// bypassing the normal DNS verification flow. Used for bootstrap domains.
    ///
    /// # Arguments
    /// * `org_sso_config_id` - The SSO config this verification belongs to
    /// * `input` - The domain verification details
    /// * `verification_token` - The random token (stored but not used for verification)
    async fn create_auto_verified(
        &self,
        org_sso_config_id: Uuid,
        input: CreateDomainVerification,
        verification_token: &str,
    ) -> DbResult<DomainVerification>;
}
