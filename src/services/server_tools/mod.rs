//! Server-executed tool framework for the Responses API.
//!
//! Tools like `file_search`, `web_search`, and (in later phases) `shell` are
//! intercepted by the gateway: when the upstream provider emits a function
//! call, the gateway pauses, executes the work locally, then continues the
//! conversation with the result folded back in. This module provides the
//! shared trait + orchestrator that all such tools build on.
//!
//! # Architecture
//!
//! ```text
//!   provider stream  ─►  ToolLoopRunner  ─►  client
//!                            │
//!                            ├─ for each event: dispatch to ServerExecutedTool::detect()
//!                            ├─ on detection: ServerExecutedTool::execute()
//!                            │   ├─ stream tool progress events to client
//!                            │   └─ produce continuation items
//!                            ├─ ServerExecutedTool::apply_to_continuation()
//!                            └─ provider_callback() → new stream → loop
//! ```
//!
//! The runner enforces a global iteration limit across all registered tools.

#![cfg(not(target_arch = "wasm32"))]

use std::{future::Future, pin::Pin, sync::Arc};

use axum::body::Body;
use bytes::Bytes;
use futures_util::stream::Stream;
use http::Response;
use serde_json::Value;
use thiserror::Error;

use crate::{
    api_types::responses::{CreateResponsesPayload, ResponsesInputItem},
    providers::ProviderError,
};

mod runner;

pub use runner::ToolLoopRunner;

/// Suppresses the rewritten `function_call` plumbing for a server tool
/// that synthesizes its own spec-shaped output items.
///
/// Every server-executed tool is driven by function-tool rewriting: the
/// model emits a `function_call` (`web_search` / `file_search` / `shell`
/// / `mcp_<label>__<tool>`), which the runner intercepts and replaces
/// with the hosted-tool item (`web_search_call`, `mcp_call`, …). OpenAI's
/// Responses output only ever carries those hosted-tool items, never the
/// `function_call` that drove them — so the underlying
/// `output_item.added` / `.done` and `function_call_arguments.delta` /
/// `.done` events must not reach the client. A tool calls
/// [`Self::suppress`] from its `transform_event`; suppressed events come
/// back as empty `Bytes`, which the runner drops.
#[derive(Default)]
pub struct FunctionCallSuppressor {
    /// Item ids of the function calls we've decided to hide. The
    /// argument-streaming events carry only `item_id` (no name), so we
    /// remember the id from the `output_item.added` / `.done` (which do
    /// carry the name) to suppress them as well.
    tracked: std::sync::Mutex<std::collections::HashSet<String>>,
}

impl FunctionCallSuppressor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return empty `Bytes` when `event` is the rewritten function-call
    /// plumbing for a call whose name satisfies `is_match`; otherwise
    /// return `event` untouched. Non-`function_call` events (including
    /// the tool's own synthesized items) always pass through.
    pub fn suppress(&self, event: Bytes, is_match: impl Fn(&str) -> bool) -> Bytes {
        let Some(data) = sse_event_data(&event) else {
            return event;
        };
        let trimmed = data.trim();
        if trimmed == "[DONE]" {
            return event;
        }
        let Ok(json) = serde_json::from_str::<Value>(trimmed) else {
            return event;
        };
        match json.get("type").and_then(|t| t.as_str()) {
            Some("response.output_item.added" | "response.output_item.done") => {
                let Some(item) = json.get("item") else {
                    return event;
                };
                if item.get("type").and_then(|t| t.as_str()) != Some("function_call") {
                    return event;
                }
                let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
                if !is_match(name) {
                    return event;
                }
                if let Some(id) = item.get("id").and_then(|v| v.as_str())
                    && let Ok(mut set) = self.tracked.lock()
                {
                    set.insert(id.to_string());
                }
                Bytes::new()
            }
            Some(
                "response.function_call_arguments.delta" | "response.function_call_arguments.done",
            ) => {
                // Real OpenAI arg events carry only `item_id`; some
                // providers (and our own fixtures) also include `name`.
                // Match on either so the plumbing is hidden regardless.
                let by_name = json
                    .get("name")
                    .and_then(|v| v.as_str())
                    .is_some_and(&is_match);
                let item_id = json.get("item_id").and_then(|v| v.as_str()).unwrap_or("");
                let by_id = self
                    .tracked
                    .lock()
                    .map(|s| s.contains(item_id))
                    .unwrap_or(false);
                if by_name || by_id {
                    Bytes::new()
                } else {
                    event
                }
            }
            _ => event,
        }
    }
}

/// Join the `data:` field(s) of a single SSE event into one string,
/// honoring CRLF framing and the spec's single-leading-space rule.
/// Returns `None` when the event carries no `data:` payload.
fn sse_event_data(event: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(event).ok()?;
    let mut parts: Vec<&str> = Vec::new();
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            parts.push(rest.strip_prefix(' ').unwrap_or(rest));
        }
    }
    (!parts.is_empty()).then(|| parts.join("\n"))
}

