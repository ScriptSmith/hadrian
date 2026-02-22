use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::DateRange;
use crate::{
    db::error::DbResult,
    models::{
        DailyModelSpend, DailyOrgSpend, DailyPricingSourceSpend, DailyProjectSpend,
        DailyProviderSpend, DailySpend, DailyTeamSpend, DailyUserSpend, ModelSpend, OrgSpend,
        PricingSourceSpend, ProjectSpend, ProviderSpend, RefererSpend, TeamSpend, UsageLogEntry,
        UsageSummary, UserSpend,
    },
};

/// Statistics for computing cost forecasts
#[derive(Debug, Clone)]
pub struct UsageStats {
    /// Average daily spend in microcents
    pub avg_daily_spend_microcents: i64,
    /// Standard deviation of daily spend in microcents
    pub std_dev_daily_spend_microcents: i64,
    /// Number of days with data
    pub sample_days: i32,
}

#[async_trait]
pub trait UsageRepo: Send + Sync {
    /// Log a single usage entry.
    async fn log(&self, entry: UsageLogEntry) -> DbResult<()>;

    /// Log a batch of usage entries efficiently.
    /// Uses a single transaction with multi-row insert for better performance.
    /// Returns the number of entries successfully inserted.
    async fn log_batch(&self, entries: Vec<UsageLogEntry>) -> DbResult<usize>;

