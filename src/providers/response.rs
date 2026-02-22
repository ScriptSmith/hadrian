//! Response building helpers for providers.
//!
//! This module provides helper functions to reduce boilerplate when building
//! HTTP responses in provider implementations. These helpers ensure consistent
//! headers and response formats across all providers.
//!
//! # Usage
//!
//! ```ignore
//! use crate::providers::response::{json_response, streaming_response, error_response};
//!
//! // Build a JSON response
//! let response = json_response(StatusCode::OK, &openai_response)?;
//!
//! // Build a streaming SSE response
//! let response = streaming_response(StatusCode::OK, transformed_stream)?;
//!
//! // Build an error response from a failed provider response
//! let response = error_response::<AnthropicErrorParser>(response).await?;
//! ```

use axum::{body::Body, response::Response};
use bytes::Bytes;
use futures_util::Stream;
use http::StatusCode;
use serde::Serialize;

use super::{
    ProviderError,
    error::{ProviderErrorParser, build_provider_error_response},
};

/// Build a JSON response with the standard `application/json` content type.
///
/// This is the most common response type for non-streaming LLM responses.
/// The body is serialized to JSON and the appropriate content-type header is set.
///
/// # Example
///
/// ```ignore
/// let openai_response = convert_response(provider_response);
/// json_response(StatusCode::OK, &openai_response)
/// ```
pub fn json_response<T: Serialize>(
    status: StatusCode,
    body: &T,
) -> Result<Response, ProviderError> {
    let json = serde_json::to_string(body).unwrap_or_default();

    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(json))
        .map_err(ProviderError::ResponseBuilder)
}

/// Build a Server-Sent Events (SSE) streaming response.
///
/// Sets the appropriate headers for SSE streaming:
/// - `content-type: text/event-stream`
/// - `cache-control: no-cache`
/// - `transfer-encoding: chunked`
///
/// The stream should emit `Bytes` chunks in SSE format:
/// ```text
/// data: {"id":"...","object":"chat.completion.chunk",...}\n\n
/// ```
///
/// # Example
///
/// ```ignore
/// let byte_stream = response.bytes_stream().map(|r| r.map_err(std::io::Error::other));
/// let transformed = AnthropicToOpenAIStream::new(byte_stream, &streaming_buffer);
/// streaming_response(StatusCode::OK, transformed)
/// ```
pub fn streaming_response<S, E>(status: StatusCode, stream: S) -> Result<Response, ProviderError>
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: Into<Box<dyn std::error::Error + Send + Sync>> + 'static,
{
    Response::builder()
        .status(status)
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-cache")
        .header("transfer-encoding", "chunked")
        .body(Body::from_stream(stream))
        .map_err(ProviderError::ResponseBuilder)
}

/// Build an error response from a failed provider HTTP response.
///
/// This helper:
/// 1. Reads the response body
/// 2. Parses the error using the provider's error parser
/// 3. Converts it to an OpenAI-compatible error response
///
/// The type parameter `P` specifies which error parser to use (e.g., `AnthropicErrorParser`).
///
/// # Example
///
/// ```ignore
/// if !response.status().is_success() {
///     return error_response::<AnthropicErrorParser>(response).await;
/// }
/// ```
pub async fn error_response<P: ProviderErrorParser>(
    response: reqwest::Response,
) -> Result<Response, ProviderError> {
    let status = response.status();
    let headers = response.headers().clone();
    let body_bytes = response.bytes().await.unwrap_or_default();

    let error_info = P::parse_error(status, &headers, &body_bytes);
    build_provider_error_response(status, error_info)
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;
    use futures_util::stream;
    use serde_json::json;

    use super::*;

    #[test]
    fn test_json_response_success() {
        let body = json!({"id": "test", "choices": []});
        let response = json_response(StatusCode::OK, &body).unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/json"
        );
    }

    #[tokio::test]
    async fn test_streaming_response() {
        let chunks: Vec<Result<Bytes, std::io::Error>> = vec![
            Ok(Bytes::from("data: {\"test\":1}\n\n")),
            Ok(Bytes::from("data: [DONE]\n\n")),
        ];
        let stream = stream::iter(chunks);

        let response = streaming_response(StatusCode::OK, stream).unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/event-stream"
        );
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-cache");
        assert_eq!(
            response.headers().get("transfer-encoding").unwrap(),
            "chunked"
        );

        // Verify body content
        let body_bytes = to_bytes(response.into_body(), 1024).await.unwrap();
        let body_str = String::from_utf8_lossy(&body_bytes);
        assert!(body_str.contains("data: {\"test\":1}"));
        assert!(body_str.contains("data: [DONE]"));
    }

    #[test]
    fn test_json_response_with_error_status() {
        let body = json!({"error": {"message": "Bad request"}});
        let response = json_response(StatusCode::BAD_REQUEST, &body).unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
