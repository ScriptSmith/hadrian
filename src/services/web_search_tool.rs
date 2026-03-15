//! Web search tool interception service for the Responses API.
//!
//! Intercepts `web_search` tool calls from the LLM and executes them against
//! the configured search provider (Tavily/Exa), feeding results back into the
//! conversation transparently — following the same pattern as `file_search_tool`.

use std::{future::Future, pin::Pin, sync::Arc, time::Instant};

use axum::body::Body;
use bytes::{Bytes, BytesMut};
use futures_util::StreamExt;
use http::Response;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::{Instrument, debug, error, info, info_span, warn};

use crate::{
    api_types::responses::{
        CreateResponsesPayload, FunctionCallOutput, FunctionCallOutputType, ResponsesInput,
        ResponsesInputItem, ResponsesToolDefinition, WebSearchCallOutput, WebSearchCallOutputType,
        WebSearchStatus,
    },
    config::WebSearchConfig,
    observability::metrics::{record_web_search, record_web_search_iteration},
    providers::ProviderError,
    routes::api::tools::{WebSearchResult, execute_web_search},
    services::file_search_tool::SseBuffer,
};

// ─────────────────────────────────────────────────────────────────────────────
// Tool Arguments (function schema for the model)
// ─────────────────────────────────────────────────────────────────────────────

/// Arguments the model produces when calling the web_search function tool.
#[derive(Debug, Clone, Deserialize)]
pub struct WebSearchToolArguments {
    pub query: String,
}

impl WebSearchToolArguments {
    pub const FUNCTION_NAME: &'static str = "web_search";

    pub fn parse(arguments_json: &str) -> Option<Self> {
        serde_json::from_str(arguments_json).ok()
    }

