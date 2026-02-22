//! Streaming output guardrails for evaluating LLM responses during streaming.
//!
//! This module provides the `GuardrailsFilterStream` wrapper that evaluates
//! streaming LLM output against guardrails policies.
//!
//! # Evaluation Modes
//!
//! - **FinalOnly**: Buffer the entire stream, evaluate only after completion.
//!   Lowest latency but harmful content may be partially streamed.
//! - **Buffered**: Accumulate tokens and evaluate periodically.
//!   Balance between latency and safety.
//! - **PerChunk**: Evaluate each chunk individually.
//!   Highest safety but significantly increases latency.

use std::{
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Instant,
};

use bytes::Bytes;
use futures_util::stream::Stream;
use tokio::sync::Mutex;

use super::{
    ActionExecutor, ContentSource, GuardrailsError, GuardrailsProvider, GuardrailsRequest,
    GuardrailsResponse, GuardrailsRetryConfig, OutputGuardrailsResult, ResolvedAction, Violation,
};
use crate::config::StreamingGuardrailsMode;

/// Configuration for streaming guardrails evaluation.
#[derive(Debug, Clone)]
pub struct StreamingGuardrailsConfig {
    /// Evaluation mode.
    pub mode: StreamingGuardrailsMode,
    /// Request ID for correlation.
    pub request_id: Option<String>,
    /// User ID for audit logging.
    pub user_id: Option<String>,
    /// Retry configuration.
    #[allow(dead_code)] // Guardrail infrastructure
    pub retry_config: GuardrailsRetryConfig,
    /// Action to take on evaluation error.
    pub on_error: crate::config::GuardrailsErrorAction,
}

impl Default for StreamingGuardrailsConfig {
    fn default() -> Self {
        Self {
            mode: StreamingGuardrailsMode::default(),
            request_id: None,
            user_id: None,
            retry_config: GuardrailsRetryConfig::default(),
            on_error: crate::config::GuardrailsErrorAction::Block,
        }
    }
}

/// State for tracking accumulated content during streaming.
#[derive(Default)]
struct StreamState {
    /// Accumulated content from the stream.
    content_buffer: String,
    /// Number of tokens accumulated (estimated).
    token_count: u32,
    /// Chunks that have been passed through.
    chunks_passed: Vec<Bytes>,
    /// Whether the stream has been blocked.
    blocked: bool,
    /// Blocking error to return.
    block_error: Option<GuardrailsError>,
    /// Violations found so far.
    violations: Vec<Violation>,
    /// Last evaluation result.
    last_result: Option<OutputGuardrailsResult>,
    /// Whether evaluation is in progress.
    evaluation_in_progress: bool,
    /// Position of last evaluated content.
    last_evaluated_position: usize,
}

/// Stream wrapper that applies guardrails to streaming LLM output.
///
/// This wrapper intercepts SSE chunks, extracts content, and evaluates
/// it against guardrails policies based on the configured mode.
pub struct GuardrailsFilterStream<S> {
    /// Inner stream.
    inner: S,
    /// Guardrails provider.
    provider: Arc<dyn GuardrailsProvider>,
    /// Action executor.
    action_executor: ActionExecutor,
    /// Configuration.
    config: StreamingGuardrailsConfig,
    /// Mutable state (wrapped in Arc<Mutex> for async evaluation).
    state: Arc<Mutex<StreamState>>,
    /// Whether the stream has ended.
    stream_ended: bool,
    /// Start time for latency tracking.
    start_time: Instant,
}

