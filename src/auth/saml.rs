//! SAML 2.0 authentication.
//!
//! This module implements SAML 2.0 SP-initiated SSO for browser-based
//! authentication. It handles:
//! - AuthnRequest generation and signing
//! - Response/Assertion parsing and validation
//! - XML signature verification
//! - Attribute extraction from assertions
//! - Session management via cookies

use std::time::{Duration, Instant};

use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::Utc;
use openssl::pkey::{PKey, Private};
use samael::{
    metadata::EntityDescriptor,
    schema::{AuthnContextClassRef, AuthnContextComparison, RequestedAuthnContext},
    service_provider::ServiceProviderBuilder,
};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::{
    AuthError,
    session_store::{
        AuthorizationState, OidcSession, SharedSessionStore, enforce_session_limit,
        validate_and_refresh_session,
    },
};
use crate::config::SessionConfig;

// ─────────────────────────────────────────────────────────────────────────────
// Configuration Types
// ─────────────────────────────────────────────────────────────────────────────

/// SAML authentication configuration.
///
/// This is the runtime config extracted from OrgSsoConfig for use by the
/// SAML authenticator.
#[derive(Debug, Clone)]
pub struct SamlAuthConfig {
    /// IdP entity identifier
    pub idp_entity_id: String,
    /// IdP Single Sign-On service URL
    pub idp_sso_url: String,
    /// IdP Single Logout service URL (optional)
    pub idp_slo_url: Option<String>,
    /// IdP X.509 certificate for signature verification (PEM format)
    pub idp_certificate: String,
    /// Service Provider entity ID (Hadrian's identifier to the IdP)
    pub sp_entity_id: String,
    /// Assertion Consumer Service URL (where IdP sends the response)
    pub sp_acs_url: String,
    /// NameID format to request (e.g., 'urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress')
    pub name_id_format: Option<String>,
    /// Whether to sign AuthnRequests
    pub sign_requests: bool,
    /// Whether to force re-authentication at IdP
    pub force_authn: bool,
    /// Requested authentication context class
    pub authn_context_class_ref: Option<String>,
    /// SAML attribute name for user identity (like identity_claim for OIDC)
    pub identity_attribute: Option<String>,
    /// SAML attribute name for email
    pub email_attribute: Option<String>,
    /// SAML attribute name for display name
    pub name_attribute: Option<String>,
    /// SAML attribute name for groups
    pub groups_attribute: Option<String>,
    /// SP private key for signing AuthnRequests (PEM format)
    pub sp_private_key: Option<String>,
    /// SP certificate (PEM format)
    pub sp_certificate: Option<String>,
    /// Session configuration
    pub session: SessionConfig,
    /// Optional metadata URL for fetching IdP metadata
    pub metadata_url: Option<String>,
}

/// Cached IdP metadata (parsed from metadata_url or constructed from fields).
struct CachedMetadata {
    entity_descriptor: EntityDescriptor,
    fetched_at: Instant,
}

// ─────────────────────────────────────────────────────────────────────────────
// SAML Authenticator
// ─────────────────────────────────────────────────────────────────────────────

/// SAML 2.0 authenticator that handles SP-initiated SSO.
pub struct SamlAuthenticator {
    config: SamlAuthConfig,
    http_client: reqwest::Client,
    metadata_cache: RwLock<Option<CachedMetadata>>,
    session_store: SharedSessionStore,
}

