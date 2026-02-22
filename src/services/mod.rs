mod access_reviews;
mod api_keys;
pub mod audit_logs;
mod conversations;
#[cfg(any(
    feature = "document-extraction-basic",
    feature = "document-extraction-full"
))]
pub mod document_processor;
#[cfg(feature = "sso")]
mod domain_verifications;
mod file_search;
mod file_storage;
mod files;
#[cfg(feature = "forecasting")]
pub mod forecasting;
mod model_pricing;
mod org_rbac_policies;
#[cfg(feature = "sso")]
mod org_sso_configs;
mod organizations;
mod projects;
#[cfg(feature = "prometheus")]
pub mod prometheus_client;
#[cfg(feature = "prometheus")]
pub mod prometheus_parser;
mod prompts;
pub mod provider_metrics;
mod providers;
mod reranker;
#[cfg(feature = "sso")]
mod scim_configs;
#[cfg(feature = "sso")]
mod scim_provisioning;
mod service_accounts;
#[cfg(feature = "sso")]
mod sso_group_mappings;
mod teams;
mod usage;
mod users;
mod vector_stores;
#[cfg(feature = "virus-scan")]
mod virus_scan;

use std::sync::Arc;

pub use access_reviews::AccessReviewService;
pub use api_keys::ApiKeyService;
pub use audit_logs::AuditLogService;
pub use conversations::ConversationService;
#[cfg(any(
    feature = "document-extraction-basic",
    feature = "document-extraction-full"
))]
pub use document_processor::{
    DocumentProcessor, DocumentProcessorConfig, DocumentProcessorError, WorkerConfig,
    start_file_processing_worker,
};
#[cfg(feature = "sso")]
pub use domain_verifications::{DomainVerificationError, DomainVerificationService};
pub use file_search::{
    FileSearchError, FileSearchRequest, FileSearchResponse, FileSearchResult, FileSearchService,
    FileSearchServiceConfig,
};
#[cfg(feature = "s3-storage")]
pub use file_storage::S3FileStorage;
pub use file_storage::{
    DatabaseFileStorage, FileStorage, FileStorageError, FileStorageResult, FilesystemFileStorage,
    create_file_storage,
};
pub use files::{FilesService, FilesServiceError, FilesServiceResult};
pub use model_pricing::ModelPricingService;
pub use org_rbac_policies::{OrgRbacPolicyError, OrgRbacPolicyService};
#[cfg(feature = "sso")]
pub use org_sso_configs::{OrgSsoConfigError, OrgSsoConfigService, OrgSsoConfigWithClientSecret};
pub use organizations::OrganizationService;
pub use projects::ProjectService;
pub use prompts::PromptService;
pub use provider_metrics::{
    ProviderMetricsError, ProviderMetricsService, ProviderStats, ProviderStatsHistorical,
    StatsGranularity, TimeBucketStats,
};
pub use providers::{
    DynamicProviderError, DynamicProviderService, validate_provider_config,
    validate_provider_config_with_url, validate_provider_type,
};
pub use reranker::{
    LlmReranker, NoOpReranker, RankedResult, RerankError, RerankRequest, RerankResponse,
    RerankUsage, Reranker,
};
#[cfg(feature = "sso")]
pub use scim_configs::{OrgScimConfigError, OrgScimConfigService};
#[cfg(feature = "sso")]
pub use scim_provisioning::ScimProvisioningService;
pub use service_accounts::ServiceAccountService;
#[cfg(feature = "sso")]
pub use sso_group_mappings::SsoGroupMappingService;
pub use teams::TeamService;
pub use usage::UsageService;
pub use users::UserService;
pub use vector_stores::VectorStoresService;
#[cfg(feature = "virus-scan")]
pub use virus_scan::{
    ClamAvScanner, NoOpScanner, ScanResult, VirusScanError, VirusScanResult, VirusScanner,
};

use crate::{db::DbPool, events::EventBus};