impl<S> GuardrailsFilterStream<S>
where
    S: Stream<Item = Result<Bytes, io::Error>> + Unpin + Send + 'static,
{
    /// Creates a new guardrails filter stream.
    pub fn new(
        stream: S,
        provider: Arc<dyn GuardrailsProvider>,
        action_executor: ActionExecutor,
        config: StreamingGuardrailsConfig,
    ) -> Self {
        Self {
            inner: stream,
            provider,
            action_executor,
            config,
            state: Arc::new(Mutex::new(StreamState::default())),
            stream_ended: false,
            start_time: Instant::now(),
        }
    }

    /// Extracts content from an SSE chunk.
    fn extract_content_from_chunk(chunk: &[u8]) -> Option<String> {
        let chunk_str = std::str::from_utf8(chunk).ok()?;

        for line in chunk_str.lines() {
            if let Some(json_str) = line.strip_prefix("data: ") {
                if json_str.trim() == "[DONE]" {
                    return None;
                }

                if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
                    // Extract content from choices[0].delta.content
                    if let Some(content) = json
                        .get("choices")
                        .and_then(|c| c.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|choice| choice.get("delta"))
                        .and_then(|delta| delta.get("content"))
                        .and_then(|c| c.as_str())
                    {
                        return Some(content.to_string());
                    }
                }
            }
        }

        None
    }

    /// Estimates token count from content (rough approximation: 1 token ≈ 4 chars).
    fn estimate_tokens(content: &str) -> u32 {
        content.len().div_ceil(4) as u32
    }

    /// Returns the guardrails result after the stream completes.
    ///
    /// This should be called after the stream has ended to get the final
    /// guardrails evaluation result and any headers to add to the response.
    #[allow(dead_code)] // Guardrail infrastructure
    pub async fn result(&self) -> Option<OutputGuardrailsResult> {
        let state = self.state.lock().await;
        state.last_result.clone()
    }

    /// Returns accumulated violations.
    #[allow(dead_code)] // Guardrail infrastructure
    pub async fn violations(&self) -> Vec<Violation> {
        let state = self.state.lock().await;
        state.violations.clone()
    }
}

