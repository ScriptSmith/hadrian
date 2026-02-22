//! Streaming response validation wrapper.
//!
//! Provides `ValidatingStream` which wraps SSE byte streams and validates
//! each parsed JSON chunk against the appropriate OpenAPI schema.

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use bytes::Bytes;
use futures_util::Stream;

use super::schema::{ResponseType, validate_responses_streaming_chunk, validate_streaming_chunk};
use crate::config::ResponseValidationMode;

/// A stream wrapper that validates each SSE chunk against the OpenAPI schema.
///
/// For `warn` mode: logs validation errors but passes chunks through.
/// For `error` mode: terminates the stream on first validation error.
pub struct ValidatingStream<S> {
    inner: S,
    response_type: ResponseType,
    mode: ResponseValidationMode,
    /// Buffer for accumulating partial SSE lines
    line_buffer: String,
    /// Whether the stream has been terminated due to an error
    terminated: bool,
}

impl<S> ValidatingStream<S> {
    /// Create a new validating stream wrapper.
    pub fn new(inner: S, response_type: ResponseType, mode: ResponseValidationMode) -> Self {
        Self {
            inner,
            response_type,
            mode,
            line_buffer: String::new(),
            terminated: false,
        }
    }

    /// Process a complete SSE line and validate if it contains data.
    fn process_line(&mut self, line: &str) -> Result<(), String> {
        // SSE data lines start with "data: "
        if let Some(json_str) = line.strip_prefix("data: ") {
            // Skip [DONE] sentinel
            if json_str.trim() == "[DONE]" {
                return Ok(());
            }

            // Parse and validate the JSON chunk
            match serde_json::from_str::<serde_json::Value>(json_str) {
                Ok(chunk) => self.validate_chunk(&chunk),
                Err(e) => {
                    // JSON parse error - log but don't fail (might be partial chunk)
                    tracing::debug!(error = %e, "Failed to parse SSE JSON chunk");
                    Ok(())
                }
            }
        } else {
            // Not a data line (event type, comment, empty line, etc.)
            Ok(())
        }
    }

    /// Validate a parsed JSON chunk against the appropriate schema.
    fn validate_chunk(&self, chunk: &serde_json::Value) -> Result<(), String> {
        match self.response_type {
            ResponseType::ChatCompletionStream => validate_streaming_chunk(chunk),
            ResponseType::ResponseStream => validate_responses_streaming_chunk(chunk),
            _ => {
                // Non-streaming types shouldn't use ValidatingStream
                tracing::warn!(
                    response_type = ?self.response_type,
                    "ValidatingStream used for non-streaming response type"
                );
                Ok(())
            }
        }
    }
}

impl<S> Stream for ValidatingStream<S>
where
    S: Stream<Item = Result<Bytes, std::io::Error>> + Unpin,
{
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // If terminated due to error, stop yielding
        if self.terminated {
            return Poll::Ready(None);
        }

        // Poll the inner stream
        let inner = Pin::new(&mut self.inner);
        match inner.poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                // Convert to string for SSE line parsing
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    // Append to buffer and process complete lines
                    self.line_buffer.push_str(text);

                    // Process complete lines (ended with \n)
                    while let Some(newline_pos) = self.line_buffer.find('\n') {
                        let line = self.line_buffer[..newline_pos].to_string();
                        self.line_buffer = self.line_buffer[newline_pos + 1..].to_string();

                        // Process the line
                        if let Err(error) = self.process_line(&line) {
                            match self.mode {
                                ResponseValidationMode::Warn => {
                                    tracing::warn!(
                                        error = %error,
                                        line = %line,
                                        "Streaming response validation failed"
                                    );
                                    // Continue processing
                                }
                                ResponseValidationMode::Error => {
                                    tracing::error!(
                                        error = %error,
                                        "Streaming response validation failed, terminating stream"
                                    );
                                    self.terminated = true;
                                    // Return an error event to the client
                                    return Poll::Ready(Some(Ok(Bytes::from(
                                        "data: {\"error\":{\"type\":\"server_error\",\"message\":\"Internal server error\"}}\n\n",
                                    ))));
                                }
                            }
                        }
                    }
                }

                // Pass through the original bytes
                Poll::Ready(Some(Ok(bytes)))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use futures_util::StreamExt;
    use tokio_stream::iter;

    use super::*;

    #[tokio::test]
    async fn test_validating_stream_passes_valid_chunks() {
        let valid_chunk = r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1694268190,"model":"gpt-4o-mini","choices":[{"index":0,"delta":{"content":"Hello"},"logprobs":null,"finish_reason":null}]}

"#;
        let stream = iter(vec![Ok(Bytes::from(valid_chunk))]);
        let mut validating = ValidatingStream::new(
            stream,
            ResponseType::ChatCompletionStream,
            ResponseValidationMode::Warn,
        );

        let result = validating.next().await;
        assert!(result.is_some());
        assert!(result.unwrap().is_ok());
    }

    #[tokio::test]
    async fn test_validating_stream_warn_mode_continues() {
        // Invalid chunk (missing required fields)
        let invalid_chunk = r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk"}

"#;
        let stream = iter(vec![
            Ok(Bytes::from(invalid_chunk)),
            Ok(Bytes::from("data: [DONE]\n\n")),
        ]);
        let mut validating = ValidatingStream::new(
            stream,
            ResponseType::ChatCompletionStream,
            ResponseValidationMode::Warn,
        );

        // Should still get both chunks in warn mode
        let first = validating.next().await;
        assert!(first.is_some());

        let second = validating.next().await;
        assert!(second.is_some());
    }

    #[cfg(feature = "response-validation")]
    #[tokio::test]
    async fn test_validating_stream_error_mode_terminates() {
        // Invalid chunk (missing required fields)
        let invalid_chunk = r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk"}

"#;
        let stream = iter(vec![
            Ok(Bytes::from(invalid_chunk)),
            Ok(Bytes::from("data: [DONE]\n\n")),
        ]);
        let mut validating = ValidatingStream::new(
            stream,
            ResponseType::ChatCompletionStream,
            ResponseValidationMode::Error,
        );

        // First chunk should return error event
        let first = validating.next().await;
        assert!(first.is_some());
        let bytes = first.unwrap().unwrap();
        assert!(std::str::from_utf8(&bytes).unwrap().contains("error"));

        // Stream should be terminated
        let second = validating.next().await;
        assert!(second.is_none());
    }

    #[tokio::test]
    async fn test_validating_stream_skips_done_sentinel() {
        let chunks = "data: [DONE]\n\n";
        let stream = iter(vec![Ok(Bytes::from(chunks))]);
        let mut validating = ValidatingStream::new(
            stream,
            ResponseType::ChatCompletionStream,
            ResponseValidationMode::Error,
        );

        // Should pass through without validation error
        let result = validating.next().await;
        assert!(result.is_some());
        assert!(result.unwrap().is_ok());
    }
}