/// Callback that re-invokes the upstream provider with a new payload.
///
/// Used to continue the conversation after a server-executed tool produces
/// results: the orchestrator builds a continuation payload with the tool
/// outputs folded in, then calls this to start the next response stream.
pub type ProviderCallback = Arc<
    dyn Fn(
            CreateResponsesPayload,
        ) -> Pin<Box<dyn Future<Output = Result<Response<Body>, ProviderError>> + Send>>
        + Send
        + Sync,
>;

/// Stream of bytes that gets forwarded to the client.
pub type EventStream = Pin<Box<dyn Stream<Item = Bytes> + Send>>;

/// Future that resolves to a tool call's final result.
pub type ResultFuture =
    Pin<Box<dyn Future<Output = Result<ToolCallResult, ToolError>> + Send + 'static>>;

/// Errors emitted by server-executed tools.
#[derive(Debug, Error)]
pub enum ToolError {
    /// Tool execution failed for an internal reason.
    #[error("tool execution failed: {0}")]
    ExecutionFailed(String),
    /// Tool received a malformed call from the model.
    #[error("malformed tool call: {0}")]
    InvalidCall(String),
    /// Upstream provider returned an error during continuation.
    #[error("provider error: {0}")]
    Provider(String),
}

/// A tool call detected in an SSE event from the upstream provider.
///
/// Carries enough information for the orchestrator to route the call to
/// the right tool implementation; the tool itself parses `arguments` into
/// its concrete arguments type.
#[derive(Debug, Clone)]
pub struct DetectedToolCall {
    /// Name of the tool, matching `ServerExecutedTool::name()`.
    pub tool_name: &'static str,
    /// Stable identifier the model assigned to this call.
    pub call_id: String,
    /// Tool-specific arguments payload — JSON value or any other structure
    /// the tool needs to execute. Each tool's `execute()` interprets this.
    pub arguments: Value,
    /// `Some(error)` when the tool recognized this call by name but could
    /// not parse its arguments. A `detect()` MUST mark such a call here
    /// rather than dropping it (a dropped call leaves the loop reporting a
    /// false `completed` with no feedback to the model). The tool's
    /// `execute()` MUST, when this is set, render its spec-shaped failure
    /// item (e.g. `shell_call_output` with a non-zero exit, `web_search_call`
    /// / `file_search_call` with status `failed`, `mcp_call` with `error`)
    /// plus a `function_call_output` carrying the error, and return `Ok`
    /// (never `Err`) so the loop continues and the model can self-correct.
    pub invalid: Option<String>,
}

impl DetectedToolCall {
    /// A well-formed detected call routed to the tool for execution.
    pub fn new(tool_name: &'static str, call_id: impl Into<String>, arguments: Value) -> Self {
        Self {
            tool_name,
            call_id: call_id.into(),
            arguments,
            invalid: None,
        }
    }

    /// A call recognized by name but whose arguments could not be parsed.
    /// See [`DetectedToolCall::invalid`] for the contract each tool's
    /// `execute()` must honor.
    pub fn invalid(
        tool_name: &'static str,
        call_id: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self {
            tool_name,
            call_id: call_id.into(),
            arguments: Value::Null,
            invalid: Some(error.into()),
        }
    }
}

/// Standard human-readable message for an unparseable tool call, fed back
/// to the model in the `function_call_output` so it can correct the call.
pub fn invalid_arguments_text(tool_name: &str, error: &str) -> String {
    format!("Invalid arguments for tool `{tool_name}`: {error}")
}

/// Result of executing one tool call.
#[derive(Clone)]
pub struct ToolCallResult {
    /// The call this result corresponds to.
    pub call_id: String,
    /// Items to fold into the next provider request's `input`.
    ///
    /// Typically one `FunctionCallOutput` containing the tool result text
    /// the model can read on its next turn.
    pub continuation_items: Vec<ResponsesInputItem>,
}

/// Handle returned by `ServerExecutedTool::execute()`.
///
/// The orchestrator interleaves `events` into the client stream while
/// awaiting `result` for the continuation payload.
pub struct ToolExecutionHandle {
    /// Progress/output events the orchestrator forwards to the client.
    ///
    /// For file_search this is `in_progress`, `searching`, the
    /// `file_search_call` output item, then `completed`. For a future
    /// `shell` tool this carries `output_chunk` events from the container.
    pub events: EventStream,
    /// Final result of the call.
    pub result: ResultFuture,
}

/// Context passed to detection and execution.
///
/// Currently minimal; will grow as tools need things like the principal
/// or per-request budget state.
#[derive(Clone)]
pub struct ToolContext {
    /// The original request payload, used to inspect things like the
    /// `include` field for `file_search` annotations.
    pub original_payload: CreateResponsesPayload,
}

/// A tool the gateway executes on behalf of the model.
///
/// Implementors define detection (what an SSE event for *their* tool looks
/// like), execution (the actual work), and continuation (how their results
/// shape the next provider request).
#[async_trait::async_trait]
pub trait ServerExecutedTool: Send + Sync {
    /// Stable identifier for routing detected calls. Must match
    /// `DetectedToolCall::tool_name`.
    fn name(&self) -> &'static str;