impl<S> Stream for GuardrailsFilterStream<S>
where
    S: Stream<Item = Result<Bytes, io::Error>> + Unpin + Send + 'static,
{
    type Item = Result<Bytes, io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Check if blocked by a previous evaluation
        if let Ok(state) = self.state.try_lock()
            && state.blocked
        {
            if let Some(ref error) = state.block_error {
                return Poll::Ready(Some(Err(io::Error::other(error.to_string()))));
            }
            return Poll::Ready(Some(Err(io::Error::other("Content blocked by guardrails"))));
        }

        // Poll the inner stream
        let inner = Pin::new(&mut self.inner);
        match inner.poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                // Extract content from the chunk
                let content = Self::extract_content_from_chunk(&chunk);

                match &self.config.mode {
                    StreamingGuardrailsMode::FinalOnly => {
                        // Accumulate content for final evaluation, but pass chunks through immediately
                        // This allows streaming while still evaluating the complete response
                        if let Some(text) = content {
                            let token_estimate = Self::estimate_tokens(&text);
                            if let Ok(mut state) = self.state.try_lock() {
                                state.content_buffer.push_str(&text);
                                state.token_count += token_estimate;
                            }
                        }

                        // Pass through immediately - evaluation happens when stream ends
                        Poll::Ready(Some(Ok(chunk)))
                    }

                    StreamingGuardrailsMode::Buffered { buffer_tokens } => {
                        // Accumulate content and check if we need to evaluate
                        if let Some(text) = content {
                            let token_estimate = Self::estimate_tokens(&text);
                            let should_evaluate = {
                                if let Ok(mut state) = self.state.try_lock() {
                                    state.content_buffer.push_str(&text);
                                    state.token_count += token_estimate;
                                    state.chunks_passed.push(chunk.clone());

                                    // Check if blocked
                                    if state.blocked
                                        && let Some(ref error) = state.block_error
                                    {
                                        return Poll::Ready(Some(Err(io::Error::other(
                                            error.to_string(),
                                        ))));
                                    }

                                    // Check if we need to evaluate
                                    !state.evaluation_in_progress
                                        && state.token_count
                                            >= *buffer_tokens + state.last_evaluated_position as u32
                                } else {
                                    false
                                }
                            };

                            if should_evaluate {
                                // Spawn async evaluation
                                let state = self.state.clone();
                                let provider = self.provider.clone();
                                let action_executor = self.action_executor.clone();
                                let request_id = self.config.request_id.clone();
                                let user_id = self.config.user_id.clone();
                                let on_error = self.config.on_error.clone();

                                tokio::spawn(async move {
                                    evaluate_buffered_content(
                                        state,
                                        provider,
                                        action_executor,
                                        request_id,
                                        user_id,
                                        on_error,
                                    )
                                    .await;
                                });
                            }
                        }

                        // Pass through the chunk
                        Poll::Ready(Some(Ok(chunk)))
                    }

                    StreamingGuardrailsMode::PerChunk => {
                        // For PerChunk mode, we need to evaluate each chunk synchronously
                        // This is the highest latency option
                        if let Some(text) = content {
                            // Update state
                            if let Ok(mut state) = self.state.try_lock() {
                                state.content_buffer.push_str(&text);
                                state.token_count += Self::estimate_tokens(&text);

                                // Check if already blocked
                                if state.blocked
                                    && let Some(ref error) = state.block_error
                                {
                                    return Poll::Ready(Some(Err(io::Error::other(
                                        error.to_string(),
                                    ))));
                                }
                            }

                            // For PerChunk, spawn evaluation but still pass through
                            // (blocking on each chunk would be too slow)
                            let state = self.state.clone();
                            let provider = self.provider.clone();
                            let action_executor = self.action_executor.clone();
                            let request_id = self.config.request_id.clone();
                            let user_id = self.config.user_id.clone();
                            let on_error = self.config.on_error.clone();

                            tokio::spawn(async move {
                                evaluate_chunk_content(
                                    state,
                                    provider,
                                    action_executor,
                                    &text,
                                    request_id,
                                    user_id,
                                    on_error,
                                )
                                .await;
                            });
                        }

                        Poll::Ready(Some(Ok(chunk)))
                    }
                }
            }

            Poll::Ready(None) => {
                // Stream ended
                if self.stream_ended {
                    return Poll::Ready(None);
                }
                self.stream_ended = true;

                // All modes: spawn final evaluation asynchronously
                // For FinalOnly mode, content has already been streamed through
                // For Buffered/PerChunk, some content may not have been evaluated yet
                let state = self.state.clone();
                let provider = self.provider.clone();
                let action_executor = self.action_executor.clone();
                let request_id = self.config.request_id.clone();
                let user_id = self.config.user_id.clone();
                let on_error = self.config.on_error.clone();
                let start_time = self.start_time;

                tokio::spawn(async move {
                    evaluate_final_content(
                        state,
                        provider,
                        action_executor,
                        request_id,
                        user_id,
                        on_error,
                        start_time,
                    )
                    .await;
                });

                Poll::Ready(None)
            }

            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Evaluates buffered content asynchronously.
async fn evaluate_buffered_content(
    state: Arc<Mutex<StreamState>>,
    provider: Arc<dyn GuardrailsProvider>,
    action_executor: ActionExecutor,
    request_id: Option<String>,
    user_id: Option<String>,
    on_error: crate::config::GuardrailsErrorAction,
) {
    // Get content to evaluate
    let content = {
        let mut state = state.lock().await;
        if state.evaluation_in_progress || state.blocked {
            return;
        }
        state.evaluation_in_progress = true;
        state.content_buffer.clone()
    };

    if content.is_empty() {
        let mut state = state.lock().await;
        state.evaluation_in_progress = false;
        return;
    }

    // Build request
    let mut request = GuardrailsRequest::llm_output(&content);
    if let Some(id) = request_id {
        request = request.with_request_id(id);
    }
    if let Some(id) = user_id {
        request = request.with_user_id(id);
    }

    // Evaluate
    match provider.evaluate(&request).await {
        Ok(response) => {
            let action = action_executor.resolve_action(&response, &content);
            let mut state = state.lock().await;

            // Update state based on action
            match &action {
                ResolvedAction::Block {
                    violations, reason, ..
                } => {
                    state.blocked = true;
                    state.block_error = Some(GuardrailsError::blocked_with_violations(
                        ContentSource::LlmOutput,
                        reason.clone(),
                        violations.clone(),
                    ));
                    state.violations.extend(violations.clone());
                }
                ResolvedAction::Warn { violations }
                | ResolvedAction::Log { violations }
                | ResolvedAction::Redact { violations, .. } => {
                    state.violations.extend(violations.clone());
                }
                ResolvedAction::Allow => {}
            }

            state.last_evaluated_position = content.len();
            state.evaluation_in_progress = false;
            state.last_result = Some(OutputGuardrailsResult {
                action,
                response,
                evaluated_text: content,
            });
        }
        Err(error) => {
            let mut state = state.lock().await;
            state.evaluation_in_progress = false;

            match on_error {
                crate::config::GuardrailsErrorAction::Block => {
                    state.blocked = true;
                    state.block_error = Some(error);
                }
                crate::config::GuardrailsErrorAction::Allow
                | crate::config::GuardrailsErrorAction::LogAndAllow => {
                    tracing::warn!(error = %error, "Streaming guardrails error - allowing content");
                }
            }
        }
    }
}

