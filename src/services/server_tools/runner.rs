//! Streaming orchestrator that runs registered `ServerExecutedTool`s in a
//! shared multi-turn loop.

use std::{collections::HashMap, sync::Arc};

use axum::body::Body;
use bytes::{Bytes, BytesMut};
use futures_util::{StreamExt, stream::FuturesUnordered};
use http::Response;
use tokio::sync::mpsc;
use tracing::{Instrument, debug, error, info, info_span, warn};

use super::{DetectedToolCall, ProviderCallback, ServerExecutedTool, ToolCallResult, ToolContext};
use crate::{
    api_types::responses::{
        CreateResponsesPayload, EasyInputMessage, EasyInputMessageContent, EasyInputMessageRole,
        OutputItemFunctionCall, OutputMessage, ResponsesInput, ResponsesInputItem,
        ResponsesReasoning,
    },
    observability::metrics::record_server_tool_iteration,
    streaming::SseBuffer,
};

/// Multi-tool orchestrator for streaming Responses API output.
///
/// Wraps the upstream response body, reads SSE events, dispatches detection
/// across all registered tools, executes detected calls, and continues the
/// conversation with the provider until either the model stops calling
/// tools or the global iteration budget is exhausted.
pub struct ToolLoopRunner {
    tools: Vec<Arc<dyn ServerExecutedTool>>,
    provider_callback: Option<ProviderCallback>,
    original_payload: CreateResponsesPayload,
    max_iterations: usize,
}

impl ToolLoopRunner {
    /// Create a new runner.
    ///
    /// `max_iterations` is the maximum number of provider continuation
    /// requests the runner will dispatch — i.e., the total number of
    /// times the loop body executes. Counted globally across all tools.
    pub fn new(original_payload: CreateResponsesPayload, max_iterations: usize) -> Self {
        Self {
            tools: Vec::new(),
            provider_callback: None,
            original_payload,
            max_iterations,
        }
    }

    /// Register a tool. Tools are dispatched in registration order; first
    /// `detect()` match wins for a given event.
    pub fn register(mut self, tool: Arc<dyn ServerExecutedTool>) -> Self {
        self.tools.push(tool);
        self
    }

    /// Set the provider callback used for continuation requests.
    ///
    /// Without a callback the runner only detects + emits in-progress
    /// events; it doesn't actually execute multi-turn. Most callers should
    /// always set this.
    pub fn with_provider_callback(mut self, callback: ProviderCallback) -> Self {
        self.provider_callback = Some(callback);
        self
    }

    /// Are any registered tools enabled for the original payload?
    pub fn has_enabled_tools(&self) -> bool {
        self.tools
            .iter()
            .any(|t| t.is_enabled_for(&self.original_payload))
    }