    /// Whether this tool should engage for the given request.
    ///
    /// Examined once per request before the loop starts. If false, the
    /// tool is skipped entirely (no detection overhead).
    fn is_enabled_for(&self, payload: &CreateResponsesPayload) -> bool;

    /// Inspect one complete SSE event and emit any tool calls of this
    /// tool's type that it contains.
    ///
    /// Called for every event of every iteration. Must be cheap.
    ///
    /// Contract: when an event carries a call this tool recognizes by name
    /// but whose arguments cannot be parsed, return it via
    /// [`DetectedToolCall::invalid`] — never silently drop it. Dropping a
    /// recognized call makes the loop end as a false `completed` with no
    /// feedback, stranding the model.
    fn detect(&self, event: &[u8], ctx: &ToolContext) -> Vec<DetectedToolCall>;

    /// Execute one detected tool call.
    ///
    /// Returns a handle exposing progress events plus the final result.
    /// The orchestrator forwards the events to the client and awaits the
    /// result to build the continuation payload.
    ///
    /// Contract: when `call.invalid` is `Some`, emit this tool's spec-shaped
    /// failure item and return `Ok` with a `function_call_output` carrying
    /// the error in `continuation_items` — do not return `Err` (that aborts
    /// the whole turn) and do not run the underlying tool.
    async fn execute(
        &self,
        call: DetectedToolCall,
        ctx: &ToolContext,
    ) -> Result<ToolExecutionHandle, ToolError>;

    /// Fold this tool's results into a continuation payload.
    ///
    /// Called once per iteration with all results for this tool. Typically
    /// appends function-call outputs to `payload.input`. On the final
    /// iteration (when the runner has exhausted its iteration budget for
    /// the next round) the tool should strip its own definitions from
    /// `payload.tools` so the model is forced to produce a text response.
    fn apply_to_continuation(
        &self,
        payload: &mut CreateResponsesPayload,
        results: &[ToolCallResult],
        is_final_iteration: bool,
    );

    /// Transform an outgoing SSE event before it is forwarded to the
    /// client. Default: pass-through. `file_search` overrides this to
    /// inject citation annotations into message-content events after
    /// search results are known.
    ///
    /// Implementations needing stateful transformation (where the result
    /// depends on prior `execute()` calls) should use interior mutability.
    fn transform_event(&self, event: Bytes) -> Bytes {
        event
    }

    /// Events to emit once, before the upstream stream starts.
    ///
    /// Used by `mcp` to surface the `mcp_list_tools` catalog snapshot
    /// at the start of the response, mirroring OpenAI's hosted MCP
    /// behavior so the persisted output records what tool surface the
    /// model saw. Default: empty.
    fn prefix_events(&self) -> Vec<Bytes> {
        Vec::new()
    }
}

#[cfg(test)]
mod suppressor_tests {
    use super::*;

    fn ev(s: &str) -> Bytes {
        Bytes::from(format!("data: {s}\n\n"))
    }

    #[test]
    fn suppresses_matching_function_call_lifecycle() {
        let s = FunctionCallSuppressor::new();
        let is_ws = |n: &str| n == "web_search";

        // output_item.added for the matching function call → dropped,
        // and its item id is remembered.
        let added = ev(
            r#"{"type":"response.output_item.added","item":{"type":"function_call","id":"fc_1","name":"web_search","arguments":""}}"#,
        );
        assert!(s.suppress(added, is_ws).is_empty());

        // The arg-streaming events carry only item_id → dropped via the
        // remembered id.
        let delta =
            ev(r#"{"type":"response.function_call_arguments.delta","item_id":"fc_1","delta":"{"}"#);
        assert!(s.suppress(delta, is_ws).is_empty());
        let done = ev(
            r#"{"type":"response.output_item.done","item":{"type":"function_call","id":"fc_1","name":"web_search","arguments":"{}"}}"#,
        );
        assert!(s.suppress(done, is_ws).is_empty());
    }

    #[test]
    fn passes_through_other_calls_and_synthesized_items() {
        let s = FunctionCallSuppressor::new();
        let is_ws = |n: &str| n == "web_search";

        // A different function call is untouched.
        let other = ev(
            r#"{"type":"response.output_item.added","item":{"type":"function_call","id":"fc_2","name":"get_weather","arguments":""}}"#,
        );
        assert_eq!(s.suppress(other.clone(), is_ws), other);
        // Arg events for an untracked id pass through.
        let delta =
            ev(r#"{"type":"response.function_call_arguments.delta","item_id":"fc_2","delta":"{"}"#);
        assert_eq!(s.suppress(delta.clone(), is_ws), delta);
        // The tool's own synthesized item passes through.
        let synth = ev(
            r#"{"type":"response.output_item.done","item":{"type":"web_search_call","id":"ws_1","status":"completed"}}"#,
        );
        assert_eq!(s.suppress(synth.clone(), is_ws), synth);
        // [DONE] and non-JSON pass through.
        let done = Bytes::from("data: [DONE]\n\n");
        assert_eq!(s.suppress(done.clone(), is_ws), done);
    }
}