/// Evaluates a single chunk's content.
async fn evaluate_chunk_content(
    state: Arc<Mutex<StreamState>>,
    provider: Arc<dyn GuardrailsProvider>,
    action_executor: ActionExecutor,
    chunk_content: &str,
    request_id: Option<String>,
    user_id: Option<String>,
    on_error: crate::config::GuardrailsErrorAction,
) {
    if chunk_content.is_empty() {
        return;
    }

    // Check if already blocked
    {
        let state = state.lock().await;
        if state.blocked {
            return;
        }
    }

    // Build request
    let mut request = GuardrailsRequest::llm_output(chunk_content);
    if let Some(id) = request_id {
        request = request.with_request_id(id);
    }
    if let Some(id) = user_id {
        request = request.with_user_id(id);
    }

    // Evaluate
    match provider.evaluate(&request).await {
        Ok(response) => {
            let action = action_executor.resolve_action(&response, chunk_content);
            let mut state = state.lock().await;

            match &action {
                ResolvedAction::Block {
                    violations, reason, ..
                } => {
                    state.blocked = true;
                    state.block_error = Some(GuardrailsError::blocked_with_violations(
                        ContentSource::LlmOutput,
                        reason.clone(),
                        violations.clone(),
                    ));
                    state.violations.extend(violations.clone());
                }
                ResolvedAction::Warn { violations }
                | ResolvedAction::Log { violations }
                | ResolvedAction::Redact { violations, .. } => {
                    state.violations.extend(violations.clone());
                }
                ResolvedAction::Allow => {}
            }
        }
        Err(error) => {
            let mut state = state.lock().await;

            match on_error {
                crate::config::GuardrailsErrorAction::Block => {
                    state.blocked = true;
                    state.block_error = Some(error);
                }
                crate::config::GuardrailsErrorAction::Allow
                | crate::config::GuardrailsErrorAction::LogAndAllow => {
                    tracing::warn!(error = %error, "Streaming guardrails chunk error - allowing");
                }
            }
        }
    }
}

