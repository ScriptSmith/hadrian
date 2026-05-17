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
    fn detect(&self, event: &[u8], ctx: &ToolContext) -> Vec<DetectedToolCall>;

    /// Execute one detected tool call.
    ///
    /// Returns a handle exposing progress events plus the final result.
    /// The orchestrator forwards the events to the client and awaits the
    /// result to build the continuation payload.
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
}