    async fn get_summary(&self, api_key_id: Uuid, range: DateRange) -> DbResult<UsageSummary>;
    async fn get_by_date(&self, api_key_id: Uuid, range: DateRange) -> DbResult<Vec<DailySpend>>;
    async fn get_by_model(&self, api_key_id: Uuid, range: DateRange) -> DbResult<Vec<ModelSpend>>;
    async fn get_by_referer(
        &self,
        api_key_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<RefererSpend>>;

    /// Get usage statistics for cost forecasting.
    /// Returns average and std deviation of daily spend over the given date range.
    async fn get_usage_stats(&self, api_key_id: Uuid, range: DateRange) -> DbResult<UsageStats>;

    /// Get total spend for the current budget period.
    /// For daily budgets, returns today's spend. For monthly, returns current month's spend.
    async fn get_current_period_spend(&self, api_key_id: Uuid, period: &str) -> DbResult<i64>;

    // ==================== Aggregated Usage Queries ====================
    // These methods aggregate usage across all API keys for a given scope.

    /// Get daily usage aggregated across all API keys in an organization.
    async fn get_daily_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailySpend>>;

    /// Get daily usage aggregated across all API keys in a project.
    async fn get_daily_usage_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailySpend>>;

    /// Get daily usage aggregated across all API keys owned by a user.
    async fn get_daily_usage_by_user(
        &self,
        user_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailySpend>>;

    /// Get daily usage aggregated by provider name across all API keys.
    async fn get_daily_usage_by_provider(
        &self,
        provider: &str,
        range: DateRange,
    ) -> DbResult<Vec<DailySpend>>;

    /// Get usage summary by provider name across all API keys.
    async fn get_summary_by_provider(
        &self,
        provider: &str,
        range: DateRange,
    ) -> DbResult<UsageSummary>;

    /// Get usage breakdown by model for a provider.
    async fn get_model_usage_by_provider(
        &self,
        provider: &str,
        range: DateRange,
    ) -> DbResult<Vec<ModelSpend>>;

    /// Get usage stats for a provider (for forecasting).
    async fn get_usage_stats_by_provider(
        &self,
        provider: &str,
        range: DateRange,
    ) -> DbResult<UsageStats>;

    /// Get usage breakdown by model for an organization.
    async fn get_model_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ModelSpend>>;

    /// Get usage breakdown by model for a project.
    async fn get_model_usage_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ModelSpend>>;

    /// Get usage breakdown by model for a user.
    async fn get_model_usage_by_user(
        &self,
        user_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ModelSpend>>;

    /// Get usage breakdown by provider for an organization.
    async fn get_provider_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ProviderSpend>>;

    /// Get usage summary for an organization.
    async fn get_summary_by_org(&self, org_id: Uuid, range: DateRange) -> DbResult<UsageSummary>;

    /// Get usage summary for a project.
    async fn get_summary_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<UsageSummary>;

    /// Get usage summary for a user.
    async fn get_summary_by_user(&self, user_id: Uuid, range: DateRange) -> DbResult<UsageSummary>;

    /// Get usage stats for an organization (for forecasting).
    async fn get_usage_stats_by_org(&self, org_id: Uuid, range: DateRange) -> DbResult<UsageStats>;

    /// Get usage stats for a project (for forecasting).
    async fn get_usage_stats_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<UsageStats>;

    /// Get usage stats for a user (for forecasting).
    async fn get_usage_stats_by_user(
        &self,
        user_id: Uuid,
        range: DateRange,
    ) -> DbResult<UsageStats>;

    // ==================== Team-Level Aggregated Queries ====================

    /// Get daily usage aggregated across all API keys in a team.
    async fn get_daily_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailySpend>>;

    /// Get usage breakdown by model for a team.
    async fn get_model_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ModelSpend>>;

    /// Get usage breakdown by provider for a team.
    async fn get_provider_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ProviderSpend>>;

    /// Get usage breakdown by provider for an API key.
    async fn get_provider_usage(
        &self,
        api_key_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ProviderSpend>>;

    /// Get usage breakdown by provider for a project.
    async fn get_provider_usage_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ProviderSpend>>;

    /// Get usage breakdown by provider for a user.
    async fn get_provider_usage_by_user(
        &self,
        user_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ProviderSpend>>;

    // ==================== Daily Time Series by Model/Provider ====================

    /// Get daily usage grouped by model for an API key.
    async fn get_daily_model_usage(
        &self,
        api_key_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyModelSpend>>;

    /// Get daily usage grouped by model for an organization.
    async fn get_daily_model_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyModelSpend>>;

    /// Get daily usage grouped by model for a project.
    async fn get_daily_model_usage_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyModelSpend>>;

    /// Get daily usage grouped by model for a user.
    async fn get_daily_model_usage_by_user(
        &self,
        user_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyModelSpend>>;

    /// Get daily usage grouped by model for a team.
    async fn get_daily_model_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyModelSpend>>;

    /// Get daily usage grouped by provider for an API key.
    async fn get_daily_provider_usage(
        &self,
        api_key_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyProviderSpend>>;

    /// Get daily usage grouped by provider for an organization.
    async fn get_daily_provider_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyProviderSpend>>;

    /// Get daily usage grouped by provider for a project.
    async fn get_daily_provider_usage_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyProviderSpend>>;

    /// Get daily usage grouped by provider for a user.
    async fn get_daily_provider_usage_by_user(
        &self,
        user_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyProviderSpend>>;

    /// Get daily usage grouped by provider for a team.
    async fn get_daily_provider_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyProviderSpend>>;

    /// Get usage summary for a team.
    async fn get_summary_by_team(&self, team_id: Uuid, range: DateRange) -> DbResult<UsageSummary>;

    /// Get usage stats for a team (for forecasting).
    async fn get_usage_stats_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<UsageStats>;

    // ==================== Pricing Source Aggregation ====================

    /// Get usage breakdown by pricing source for an API key.
    async fn get_pricing_source_usage(
        &self,
        api_key_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<PricingSourceSpend>>;

    /// Get usage breakdown by pricing source for an organization.
    async fn get_pricing_source_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<PricingSourceSpend>>;

    /// Get usage breakdown by pricing source for a project.
    async fn get_pricing_source_usage_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<PricingSourceSpend>>;

    /// Get usage breakdown by pricing source for a user.
    async fn get_pricing_source_usage_by_user(
        &self,
        user_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<PricingSourceSpend>>;

    /// Get usage breakdown by pricing source for a team.
    async fn get_pricing_source_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<PricingSourceSpend>>;

    /// Get daily usage grouped by pricing source for an API key.
    async fn get_daily_pricing_source_usage(
        &self,
        api_key_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyPricingSourceSpend>>;

    /// Get daily usage grouped by pricing source for an organization.
    async fn get_daily_pricing_source_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyPricingSourceSpend>>;

    /// Get daily usage grouped by pricing source for a project.
    async fn get_daily_pricing_source_usage_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyPricingSourceSpend>>;

    /// Get daily usage grouped by pricing source for a user.
    async fn get_daily_pricing_source_usage_by_user(
        &self,
        user_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyPricingSourceSpend>>;

    /// Get daily usage grouped by pricing source for a team.
    async fn get_daily_pricing_source_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyPricingSourceSpend>>;

    // ==================== Entity Breakdown Queries ====================

    // --- Project scope: by user ---

    /// Get usage breakdown by user for a project.
    async fn get_user_usage_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<UserSpend>>;

    /// Get daily usage grouped by user for a project.
    async fn get_daily_user_usage_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyUserSpend>>;

    // --- Team scope: by user, by project ---

    /// Get usage breakdown by user for a team.
    async fn get_user_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<UserSpend>>;

    /// Get daily usage grouped by user for a team.
    async fn get_daily_user_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyUserSpend>>;

    /// Get usage breakdown by project for a team.
    async fn get_project_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ProjectSpend>>;

    /// Get daily usage grouped by project for a team.
    async fn get_daily_project_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyProjectSpend>>;

    // --- Org scope: by user, by project, by team ---

    /// Get usage breakdown by user for an organization.
    async fn get_user_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<UserSpend>>;

    /// Get daily usage grouped by user for an organization.
    async fn get_daily_user_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyUserSpend>>;

    /// Get usage breakdown by project for an organization.
    async fn get_project_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ProjectSpend>>;

    /// Get daily usage grouped by project for an organization.
    async fn get_daily_project_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyProjectSpend>>;

    /// Get usage breakdown by team for an organization.
    async fn get_team_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<TeamSpend>>;

    /// Get daily usage grouped by team for an organization.
    async fn get_daily_team_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyTeamSpend>>;

    // --- Global scope: base queries ---

    /// Get usage summary across all records (global).
    async fn get_summary_global(&self, range: DateRange) -> DbResult<UsageSummary>;

    /// Get daily usage across all records (global).
    async fn get_daily_usage_global(&self, range: DateRange) -> DbResult<Vec<DailySpend>>;

    /// Get usage breakdown by model (global).
    async fn get_model_usage_global(&self, range: DateRange) -> DbResult<Vec<ModelSpend>>;

    /// Get usage breakdown by provider (global).
    async fn get_provider_usage_global(&self, range: DateRange) -> DbResult<Vec<ProviderSpend>>;

    /// Get usage breakdown by pricing source (global).
    async fn get_pricing_source_usage_global(
        &self,
        range: DateRange,
    ) -> DbResult<Vec<PricingSourceSpend>>;

    /// Get daily usage grouped by model (global).
    async fn get_daily_model_usage_global(
        &self,
        range: DateRange,
    ) -> DbResult<Vec<DailyModelSpend>>;

    /// Get daily usage grouped by provider (global).
    async fn get_daily_provider_usage_global(
        &self,
        range: DateRange,
    ) -> DbResult<Vec<DailyProviderSpend>>;

    /// Get daily usage grouped by pricing source (global).
    async fn get_daily_pricing_source_usage_global(
        &self,
        range: DateRange,
    ) -> DbResult<Vec<DailyPricingSourceSpend>>;

    /// Get usage stats (global, for forecasting).
    async fn get_usage_stats_global(&self, range: DateRange) -> DbResult<UsageStats>;

    // --- Global scope: entity breakdowns ---

    /// Get usage breakdown by user (global).
    async fn get_user_usage_global(&self, range: DateRange) -> DbResult<Vec<UserSpend>>;

    /// Get daily usage grouped by user (global).
    async fn get_daily_user_usage_global(&self, range: DateRange) -> DbResult<Vec<DailyUserSpend>>;

    /// Get usage breakdown by project (global).
    async fn get_project_usage_global(&self, range: DateRange) -> DbResult<Vec<ProjectSpend>>;

    /// Get daily usage grouped by project (global).
    async fn get_daily_project_usage_global(
        &self,
        range: DateRange,
    ) -> DbResult<Vec<DailyProjectSpend>>;

    /// Get usage breakdown by team (global).
    async fn get_team_usage_global(&self, range: DateRange) -> DbResult<Vec<TeamSpend>>;

    /// Get daily usage grouped by team (global).
    async fn get_daily_team_usage_global(&self, range: DateRange) -> DbResult<Vec<DailyTeamSpend>>;

    /// Get usage breakdown by organization (global).
    async fn get_org_usage_global(&self, range: DateRange) -> DbResult<Vec<OrgSpend>>;

    /// Get daily usage grouped by organization (global).
    async fn get_daily_org_usage_global(&self, range: DateRange) -> DbResult<Vec<DailyOrgSpend>>;

    // ==================== Retention Operations ====================
    // These methods support data retention policies.

    /// Delete usage records older than the given cutoff date.
    ///
    /// Deletes in batches to avoid locking the database.
    /// Returns the total number of records deleted.
    async fn delete_usage_records_before(
        &self,
        cutoff: DateTime<Utc>,
        batch_size: u32,
        max_deletes: u64,
    ) -> DbResult<u64>;

    /// Delete daily spend aggregates older than the given cutoff date.
    ///
    /// Deletes in batches to avoid locking the database.
    /// Returns the total number of records deleted.
    async fn delete_daily_spend_before(
        &self,
        cutoff: DateTime<Utc>,
        batch_size: u32,
        max_deletes: u64,
    ) -> DbResult<u64>;
}