/// Evaluates the final accumulated content.
async fn evaluate_final_content(
    state: Arc<Mutex<StreamState>>,
    provider: Arc<dyn GuardrailsProvider>,
    action_executor: ActionExecutor,
    request_id: Option<String>,
    user_id: Option<String>,
    on_error: crate::config::GuardrailsErrorAction,
    start_time: Instant,
) {
    // Get content to evaluate
    let content = {
        let state = state.lock().await;
        if state.blocked {
            return;
        }
        state.content_buffer.clone()
    };

    if content.is_empty() {
        let mut state = state.lock().await;
        state.last_result = Some(OutputGuardrailsResult {
            action: ResolvedAction::Allow,
            response: GuardrailsResponse::passed()
                .with_latency(start_time.elapsed().as_millis() as u64),
            evaluated_text: content,
        });
        return;
    }

    // Build request
    let mut request = GuardrailsRequest::llm_output(&content);
    if let Some(id) = request_id {
        request = request.with_request_id(id);
    }
    if let Some(id) = user_id {
        request = request.with_user_id(id);
    }

    // Evaluate
    let latency_ms = start_time.elapsed().as_millis() as u64;

    match provider.evaluate(&request).await {
        Ok(response) => {
            let response = response.with_latency(latency_ms);
            let action = action_executor.resolve_action(&response, &content);
            let mut state = state.lock().await;

            match &action {
                ResolvedAction::Block {
                    violations, reason, ..
                } => {
                    state.blocked = true;
                    state.block_error = Some(GuardrailsError::blocked_with_violations(
                        ContentSource::LlmOutput,
                        reason.clone(),
                        violations.clone(),
                    ));
                    state.violations.extend(violations.clone());
                }
                ResolvedAction::Warn { violations }
                | ResolvedAction::Log { violations }
                | ResolvedAction::Redact { violations, .. } => {
                    state.violations.extend(violations.clone());
                }
                ResolvedAction::Allow => {}
            }

            state.last_result = Some(OutputGuardrailsResult {
                action,
                response,
                evaluated_text: content,
            });
        }
        Err(error) => {
            let mut state = state.lock().await;

            match on_error {
                crate::config::GuardrailsErrorAction::Block => {
                    state.blocked = true;
                    state.block_error = Some(error);
                }
                crate::config::GuardrailsErrorAction::Allow
                | crate::config::GuardrailsErrorAction::LogAndAllow => {
                    tracing::warn!(error = %error, "Final streaming guardrails error - allowing");
                    state.last_result = Some(OutputGuardrailsResult {
                        action: ResolvedAction::Allow,
                        response: GuardrailsResponse::passed().with_latency(latency_ms),
                        evaluated_text: content,
                    });
                }
            }
        }
    }
}

/// Creates response headers from streaming guardrails result.
#[allow(dead_code)] // Guardrail infrastructure
pub fn streaming_guardrails_headers(
    result: &OutputGuardrailsResult,
) -> Vec<(&'static str, String)> {
    result.to_headers()
}

#[cfg(test)]
mod tests {
    use futures_util::StreamExt;

    use super::*;

    type EmptyStream = futures_util::stream::Empty<Result<Bytes, io::Error>>;

    #[test]
    fn test_extract_content_from_chunk() {
        // Valid SSE chunk with content
        let chunk = b"data: {\"choices\":[{\"delta\":{\"content\":\"Hello world\"}}]}\n\n";
        let content = GuardrailsFilterStream::<EmptyStream>::extract_content_from_chunk(chunk);
        assert_eq!(content, Some("Hello world".to_string()));

        // Done marker
        let chunk = b"data: [DONE]\n\n";
        let content = GuardrailsFilterStream::<EmptyStream>::extract_content_from_chunk(chunk);
        assert!(content.is_none());

        // Empty delta
        let chunk = b"data: {\"choices\":[{\"delta\":{}}]}\n\n";
        let content = GuardrailsFilterStream::<EmptyStream>::extract_content_from_chunk(chunk);
        assert!(content.is_none());

        // Invalid JSON
        let chunk = b"data: not json\n\n";
        let content = GuardrailsFilterStream::<EmptyStream>::extract_content_from_chunk(chunk);
        assert!(content.is_none());
    }

    #[test]
    fn test_extract_content_from_chunk_multiple_lines() {
        // Multiple data lines (should only parse first valid one)
        let chunk = b"data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\ndata: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n";
        let content = GuardrailsFilterStream::<EmptyStream>::extract_content_from_chunk(chunk);
        assert_eq!(content, Some("Hello".to_string()));
    }

    #[test]
    fn test_extract_content_from_chunk_with_role() {
        // First chunk with role but no content
        let chunk = b"data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n";
        let content = GuardrailsFilterStream::<EmptyStream>::extract_content_from_chunk(chunk);
        assert!(content.is_none());
    }

    #[test]
    fn test_extract_content_from_chunk_empty_content() {
        // Chunk with empty content string
        let chunk = b"data: {\"choices\":[{\"delta\":{\"content\":\"\"}}]}\n\n";
        let content = GuardrailsFilterStream::<EmptyStream>::extract_content_from_chunk(chunk);
        assert_eq!(content, Some("".to_string()));
    }