impl SamlAuthenticator {
    /// Create a new SAML authenticator with a session store.
    ///
    /// For multi-node deployments, pass a `CacheSessionStore` backed by Redis.
    /// For single-node deployments, a `MemorySessionStore` can be used.
    pub fn new(config: SamlAuthConfig, session_store: SharedSessionStore) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
            metadata_cache: RwLock::new(None),
            session_store,
        }
    }

    /// Create a new SAML authenticator with a custom HTTP client.
    pub fn with_client(
        config: SamlAuthConfig,
        http_client: reqwest::Client,
        session_store: SharedSessionStore,
    ) -> Self {
        Self {
            config,
            http_client,
            metadata_cache: RwLock::new(None),
            session_store,
        }
    }

    /// Get the session store.
    pub fn session_store(&self) -> &SharedSessionStore {
        &self.session_store
    }

    /// Get the IdP metadata, fetching from URL if configured and not cached.
    pub async fn get_metadata(&self) -> Result<Option<EntityDescriptor>, AuthError> {
        // Check cache first
        {
            let cache = self.metadata_cache.read().await;
            if let Some(cached) = cache.as_ref() {
                // Cache for 1 hour - matches OIDC discovery cache pattern. IdP metadata
                // (certificates, endpoints) changes infrequently and IdPs typically overlap
                // old/new certificates during rotations. This is an industry-standard duration.
                if cached.fetched_at.elapsed() < Duration::from_secs(3600) {
                    return Ok(Some(cached.entity_descriptor.clone()));
                }
            }
        }

        // Fetch metadata if URL is configured
        let Some(metadata_url) = &self.config.metadata_url else {
            return Ok(None);
        };

        tracing::debug!(url = %metadata_url, "Fetching SAML IdP metadata");

        // Enforce HTTPS for metadata URLs
        if let Err(e) = crate::validation::require_https(metadata_url) {
            return Err(AuthError::Internal(format!(
                "SAML metadata URL must use HTTPS: {e}"
            )));
        }

        let response = self
            .http_client
            .get(metadata_url)
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, url = %metadata_url, "Failed to fetch SAML metadata");
                AuthError::Internal(format!("Failed to fetch SAML metadata: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            tracing::error!(status = %status, "SAML metadata endpoint returned error");
            return Err(AuthError::Internal(format!(
                "SAML metadata returned {}",
                status
            )));
        }

        let metadata_xml = response.text().await.map_err(|e| {
            tracing::error!(error = %e, "Failed to read SAML metadata response");
            AuthError::Internal(format!("Failed to read SAML metadata: {}", e))
        })?;

        let entity_descriptor: EntityDescriptor = samael::metadata::de::from_str(&metadata_xml)
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to parse SAML metadata");
                AuthError::Internal(format!("Failed to parse SAML metadata: {}", e))
            })?;

        // Update cache
        {
            let mut cache = self.metadata_cache.write().await;
            *cache = Some(CachedMetadata {
                entity_descriptor: entity_descriptor.clone(),
                fetched_at: Instant::now(),
            });
        }

        Ok(Some(entity_descriptor))
    }

    /// Generate an authorization URL (AuthnRequest) for the SAML flow.
    ///
    /// The `org_id` parameter is used for per-organization SSO. When set, the callback
    /// will use the org-specific authenticator from the registry instead of the global one.
    pub async fn authorization_url_with_org(
        &self,
        return_to: Option<String>,
        org_id: Option<Uuid>,
    ) -> Result<(String, AuthorizationState), AuthError> {
        // Generate RelayState (used like OIDC state parameter for CSRF protection)
        let relay_state = Uuid::new_v4().to_string();

        // Build IdP metadata for samael ServiceProvider
        let idp_metadata = self.build_idp_metadata()?;

        // Build samael ServiceProvider to generate AuthnRequest
        let sp = ServiceProviderBuilder::default()
            .entity_id(self.config.sp_entity_id.clone())
            .acs_url(self.config.sp_acs_url.clone())
            .idp_metadata(idp_metadata)
            .authn_name_id_format(self.config.name_id_format.clone().unwrap_or_default())
            .force_authn(self.config.force_authn)
            .build()
            .map_err(|e| AuthError::Internal(format!("Failed to build ServiceProvider: {}", e)))?;

        // Generate the AuthnRequest
        let mut authn_request = sp
            .make_authentication_request(&self.config.idp_sso_url)
            .map_err(|e| AuthError::Internal(format!("Failed to create AuthnRequest: {}", e)))?;

        // Add RequestedAuthnContext if configured
        if let Some(authn_context) = &self.config.authn_context_class_ref {
            authn_request.requested_authn_context = Some(RequestedAuthnContext {
                authn_context_class_refs: Some(vec![AuthnContextClassRef {
                    value: Some(authn_context.clone()),
                }]),
                authn_context_decl_refs: None,
                comparison: Some(AuthnContextComparison::Exact),
            });
        }

        // Store the request ID for later verification
        let request_id = authn_request.id.clone();

        // Build the redirect URL (signed or unsigned based on config)
        let url = if self.config.sign_requests {
            let private_key = self.load_private_key()?;
            authn_request
                .signed_redirect(&relay_state, private_key)
                .map_err(|e| AuthError::Internal(format!("Failed to sign AuthnRequest: {}", e)))?
                .ok_or_else(|| AuthError::Internal("AuthnRequest has no destination".to_string()))?
        } else {
            authn_request
                .redirect(&relay_state)
                .map_err(|e| AuthError::Internal(format!("Failed to encode AuthnRequest: {}", e)))?
                .ok_or_else(|| AuthError::Internal("AuthnRequest has no destination".to_string()))?
        };

        // Store the auth state for later verification
        // Note: For SAML, we store the request_id in code_verifier field (reusing OIDC structure)
        let auth_state = AuthorizationState {
            state: relay_state.clone(),
            nonce: String::new(),      // Nonce is not used in SAML flows
            code_verifier: request_id, // Store SAML request ID here
            return_to,
            org_id,
            created_at: Utc::now(),
        };

        self.session_store
            .store_auth_state(auth_state.clone())
            .await
            .map_err(|e| AuthError::Internal(format!("Failed to store auth state: {}", e)))?;

        Ok((url.to_string(), auth_state))
    }

    /// Generate an authorization URL without org context.
    pub async fn authorization_url(
        &self,
        return_to: Option<String>,
    ) -> Result<(String, AuthorizationState), AuthError> {
        self.authorization_url_with_org(return_to, None).await
    }

    /// Load the SP private key for signing AuthnRequests.
    ///
    /// The private key must be in PEM format (PKCS#8 or PKCS#1).
    fn load_private_key(&self) -> Result<PKey<Private>, AuthError> {
        let private_key_pem = self.config.sp_private_key.as_ref().ok_or_else(|| {
            AuthError::Internal(
                "sign_requests is enabled but sp_private_key is not configured".to_string(),
            )
        })?;

        // Try PKCS#8 format first, then fall back to PKCS#1 (RSA)
        PKey::private_key_from_pem(private_key_pem.as_bytes()).map_err(|e| {
            AuthError::Internal(format!(
                "Failed to parse SP private key (expected PEM format): {}",
                e
            ))
        })
    }

    /// Parse and validate a SAML Response from the IdP.
    ///
    /// Returns the validated assertion data and the optional `return_to` URL.
    pub async fn exchange_response(
        &self,
        saml_response: &str,
        relay_state: &str,
    ) -> Result<(OidcSession, Option<String>), AuthError> {
        // Verify and retrieve the auth state
        let auth_state = self
            .session_store
            .take_auth_state(relay_state)
            .await
            .map_err(|e| AuthError::Internal(format!("Failed to retrieve auth state: {}", e)))?
            .ok_or(AuthError::InvalidToken)?;

        // Check if state is too old (10 minute limit)
        let age = Utc::now() - auth_state.created_at;
        if age > chrono::Duration::minutes(10) {
            return Err(AuthError::ExpiredToken);
        }

        // The request_id is stored in code_verifier
        let request_id = &auth_state.code_verifier;

        // Parse and validate the SAML Response
        let assertion = self.parse_and_validate_response(saml_response, Some(request_id))?;

        // Create session from assertion
        let now = Utc::now();
        let session_duration = chrono::Duration::seconds(self.config.session.duration_secs as i64);

        let session = OidcSession {
            id: Uuid::new_v4(),
            external_id: assertion.name_id,
            email: assertion.email,
            name: assertion.name,
            org: None, // SAML doesn't have org claim like OIDC
            groups: assertion.groups,
            roles: vec![],      // Roles would need to be mapped from groups
            access_token: None, // SAML doesn't use access tokens
            refresh_token: None,
            created_at: now,
            expires_at: now + session_duration,
            token_expires_at: None,
            sso_org_id: auth_state.org_id,
            session_index: assertion.session_index,
            device: None, // Device info set by route handler
            last_activity: Some(now),
        };

        // Store session
        self.session_store
            .create_session(session.clone())
            .await
            .map_err(|e| AuthError::Internal(format!("Failed to store session: {}", e)))?;

        // Enforce concurrent session limit (Phase 2)
        let enhanced = &self.config.session.enhanced;
        if enhanced.enabled
            && enhanced.max_concurrent_sessions > 0
            && let Err(e) = enforce_session_limit(
                self.session_store.as_ref(),
                &session.external_id,
                enhanced.max_concurrent_sessions,
            )
            .await
        {
            // Non-fatal: log but don't fail the login
            tracing::warn!(
                external_id = %session.external_id,
                error = %e,
                "Failed to enforce SAML session limit"
            );
        }

        Ok((session, auth_state.return_to))
    }

    /// Parse and validate a base64-encoded SAML Response.
    fn parse_and_validate_response(
        &self,
        saml_response_b64: &str,
        expected_request_id: Option<&str>,
    ) -> Result<SamlAssertionData, AuthError> {
        // Decode base64
        let response_bytes = STANDARD.decode(saml_response_b64).map_err(|e| {
            tracing::error!(error = %e, "Failed to decode SAML response base64");
            AuthError::InvalidToken
        })?;

        let response_xml = String::from_utf8(response_bytes).map_err(|e| {
            tracing::error!(error = %e, "SAML response is not valid UTF-8");
            AuthError::InvalidToken
        })?;

        tracing::debug!(xml_len = response_xml.len(), "Parsing SAML response");

        // Build a ServiceProvider for validation
        let idp_metadata = self.build_idp_metadata()?;

        let sp = ServiceProviderBuilder::default()
            .entity_id(self.config.sp_entity_id.clone())
            .acs_url(self.config.sp_acs_url.clone())
            .idp_metadata(idp_metadata)
            .build()
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to build ServiceProvider");
                AuthError::Internal(format!("Failed to build ServiceProvider: {}", e))
            })?;

        // Parse and validate the response
        let possible_request_ids: Vec<&str> = expected_request_id.into_iter().collect();
        let assertion = sp
            .parse_base64_response(saml_response_b64, Some(&possible_request_ids))
            .map_err(|e| {
                tracing::error!(error = %e, "SAML response validation failed");
                AuthError::Internal(format!("SAML response validation failed: {}", e))
            })?;

        // Extract user data from assertion
        let name_id = assertion
            .subject
            .as_ref()
            .and_then(|s| s.name_id.as_ref())
            .map(|n| n.value.clone())
            .ok_or_else(|| {
                tracing::error!("SAML assertion missing NameID");
                AuthError::Internal("SAML assertion missing NameID".to_string())
            })?;

        // Extract attributes
        let email = self.extract_attribute(&assertion, &self.config.email_attribute);
        let name = self.extract_attribute(&assertion, &self.config.name_attribute);
        let groups = self.extract_attribute_values(&assertion, &self.config.groups_attribute);

        // If identity_attribute is set, use that instead of NameID
        let external_id = if let Some(identity_attr) = &self.config.identity_attribute {
            self.extract_attribute(&assertion, &Some(identity_attr.clone()))
                .unwrap_or(name_id)
        } else {
            name_id
        };

        // Extract session index from AuthnStatement for SLO
        let session_index = assertion
            .authn_statements
            .as_ref()
            .and_then(|stmts| stmts.first())
            .and_then(|stmt| stmt.session_index.clone());

        Ok(SamlAssertionData {
            name_id: external_id,
            email,
            name,
            groups,
            session_index,
        })
    }

    /// Build an EntityDescriptor from config for the IdP.
    fn build_idp_metadata(&self) -> Result<EntityDescriptor, AuthError> {
        // Create a minimal EntityDescriptor from our config
        // This is used when metadata_url is not provided
        let xml = format!(
            r#"<md:EntityDescriptor xmlns:md="urn:oasis:names:tc:SAML:2.0:metadata" entityID="{}">
    <md:IDPSSODescriptor protocolSupportEnumeration="urn:oasis:names:tc:SAML:2.0:protocol">
        <md:KeyDescriptor use="signing">
            <ds:KeyInfo xmlns:ds="http://www.w3.org/2000/09/xmldsig#">
                <ds:X509Data>
                    <ds:X509Certificate>{}</ds:X509Certificate>
                </ds:X509Data>
            </ds:KeyInfo>
        </md:KeyDescriptor>
        <md:SingleSignOnService Binding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-Redirect" Location="{}"/>
        {}
    </md:IDPSSODescriptor>
</md:EntityDescriptor>"#,
            self.config.idp_entity_id,
            self.strip_pem_headers(&self.config.idp_certificate),
            self.config.idp_sso_url,
            self.config
                .idp_slo_url
                .as_ref()
                .map(|url| format!(
                    r#"<md:SingleLogoutService Binding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-Redirect" Location="{}"/>"#,
                    url
                ))
                .unwrap_or_default()
        );

        samael::metadata::de::from_str(&xml).map_err(|e| {
            tracing::error!(error = %e, "Failed to build IdP metadata from config");
            AuthError::Internal(format!("Failed to build IdP metadata: {}", e))
        })
    }

    /// Strip PEM headers from a certificate.
    fn strip_pem_headers(&self, pem: &str) -> String {
        pem.lines()
            .filter(|line| !line.starts_with("-----BEGIN") && !line.starts_with("-----END"))
            .collect::<Vec<_>>()
            .join("")
    }

    /// Extract a single attribute value from a SAML assertion.
    fn extract_attribute(
        &self,
        assertion: &samael::schema::Assertion,
        attr_name: &Option<String>,
    ) -> Option<String> {
        let attr_name = attr_name.as_ref()?;
        let statements = assertion.attribute_statements.as_ref()?;

        for statement in statements {
            for attr in &statement.attributes {
                if attr.name.as_deref() == Some(attr_name)
                    || attr.friendly_name.as_deref() == Some(attr_name)
                {
                    return attr
                        .values
                        .first()
                        .map(|v| v.value.clone().unwrap_or_default());
                }
            }
        }

        None
    }

    /// Extract multiple attribute values from a SAML assertion (for groups).
    fn extract_attribute_values(
        &self,
        assertion: &samael::schema::Assertion,
        attr_name: &Option<String>,
    ) -> Vec<String> {
        let Some(attr_name) = attr_name.as_ref() else {
            return vec![];
        };

        let Some(statements) = assertion.attribute_statements.as_ref() else {
            return vec![];
        };

        for statement in statements {
            for attr in &statement.attributes {
                if attr.name.as_deref() == Some(attr_name)
                    || attr.friendly_name.as_deref() == Some(attr_name)
                {
                    return attr.values.iter().filter_map(|v| v.value.clone()).collect();
                }
            }
        }

        vec![]
    }

    /// Get a session by ID.
    ///
    /// This method performs the following checks:
    /// 1. Verifies the session exists
    /// 2. Checks absolute expiration (`expires_at`)
    /// 3. Checks inactivity timeout (if enhanced sessions are enabled)
    /// 4. Updates `last_activity` timestamp (if enhanced sessions are enabled)
    pub async fn get_session(&self, session_id: Uuid) -> Result<OidcSession, AuthError> {
        validate_and_refresh_session(
            self.session_store.as_ref(),
            session_id,
            &self.config.session.enhanced,
        )
        .await
        .map_err(|e| match e {
            super::session_store::SessionError::NotFound => AuthError::SessionNotFound,
            super::session_store::SessionError::Expired => AuthError::SessionExpired,
            _ => AuthError::Internal(format!("Session error: {}", e)),
        })
    }

    /// Delete a session (logout).
    pub async fn logout(&self, session_id: Uuid) -> Result<Option<String>, AuthError> {
        let _ = self.session_store.delete_session(session_id).await;

        // Return SLO URL if available
        Ok(self.config.idp_slo_url.clone())
    }

    /// Generate a LogoutRequest URL for SP-initiated Single Logout (SLO).
    ///
    /// This creates a SAML LogoutRequest, DEFLATE compresses and Base64 encodes it,
    /// and builds a redirect URL to the IdP's SLO endpoint. If request signing is
    /// enabled, the URL will include signature parameters.
    ///
    /// # Arguments
    /// * `name_id` - The user's NameID value (typically from session's `external_id`)
    /// * `session_index` - Optional SAML SessionIndex for identifying the session at the IdP
    /// * `relay_state` - RelayState parameter for the redirect (typically where to return after logout)
    ///
    /// # Returns
    /// * `Ok(Some(url))` - The IdP SLO redirect URL with SAMLRequest parameter
    /// * `Ok(None)` - If IdP SLO URL is not configured
    /// * `Err(AuthError)` - If URL generation fails
    pub fn generate_logout_request_url(
        &self,
        name_id: &str,
        session_index: Option<&str>,
        relay_state: &str,
    ) -> Result<Option<String>, AuthError> {
        use std::io::Write;

        use flate2::{Compression, write::DeflateEncoder};
        use samael::traits::ToXml;

        // Check if IdP SLO URL is configured
        let idp_slo_url = match &self.config.idp_slo_url {
            Some(url) => url,
            None => return Ok(None),
        };

        // Build the LogoutRequest
        let logout_request = self.build_logout_request(name_id, idp_slo_url, session_index)?;

        // Serialize to XML
        let xml = logout_request.to_string().map_err(|e| {
            AuthError::Internal(format!("Failed to serialize LogoutRequest: {:?}", e))
        })?;

        // DEFLATE compress
        let mut compressed_buf = vec![];
        {
            let mut encoder = DeflateEncoder::new(&mut compressed_buf, Compression::default());
            encoder.write_all(xml.as_bytes()).map_err(|e| {
                AuthError::Internal(format!("Failed to compress LogoutRequest: {}", e))
            })?;
        }

        // Base64 encode
        let encoded = STANDARD.encode(&compressed_buf);

        // Build the URL
        let mut url: reqwest::Url = idp_slo_url
            .parse()
            .map_err(|e| AuthError::Internal(format!("Failed to parse IdP SLO URL: {}", e)))?;

        url.query_pairs_mut().append_pair("SAMLRequest", &encoded);
        if !relay_state.is_empty() {
            url.query_pairs_mut().append_pair("RelayState", relay_state);
        }

        // Sign the URL if signing is enabled
        let final_url = if self.config.sign_requests {
            self.sign_redirect_url(url)?
        } else {
            url
        };

        tracing::debug!(
            idp_slo_url = %idp_slo_url,
            name_id = %name_id,
            signed = self.config.sign_requests,
            "Generated SAML LogoutRequest URL"
        );

        Ok(Some(final_url.to_string()))
    }

    /// Build a SAML LogoutRequest.
    fn build_logout_request(
        &self,
        name_id: &str,
        destination: &str,
        session_index: Option<&str>,
    ) -> Result<samael::schema::LogoutRequest, AuthError> {
        use samael::schema::{Issuer, LogoutRequest, NameID};

        // Determine NameID format (use configured format or default to emailAddress)
        let name_id_format = self.config.name_id_format.clone().unwrap_or_else(|| {
            "urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress".to_string()
        });

        let logout_request = LogoutRequest {
            id: Some(format!("_logout_{}", Uuid::new_v4())),
            version: Some("2.0".to_string()),
            issue_instant: Some(Utc::now()),
            destination: Some(destination.to_string()),
            issuer: Some(Issuer {
                value: Some(self.config.sp_entity_id.clone()),
                ..Default::default()
            }),
            name_id: Some(NameID {
                value: name_id.to_string(),
                format: Some(name_id_format),
            }),
            session_index: session_index.map(|s| s.to_string()),
            signature: None,
        };

        Ok(logout_request)
    }

    /// Sign a redirect URL for SAML HTTP-Redirect binding.
    ///
    /// Creates the signature over the query string parameters and appends
    /// SigAlg and Signature parameters per SAML 2.0 Bindings specification.
    fn sign_redirect_url(&self, mut url: reqwest::Url) -> Result<reqwest::Url, AuthError> {
        use openssl::{hash::MessageDigest, sign::Signer};

        let private_key = self.load_private_key()?;

        // Determine signature algorithm based on key type
        let sig_alg = if private_key.ec_key().is_ok() {
            "http://www.w3.org/2001/04/xmldsig-more#ecdsa-sha256"
        } else {
            "http://www.w3.org/2001/04/xmldsig-more#rsa-sha256"
        };

        // Add SigAlg to the URL first (must be included in signed content)
        url.query_pairs_mut().append_pair("SigAlg", sig_alg);

        // Sign the entire query string as-is (already URL-encoded by Url type)
        // Per SAML spec section 3.4.4.1: sign SAMLRequest=value&RelayState=value&SigAlg=value
        let query_string = url
            .query()
            .ok_or_else(|| AuthError::Internal("No query string to sign".to_string()))?;

        let mut signer = Signer::new(MessageDigest::sha256(), &private_key)
            .map_err(|e| AuthError::Internal(format!("Failed to create signer: {}", e)))?;
        signer
            .update(query_string.as_bytes())
            .map_err(|e| AuthError::Internal(format!("Failed to update signer: {}", e)))?;
        let signature = signer
            .sign_to_vec()
            .map_err(|e| AuthError::Internal(format!("Failed to sign: {}", e)))?;

        // Base64 encode and append signature
        let signature_b64 = STANDARD.encode(&signature);
        url.query_pairs_mut()
            .append_pair("Signature", &signature_b64);

        Ok(url)
    }

    /// Get the session cookie name from config.
    #[allow(dead_code)] // Auth infrastructure
    pub fn cookie_name(&self) -> &str {
        &self.config.session.cookie_name
    }

    /// Get session configuration.
    #[allow(dead_code)] // Auth infrastructure
    pub fn session_config(&self) -> &SessionConfig {
        &self.config.session
    }

    /// Get the SP entity ID.
    #[allow(dead_code)] // Auth infrastructure
    pub fn sp_entity_id(&self) -> &str {
        &self.config.sp_entity_id
    }

    /// Get the ACS URL.
    #[allow(dead_code)] // Auth infrastructure
    pub fn acs_url(&self) -> &str {
        &self.config.sp_acs_url
    }

    /// Generate SP metadata XML for IdP auto-configuration.
    ///
    /// This returns a SAML 2.0 SP metadata document that IdPs can use to
    /// automatically configure their side of the SAML integration. The metadata
    /// includes:
    /// - SP entity ID
    /// - Assertion Consumer Service (ACS) URL with HTTP-POST binding
    /// - Signing certificate (if configured)
    /// - Supported NameID formats
    pub fn generate_sp_metadata(&self) -> String {
        let mut xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<md:EntityDescriptor xmlns:md="urn:oasis:names:tc:SAML:2.0:metadata" entityID="{}">
  <md:SPSSODescriptor protocolSupportEnumeration="urn:oasis:names:tc:SAML:2.0:protocol">"#,
            self.config.sp_entity_id
        );

        // Add signing key descriptor if SP certificate is configured
        if let Some(sp_cert) = &self.config.sp_certificate {
            let cert_data = self.strip_pem_headers(sp_cert);
            xml.push_str(&format!(
                r#"
    <md:KeyDescriptor use="signing">
      <ds:KeyInfo xmlns:ds="http://www.w3.org/2000/09/xmldsig#">
        <ds:X509Data>
          <ds:X509Certificate>{}</ds:X509Certificate>
        </ds:X509Data>
      </ds:KeyInfo>
    </md:KeyDescriptor>"#,
                cert_data
            ));
        }

        // Add NameIDFormat elements
        // Use configured format or default to emailAddress
        let name_id_format = self
            .config
            .name_id_format
            .as_deref()
            .unwrap_or("urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress");
        xml.push_str(&format!(
            r#"
    <md:NameIDFormat>{}</md:NameIDFormat>"#,
            name_id_format
        ));

        // Add AssertionConsumerService with HTTP-POST binding
        xml.push_str(&format!(
            r#"
    <md:AssertionConsumerService
        Binding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST"
        Location="{}"
        index="0"/>"#,
            self.config.sp_acs_url
        ));

        // Close SPSSODescriptor and EntityDescriptor
        xml.push_str(
            r#"
  </md:SPSSODescriptor>
</md:EntityDescriptor>"#,
        );

        xml
    }
}

