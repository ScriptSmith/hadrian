use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::pricing::CostPricingSource;

/// Usage log entry for a single API request.
///
/// Costs are stored in microcents (1/1,000,000 of a dollar) for precision.
/// For example, $0.000207 = 207 microcents.
///
/// Attribution context is stored at write time for efficient aggregation queries.
/// `api_key_id` is None for session-based users (OIDC/SAML/proxy auth).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageLogEntry {
    /// Unique request identifier for idempotency (prevents duplicate charges)
    pub request_id: String,
    /// API key used (None for session-based auth)
    pub api_key_id: Option<Uuid>,
    /// User who made the request (from session or user-owned API key)
    pub user_id: Option<Uuid>,
    /// Organization context (always present when determinable)
    pub org_id: Option<Uuid>,
    /// Project context (from project-scoped key or X-Hadrian-Project header)
    pub project_id: Option<Uuid>,
    /// Team context (from team-scoped API key only)
    pub team_id: Option<Uuid>,
    /// Service account that owns the API key (if applicable)
    pub service_account_id: Option<Uuid>,
    pub model: String,
    pub provider: String,
    pub http_referer: Option<String>,
    pub input_tokens: i32,
    pub output_tokens: i32,
    /// Cost in microcents (1/1,000,000 of a dollar)
    pub cost_microcents: Option<i64>,
    pub request_at: DateTime<Utc>,
    /// Whether this was a streaming request
    pub streamed: bool,
    /// Cached prompt tokens (for Anthropic prompt caching, OpenAI cached context, etc.)
    pub cached_tokens: i32,
    /// Reasoning tokens (for o1, Claude extended thinking, etc.)
    pub reasoning_tokens: i32,
    /// How the generation ended (stop, length, content_filter, tool_calls, error, cancelled)
    pub finish_reason: Option<String>,
    /// Total request latency in milliseconds
    pub latency_ms: Option<i32>,
    /// Whether the request was cancelled mid-stream
    pub cancelled: bool,
    /// HTTP status code of the response
    pub status_code: Option<i16>,
    /// Where the cost data came from (provider, provider_config, pricing_config, catalog, none)
    #[serde(default)]
    pub pricing_source: CostPricingSource,
    /// Number of images generated (for image generation requests)
    #[serde(default)]
    pub image_count: Option<i32>,
    /// Audio duration in seconds (for TTS, transcription, translation requests)
    #[serde(default)]
    pub audio_seconds: Option<i32>,
    /// Character count (for TTS input text)
    #[serde(default)]
    pub character_count: Option<i32>,
    /// Whether this request used a static or dynamic provider
    #[serde(default)]
    pub provider_source: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DailySpend {
    pub date: NaiveDate,
    /// Total cost in microcents (1/1,000,000 of a dollar)
    pub total_cost_microcents: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub request_count: i64,
    pub image_count: i64,
    pub audio_seconds: i64,
    pub character_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelSpend {
    pub model: String,
    /// Total cost in microcents (1/1,000,000 of a dollar)
    pub total_cost_microcents: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub request_count: i64,
    pub image_count: i64,
    pub audio_seconds: i64,
    pub character_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RefererSpend {
    pub referer: Option<String>,
    /// Total cost in microcents (1/1,000,000 of a dollar)
    pub total_cost_microcents: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub request_count: i64,
    pub image_count: i64,
    pub audio_seconds: i64,
    pub character_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderSpend {
    pub provider: String,
    /// Total cost in microcents (1/1,000,000 of a dollar)
    pub total_cost_microcents: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub request_count: i64,
    pub image_count: i64,
    pub audio_seconds: i64,
    pub character_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageSummary {
    /// Total cost in microcents (1/1,000,000 of a dollar)
    pub total_cost_microcents: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub request_count: i64,
    pub first_request_at: Option<DateTime<Utc>>,
    pub last_request_at: Option<DateTime<Utc>>,
    pub image_count: i64,
    pub audio_seconds: i64,
    pub character_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DailyModelSpend {
    pub date: NaiveDate,
    pub model: String,
    /// Total cost in microcents (1/1,000,000 of a dollar)
    pub total_cost_microcents: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub request_count: i64,
    pub image_count: i64,
    pub audio_seconds: i64,
    pub character_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DailyProviderSpend {
    pub date: NaiveDate,
    pub provider: String,
    /// Total cost in microcents (1/1,000,000 of a dollar)
    pub total_cost_microcents: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub request_count: i64,
    pub image_count: i64,
    pub audio_seconds: i64,
    pub character_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PricingSourceSpend {
    pub pricing_source: String,
    /// Total cost in microcents (1/1,000,000 of a dollar)
    pub total_cost_microcents: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub request_count: i64,
    pub image_count: i64,
    pub audio_seconds: i64,
    pub character_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DailyPricingSourceSpend {
    pub date: NaiveDate,
    pub pricing_source: String,
    /// Total cost in microcents (1/1,000,000 of a dollar)
    pub total_cost_microcents: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub request_count: i64,
    pub image_count: i64,
    pub audio_seconds: i64,
    pub character_count: i64,
}

/// Usage breakdown by user
#[derive(Debug, Clone, Serialize)]
pub struct UserSpend {
    pub user_id: Option<Uuid>,
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    /// Total cost in microcents (1/1,000,000 of a dollar)
    pub total_cost_microcents: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub request_count: i64,
    pub image_count: i64,
    pub audio_seconds: i64,
    pub character_count: i64,
}

/// Daily usage breakdown by user
#[derive(Debug, Clone, Serialize)]
pub struct DailyUserSpend {
    pub date: NaiveDate,
    pub user_id: Option<Uuid>,
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    /// Total cost in microcents (1/1,000,000 of a dollar)
    pub total_cost_microcents: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub request_count: i64,
    pub image_count: i64,
    pub audio_seconds: i64,
    pub character_count: i64,
}

/// Usage breakdown by project
#[derive(Debug, Clone, Serialize)]
pub struct ProjectSpend {
    pub project_id: Option<Uuid>,
    pub project_name: Option<String>,
    /// Total cost in microcents (1/1,000,000 of a dollar)
    pub total_cost_microcents: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub request_count: i64,
    pub image_count: i64,
    pub audio_seconds: i64,
    pub character_count: i64,
}

/// Daily usage breakdown by project
#[derive(Debug, Clone, Serialize)]
pub struct DailyProjectSpend {
    pub date: NaiveDate,
    pub project_id: Option<Uuid>,
    pub project_name: Option<String>,
    /// Total cost in microcents (1/1,000,000 of a dollar)
    pub total_cost_microcents: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub request_count: i64,
    pub image_count: i64,
    pub audio_seconds: i64,
    pub character_count: i64,
}

/// Usage breakdown by team
#[derive(Debug, Clone, Serialize)]
pub struct TeamSpend {
    pub team_id: Option<Uuid>,
    pub team_name: Option<String>,
    /// Total cost in microcents (1/1,000,000 of a dollar)
    pub total_cost_microcents: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub request_count: i64,
    pub image_count: i64,
    pub audio_seconds: i64,
    pub character_count: i64,
}

/// Daily usage breakdown by team
#[derive(Debug, Clone, Serialize)]
pub struct DailyTeamSpend {
    pub date: NaiveDate,
    pub team_id: Option<Uuid>,
    pub team_name: Option<String>,
    /// Total cost in microcents (1/1,000,000 of a dollar)
    pub total_cost_microcents: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub request_count: i64,
    pub image_count: i64,
    pub audio_seconds: i64,
    pub character_count: i64,
}

/// Usage breakdown by organization
#[derive(Debug, Clone, Serialize)]
pub struct OrgSpend {
    pub org_id: Option<Uuid>,
    pub org_name: Option<String>,
    /// Total cost in microcents (1/1,000,000 of a dollar)
    pub total_cost_microcents: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub request_count: i64,
    pub image_count: i64,
    pub audio_seconds: i64,
    pub character_count: i64,
}

/// Daily usage breakdown by organization
#[derive(Debug, Clone, Serialize)]
pub struct DailyOrgSpend {
    pub date: NaiveDate,
    pub org_id: Option<Uuid>,
    pub org_name: Option<String>,
    /// Total cost in microcents (1/1,000,000 of a dollar)
    pub total_cost_microcents: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub request_count: i64,
    pub image_count: i64,
    pub audio_seconds: i64,
    pub character_count: i64,
}

/// Cost forecast for predicting remaining budget lifespan
#[derive(Debug, Clone, Serialize)]
pub struct CostForecast {
    /// Current spend in the budget period (microcents)
    pub current_spend_microcents: i64,
    /// Budget limit (microcents), None if no budget configured
    pub budget_limit_microcents: Option<i64>,
    /// Budget period (daily or monthly)
    pub budget_period: Option<String>,
    /// Average daily spend (microcents) based on historical data
    pub avg_daily_spend_microcents: i64,
    /// Standard deviation of daily spend (microcents)
    pub std_dev_daily_spend_microcents: i64,
    /// Number of days of historical data used
    pub sample_days: i32,
    /// Projected days until budget exhaustion (None if no budget or zero spend rate)
    pub days_until_exhaustion: Option<f64>,
    /// Projected exhaustion date (None if no budget or zero spend rate)
    pub projected_exhaustion_date: Option<NaiveDate>,
    /// Lower bound days until exhaustion (95% confidence, assumes +1 std dev spend)
    pub days_until_exhaustion_lower: Option<f64>,
    /// Upper bound days until exhaustion (95% confidence, assumes -1 std dev spend)
    pub days_until_exhaustion_upper: Option<f64>,
    /// Percentage of budget used in current period
    pub budget_utilization_percent: Option<f64>,
    /// Projected end-of-period spend at current rate (microcents)
    pub projected_period_spend_microcents: Option<i64>,
    /// Multi-step time series forecast (None if insufficient data for forecasting)
    pub time_series_forecast: Option<ForecastTimeSeries>,
}

/// Multi-step time series forecast with prediction intervals
#[derive(Debug, Clone, Serialize)]
pub struct ForecastTimeSeries {
    /// Dates for each forecast point
    pub dates: Vec<NaiveDate>,
    /// Point forecasts (daily spend in microcents)
    pub point_forecasts: Vec<f64>,
    /// Lower bound of prediction interval (microcents)
    pub lower_bounds: Vec<f64>,
    /// Upper bound of prediction interval (microcents)
    pub upper_bounds: Vec<f64>,
    /// Confidence level for prediction intervals (e.g., 0.95 for 95%)
    pub confidence_level: f64,
    /// Whether MSTL decomposition was used (false = simple ETS)
    pub used_seasonal_decomposition: bool,
}
