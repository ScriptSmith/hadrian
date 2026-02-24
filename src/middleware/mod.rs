mod admin;
mod authz;
mod budget;
mod combined;
mod file_search;
mod rate_limit;
mod request_id;
mod scope;
mod security_headers;
mod usage;

pub use admin::{AdminAuth, admin_auth_middleware};

/// Client connection metadata extracted by middleware for audit logging.
#[derive(Debug, Clone, Default)]
pub struct ClientInfo {
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}
pub use authz::{
    AuthzContext, api_authz_middleware, authz_middleware, permissive_authz_middleware,
};
pub use combined::api_middleware;
pub use file_search::{
    FileSearchAuthContext, FileSearchContext, FileSearchToolArguments, ProviderCallback,
    preprocess_file_search_tools, wrap_streaming_with_file_search,
};
// These are available for non-streaming responses and client-side integration:
// FileSearchMiddlewareError, FileSearchToolCall, FileSearchToolResult,
// check_response_for_file_search, detect_file_search_in_chunk,
// format_tool_result_json, parse_file_search_tool_call
#[cfg(feature = "sso")]
pub use rate_limit::extract_client_ip_from_parts;
pub use rate_limit::rate_limit_middleware;
pub use request_id::{RequestId, request_id_middleware};
pub use scope::required_scope_for_path;
pub use security_headers::security_headers_middleware;
pub use usage::{UsageTracker, extract_full_usage_from_response, tracker_from_headers};
