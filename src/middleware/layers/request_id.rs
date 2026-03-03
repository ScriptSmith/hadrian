//! Request ID middleware for request correlation.
//!
//! Generates or propagates a unique request ID for each request,
//! enabling distributed tracing and log correlation.

use axum::{
    body::Body,
    extract::Request,
    http::header::CONTENT_TYPE,
    middleware::Next,
    response::{IntoResponse, Response},
};
use http_body_util::BodyExt;
use uuid::Uuid;

/// Header name for the request ID.
pub const REQUEST_ID_HEADER: &str = "X-Request-Id";

/// Extension containing the request ID for the current request.
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

impl RequestId {
    /// Generate a new request ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create from an existing ID.
    pub fn from_string(id: String) -> Self {
        Self(id)
    }

    /// Get the ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Middleware that adds a request ID to each request.
///
/// If the request already has an X-Request-Id header, it's used.
/// Otherwise, a new UUID is generated.
///
/// For error responses (4xx/5xx with JSON body), the request ID is also
/// injected into the `error.request_id` field for correlation with logs.
pub async fn request_id_middleware(mut req: Request, next: Next) -> Response {
    // Check for existing request ID in headers
    let request_id = req
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| RequestId::from_string(s.to_string()))
        .unwrap_or_else(RequestId::new);

    // Add to extensions for use by handlers and other middleware
    req.extensions_mut().insert(request_id.clone());

    // Create a span with the request ID for structured logging
    let span = tracing::info_span!(
        "request",
        request_id = %request_id,
        method = %req.method(),
        path = %req.uri().path(),
    );

    // Run the request within the span
    let _guard = span.enter();

    let response = next.run(req).await;

    // Inject request_id into error responses
    let response = inject_request_id_into_error(response, &request_id).await;

    // Add request ID to response headers
    let mut response = response;
    if let Ok(value) = request_id.0.parse() {
        response.headers_mut().insert(REQUEST_ID_HEADER, value);
    }

    response
}

/// Inject request_id into JSON error responses.
///
/// For error responses (4xx/5xx status codes) with JSON content type,
/// this function parses the body and adds the request_id to the
/// `error.request_id` field if the response has an `error` object.
async fn inject_request_id_into_error(response: Response, request_id: &RequestId) -> Response {
    let status = response.status();

    // Only process error responses
    if !status.is_client_error() && !status.is_server_error() {
        return response;
    }

    // Check if content type is JSON
    let is_json = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("application/json"));

    if !is_json {
        return response;
    }

    // Extract parts and body
    let (parts, body) = response.into_parts();

    // Collect body bytes
    let bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => {
            // If we can't read the body, return an empty error response
            return (parts, Body::empty()).into_response();
        }
    };

    // Try to parse and modify the JSON
    let modified_bytes = match serde_json::from_slice::<serde_json::Value>(&bytes) {
        Ok(mut json) => {
            // Inject request_id into error.request_id if error object exists
            if let Some(error) = json.get_mut("error").and_then(|e| e.as_object_mut()) {
                error.insert(
                    "request_id".to_string(),
                    serde_json::Value::String(request_id.0.clone()),
                );
            }
            // Serialize back to bytes
            serde_json::to_vec(&json).unwrap_or_else(|_| bytes.to_vec())
        }
        Err(_) => {
            // Not valid JSON, return original bytes
            bytes.to_vec()
        }
    };

    // Rebuild response with modified body
    Response::from_parts(parts, Body::from(modified_bytes))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::*;

    #[test]
    fn test_request_id_generation() {
        let id1 = RequestId::new();
        let id2 = RequestId::new();
        assert_ne!(id1.0, id2.0);
    }

    #[test]
    fn test_request_id_from_string() {
        let id = RequestId::from_string("test-123".to_string());
        assert_eq!(id.as_str(), "test-123");
    }

    #[tokio::test]
    async fn test_inject_request_id_into_error_response() {
        let request_id = RequestId::from_string("test-req-123".to_string());

        // Create a mock error response with JSON body
        let error_body = serde_json::json!({
            "error": {
                "type": "invalid_request_error",
                "message": "Test error",
                "code": "test_error"
            }
        });

        let response = Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&error_body).unwrap()))
            .unwrap();

        // Inject request_id
        let modified = inject_request_id_into_error(response, &request_id).await;

        // Verify status is preserved
        assert_eq!(modified.status(), StatusCode::BAD_REQUEST);

        // Verify body contains request_id
        let (_, body) = modified.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(json["error"]["request_id"].as_str(), Some("test-req-123"));
        assert_eq!(
            json["error"]["type"].as_str(),
            Some("invalid_request_error")
        );
        assert_eq!(json["error"]["message"].as_str(), Some("Test error"));
    }

    #[tokio::test]
    async fn test_inject_request_id_skips_success_response() {
        let request_id = RequestId::from_string("test-req-123".to_string());

        // Create a success response
        let body = serde_json::json!({"data": "test"});
        let response = Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let modified = inject_request_id_into_error(response, &request_id).await;

        // Verify body is unchanged (no request_id added)
        let (_, body) = modified.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert!(json.get("error").is_none());
        assert_eq!(json["data"].as_str(), Some("test"));
    }

    #[tokio::test]
    async fn test_inject_request_id_skips_non_json() {
        let request_id = RequestId::from_string("test-req-123".to_string());

        // Create an error response with non-JSON content
        let response = Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(CONTENT_TYPE, "text/plain")
            .body(Body::from("Bad Request"))
            .unwrap();

        let modified = inject_request_id_into_error(response, &request_id).await;

        // Verify body is unchanged
        let (_, body) = modified.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        assert_eq!(bytes.as_ref(), b"Bad Request");
    }

    #[tokio::test]
    async fn test_inject_request_id_handles_non_error_json() {
        let request_id = RequestId::from_string("test-req-123".to_string());

        // Create an error response with JSON but no "error" field
        let body = serde_json::json!({"status": "error", "message": "test"});
        let response = Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let modified = inject_request_id_into_error(response, &request_id).await;

        // Verify body is unchanged (no "error" object to inject into)
        let (_, body) = modified.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert!(json.get("request_id").is_none());
        assert_eq!(json["status"].as_str(), Some("error"));
    }
}
