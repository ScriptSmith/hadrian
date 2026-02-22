use std::sync::Arc;

use uuid::Uuid;

use crate::{
    AppState,
    db::{
        DbPool, DbResult,
        repos::{ListParams, ListResult},
    },
    models::{
        ConnectivityTestResponse, CreateDynamicProvider, DynamicProvider, ProviderOwner,
        UpdateDynamicProvider,
    },
    routes::admin::AdminError,
    secrets::SecretManager,
};

/// Provider types supported by the dynamic resolver.
pub const SUPPORTED_PROVIDER_TYPES: &[&str] = &[
    "openai",
    "open_ai",
    "openai_compatible",
    "anthropic",
    #[cfg(feature = "provider-azure")]
    "azure_openai",
    #[cfg(feature = "provider-azure")]
    "azure_open_ai",
    #[cfg(feature = "provider-bedrock")]
    "bedrock",
    #[cfg(feature = "provider-vertex")]
    "vertex",
    "test",
];

/// Validate that a provider type is supported.
pub fn validate_provider_type(provider_type: &str) -> Result<(), AdminError> {
    if SUPPORTED_PROVIDER_TYPES.contains(&provider_type) {
        Ok(())
    } else {
        Err(AdminError::Validation(format!(
            "Unsupported provider type '{}'. Supported types: {}",
            provider_type,
            SUPPORTED_PROVIDER_TYPES.join(", ")
        )))
    }
}

/// Credential types that source from the server's environment or filesystem.
/// These must not be allowed for dynamic (user-created) providers because they
/// would let users access the gateway's own cloud credentials.
#[cfg(feature = "provider-bedrock")]
const FORBIDDEN_AWS_CREDENTIAL_TYPES: &[&str] = &["default", "profile", "assume_role"];
#[cfg(feature = "provider-vertex")]
const FORBIDDEN_GCP_CREDENTIAL_TYPES: &[&str] = &["default", "service_account"];

/// Validate provider-specific configuration.
///
/// Different provider types require different config fields:
/// - Bedrock: requires `config.region` and `config.credentials.type` = "static"
/// - Vertex: requires either `api_key` OR (`config.project` + `config.region`
///   with `config.credentials.type` = "service_account_json")
/// - Other types: no config validation needed
///
/// Dynamic providers must not use credential types that source from the server's
/// environment or filesystem (e.g., "default", "profile", "assume_role" for AWS;
/// "default", "service_account" for GCP). Only explicitly-provided credentials are
/// allowed to prevent users from accessing the gateway's own cloud identity.
pub fn validate_provider_config(
    provider_type: &str,
    config: Option<&serde_json::Value>,
    api_key: Option<&str>,
) -> Result<(), AdminError> {
    validate_provider_config_inner(provider_type, config, api_key, false)
}

/// Validate provider-specific configuration with SSRF protection.
pub fn validate_provider_config_with_url(
    provider_type: &str,
    base_url: &str,
    config: Option<&serde_json::Value>,
    api_key: Option<&str>,
    allow_loopback: bool,
) -> Result<(), AdminError> {
    // Validate base URL against SSRF if non-empty
    if !base_url.is_empty() {
        crate::validation::validate_base_url(base_url, allow_loopback)
            .map_err(|e| AdminError::Validation(format!("Invalid base URL: {e}")))?;
    }
    validate_provider_config_inner(provider_type, config, api_key, allow_loopback)
}