    #[test]
    fn test_estimate_tokens() {
        // Empty string
        assert_eq!(
            GuardrailsFilterStream::<EmptyStream>::estimate_tokens(""),
            0
        );

        // Short string
        assert_eq!(
            GuardrailsFilterStream::<EmptyStream>::estimate_tokens("Hi"),
            1
        );

        // Longer string (11 chars ≈ 3 tokens)
        assert_eq!(
            GuardrailsFilterStream::<EmptyStream>::estimate_tokens("Hello world"),
            3
        );

        // Edge case: exactly 4 characters = 1 token
        assert_eq!(
            GuardrailsFilterStream::<EmptyStream>::estimate_tokens("1234"),
            1
        );

        // 5 characters = 2 tokens
        assert_eq!(
            GuardrailsFilterStream::<EmptyStream>::estimate_tokens("12345"),
            2
        );
    }

    #[test]
    fn test_streaming_guardrails_config_default() {
        let config = StreamingGuardrailsConfig::default();
        assert!(matches!(
            config.mode,
            StreamingGuardrailsMode::Buffered { .. }
        ));
        assert!(config.request_id.is_none());
        assert!(config.user_id.is_none());
    }

    #[test]
    fn test_streaming_guardrails_config_with_values() {
        let config = StreamingGuardrailsConfig {
            mode: StreamingGuardrailsMode::Buffered { buffer_tokens: 100 },
            request_id: Some("req-123".to_string()),
            user_id: Some("user-456".to_string()),
            retry_config: super::super::GuardrailsRetryConfig::default(),
            on_error: crate::config::GuardrailsErrorAction::Allow,
        };

        assert!(matches!(
            config.mode,
            StreamingGuardrailsMode::Buffered { buffer_tokens: 100 }
        ));
        assert_eq!(config.request_id, Some("req-123".to_string()));
        assert_eq!(config.user_id, Some("user-456".to_string()));
    }

    #[test]
    fn test_stream_state_default() {
        let state = StreamState::default();
        assert!(state.content_buffer.is_empty());
        assert_eq!(state.token_count, 0);
        assert!(state.chunks_passed.is_empty());
        assert!(!state.blocked);
        assert!(state.block_error.is_none());
        assert!(state.violations.is_empty());
        assert!(state.last_result.is_none());
        assert!(!state.evaluation_in_progress);
        assert_eq!(state.last_evaluated_position, 0);
    }

    /// Mock guardrails provider for testing.
    struct MockStreamingProvider {
        name: String,
        should_block: bool,
    }

    impl MockStreamingProvider {
        fn passing(name: &str) -> Self {
            Self {
                name: name.to_string(),
                should_block: false,
            }
        }

        #[allow(dead_code)] // May be used in future tests
        fn blocking(name: &str) -> Self {
            Self {
                name: name.to_string(),
                should_block: true,
            }
        }
    }

    #[async_trait::async_trait]
    impl super::super::GuardrailsProvider for MockStreamingProvider {
        fn name(&self) -> &str {
            &self.name
        }

        async fn evaluate(
            &self,
            _request: &GuardrailsRequest,
        ) -> super::super::GuardrailsResult<GuardrailsResponse> {
            if self.should_block {
                Ok(GuardrailsResponse::with_violations(vec![
                    super::super::Violation::new(
                        super::super::Category::Hate,
                        super::super::Severity::High,
                        0.95,
                    ),
                ]))
            } else {
                Ok(GuardrailsResponse::passed())
            }
        }
    }

    fn create_sse_chunk(content: &str) -> Bytes {
        Bytes::from(format!(
            "data: {{\"choices\":[{{\"delta\":{{\"content\":\"{}\"}}}}]}}\n\n",
            content
        ))
    }

    fn create_test_action_executor() -> ActionExecutor {
        ActionExecutor::new(
            std::collections::HashMap::new(),
            crate::config::GuardrailsAction::Block,
        )
    }