    /// Wrap a streaming HTTP response, intercepting and executing tool
    /// calls along the way.
    ///
    /// If no registered tool is enabled for the request, returns the
    /// response unchanged.
    pub fn wrap_streaming(self, response: Response<Body>) -> Response<Body> {
        // Filter to enabled tools up-front so detection loops are tight.
        let enabled_tools: Vec<Arc<dyn ServerExecutedTool>> = self
            .tools
            .into_iter()
            .filter(|t| t.is_enabled_for(&self.original_payload))
            .collect();

        if enabled_tools.is_empty() {
            return response;
        }

        let (parts, body) = response.into_parts();
        let max_iterations = self.max_iterations;
        let has_callback = self.provider_callback.is_some();
        let provider_callback = self.provider_callback;
        let original_payload = self.original_payload;

        let span = info_span!(
            "tool_loop_runner",
            tool_count = enabled_tools.len(),
            max_iterations = max_iterations,
            has_callback = has_callback,
        );

        let (tx, rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(32);

        crate::compat::spawn_detached(
            async move {
                let ctx = ToolContext {
                    original_payload: original_payload.clone(),
                };
                let tool_by_name: HashMap<&'static str, Arc<dyn ServerExecutedTool>> =
                    enabled_tools
                        .iter()
                        .map(|t| (t.name(), t.clone()))
                        .collect();
                let tool_names: Vec<&'static str> =
                    enabled_tools.iter().map(|t| t.name()).collect();

                let mut iteration: usize = 0;
                let mut current_body = body;
                // Continuation payload carried across iterations. Each
                // turn appends the assistant items the upstream emitted
                // plus the tool function-call outputs, so the model
                // sees its own prior tool_use/tool_result pairs on
                // subsequent turns. Without this, providers that
                // translate Responses items into native pairwise
                // formats (e.g. Anthropic via OpenRouter) drop the
                // orphan tool outputs on the floor and the model loops
                // forever as if it had never run anything.
                let mut continuation_payload = original_payload.clone();

                loop {
                    iteration += 1;
                    let at_iteration_limit = iteration > max_iterations;

                    let iter_span = info_span!(
                        "tool_loop_iteration",
                        iteration = iteration,
                        at_limit = at_iteration_limit,
                    );
                    let _iter_guard = iter_span.enter();

                    let mut body_stream = current_body.into_data_stream();
                    let mut accumulated = BytesMut::new();
                    let mut detected: Vec<DetectedToolCall> = Vec::new();
                    let mut sse_buffer = SseBuffer::new();
                    // Assistant items the upstream emitted this turn.
                    // Threaded into the continuation payload below so
                    // the function-call outputs from this iteration
                    // have matching function_call items to anchor to.
                    let mut captured_assistant_items: Vec<ResponsesInputItem> = Vec::new();

                    // Read the current response stream, forwarding events
                    // until we've finished consuming or detected calls.
                    while let Some(chunk_result) = body_stream.next().await {
                        match chunk_result {
                            Ok(chunk) => {
                                accumulated.extend_from_slice(&chunk);
                                sse_buffer.extend(&chunk);

                                for event in sse_buffer.extract_complete_events() {
                                    if !at_iteration_limit {
                                        for tool in &enabled_tools {
                                            let calls = tool.detect(&event, &ctx);
                                            for call in calls {
                                                info!(
                                                    stage = "tool_call_detected",
                                                    tool = call.tool_name,
                                                    call_id = %call.call_id,
                                                    iteration = iteration,
                                                    "Detected tool call"
                                                );
                                                detected.push(call);
                                            }
                                        }

                                        if let Some(item) = parse_assistant_item(&event) {
                                            captured_assistant_items.push(item);
                                        }

                                        // Once a tool call has been detected for
                                        // this iteration, hold back only the
                                        // iteration-terminator events
                                        // (`response.created`,
                                        // `response.in_progress`,
                                        // `response.completed`, ...) — they would
                                        // confuse a client into thinking the
                                        // upstream is finished when in fact we're
                                        // about to continue the loop. Item-level
                                        // events (`output_item.done`,
                                        // `content_part.done`, etc.) are
                                        // informational and must be forwarded so
                                        // both streaming clients and the
                                        // non-streaming bridge can reconstruct
                                        // the full transcript.
                                        if !detected.is_empty()
                                            && has_callback
                                            && is_iteration_terminator(&event)
                                        {
                                            continue;
                                        }
                                    }

                                    let to_send = apply_transforms(&enabled_tools, event);
                                    if tx.send(Ok(to_send)).await.is_err() {
                                        return; // client disconnected
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

                    // Flush any trailing partial event.
                    if !sse_buffer.is_empty() {
                        let remaining = sse_buffer.take_remaining();
                        if !remaining.is_empty() && (detected.is_empty() || !has_callback) {
                            let to_send = apply_transforms(&enabled_tools, remaining);
                            if tx.send(Ok(to_send)).await.is_err() {
                                return;
                            }
                        }
                    }

                    if at_iteration_limit {
                        warn!(
                            stage = "iteration_limit_reached",
                            iteration = iteration,
                            max_iterations = max_iterations,
                            "Maximum server-tool iterations exceeded; forwarding final response"
                        );
                        record_server_tool_iteration(
                            iteration as u32,
                            true,
                            "limit_reached",
                            &tool_names,
                        );
                        break;
                    }

                    if detected.is_empty() {
                        debug!(
                            stage = "stream_completed",
                            iteration = iteration,
                            "No tool calls detected; stream complete"
                        );
                        record_server_tool_iteration(
                            iteration as u32,
                            true,
                            "completed",
                            &tool_names,
                        );
                        break;
                    }

                    // Execute all detected calls in parallel, interleaving
                    // their progress events into the client stream.
                    let mut exec_handles = FuturesUnordered::new();
                    for call in detected.drain(..) {
                        let Some(tool) = tool_by_name.get(call.tool_name).cloned() else {
                            error!(
                                stage = "unknown_tool",
                                tool = call.tool_name,
                                "Detected call references unregistered tool; skipping"
                            );
                            continue;
                        };
                        let ctx = ctx.clone();
                        let call_id = call.call_id.clone();
                        let tool_name = call.tool_name;
                        exec_handles.push(async move {
                            let handle = tool.execute(call, &ctx).await;
                            (tool_name, call_id, handle)
                        });
                    }

                    // results_by_tool[tool_name] = Vec<ToolCallResult>
                    let mut results_by_tool: HashMap<&'static str, Vec<ToolCallResult>> =
                        HashMap::new();
                    let mut had_failure = false;

                    while let Some((tool_name, call_id, handle)) = exec_handles.next().await {
                        let handle = match handle {
                            Ok(h) => h,
                            Err(e) => {
                                error!(
                                    stage = "execute_failed",
                                    tool = tool_name,
                                    call_id = %call_id,
                                    error = %e,
                                    "Tool execute() returned error"
                                );
                                had_failure = true;
                                continue;
                            }
                        };

                        let mut events = handle.events;
                        while let Some(event) = events.next().await {
                            let to_send = apply_transforms(&enabled_tools, event);
                            if tx.send(Ok(to_send)).await.is_err() {
                                return;
                            }
                        }

                        match handle.result.await {
                            Ok(result) => {
                                results_by_tool.entry(tool_name).or_default().push(result);
                            }
                            Err(e) => {
                                error!(
                                    stage = "result_failed",
                                    tool = tool_name,
                                    call_id = %call_id,
                                    error = %e,
                                    "Tool result returned error"
                                );
                                had_failure = true;
                            }
                        }
                    }

                    if had_failure {
                        // Forward accumulated raw bytes for the model's
                        // benefit and stop the loop — matches existing
                        // file_search behaviour.
                        if tx.send(Ok(accumulated.freeze())).await.is_err() {
                            return;
                        }
                        record_server_tool_iteration(iteration as u32, true, "error", &tool_names);
                        break;
                    }

                    // Build the continuation payload by letting each tool
                    // fold its results in.
                    let Some(ref callback) = provider_callback else {
                        // No callback: forward what we have and stop.
                        if tx.send(Ok(accumulated.freeze())).await.is_err() {
                            return;
                        }
                        record_server_tool_iteration(
                            iteration as u32,
                            true,
                            "no_callback",
                            &tool_names,
                        );
                        break;
                    };

                    let is_final_iteration = iteration == max_iterations;
                    // The continuation payload accumulates across
                    // iterations: each turn it grows by the assistant
                    // items the upstream emitted plus the function-call
                    // outputs this turn's tools produced. Pairing the
                    // assistant's function_call items with their
                    // corresponding function_call_output items is what
                    // lets non-OpenAI providers (e.g. Anthropic via
                    // OpenRouter) reconstruct valid tool_use/tool_result
                    // pairs on the wire.
                    normalize_input_to_items(&mut continuation_payload);
                    if let Some(ResponsesInput::Items(ref mut items)) = continuation_payload.input {
                        items.append(&mut captured_assistant_items);
                    }
                    for tool in &enabled_tools {
                        if let Some(results) = results_by_tool.get(tool.name()) {
                            tool.apply_to_continuation(
                                &mut continuation_payload,
                                results,
                                is_final_iteration,
                            );
                        }
                    }
                    let mut continuation_payload_for_call = continuation_payload.clone();
                    continuation_payload_for_call.stream = true;

                    info!(
                        stage = "continuation_sent",
                        iteration = iteration,
                        is_final_iteration = is_final_iteration,
                        tools_with_results = results_by_tool.len(),
                        "Sending continuation request to provider"
                    );

                    record_server_tool_iteration(
                        iteration as u32,
                        false,
                        "continuation",
                        &tool_names,
                    );

                    match callback(continuation_payload_for_call).await {
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
                            record_server_tool_iteration(
                                iteration as u32,
                                true,
                                "continuation_error",
                                &tool_names,
                            );
                            break;
                        }
                    }
                }

                debug!(
                    stage = "processing_completed",
                    "Tool loop processing complete"
                );
            }
            .instrument(span),
        );

        let stream = futures_util::stream::unfold(rx, |mut rx| async move {
            rx.recv().await.map(|item| (item, rx))
        });
        let body = Body::from_stream(stream);
        Response::from_parts(parts, body)
    }
}

fn apply_transforms(tools: &[Arc<dyn ServerExecutedTool>], event: Bytes) -> Bytes {
    let mut out = event;
    for t in tools {
        out = t.transform_event(out);
    }
    out
}

/// True for SSE events that mark a turn boundary: the start
/// (`response.created` / `response.in_progress`) or end
/// (`response.completed` / `response.failed` / `response.incomplete`)
/// of one upstream stream. The runner holds these back across
/// intermediate iterations so the client sees one coherent timeline,
/// not N concatenated mini-streams.
fn is_iteration_terminator(event: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(event) else {
        return false;
    };
    let Some(data) = text
        .lines()
        .find_map(|line| line.strip_prefix("data:").map(str::trim))
    else {
        return false;
    };
    if data == "[DONE]" {
        return false;
    }
    let Ok(value): Result<serde_json::Value, _> = serde_json::from_str(data) else {
        return false;
    };
    matches!(
        value.get("type").and_then(|t| t.as_str()),
        Some(
            "response.created"
                | "response.in_progress"
                | "response.completed"
                | "response.failed"
                | "response.incomplete"
        )
    )
}

/// Inspect one SSE event and extract the assistant item it carries,
/// if any. Returns `Some(item)` for `response.output_item.done` events
/// whose `item` is a model-emitted `message`, `function_call`, or
/// `reasoning`. Gateway-synthesized items (`shell_call_output`,
/// `web_search_call`, `file_search_call`) are skipped — tools fold
/// their own continuation items in via `apply_to_continuation`.
fn parse_assistant_item(event: &[u8]) -> Option<ResponsesInputItem> {
    let text = std::str::from_utf8(event).ok()?;
    let data = text
        .lines()
        .find_map(|line| line.strip_prefix("data:").map(str::trim))?;
    if data == "[DONE]" {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(data).ok()?;
    if value.get("type").and_then(|t| t.as_str()) != Some("response.output_item.done") {
        return None;
    }
    let item = value.get("item")?;
    match item.get("type").and_then(|t| t.as_str())? {
        "message" => serde_json::from_value::<OutputMessage>(item.clone())
            .ok()
            .map(ResponsesInputItem::OutputMessage),
        "function_call" => serde_json::from_value::<OutputItemFunctionCall>(item.clone())
            .ok()
            .map(ResponsesInputItem::OutputFunctionCall),
        "reasoning" => serde_json::from_value::<ResponsesReasoning>(item.clone())
            .ok()
            .map(ResponsesInputItem::Reasoning),
        _ => None,
    }
}

/// Ensure `payload.input` is `Items` so callers can append to it.
/// A `Text` input is rewrapped as a single user `EasyMessage`; `None`
/// becomes an empty `Items` vec.
fn normalize_input_to_items(payload: &mut CreateResponsesPayload) {
    match payload.input.take() {
        Some(ResponsesInput::Items(items)) => {
            payload.input = Some(ResponsesInput::Items(items));
        }
        Some(ResponsesInput::Text(text)) => {
            payload.input = Some(ResponsesInput::Items(vec![
                ResponsesInputItem::EasyMessage(EasyInputMessage {
                    type_: None,
                    role: EasyInputMessageRole::User,
                    content: EasyInputMessageContent::Text(text),
                }),
            ]));
        }
        None => {
            payload.input = Some(ResponsesInput::Items(Vec::new()));
        }
    }
}