/// Container for all services
#[derive(Clone)]
pub struct Services {
    pub organizations: OrganizationService,
    pub teams: TeamService,
    pub projects: ProjectService,
    pub users: UserService,
    pub api_keys: ApiKeyService,
    pub providers: DynamicProviderService,
    pub usage: UsageService,
    pub model_pricing: ModelPricingService,
    pub conversations: ConversationService,
    pub prompts: PromptService,
    pub audit_logs: AuditLogService,
    pub access_reviews: AccessReviewService,
    pub vector_stores: VectorStoresService,
    pub files: FilesService,
    #[cfg(feature = "sso")]
    pub sso_group_mappings: SsoGroupMappingService,
    #[cfg(feature = "sso")]
    pub org_sso_configs: OrgSsoConfigService,
    #[cfg(feature = "sso")]
    pub domain_verifications: DomainVerificationService,
    #[cfg(feature = "sso")]
    pub scim_configs: OrgScimConfigService,
    #[cfg(feature = "sso")]
    pub scim_provisioning: ScimProvisioningService,
    pub org_rbac_policies: OrgRbacPolicyService,
    pub service_accounts: ServiceAccountService,
}

impl Services {
    pub fn new(
        db: Arc<DbPool>,
        file_storage: Arc<dyn FileStorage>,
        max_expression_length: usize,
    ) -> Self {
        Self {
            organizations: OrganizationService::new(db.clone()),
            teams: TeamService::new(db.clone()),
            projects: ProjectService::new(db.clone()),
            users: UserService::new(db.clone()),
            api_keys: ApiKeyService::new(db.clone()),
            providers: DynamicProviderService::new(db.clone()),
            usage: UsageService::new(db.clone()),
            model_pricing: ModelPricingService::new(db.clone()),
            conversations: ConversationService::new(db.clone()),
            prompts: PromptService::new(db.clone()),
            audit_logs: AuditLogService::new(db.clone()),
            access_reviews: AccessReviewService::new(db.clone()),
            vector_stores: VectorStoresService::new(db.clone()),
            #[cfg(feature = "sso")]
            sso_group_mappings: SsoGroupMappingService::new(db.clone()),
            #[cfg(feature = "sso")]
            org_sso_configs: OrgSsoConfigService::new(db.clone()),
            #[cfg(feature = "sso")]
            domain_verifications: DomainVerificationService::new(db.clone()),
            #[cfg(feature = "sso")]
            scim_configs: OrgScimConfigService::new(db.clone()),
            #[cfg(feature = "sso")]
            scim_provisioning: ScimProvisioningService::new(db.clone()),
            org_rbac_policies: OrgRbacPolicyService::new(db.clone(), max_expression_length),
            service_accounts: ServiceAccountService::new(db.clone()),
            files: FilesService::new(db, file_storage),
        }
    }

    /// Create services with an EventBus for real-time event broadcasting.
    pub fn with_event_bus(
        db: Arc<DbPool>,
        file_storage: Arc<dyn FileStorage>,
        event_bus: Arc<EventBus>,
        max_expression_length: usize,
    ) -> Self {
        Self {
            organizations: OrganizationService::new(db.clone()),
            teams: TeamService::new(db.clone()),
            projects: ProjectService::new(db.clone()),
            users: UserService::new(db.clone()),
            api_keys: ApiKeyService::new(db.clone()),
            providers: DynamicProviderService::new(db.clone()),
            usage: UsageService::new(db.clone()),
            model_pricing: ModelPricingService::new(db.clone()),
            conversations: ConversationService::new(db.clone()),
            prompts: PromptService::new(db.clone()),
            audit_logs: AuditLogService::with_event_bus(db.clone(), event_bus),
            access_reviews: AccessReviewService::new(db.clone()),
            vector_stores: VectorStoresService::new(db.clone()),
            #[cfg(feature = "sso")]
            sso_group_mappings: SsoGroupMappingService::new(db.clone()),
            #[cfg(feature = "sso")]
            org_sso_configs: OrgSsoConfigService::new(db.clone()),
            #[cfg(feature = "sso")]
            domain_verifications: DomainVerificationService::new(db.clone()),
            #[cfg(feature = "sso")]
            scim_configs: OrgScimConfigService::new(db.clone()),
            #[cfg(feature = "sso")]
            scim_provisioning: ScimProvisioningService::new(db.clone()),
            org_rbac_policies: OrgRbacPolicyService::new(db.clone(), max_expression_length),
            service_accounts: ServiceAccountService::new(db.clone()),
            files: FilesService::new(db, file_storage),
        }
    }
}