/// Extracted data from a validated SAML assertion.
#[derive(Debug, Clone)]
struct SamlAssertionData {
    /// NameID or identity attribute value
    name_id: String,
    /// Email address (if extracted)
    email: Option<String>,
    /// Display name (if extracted)
    name: Option<String>,
    /// Group memberships (if extracted)
    groups: Vec<String>,
    /// SessionIndex from AuthnStatement (for SLO)
    session_index: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// URL Utilities
// ─────────────────────────────────────────────────────────────────────────────

/// Derive the ACS (Assertion Consumer Service) URL from the SP entity ID.
///
/// The SP entity ID is typically the base URL of the service provider, e.g.,
/// `http://localhost:8080/saml` or `http://localhost:8080`. This function
/// extracts the scheme and authority (host:port) and appends `/auth/saml/acs`
/// to form the ACS URL.
///
/// This is necessary because in Docker/Kubernetes environments, the server
/// may bind to `0.0.0.0:8080` internally but be accessible externally at
/// `http://localhost:8080`. The SP entity ID contains the external URL that
/// clients use, so we can derive the correct ACS URL from it.
///
/// # Examples
///
/// - `http://localhost:8080/saml` → `http://localhost:8080/auth/saml/acs`
/// - `https://gateway.example.com/saml` → `https://gateway.example.com/auth/saml/acs`
/// - `http://example.com` → `http://example.com/auth/saml/acs`
pub fn derive_acs_url_from_entity_id(sp_entity_id: &str) -> Option<String> {
    use reqwest::Url;
    let url = Url::parse(sp_entity_id).ok()?;

    let scheme = url.scheme();
    let host = url.host_str()?;
    let port = url.port();

    let acs_url = if let Some(port) = port {
        format!("{}://{}:{}/auth/saml/acs", scheme, host, port)
    } else {
        format!("{}://{}/auth/saml/acs", scheme, host)
    };

    Some(acs_url)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::auth::session_store::{MemorySessionStore, SessionStore};

    /// Generate a self-signed X.509 certificate and private key for testing.
    ///
    /// Returns (certificate_pem, private_key_pem).
    fn generate_test_certificate() -> (String, String) {
        use openssl::{
            asn1::Asn1Time,
            bn::BigNum,
            hash::MessageDigest,
            pkey::PKey,
            rsa::Rsa,
            x509::{X509Builder, X509NameBuilder},
        };

        // Generate RSA key pair
        let rsa = Rsa::generate(2048).unwrap();
        let private_key = PKey::from_rsa(rsa).unwrap();

        // Build X.509 certificate
        let mut x509_name = X509NameBuilder::new().unwrap();
        x509_name
            .append_entry_by_text("CN", "test-idp.example.com")
            .unwrap();
        let x509_name = x509_name.build();

        let mut builder = X509Builder::new().unwrap();
        builder.set_version(2).unwrap();

        // Set a random serial number
        let serial_number = BigNum::from_u32(1).unwrap();
        builder
            .set_serial_number(&serial_number.to_asn1_integer().unwrap())
            .unwrap();

        builder.set_subject_name(&x509_name).unwrap();
        builder.set_issuer_name(&x509_name).unwrap();
        builder.set_pubkey(&private_key).unwrap();
        builder
            .set_not_before(&Asn1Time::days_from_now(0).unwrap())
            .unwrap();
        builder
            .set_not_after(&Asn1Time::days_from_now(365).unwrap())
            .unwrap();
        builder.sign(&private_key, MessageDigest::sha256()).unwrap();

        let cert = builder.build();

        let cert_pem = String::from_utf8(cert.to_pem().unwrap()).unwrap();
        let key_pem = String::from_utf8(private_key.private_key_to_pem_pkcs8().unwrap()).unwrap();

        (cert_pem, key_pem)
    }

    fn create_test_config() -> SamlAuthConfig {
        let (cert_pem, _) = generate_test_certificate();

        SamlAuthConfig {
            idp_entity_id: "https://idp.example.com".to_string(),
            idp_sso_url: "https://idp.example.com/sso".to_string(),
            idp_slo_url: Some("https://idp.example.com/slo".to_string()),
            idp_certificate: cert_pem,
            sp_entity_id: "https://gateway.example.com".to_string(),
            sp_acs_url: "https://gateway.example.com/auth/saml/acs".to_string(),
            name_id_format: Some(
                "urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress".to_string(),
            ),
            sign_requests: false,
            force_authn: false,
            authn_context_class_ref: None,
            identity_attribute: None,
            email_attribute: Some("email".to_string()),
            name_attribute: Some("displayName".to_string()),
            groups_attribute: Some("groups".to_string()),
            sp_private_key: None,
            sp_certificate: None,
            session: SessionConfig::default(),
            metadata_url: None,
        }
    }

    #[test]
    fn test_strip_pem_headers() {
        let config = create_test_config();
        let session_store = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store);

        let pem = r#"-----BEGIN CERTIFICATE-----
MIICpDCCAYwCCQCqhQ5lgj5e6TANBgkqhkiG9w0BAQsFADAUMRIwEAYDVQQDDAls
b2NhbGhvc3QwHhcNMjEwMTAxMDAwMDAwWhcNMzEwMTAxMDAwMDAwWjAUMRIwEAYD
-----END CERTIFICATE-----"#;

        let stripped = authenticator.strip_pem_headers(pem);

        assert!(!stripped.contains("BEGIN"));
        assert!(!stripped.contains("END"));
        assert!(
            stripped.contains("MIICpDCCAYwCCQCqhQ5lgj5e6TANBgkqhkiG9w0BAQsFADAUMRIwEAYDVQQDDAls")
        );
    }