    pub fn function_description() -> &'static str {
        "Search the web for current information. Use this when you need up-to-date facts, recent events, or information that may not be in your training data."
    }

    pub fn function_parameters_schema() -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query to find relevant information on the web"
                }
            },
            "required": ["query"],
            "additionalProperties": false
        })
    }

    pub fn function_tool_definition() -> Value {
        serde_json::json!({
            "type": "function",
            "name": Self::FUNCTION_NAME,
            "description": Self::function_description(),
            "parameters": Self::function_parameters_schema(),
            "strict": false,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Payload Preprocessing
// ─────────────────────────────────────────────────────────────────────────────

/// Convert all `WebSearch*` tool definitions to function tools that models can call.
///
/// After preprocessing, the model sees a standard function tool named `"web_search"`.
/// The streaming middleware intercepts calls to this function and executes them.
pub fn preprocess_web_search_tools(payload: &mut CreateResponsesPayload) {
    let Some(tools) = payload.tools.as_mut() else {
        return;
    };

    for tool in tools.iter_mut() {
        if tool.is_web_search() {
            let function_def = WebSearchToolArguments::function_tool_definition();
            *tool = ResponsesToolDefinition::Function(function_def);
            debug!(
                stage = "tool_preprocessed",
                "Preprocessed web_search tool to function definition"
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Context
// ─────────────────────────────────────────────────────────────────────────────

/// Type alias for the provider callback (reuses the same signature as file_search).
#[cfg(not(target_arch = "wasm32"))]
type ProviderCallback = Arc<
    dyn Fn(
            CreateResponsesPayload,
        ) -> Pin<Box<dyn Future<Output = Result<Response<Body>, ProviderError>> + Send>>
        + Send
        + Sync,
>;

#[cfg(target_arch = "wasm32")]
type ProviderCallback = Arc<
    dyn Fn(
        CreateResponsesPayload,
    ) -> Pin<Box<dyn Future<Output = Result<Response<Body>, ProviderError>>>>,
>;

/// Context for web search middleware operations.
#[derive(Clone)]
pub struct WebSearchContext {
    pub http_client: reqwest::Client,
    pub config: WebSearchConfig,
    pub max_iterations: usize,
    original_payload: CreateResponsesPayload,
    provider_callback: Option<ProviderCallback>,
}

impl WebSearchContext {
    pub fn new(
        http_client: reqwest::Client,
        config: WebSearchConfig,
        max_iterations: usize,
        original_payload: CreateResponsesPayload,
    ) -> Self {
        Self {
            http_client,
            config,
            max_iterations,
            original_payload,
            provider_callback: None,
        }
    }

    pub fn with_provider_callback(mut self, callback: crate::services::ProviderCallback) -> Self {
        self.provider_callback = Some(callback);
        self
    }

    pub fn is_enabled(&self) -> bool {
        true // If we have a context, we're enabled
    }

    /// Execute a web search using the configured provider.
    async fn execute_search(&self, query: &str) -> Result<Vec<WebSearchResult>, String> {
        let max_results = self.config.max_results;
        execute_web_search(&self.http_client, &self.config, query, max_results)
            .await
            .map_err(|e| e.to_string())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Detected tool call
// ─────────────────────────────────────────────────────────────────────────────

/// A detected web_search tool call from the model.
#[derive(Debug, Clone)]
struct WebSearchToolCall {
    id: String,
    query: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Detection
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a web_search tool call from a JSON value.
fn parse_web_search_tool_call(value: &Value) -> Option<WebSearchToolCall> {
    let obj = value.as_object()?;

    let type_val = obj.get("type")?.as_str()?;
    if type_val != "function_call" {
        return None;
    }

    let name = obj.get("name")?.as_str()?;
    if name != WebSearchToolArguments::FUNCTION_NAME {
        return None;
    }

    let id = obj
        .get("call_id")
        .or_else(|| obj.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let arguments_str = obj.get("arguments")?.as_str()?;
    let args = WebSearchToolArguments::parse(arguments_str)?;

    Some(WebSearchToolCall {
        id,
        query: args.query,
    })
}

/// Detect web_search tool calls in an SSE chunk.
fn detect_web_search_in_chunk(chunk: &[u8]) -> Vec<WebSearchToolCall> {
    let Some(chunk_str) = std::str::from_utf8(chunk).ok() else {
        return Vec::new();
    };

    let mut found_calls = Vec::new();

    for line in chunk_str.lines() {
        if let Some(data) = line.strip_prefix("data:") {
            let data = data.trim();
            if data == "[DONE]" {
                continue;
            }

            if let Ok(json) = serde_json::from_str::<Value>(data) {
                // Responses API: output array
                if let Some(output) = json.get("output").and_then(|o| o.as_array()) {
                    for item in output {
                        if let Some(tc) = parse_web_search_tool_call(item) {
                            found_calls.push(tc);
                        }
                    }
                }

                // Direct function_call
                if let Some(tc) = parse_web_search_tool_call(&json) {
                    found_calls.push(tc);
                }

                // response.output_item.done — canonical event for complete function calls.
                // Note: we intentionally skip `response.function_call_arguments.done`
                // because the Responses API emits both events for the same tool call,
                // which would cause duplicate search executions. The output_item.done
                // event contains the complete function call with the correct `call_id`.
                if json.get("type").and_then(|t| t.as_str()) == Some("response.output_item.done")
                    && let Some(item) = json.get("item")
                    && let Some(tc) = parse_web_search_tool_call(item)
                {
                    found_calls.push(tc);
                }

                // Chat completion delta format
                if let Some(choices) = json.get("choices").and_then(|c| c.as_array()) {
                    for choice in choices {
                        if let Some(delta) = choice.get("delta")
                            && let Some(tool_calls) =
                                delta.get("tool_calls").and_then(|t| t.as_array())
                        {
                            for tc in tool_calls {
                                if let Some(tc) = parse_web_search_tool_call(tc) {
                                    found_calls.push(tc);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    found_calls
}

// ─────────────────────────────────────────────────────────────────────────────
// Result formatting
// ─────────────────────────────────────────────────────────────────────────────

/// Format web search results as text for the model to consume.
fn format_web_search_results(query: &str, results: &[WebSearchResult]) -> String {
    let mut output = format!(
        "Web search results for \"{}\" ({} results):\n\n",
        query,
        results.len()
    );

    for (i, result) in results.iter().enumerate() {
        output.push_str(&format!("[{}] {} - {}\n", i + 1, result.title, result.url));
        output.push_str(&result.content);
        output.push_str("\n\n");
    }

    output
        .push_str("Cite sources using their URLs when referencing information from these results.");
    output
}

// ─────────────────────────────────────────────────────────────────────────────
// SSE event formatters
// ─────────────────────────────────────────────────────────────────────────────

fn format_web_search_in_progress_event(item_id: &str, output_index: usize) -> Bytes {
    let event_data = serde_json::json!({
        "type": "response.web_search_call.in_progress",
        "output_index": output_index,
        "item_id": item_id,
    });
    let json_str = serde_json::to_string(&event_data).unwrap_or_default();
    Bytes::from(format!("data: {}\n\n", json_str))
}

fn format_web_search_searching_event(item_id: &str, output_index: usize) -> Bytes {
    let event_data = serde_json::json!({
        "type": "response.web_search_call.searching",
        "output_index": output_index,
        "item_id": item_id,
    });
    let json_str = serde_json::to_string(&event_data).unwrap_or_default();
    Bytes::from(format!("data: {}\n\n", json_str))
}

fn format_web_search_completed_event(item_id: &str, output_index: usize) -> Bytes {
    let event_data = serde_json::json!({
        "type": "response.web_search_call.completed",
        "output_index": output_index,
        "item_id": item_id,
    });
    let json_str = serde_json::to_string(&event_data).unwrap_or_default();
    Bytes::from(format!("data: {}\n\n", json_str))
}

fn format_web_search_call_output_event(item_id: &str) -> Option<Bytes> {
    let output = WebSearchCallOutput {
        type_: WebSearchCallOutputType::WebSearchCall,
        id: item_id.to_string(),
        status: WebSearchStatus::Completed,
    };
    let event_data = serde_json::json!({
        "type": "response.output_item.done",
        "output_index": 0,
        "item": output,
    });
    let json_str = serde_json::to_string(&event_data).ok()?;
    Some(Bytes::from(format!("data: {}\n\n", json_str)))
}

// ─────────────────────────────────────────────────────────────────────────────
// Continuation payload
// ─────────────────────────────────────────────────────────────────────────────

fn build_web_search_continuation_payload(
    original: &CreateResponsesPayload,
    tool_results: &[(&WebSearchToolCall, String)],
    is_final_iteration: bool,
) -> CreateResponsesPayload {
    let mut payload = original.clone();

    let function_outputs: Vec<ResponsesInputItem> = tool_results
        .iter()
        .map(|(tool_call, content)| {
            ResponsesInputItem::FunctionCallOutput(FunctionCallOutput {
                type_: FunctionCallOutputType::FunctionCallOutput,
                id: Some(tool_call.id.clone()),
                call_id: tool_call.id.clone(),
                output: content.clone(),
                status: None,
            })
        })
        .collect();

    match payload.input {
        Some(ResponsesInput::Items(ref mut items)) => {
            items.extend(function_outputs);
        }
        Some(ResponsesInput::Text(text)) => {
            let mut items = vec![ResponsesInputItem::EasyMessage(
                crate::api_types::responses::EasyInputMessage {
                    type_: None,
                    role: crate::api_types::responses::EasyInputMessageRole::User,
                    content: crate::api_types::responses::EasyInputMessageContent::Text(text),
                },
            )];
            items.extend(function_outputs);
            payload.input = Some(ResponsesInput::Items(items));
        }
        None => {
            payload.input = Some(ResponsesInput::Items(function_outputs));
        }
    }

    // On final iteration, remove web_search tools to force text completion
    if is_final_iteration && let Some(ref mut tools) = payload.tools {
        let original_count = tools.len();
        tools.retain(|t| !t.is_web_search());
        // Also remove function tools named "web_search" (from preprocessing)
        tools.retain(|t| {
            if let ResponsesToolDefinition::Function(v) = t {
                v.get("name").and_then(|n| n.as_str())
                    != Some(WebSearchToolArguments::FUNCTION_NAME)
            } else {
                true
            }
        });
        let removed_count = original_count - tools.len();
        if removed_count > 0 {
            info!(
                stage = "tools_removed",
                removed_count = removed_count,
                "Removed web_search tools on final iteration to force completion"
            );
        }
        if tools.is_empty() {
            payload.tools = None;
        }
    }

    payload.stream = true;
    payload
}

// ─────────────────────────────────────────────────────────────────────────────
// Streaming wrapper
// ─────────────────────────────────────────────────────────────────────────────

/// Wrap a streaming response with web_search tool interception and multi-turn execution.
///
/// Monitors the stream for web_search function calls. When detected:
/// 1. Executes the search against the configured provider (Tavily/Exa)
/// 2. Builds a continuation payload with the search results
/// 3. Sends the continuation to the provider via the callback
/// 4. Streams the continuation response to the client
pub fn wrap_streaming_with_web_search(
    response: Response<Body>,
    context: WebSearchContext,
) -> Response<Body> {
    if !context.is_enabled() {
        return response;
    }

    let (parts, body) = response.into_parts();
    let max_iterations = context.max_iterations;
    let has_callback = context.provider_callback.is_some();

    let web_search_span = info_span!(
        "web_search_stream",
        max_iterations = max_iterations,
        has_callback = has_callback,
    );

    let (tx, rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(32);

    crate::compat::spawn_detached(
        async move {
            let mut iteration = 0;
            let mut current_body = body;

            loop {
                iteration += 1;
                let at_iteration_limit = iteration > max_iterations;

                debug!(
                    iteration = iteration,
                    at_limit = at_iteration_limit,
                    "Starting web_search iteration"
                );

                let mut body_stream = current_body.into_data_stream();
                let mut accumulated = BytesMut::new();
                let mut detected_tool_calls: Vec<WebSearchToolCall> = Vec::new();
                let mut sse_buffer = SseBuffer::new();

                while let Some(chunk_result) = body_stream.next().await {
                    match chunk_result {
                        Ok(chunk) => {
                            accumulated.extend_from_slice(&chunk);
                            sse_buffer.extend(&chunk);

                            let complete_events = sse_buffer.extract_complete_events();

                            for event in complete_events {
                                if !at_iteration_limit {
                                    let tool_calls = detect_web_search_in_chunk(&event);
                                    for tool_call in tool_calls {
                                        info!(
                                            stage = "tool_call_detected",
                                            tool_call_id = %tool_call.id,
                                            query = %tool_call.query,
                                            iteration = iteration,
                                            "Detected web_search tool call in stream"
                                        );
                                        detected_tool_calls.push(tool_call);
                                    }

                                    if !detected_tool_calls.is_empty() && has_callback {
                                        continue;
                                    }
                                }

                                if tx.send(Ok(event)).await.is_err() {
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            warn!(
                                stage = "stream_error",
                                error = %e,
                                iteration = iteration,
                                "Error reading stream chunk"
                            );
                            let _ = tx.send(Err(std::io::Error::other(e))).await;
                            return;
                        }
                    }
                }

                // Forward remaining incomplete data
                if !sse_buffer.is_empty() {
                    let remaining = sse_buffer.take_remaining();
                    if !remaining.is_empty()
                        && (detected_tool_calls.is_empty() || !has_callback)
                        && tx.send(Ok(remaining)).await.is_err()
                    {
                        return;
                    }
                }

                if at_iteration_limit {
                    warn!(
                        stage = "iteration_limit_reached",
                        iteration = iteration,
                        max_iterations = max_iterations,
                        "Maximum web_search iterations exceeded, forwarding final response"
                    );
                    record_web_search_iteration(iteration as u32, true, "limit_reached");
                    break;
                }

                if !detected_tool_calls.is_empty() {
                    let tool_call_count = detected_tool_calls.len();

                    // Emit in_progress events
                    for (idx, tool_call) in detected_tool_calls.iter().enumerate() {
                        let event = format_web_search_in_progress_event(&tool_call.id, idx);
                        if tx.send(Ok(event)).await.is_err() {
                            return;
                        }
                    }

                    info!(
                        stage = "batch_search_starting",
                        tool_call_count = tool_call_count,
                        iteration = iteration,
                        "Executing {} web_search tool calls in parallel",
                        tool_call_count
                    );

                    // Emit searching events
                    for (idx, tool_call) in detected_tool_calls.iter().enumerate() {
                        let event = format_web_search_searching_event(&tool_call.id, idx);
                        if tx.send(Ok(event)).await.is_err() {
                            return;
                        }
                    }

                    // Execute all searches in parallel
                    let search_futures: Vec<_> = detected_tool_calls
                        .iter()
                        .map(|tool_call| {
                            let ctx = context.clone();
                            let query = tool_call.query.clone();
                            async move {
                                let start = Instant::now();
                                match ctx.execute_search(&query).await {
                                    Ok(results) => {
                                        let count = results.len() as u32;
                                        record_web_search(
                                            "success",
                                            start.elapsed().as_secs_f64(),
                                            count,
                                        );
                                        Ok(results)
                                    }
                                    Err(e) => {
                                        record_web_search(
                                            "error",
                                            start.elapsed().as_secs_f64(),
                                            0,
                                        );
                                        Err(e)
                                    }
                                }
                            }
                        })
                        .collect();

                    let search_results = futures_util::future::join_all(search_futures).await;

                    // Process results — on failure, synthesize an error message
                    // for the model instead of forwarding raw internal SSE.
                    let mut tool_results: Vec<(&WebSearchToolCall, String)> = Vec::new();

                    for (tool_call, result) in detected_tool_calls.iter().zip(search_results) {
                        match result {
                            Ok(results) => {
                                let content = format_web_search_results(&tool_call.query, &results);

                                // Emit output_item.done with WebSearchCallOutput
                                if let Some(sse_event) =
                                    format_web_search_call_output_event(&tool_call.id)
                                    && tx.send(Ok(sse_event)).await.is_err()
                                {
                                    return;
                                }

                                // Emit completed event
                                let output_index = detected_tool_calls
                                    .iter()
                                    .position(|tc| tc.id == tool_call.id)
                                    .unwrap_or(0);
                                let completed =
                                    format_web_search_completed_event(&tool_call.id, output_index);
                                if tx.send(Ok(completed)).await.is_err() {
                                    return;
                                }

                                tool_results.push((tool_call, content));
                            }
                            Err(e) => {
                                error!(
                                    stage = "search_failed",
                                    tool_call_id = %tool_call.id,
                                    error = %e,
                                    "Web search execution failed"
                                );
                                // Provide the model with an error message instead of
                                // dropping the result or leaking raw SSE.
                                tool_results.push((
                                    tool_call,
                                    format!(
                                        "Web search failed for query \"{}\": {}",
                                        tool_call.query, e
                                    ),
                                ));
                            }
                        }
                    }

                    // Continue with provider callback
                    if let Some(ref callback) = context.provider_callback {
                        let is_final_iteration = iteration == max_iterations;
                        let continuation_payload = build_web_search_continuation_payload(
                            &context.original_payload,
                            &tool_results,
                            is_final_iteration,
                        );

                        info!(
                            stage = "continuation_sent",
                            tool_call_count = tool_results.len(),
                            iteration = iteration,
                            is_final_iteration = is_final_iteration,
                            "Sending continuation request to provider with {} web search results",
                            tool_results.len()
                        );

                        match callback(continuation_payload).await {
                            Ok(continuation_response) => {
                                let (_, new_body) = continuation_response.into_parts();
                                current_body = new_body;
                                continue;
                            }
                            Err(e) => {
                                error!(
                                    stage = "continuation_failed",
                                    iteration = iteration,
                                    error = %e,
                                    "Provider continuation request failed"
                                );
                                if tx.send(Ok(accumulated.freeze())).await.is_err() {
                                    return;
                                }
                                record_web_search_iteration(iteration as u32, true, "error");
                                break;
                            }
                        }
                    } else {
                        debug!(
                            stage = "no_callback",
                            iteration = iteration,
                            "No provider callback configured, forwarding original response"
                        );
                        if tx.send(Ok(accumulated.freeze())).await.is_err() {
                            return;
                        }
                        record_web_search_iteration(iteration as u32, true, "no_callback");
                        break;
                    }
                } else {
                    debug!(
                        stage = "stream_completed",
                        iteration = iteration,
                        "No web_search tool calls detected, stream complete"
                    );
                    record_web_search_iteration(iteration as u32, true, "completed");
                    break;
                }
            }

            debug!(
                stage = "processing_completed",
                "Web search stream processing completed"
            );
        }
        .instrument(web_search_span),
    );

    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    });
    let body = Body::from_stream(stream);

    Response::from_parts(parts, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_web_search_tool_call() {
        let value = serde_json::json!({
            "type": "function_call",
            "name": "web_search",
            "call_id": "call_123",
            "arguments": "{\"query\": \"rust async programming\"}"
        });
        let tc = parse_web_search_tool_call(&value).unwrap();
        assert_eq!(tc.id, "call_123");
        assert_eq!(tc.query, "rust async programming");
    }

    #[test]
    fn test_parse_web_search_tool_call_not_web_search() {
        let value = serde_json::json!({
            "type": "function_call",
            "name": "file_search",
            "call_id": "call_123",
            "arguments": "{\"query\": \"test\"}"
        });
        assert!(parse_web_search_tool_call(&value).is_none());
    }

    #[test]
    fn test_parse_web_search_tool_call_wrong_type() {
        let value = serde_json::json!({
            "type": "message",
            "name": "web_search",
        });
        assert!(parse_web_search_tool_call(&value).is_none());
    }

    #[test]
    fn test_detect_web_search_in_chunk_output_item_done() {
        let chunk = br#"data: {"type": "response.output_item.done", "item": {"type": "function_call", "name": "web_search", "call_id": "call_456", "arguments": "{\"query\": \"latest news\"}"}}

"#;
        let calls = detect_web_search_in_chunk(chunk);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].query, "latest news");
    }

    #[test]
    fn test_detect_web_search_ignores_function_call_arguments_done() {
        // The Responses API emits both `response.function_call_arguments.done` and
        // `response.output_item.done` for the same tool call. We only detect from
        // `response.output_item.done` to avoid duplicates.
        let chunk = br#"data: {"type": "response.function_call_arguments.done", "name": "web_search", "item_id": "item_789", "arguments": "{\"query\": \"weather today\"}"}

"#;
        let calls = detect_web_search_in_chunk(chunk);
        assert!(calls.is_empty());
    }

    #[test]
    fn test_detect_web_search_no_duplicate_across_event_types() {
        // Simulate both events arriving in the same chunk — only one detection expected.
        let chunk = b"data: {\"type\": \"response.function_call_arguments.done\", \"name\": \"web_search\", \"item_id\": \"item_789\", \"arguments\": \"{\\\"query\\\": \\\"weather today\\\"}\"}\n\ndata: {\"type\": \"response.output_item.done\", \"item\": {\"type\": \"function_call\", \"name\": \"web_search\", \"call_id\": \"call_789\", \"arguments\": \"{\\\"query\\\": \\\"weather today\\\"}\"}}\n\n";
        let calls = detect_web_search_in_chunk(chunk);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_789");
        assert_eq!(calls[0].query, "weather today");
    }

    #[test]
    fn test_detect_web_search_in_chunk_no_match() {
        let chunk = br#"data: {"type": "response.output_item.done", "item": {"type": "function_call", "name": "file_search", "call_id": "call_123", "arguments": "{\"query\": \"test\"}"}}

"#;
        let calls = detect_web_search_in_chunk(chunk);
        assert!(calls.is_empty());
    }

    #[test]
    fn test_format_web_search_results() {
        let results = vec![
            WebSearchResult {
                title: "Example".to_string(),
                url: "https://example.com".to_string(),
                content: "Example content".to_string(),
                score: Some(0.9),
            },
            WebSearchResult {
                title: "Other".to_string(),
                url: "https://other.com".to_string(),
                content: "Other content".to_string(),
                score: None,
            },
        ];
        let output = format_web_search_results("test query", &results);
        assert!(output.contains("test query"));
        assert!(output.contains("[1] Example"));
        assert!(output.contains("[2] Other"));
        assert!(output.contains("https://example.com"));
    }

    #[test]
    fn test_preprocess_web_search_tools() {
        let json = serde_json::json!({
            "tools": [{"type": "web_search"}],
            "stream": false,
        });
        let mut payload: CreateResponsesPayload = serde_json::from_value(json).unwrap();
        preprocess_web_search_tools(&mut payload);
        let tools = payload.tools.unwrap();
        assert_eq!(tools.len(), 1);
        assert!(matches!(tools[0], ResponsesToolDefinition::Function(_)));
        if let ResponsesToolDefinition::Function(ref v) = tools[0] {
            assert_eq!(v.get("name").unwrap().as_str().unwrap(), "web_search");
        }
    }
}