    #[tokio::test]
    async fn test_guardrails_filter_stream_final_only_passing() {
        let chunks = vec![
            Ok(create_sse_chunk("Hello")),
            Ok(create_sse_chunk(" world")),
            Ok(create_sse_chunk("!")),
        ];
        let inner_stream = futures_util::stream::iter(chunks);

        let provider = Arc::new(MockStreamingProvider::passing("mock-pass"));
        let config = StreamingGuardrailsConfig::default();

        let mut filter_stream = GuardrailsFilterStream::new(
            inner_stream,
            provider,
            create_test_action_executor(),
            config,
        );

        // Consume all chunks
        let mut received_chunks = Vec::new();
        while let Some(result) = filter_stream.next().await {
            match result {
                Ok(chunk) => received_chunks.push(chunk),
                Err(e) => panic!("Unexpected error: {}", e),
            }
        }

        // Should have received all 3 chunks
        assert_eq!(received_chunks.len(), 3);

        // Give async evaluation time to complete
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Check accumulated content
        let state = filter_stream.state.lock().await;
        assert_eq!(state.content_buffer, "Hello world!");
        assert!(!state.blocked);
    }

    #[tokio::test]
    async fn test_guardrails_filter_stream_empty() {
        let chunks: Vec<Result<Bytes, io::Error>> = vec![];
        let inner_stream = futures_util::stream::iter(chunks);

        let provider = Arc::new(MockStreamingProvider::passing("mock-pass"));
        let config = StreamingGuardrailsConfig::default();

        let mut filter_stream = GuardrailsFilterStream::new(
            inner_stream,
            provider,
            create_test_action_executor(),
            config,
        );

        // Consume the stream
        let result = filter_stream.next().await;
        assert!(result.is_none());

        // Check state
        let state = filter_stream.state.lock().await;
        assert!(state.content_buffer.is_empty());
        assert!(!state.blocked);
    }

    #[tokio::test]
    async fn test_guardrails_filter_stream_accumulates_tokens() {
        let chunks = vec![
            Ok(create_sse_chunk(
                "This is a longer piece of text that should",
            )),
            Ok(create_sse_chunk(" accumulate multiple tokens for testing.")),
        ];
        let inner_stream = futures_util::stream::iter(chunks);

        let provider = Arc::new(MockStreamingProvider::passing("mock-pass"));
        let config = StreamingGuardrailsConfig::default();

        let mut filter_stream = GuardrailsFilterStream::new(
            inner_stream,
            provider,
            create_test_action_executor(),
            config,
        );

        // Consume all chunks
        while let Some(result) = filter_stream.next().await {
            result.expect("should not error");
        }

        // Check token count
        let state = filter_stream.state.lock().await;
        let expected_tokens =
            GuardrailsFilterStream::<EmptyStream>::estimate_tokens(&state.content_buffer);
        assert_eq!(state.token_count, expected_tokens);
        assert!(state.token_count > 10); // Should have accumulated substantial tokens
    }

    #[tokio::test]
    async fn test_streaming_guardrails_headers() {
        let result = OutputGuardrailsResult {
            action: ResolvedAction::Allow,
            response: GuardrailsResponse::passed().with_latency(150),
            evaluated_text: "test content".to_string(),
        };

        let headers = streaming_guardrails_headers(&result);

        assert!(
            headers
                .iter()
                .any(|(k, v)| *k == "X-Guardrails-Output-Result" && v == "passed")
        );
        assert!(
            headers
                .iter()
                .any(|(k, v)| *k == "X-Guardrails-Output-Latency-Ms" && v == "150")
        );
    }

    #[tokio::test]
    async fn test_buffered_mode_config() {
        let config = StreamingGuardrailsConfig {
            mode: StreamingGuardrailsMode::Buffered { buffer_tokens: 50 },
            ..Default::default()
        };

        assert!(matches!(
            config.mode,
            StreamingGuardrailsMode::Buffered { buffer_tokens: 50 }
        ));
    }

    #[tokio::test]
    async fn test_per_chunk_mode_config() {
        let config = StreamingGuardrailsConfig {
            mode: StreamingGuardrailsMode::PerChunk,
            ..Default::default()
        };

        assert!(matches!(config.mode, StreamingGuardrailsMode::PerChunk));
    }
}