fn validate_provider_config_inner(
    provider_type: &str,
    config: Option<&serde_json::Value>,
    api_key: Option<&str>,
    _allow_loopback: bool,
) -> Result<(), AdminError> {
    match provider_type {
        #[cfg(feature = "provider-bedrock")]
        "bedrock" => {
            let config = config.ok_or_else(|| {
                AdminError::Validation(
                    "Bedrock providers require a 'config' with at least 'region'".to_string(),
                )
            })?;
            if config.get("region").and_then(|v| v.as_str()).is_none() {
                return Err(AdminError::Validation(
                    "Bedrock config requires 'region' (e.g., \"us-east-1\")".to_string(),
                ));
            }
            // Validate credential type — only explicit credentials allowed
            let cred_type = config
                .get("credentials")
                .and_then(|c| c.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            if FORBIDDEN_AWS_CREDENTIAL_TYPES.contains(&cred_type) {
                return Err(AdminError::Validation(format!(
                    "Dynamic providers cannot use AWS credential type '{cred_type}' \
                     (sources from server environment). Use 'static' credentials instead."
                )));
            }
            Ok(())
        }
        #[cfg(feature = "provider-vertex")]
        "vertex" => {
            // Vertex supports two modes:
            // 1. API key mode: api_key is set, no config needed
            // 2. OAuth mode: config.project + config.region + explicit credentials
            if api_key.is_some() {
                // Validate that config credentials (if present) don't use server env
                if let Some(cred_type) = config
                    .and_then(|c| c.get("credentials"))
                    .and_then(|c| c.get("type"))
                    .and_then(|v| v.as_str())
                    && FORBIDDEN_GCP_CREDENTIAL_TYPES.contains(&cred_type)
                {
                    return Err(AdminError::Validation(format!(
                        "Dynamic providers cannot use GCP credential type '{cred_type}' \
                             (sources from server environment). Use 'service_account_json' \
                             or API key mode instead."
                    )));
                }
                return Ok(());
            }
            let config = config.ok_or_else(|| {
                AdminError::Validation(
                    "Vertex providers require either 'api_key' or a 'config' with 'project' and 'region'".to_string(),
                )
            })?;
            if config.get("project").and_then(|v| v.as_str()).is_none() {
                return Err(AdminError::Validation(
                    "Vertex OAuth config requires 'project' (GCP project ID)".to_string(),
                ));
            }
            if config.get("region").and_then(|v| v.as_str()).is_none() {
                return Err(AdminError::Validation(
                    "Vertex OAuth config requires 'region' (e.g., \"us-central1\")".to_string(),
                ));
            }
            // Validate credential type — only explicit credentials allowed
            let cred_type = config
                .get("credentials")
                .and_then(|c| c.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            if FORBIDDEN_GCP_CREDENTIAL_TYPES.contains(&cred_type) {
                return Err(AdminError::Validation(format!(
                    "Dynamic providers cannot use GCP credential type '{cred_type}' \
                     (sources from server environment). Use 'service_account_json' \
                     or API key mode instead."
                )));
            }
            Ok(())
        }
        _ => {
            let _ = (config, api_key);
            Ok(())
        }
    }
}

/// Errors that can occur in DynamicProviderService operations.
#[derive(Debug, thiserror::Error)]
pub enum DynamicProviderError {
    #[error("Database error: {0}")]
    Database(#[from] crate::db::DbError),

    #[error("Failed to store secret: {0}")]
    SecretStorage(String),

    #[error("Provider not found")]
    NotFound,
}

impl From<DynamicProviderError> for AdminError {
    fn from(err: DynamicProviderError) -> Self {
        match err {
            DynamicProviderError::Database(db_err) => AdminError::from(db_err),
            DynamicProviderError::SecretStorage(msg) => {
                tracing::error!(error = %msg, "Secret storage error for dynamic provider");
                AdminError::Internal("An internal error occurred".to_string())
            }
            DynamicProviderError::NotFound => {
                AdminError::NotFound("Dynamic provider not found".to_string())
            }
        }
    }
}

/// Build the scoped secret key for a dynamic provider's API key.
fn secret_key(owner: &ProviderOwner, provider_id: Uuid) -> String {
    let (scope, owner_id) = owner.secret_namespace();
    format!("dynamicproviders/{scope}/{owner_id}/{provider_id}")
}

/// Service layer for dynamic provider operations
#[derive(Clone)]
pub struct DynamicProviderService {
    db: Arc<DbPool>,
}

impl DynamicProviderService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
    }

    /// Create a new dynamic provider, storing the API key in the secrets manager
    /// if available.
    pub async fn create(
        &self,
        mut input: CreateDynamicProvider,
        secrets: Option<&Arc<dyn SecretManager>>,
    ) -> Result<DynamicProvider, DynamicProviderError> {
        let id = Uuid::new_v4();

        // Store API key in secrets manager if available
        let stored_secret_path = if let Some(raw_key) = &input.api_key
            && let Some(sm) = secrets
        {
            let key_path = secret_key(&input.owner, id);
            sm.set(&key_path, raw_key)
                .await
                .map_err(|e| DynamicProviderError::SecretStorage(e.to_string()))?;
            input.api_key = Some(key_path.clone());
            Some(key_path)
        } else {
            None
        };
        // If no SM, the raw key is stored directly as api_key_secret_ref

        let result = self.db.providers().create(id, input).await;

        // On DB failure, clean up the orphaned secret
        if result.is_err()
            && let (Some(path), Some(sm)) = (&stored_secret_path, secrets)
            && let Err(e) = sm.delete(path).await
        {
            tracing::warn!(
                error = %e,
                secret_path = %path,
                "Failed to clean up secret after DB create failure"
            );
        }

        Ok(result?)
    }

    /// Get provider by ID
    pub async fn get_by_id(&self, id: Uuid) -> DbResult<Option<DynamicProvider>> {
        self.db.providers().get_by_id(id).await
    }

    /// Get provider by name within an owner scope
    pub async fn get_by_name(
        &self,
        owner: &ProviderOwner,
        name: &str,
    ) -> DbResult<Option<DynamicProvider>> {
        self.db.providers().get_by_name(owner, name).await
    }

    /// List providers for an organization
    pub async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>> {
        self.db.providers().list_by_org(org_id, params).await
    }

    /// Count providers for an organization
    pub async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64> {
        self.db.providers().count_by_org(org_id).await
    }

    /// List providers for a project
    pub async fn list_by_project(
        &self,
        project_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>> {
        self.db
            .providers()
            .list_by_project(project_id, params)
            .await
    }

    /// Count providers for a project
    pub async fn count_by_project(&self, project_id: Uuid) -> DbResult<i64> {
        self.db.providers().count_by_project(project_id).await
    }

    /// List providers for a user
    pub async fn list_by_user(
        &self,
        user_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>> {
        self.db.providers().list_by_user(user_id, params).await
    }

    /// Count providers for a user
    pub async fn count_by_user(&self, user_id: Uuid) -> DbResult<i64> {
        self.db.providers().count_by_user(user_id).await
    }

    /// List enabled providers for a user with cursor-based pagination
    pub async fn list_enabled_by_user(
        &self,
        user_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>> {
        self.db
            .providers()
            .list_enabled_by_user(user_id, params)
            .await
    }

    /// List enabled providers for an organization with cursor-based pagination
    pub async fn list_enabled_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>> {
        self.db
            .providers()
            .list_enabled_by_org(org_id, params)
            .await
    }

    /// Update a provider, storing the new API key in the secrets manager if provided.
    pub async fn update(
        &self,
        id: Uuid,
        mut input: UpdateDynamicProvider,
        secrets: Option<&Arc<dyn SecretManager>>,
    ) -> Result<DynamicProvider, DynamicProviderError> {
        // If a new API key is provided, resolve it through SM
        let stored_secret_path = if let Some(ref raw_key) = input.api_key {
            // Fetch existing provider to get owner for path construction
            let existing = self
                .db
                .providers()
                .get_by_id(id)
                .await?
                .ok_or(DynamicProviderError::NotFound)?;

            if let Some(sm) = secrets {
                let key_path = secret_key(&existing.owner, id);
                sm.set(&key_path, raw_key)
                    .await
                    .map_err(|e| DynamicProviderError::SecretStorage(e.to_string()))?;
                input.api_key = Some(key_path.clone());
                Some(key_path)
            } else {
                None
            }
            // If no SM, the raw key is stored directly
        } else {
            None
        };

        let result = self.db.providers().update(id, input).await;

        // On DB failure, clean up the orphaned secret
        if result.is_err()
            && let (Some(path), Some(sm)) = (&stored_secret_path, secrets)
            && let Err(e) = sm.delete(path).await
        {
            tracing::warn!(
                error = %e,
                secret_path = %path,
                "Failed to clean up secret after DB update failure"
            );
        }

        Ok(result?)
    }

    /// List enabled providers for a project with cursor-based pagination
    pub async fn list_enabled_by_project(
        &self,
        project_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>> {
        self.db
            .providers()
            .list_enabled_by_project(project_id, params)
            .await
    }

    /// List enabled providers for a team with cursor-based pagination
    pub async fn list_enabled_by_team(
        &self,
        team_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>> {
        self.db
            .providers()
            .list_enabled_by_team(team_id, params)
            .await
    }

    /// Delete a provider, cleaning up the secret from the secrets manager.
    pub async fn delete(
        &self,
        id: Uuid,
        secrets: Option<&Arc<dyn SecretManager>>,
    ) -> Result<(), DynamicProviderError> {
        // Fetch provider to get owner and api_key_secret_ref for SM cleanup
        let provider = self
            .db
            .providers()
            .get_by_id(id)
            .await?
            .ok_or(DynamicProviderError::NotFound)?;

        let secret_ref = provider.api_key_secret_ref.clone();

        // Delete from DB first
        self.db.providers().delete(id).await?;

        // Best-effort SM cleanup
        if let (Some(sm), Some(key_ref)) = (secrets, &secret_ref)
            && let Err(e) = sm.delete(key_ref).await
        {
            tracing::warn!(
                error = %e,
                provider_id = %id,
                "Failed to delete secret for dynamic provider (best-effort cleanup)"
            );
        }

        Ok(())
    }

    /// Run a connectivity test against a dynamic provider.
    ///
    /// Resolves the provider config (including secrets) and attempts to list
    /// models from the remote endpoint.
    ///
    /// The `secrets` parameter allows callers to override the secret resolution:
    /// - `Some(sm)`: use the provided secrets manager
    /// - `None`: use the raw `api_key_secret_ref` value as a literal key
    pub async fn run_connectivity_test(
        provider: &DynamicProvider,
        state: &AppState,
        secrets: Option<&Arc<dyn SecretManager>>,
    ) -> ConnectivityTestResponse {
        let start = std::time::Instant::now();

        let config_result =
            crate::routing::resolver::dynamic_provider_to_config(provider, secrets).await;

        let provider_config = match config_result {
            Ok(config) => config,
            Err(e) => {
                tracing::error!(
                    provider_id = %provider.id,
                    provider_name = %provider.name,
                    error = %e,
                    "Connectivity test failed: could not resolve provider config"
                );
                return ConnectivityTestResponse {
                    status: "error".to_string(),
                    message: "Failed to resolve provider configuration".to_string(),
                    latency_ms: Some(start.elapsed().as_millis() as u64),
                };
            }
        };

        let result = crate::providers::list_models_for_config(
            &provider_config,
            &provider.name,
            &state.http_client,
            &state.circuit_breakers,
        )
        .await;

        let latency_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(models) => ConnectivityTestResponse {
                status: "ok".to_string(),
                message: format!(
                    "Connected successfully. {} models available.",
                    models.data.len()
                ),
                latency_ms: Some(latency_ms),
            },
            Err(e) => {
                tracing::error!(
                    provider_id = %provider.id,
                    provider_name = %provider.name,
                    error = %e,
                    "Connectivity test failed: connection error"
                );
                ConnectivityTestResponse {
                    status: "error".to_string(),
                    message: "Connection failed: unable to reach provider endpoint".to_string(),
                    latency_ms: Some(latency_ms),
                }
            }
        }
    }
}