    #[tokio::test]
    async fn test_authorization_url_generation() {
        let config = create_test_config();
        let session_store = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store.clone());

        let result = authenticator
            .authorization_url(Some("/dashboard".to_string()))
            .await;

        assert!(result.is_ok());
        let (url, auth_state) = result.unwrap();

        // Verify URL structure
        assert!(url.starts_with("https://idp.example.com/sso?"));
        assert!(url.contains("SAMLRequest="));
        assert!(url.contains("RelayState="));

        // Verify auth state was stored
        let stored_state = session_store.peek_auth_state(&auth_state.state).await;
        assert!(stored_state.is_ok());
        assert!(stored_state.unwrap().is_some());

        // Verify return_to is preserved
        assert_eq!(auth_state.return_to, Some("/dashboard".to_string()));
    }

    #[tokio::test]
    async fn test_authorization_url_with_org() {
        let config = create_test_config();
        let session_store = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store);

        let org_id = Uuid::new_v4();
        let result = authenticator
            .authorization_url_with_org(None, Some(org_id))
            .await;

        assert!(result.is_ok());
        let (_, auth_state) = result.unwrap();

        // Verify org_id is preserved
        assert_eq!(auth_state.org_id, Some(org_id));
    }

    #[test]
    fn test_generate_sp_metadata_basic() {
        let config = create_test_config();
        let session_store = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store);

        let metadata = authenticator.generate_sp_metadata();

        // Verify XML structure
        assert!(metadata.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(metadata.contains("<md:EntityDescriptor"));
        assert!(metadata.contains("entityID=\"https://gateway.example.com\""));
        assert!(metadata.contains("<md:SPSSODescriptor"));
        assert!(
            metadata
                .contains("protocolSupportEnumeration=\"urn:oasis:names:tc:SAML:2.0:protocol\"")
        );

        // Verify ACS URL
        assert!(metadata.contains("<md:AssertionConsumerService"));
        assert!(metadata.contains("Binding=\"urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST\""));
        assert!(metadata.contains("Location=\"https://gateway.example.com/auth/saml/acs\""));
        assert!(metadata.contains("index=\"0\""));

        // Verify NameIDFormat (from config)
        assert!(metadata.contains("<md:NameIDFormat>urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress</md:NameIDFormat>"));

        // Verify proper closing tags
        assert!(metadata.contains("</md:SPSSODescriptor>"));
        assert!(metadata.contains("</md:EntityDescriptor>"));
    }

    #[test]
    fn test_generate_sp_metadata_with_certificate() {
        let mut config = create_test_config();
        config.sp_certificate = Some(
            r#"-----BEGIN CERTIFICATE-----
MIICpDCCAYwCCQCqhQ5lgj5e6TANBgkqhkiG9w0BAQsFADAUMRIwEAYDVQQDDAls
b2NhbGhvc3QwHhcNMjEwMTAxMDAwMDAwWhcNMzEwMTAxMDAwMDAwWjAUMRIwEAYD
-----END CERTIFICATE-----"#
                .to_string(),
        );
        let session_store = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store);

        let metadata = authenticator.generate_sp_metadata();

        // Verify KeyDescriptor is present
        assert!(metadata.contains("<md:KeyDescriptor use=\"signing\">"));
        assert!(metadata.contains("<ds:KeyInfo"));
        assert!(metadata.contains("xmlns:ds=\"http://www.w3.org/2000/09/xmldsig#\""));
        assert!(metadata.contains("<ds:X509Data>"));
        assert!(metadata.contains("<ds:X509Certificate>"));

        // Verify certificate data is included without PEM headers
        assert!(
            metadata.contains("MIICpDCCAYwCCQCqhQ5lgj5e6TANBgkqhkiG9w0BAQsFADAUMRIwEAYDVQQDDAls")
        );
        assert!(!metadata.contains("-----BEGIN CERTIFICATE-----"));
        assert!(!metadata.contains("-----END CERTIFICATE-----"));

        // Verify proper closing tags
        assert!(metadata.contains("</ds:X509Certificate>"));
        assert!(metadata.contains("</ds:X509Data>"));
        assert!(metadata.contains("</ds:KeyInfo>"));
        assert!(metadata.contains("</md:KeyDescriptor>"));
    }

    #[test]
    fn test_generate_sp_metadata_with_custom_name_id_format() {
        let mut config = create_test_config();
        config.name_id_format =
            Some("urn:oasis:names:tc:SAML:2.0:nameid-format:persistent".to_string());
        let session_store = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store);

        let metadata = authenticator.generate_sp_metadata();

        // Verify custom NameIDFormat
        assert!(metadata.contains("<md:NameIDFormat>urn:oasis:names:tc:SAML:2.0:nameid-format:persistent</md:NameIDFormat>"));
    }

    #[test]
    fn test_generate_sp_metadata_default_name_id_format() {
        let mut config = create_test_config();
        config.name_id_format = None; // No NameID format configured
        let session_store = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store);

        let metadata = authenticator.generate_sp_metadata();

        // Verify default NameIDFormat (emailAddress)
        assert!(metadata.contains("<md:NameIDFormat>urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress</md:NameIDFormat>"));
    }

    #[test]
    fn test_generate_sp_metadata_no_certificate() {
        let mut config = create_test_config();
        config.sp_certificate = None;
        let session_store = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store);

        let metadata = authenticator.generate_sp_metadata();

        // Verify no KeyDescriptor when no certificate
        assert!(!metadata.contains("<md:KeyDescriptor"));
        assert!(!metadata.contains("<ds:KeyInfo"));

        // Rest of metadata should still be valid
        assert!(metadata.contains("<md:EntityDescriptor"));
        assert!(metadata.contains("<md:AssertionConsumerService"));
    }

    #[tokio::test]
    async fn test_signed_authorization_url() {
        // Generate a test RSA key pair
        let rsa = openssl::rsa::Rsa::generate(2048).unwrap();
        let private_key = openssl::pkey::PKey::from_rsa(rsa).unwrap();
        let private_key_pem =
            String::from_utf8(private_key.private_key_to_pem_pkcs8().unwrap()).unwrap();

        let mut config = create_test_config();
        config.sign_requests = true;
        config.sp_private_key = Some(private_key_pem);

        let session_store = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store);

        let result = authenticator.authorization_url(None).await;
        assert!(result.is_ok());
        let (url, _auth_state) = result.unwrap();

        // Verify the URL contains signature parameters
        assert!(url.contains("SAMLRequest="));
        assert!(url.contains("RelayState="));
        assert!(url.contains("SigAlg="));
        assert!(url.contains("Signature="));
        // Verify the signature algorithm is RSA-SHA256
        assert!(url.contains("http%3A%2F%2Fwww.w3.org%2F2001%2F04%2Fxmldsig-more%23rsa-sha256"));
    }

    #[test]
    fn test_load_private_key_missing() {
        let mut config = create_test_config();
        config.sign_requests = true;
        config.sp_private_key = None;

        let session_store = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store);

        let result = authenticator.load_private_key();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("sp_private_key is not configured"));
    }

    #[test]
    fn test_load_private_key_invalid() {
        let mut config = create_test_config();
        config.sign_requests = true;
        config.sp_private_key = Some("not a valid key".to_string());

        let session_store = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store);

        let result = authenticator.load_private_key();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Failed to parse SP private key"));
    }

    #[test]
    fn test_load_private_key_valid() {
        // Generate a test RSA key
        let rsa = openssl::rsa::Rsa::generate(2048).unwrap();
        let private_key = openssl::pkey::PKey::from_rsa(rsa).unwrap();
        let private_key_pem =
            String::from_utf8(private_key.private_key_to_pem_pkcs8().unwrap()).unwrap();

        let mut config = create_test_config();
        config.sign_requests = true;
        config.sp_private_key = Some(private_key_pem);

        let session_store = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store);

        let result = authenticator.load_private_key();
        assert!(result.is_ok());
    }

    #[test]
    fn test_generate_logout_request_url_basic() {
        let config = create_test_config();
        let session_store = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store);

        let result =
            authenticator.generate_logout_request_url("user@example.com", None, "relay-state-123");
        assert!(result.is_ok());

        let url = result.unwrap();
        assert!(url.is_some());
        let url = url.unwrap();

        // Verify URL structure
        assert!(url.starts_with("https://idp.example.com/slo?"));
        assert!(url.contains("SAMLRequest="));
        assert!(url.contains("RelayState=relay-state-123"));
    }

    #[test]
    fn test_generate_logout_request_url_no_slo_url() {
        let mut config = create_test_config();
        config.idp_slo_url = None; // No SLO URL configured

        let session_store = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store);

        let result =
            authenticator.generate_logout_request_url("user@example.com", None, "relay-state");
        assert!(result.is_ok());

        // Should return None when no SLO URL configured
        let url = result.unwrap();
        assert!(url.is_none());
    }

    #[test]
    fn test_generate_logout_request_url_signed() {
        // Generate a test RSA key pair
        let rsa = openssl::rsa::Rsa::generate(2048).unwrap();
        let private_key = openssl::pkey::PKey::from_rsa(rsa).unwrap();
        let private_key_pem =
            String::from_utf8(private_key.private_key_to_pem_pkcs8().unwrap()).unwrap();

        let mut config = create_test_config();
        config.sign_requests = true;
        config.sp_private_key = Some(private_key_pem);

        let session_store = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store);

        let result = authenticator.generate_logout_request_url(
            "user@example.com",
            Some("session-idx-123"),
            "relay-state",
        );
        assert!(result.is_ok());

        let url = result.unwrap().unwrap();

        // Verify the URL contains signature parameters
        assert!(url.contains("SAMLRequest="));
        assert!(url.contains("RelayState="));
        assert!(url.contains("SigAlg="));
        assert!(url.contains("Signature="));
    }

    #[test]
    fn test_build_logout_request() {
        let config = create_test_config();
        let session_store = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store);

        let logout_request = authenticator
            .build_logout_request("user@example.com", "https://idp.example.com/slo", None)
            .unwrap();

        // Verify LogoutRequest structure
        assert!(logout_request.id.as_ref().unwrap().starts_with("_logout_"));
        assert_eq!(logout_request.version, Some("2.0".to_string()));
        assert_eq!(
            logout_request.destination,
            Some("https://idp.example.com/slo".to_string())
        );

        // Verify Issuer
        let issuer = logout_request.issuer.unwrap();
        assert_eq!(
            issuer.value,
            Some("https://gateway.example.com".to_string())
        );

        // Verify NameID
        let name_id = logout_request.name_id.unwrap();
        assert_eq!(name_id.value, "user@example.com");
        assert_eq!(
            name_id.format,
            Some("urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress".to_string())
        );
    }

    #[test]
    fn test_build_logout_request_default_name_id_format() {
        let mut config = create_test_config();
        config.name_id_format = None; // No NameID format configured

        let session_store = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store);

        let logout_request = authenticator
            .build_logout_request(
                "user@example.com",
                "https://idp.example.com/slo",
                Some("session-idx"),
            )
            .unwrap();

        // Should use default emailAddress format
        let name_id = logout_request.name_id.unwrap();
        assert_eq!(
            name_id.format,
            Some("urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress".to_string())
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // exchange_response() error case tests
    // ─────────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_exchange_response_invalid_base64() {
        let config = create_test_config();
        let session_store = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store);

        // Pass garbage that isn't valid base64
        let result = authenticator
            .exchange_response("!!!not-valid-base64!!!", "some-relay-state")
            .await;

        // Should fail because the relay state doesn't exist (checked before base64 decode)
        assert!(result.is_err());
        match result.unwrap_err() {
            AuthError::InvalidToken => {} // Expected - state not found
            e => panic!("Expected InvalidToken, got: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_exchange_response_invalid_state() {
        let config = create_test_config();
        let session_store = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store);

        // Pass valid base64 but with unknown RelayState
        let valid_b64 = STANDARD.encode(b"<xml>test</xml>");
        let result = authenticator
            .exchange_response(&valid_b64, "unknown-relay-state")
            .await;

        // Should fail because relay state doesn't exist in session store
        assert!(result.is_err());
        match result.unwrap_err() {
            AuthError::InvalidToken => {} // Expected - state not found
            e => panic!("Expected InvalidToken, got: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_exchange_response_expired_state() {
        let config = create_test_config();
        let session_store: SharedSessionStore = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store.clone());

        // Create auth state with created_at more than 10 minutes in the past
        let expired_state = AuthorizationState {
            state: "expired-relay-state".to_string(),
            nonce: String::new(),
            code_verifier: "request-id".to_string(),
            return_to: None,
            org_id: None,
            created_at: Utc::now() - chrono::Duration::minutes(15),
        };
        session_store.store_auth_state(expired_state).await.unwrap();

        // Pass valid base64 with the expired relay state
        let valid_b64 = STANDARD.encode(b"<xml>test</xml>");
        let result = authenticator
            .exchange_response(&valid_b64, "expired-relay-state")
            .await;

        // Should fail because state is too old
        assert!(result.is_err());
        match result.unwrap_err() {
            AuthError::ExpiredToken => {} // Expected - state expired
            e => panic!("Expected ExpiredToken, got: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_exchange_response_invalid_xml() {
        let config = create_test_config();
        let session_store: SharedSessionStore = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store.clone());

        // Create valid auth state
        let auth_state = AuthorizationState {
            state: "valid-relay-state".to_string(),
            nonce: String::new(),
            code_verifier: "request-id".to_string(),
            return_to: None,
            org_id: None,
            created_at: Utc::now(),
        };
        session_store.store_auth_state(auth_state).await.unwrap();

        // Pass base64-encoded garbage that isn't valid XML
        let invalid_xml_b64 = STANDARD.encode(b"this is not xml at all!");
        let result = authenticator
            .exchange_response(&invalid_xml_b64, "valid-relay-state")
            .await;

        // Should fail during SAML response parsing
        assert!(result.is_err());
        match result.unwrap_err() {
            AuthError::Internal(msg) => {
                assert!(
                    msg.contains("SAML response validation failed"),
                    "Expected SAML parsing error, got: {}",
                    msg
                );
            }
            e => panic!("Expected Internal error, got: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_exchange_response_malformed_saml() {
        let config = create_test_config();
        let session_store: SharedSessionStore = Arc::new(MemorySessionStore::new());
        let authenticator = SamlAuthenticator::new(config, session_store.clone());

        // Create valid auth state
        let auth_state = AuthorizationState {
            state: "valid-relay-state".to_string(),
            nonce: String::new(),
            code_verifier: "request-id".to_string(),
            return_to: None,
            org_id: None,
            created_at: Utc::now(),
        };
        session_store.store_auth_state(auth_state).await.unwrap();

        // Pass base64-encoded XML that's not a valid SAML response
        let malformed_saml = r#"<?xml version="1.0"?><NotASamlResponse>test</NotASamlResponse>"#;
        let malformed_saml_b64 = STANDARD.encode(malformed_saml.as_bytes());
        let result = authenticator
            .exchange_response(&malformed_saml_b64, "valid-relay-state")
            .await;

        // Should fail during SAML response validation
        assert!(result.is_err());
        match result.unwrap_err() {
            AuthError::Internal(msg) => {
                assert!(
                    msg.contains("SAML response validation failed"),
                    "Expected SAML validation error, got: {}",
                    msg
                );
            }
            e => panic!("Expected Internal error, got: {:?}", e),
        }
    }

    #[test]
    fn test_generate_test_certificate_is_valid() {
        // Verify that our test certificate generator produces valid certificates
        let (cert_pem, key_pem) = generate_test_certificate();

        // Should be valid PEM format
        assert!(cert_pem.contains("-----BEGIN CERTIFICATE-----"));
        assert!(cert_pem.contains("-----END CERTIFICATE-----"));
        assert!(key_pem.contains("-----BEGIN PRIVATE KEY-----"));
        assert!(key_pem.contains("-----END PRIVATE KEY-----"));

        // Should be parseable by openssl
        let cert = openssl::x509::X509::from_pem(cert_pem.as_bytes()).unwrap();
        let key = openssl::pkey::PKey::private_key_from_pem(key_pem.as_bytes()).unwrap();

        // Certificate subject should match what we set
        let subject = cert.subject_name();
        let cn = subject
            .entries_by_nid(openssl::nid::Nid::COMMONNAME)
            .next()
            .unwrap();
        assert_eq!(
            cn.data().as_utf8().unwrap().to_string(),
            "test-idp.example.com"
        );

        // Key should match certificate's public key
        assert!(cert.public_key().unwrap().public_eq(&key));
    }
}
