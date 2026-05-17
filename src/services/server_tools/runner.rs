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
    api_types::responses::CreateResponsesPayload,
    observability::metrics::record_server_tool_iteration, streaming::SseBuffer,
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

                                        // Hold back forwarding while we
                                        // accumulate a batch — same as
                                        // current per-tool wrappers do.
                                        if !detected.is_empty() && has_callback {
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
                    // Build the continuation from the *original* payload
                    // each iteration: tool `apply_to_continuation`
                    // implementations append this iteration's
                    // function-call outputs onto `payload.input`, so
                    // reusing the previous iteration's mutated payload
                    // would double-record prior outputs. Cloning is
                    // therefore intentional and necessary — but only on
                    // iterations where a continuation actually fires
                    // (the early-`break` paths above skip this work).
                    //
                    // For requests with large `input` (uploaded files,
                    // long instructions) the dominant cost here is the
                    // `Vec<ResponsesInputItem>` clone inside `input`.
                    // If that becomes hot in profiles, the next step is
                    // to swap `CreateResponsesPayload.input` for an
                    // `Arc<Vec<…>>` + copy-on-write on first append.
                    let mut continuation_payload = original_payload.clone();
                    for tool in &enabled_tools {
                        if let Some(results) = results_by_tool.get(tool.name()) {
                            tool.apply_to_continuation(
                                &mut continuation_payload,
                                results,
                                is_final_iteration,
                            );
                        }
                    }
                    continuation_payload.stream = true;

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
