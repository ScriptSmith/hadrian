//! Shell tool interception service for the Responses API.
//!
//! Detects `shell` tool calls in upstream responses, dispatches them to
//! the configured `ShellRuntime`, streams the runtime's output back to
//! the client as `response.shell_call.*` SSE events, and folds the
//! final result into the next provider continuation request.
//!
//! Passthrough mode is handled at registration time: the orchestrator
//! simply doesn't register a `ShellExecutor` when the configured
//! runtime advertises `passthrough_only`. In that case the upstream
//! provider's shell tool spec flows through unchanged.

#![cfg(not(target_arch = "wasm32"))]

use std::{sync::Arc, time::Instant};

use bytes::Bytes;
use chrono::Utc;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::{
    api_types::responses::{
        CreateResponsesPayload, FunctionCallOutput, FunctionCallOutputType, ResponsesInput,
        ResponsesInputItem, ResponsesToolDefinition,
    },
    models::UsageLogEntry,
    pricing::CostPricingSource,
    runtimes::{ExecEvent, ExecRequest, RuntimeError, SessionSpec, ShellRuntime, SkillMount},
    services::server_tools::{
        DetectedToolCall, ServerExecutedTool, ToolCallResult, ToolContext, ToolError,
        ToolExecutionHandle,
    },
};

/// Identity fields captured at request time for shell-tool usage
/// attribution. Mirrors the tuple `extract_identity` returns elsewhere.
#[derive(Debug, Clone, Default)]
pub struct ShellPrincipal {
    pub api_key_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub org_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub team_id: Option<Uuid>,
    pub service_account_id: Option<Uuid>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tool arguments (function schema the model sees)
// ─────────────────────────────────────────────────────────────────────────────

/// Arguments the model emits when invoking the function-mode shell
/// tool. Non-OpenAI providers (Anthropic, etc.) see the `shell` tool
/// rewritten as a function tool with this schema.
#[derive(Debug, Clone, Deserialize)]
pub struct ShellToolArguments {
    pub command: String,
    /// Optional stdin to pipe to the command. Kept short — for larger
    /// inputs, prefer writing files via the runtime's file_io and
    /// referring to them from the command.
    #[serde(default)]
    pub stdin: Option<String>,
}

impl ShellToolArguments {
    pub const FUNCTION_NAME: &'static str = "shell";

    pub fn parse(arguments_json: &str) -> Option<Self> {
        serde_json::from_str(arguments_json).ok()
    }

    pub fn function_description() -> &'static str {
        "Execute a shell command in a sandboxed environment and return its output. \
         Use this for running scripts, querying tools, processing data, or any task \
         that benefits from a shell."
    }

    pub fn function_parameters_schema() -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "stdin": {
                    "type": "string",
                    "description": "Optional stdin to pipe to the command"
                }
            },
            "required": ["command"],
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

/// Rewrite `shell` tool definitions in the payload to function tools so
/// non-OpenAI models can invoke them.
///
/// Called by chat.rs when the configured runtime is **not** passthrough.
/// In passthrough mode, the spec is left intact so OpenAI sees the
/// native tool definition.
pub fn preprocess_shell_tools(payload: &mut CreateResponsesPayload) {
    let Some(tools) = payload.tools.as_mut() else {
        return;
    };
    for tool in tools.iter_mut() {
        if tool.is_shell() {
            *tool =
                ResponsesToolDefinition::Function(ShellToolArguments::function_tool_definition());
            debug!(
                stage = "tool_preprocessed",
                "Preprocessed shell tool to function definition"
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Detection
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct ShellToolCall {
    id: String,
    command: String,
    stdin: Option<String>,
}

fn parse_shell_tool_call(value: &Value) -> Option<ShellToolCall> {
    let obj = value.as_object()?;
    if obj.get("type").and_then(|t| t.as_str())? != "function_call" {
        return None;
    }
    if obj.get("name").and_then(|n| n.as_str())? != ShellToolArguments::FUNCTION_NAME {
        return None;
    }
    let id = obj
        .get("call_id")
        .or_else(|| obj.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let arguments_str = obj.get("arguments")?.as_str()?;
    let args = ShellToolArguments::parse(arguments_str)?;
    Some(ShellToolCall {
        id,
        command: args.command,
        stdin: args.stdin,
    })
}

fn detect_shell_in_chunk(chunk: &[u8]) -> Vec<ShellToolCall> {
    let Ok(chunk_str) = std::str::from_utf8(chunk) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for line in chunk_str.lines() {
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data == "[DONE]" {
            continue;
        }
        let Ok(json) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        // Same canonical detection as web_search: only emit on
        // response.output_item.done to avoid duplicates.
        if json.get("type").and_then(|t| t.as_str()) == Some("response.output_item.done")
            && let Some(item) = json.get("item")
            && let Some(tc) = parse_shell_tool_call(item)
        {
            found.push(tc);
        }
    }
    found
}

// ─────────────────────────────────────────────────────────────────────────────
// SSE event formatters
// ─────────────────────────────────────────────────────────────────────────────

fn sse_event(payload: Value) -> Bytes {
    let s = serde_json::to_string(&payload).unwrap_or_default();
    Bytes::from(format!("data: {}\n\n", s))
}

fn format_in_progress(item_id: &str, output_index: usize) -> Bytes {
    sse_event(serde_json::json!({
        "type": "response.shell_call.in_progress",
        "output_index": output_index,
        "item_id": item_id,
    }))
}

fn format_command_started(item_id: &str, output_index: usize, command: &str) -> Bytes {
    sse_event(serde_json::json!({
        "type": "response.shell_call.command_started",
        "output_index": output_index,
        "item_id": item_id,
        "command": command,
    }))
}

fn format_output_chunk(item_id: &str, output_index: usize, stream: &str, data: &[u8]) -> Bytes {
    // Encode chunk bytes as UTF-8 with replacement to keep SSE
    // line-safe; binary data should be rare in shell stdout.
    let text = String::from_utf8_lossy(data).to_string();
    sse_event(serde_json::json!({
        "type": "response.shell_call.output_chunk",
        "output_index": output_index,
        "item_id": item_id,
        "stream": stream, // "stdout" | "stderr"
        "data": text,
    }))
}

fn format_completed(item_id: &str, output_index: usize, exit_code: i32) -> Bytes {
    sse_event(serde_json::json!({
        "type": "response.shell_call.completed",
        "output_index": output_index,
        "item_id": item_id,
        "exit_code": exit_code,
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// Output trimming for continuation payload
// ─────────────────────────────────────────────────────────────────────────────

/// Max characters of stdout/stderr we feed back to the model per call,
/// preserving head + tail like OpenAI's `output_text_truncation`.
const MAX_OUTPUT_CHARS: usize = 8_000;

fn trim_output(s: String) -> String {
    if s.len() <= MAX_OUTPUT_CHARS {
        return s;
    }
    let half = MAX_OUTPUT_CHARS / 2;
    let head: String = s.chars().take(half).collect();
    let tail: String = s
        .chars()
        .rev()
        .take(half)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!(
        "{head}\n... [{} chars truncated] ...\n{tail}",
        s.len() - MAX_OUTPUT_CHARS
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// ShellExecutor
// ─────────────────────────────────────────────────────────────────────────────

/// `ServerExecutedTool` implementation that runs shell commands against
/// any [`ShellRuntime`] whose `capabilities().passthrough_only` is
/// false.
///
/// **Not registered for passthrough runtimes** — the orchestrator
/// inspects the runtime's capabilities and skips registration entirely
/// when passthrough is in effect.
pub struct ShellExecutor {
    runtime: Arc<dyn ShellRuntime>,
    /// Cost per second of runtime time, in microcents. Multiplied by
    /// the wall-clock duration of each shell call to compute the
    /// chargeable cost emitted to metrics and the usage record.
    cost_microcents_per_second: u64,
    /// Label used for the runtime axis of cost metrics
    /// (e.g. `"microsandbox"`, `"passthrough_openai"`).
    runtime_label: &'static str,
    /// Identity context attached to the per-shell-call usage record so
    /// runtime time is attributed to the right principal.
    principal: ShellPrincipal,
    /// Skill bundles to mount into every session started by this
    /// executor. Resolved upstream from the request's `skills` field;
    /// empty when the request didn't ask for any. Cloned into each
    /// `SessionSpec` because shell tool calls can repeat and each one
    /// boots a fresh session.
    mounted_skills: Vec<SkillMount>,
    /// Usage log buffer. When set, the executor pushes a `record_type:
    /// "tool"` entry per completed call with `tool_runtime_seconds` set.
    #[cfg(feature = "concurrency")]
    usage_buffer: Option<Arc<crate::usage_buffer::UsageLogBuffer>>,
}

impl ShellExecutor {
    pub fn new(
        runtime: Arc<dyn ShellRuntime>,
        cost_microcents_per_second: u64,
        runtime_label: &'static str,
        principal: ShellPrincipal,
        mounted_skills: Vec<SkillMount>,
        #[cfg(feature = "concurrency")] usage_buffer: Option<
            Arc<crate::usage_buffer::UsageLogBuffer>,
        >,
    ) -> Self {
        Self {
            runtime,
            cost_microcents_per_second,
            runtime_label,
            principal,
            mounted_skills,
            #[cfg(feature = "concurrency")]
            usage_buffer,
        }
    }
}

#[async_trait::async_trait]
impl ServerExecutedTool for ShellExecutor {
    fn name(&self) -> &'static str {
        ShellToolArguments::FUNCTION_NAME
    }

    fn is_enabled_for(&self, payload: &CreateResponsesPayload) -> bool {
        // We only engage if there's a shell tool — or a function tool
        // already preprocessed from a shell tool — in the request.
        payload
            .tools
            .as_ref()
            .map(|tools| {
                tools.iter().any(|t| {
                    t.is_shell()
                        || matches!(
                            t,
                            ResponsesToolDefinition::Function(v)
                                if v.get("name").and_then(|n| n.as_str())
                                    == Some(ShellToolArguments::FUNCTION_NAME)
                        )
                })
            })
            .unwrap_or(false)
    }

    fn detect(&self, event: &[u8], _ctx: &ToolContext) -> Vec<DetectedToolCall> {
        detect_shell_in_chunk(event)
            .into_iter()
            .map(|tc| DetectedToolCall {
                tool_name: ShellToolArguments::FUNCTION_NAME,
                call_id: tc.id.clone(),
                arguments: serde_json::json!({
                    "id": tc.id,
                    "command": tc.command,
                    "stdin": tc.stdin,
                }),
            })
            .collect()
    }

    async fn execute(
        &self,
        call: DetectedToolCall,
        _ctx: &ToolContext,
    ) -> Result<ToolExecutionHandle, ToolError> {
        let command = call
            .arguments
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let stdin = call
            .arguments
            .get("stdin")
            .and_then(|v| v.as_str())
            .map(|s| Bytes::from(s.to_string()));
        let id = call.call_id.clone();
        let runtime = self.runtime.clone();
        let cost_per_sec = self.cost_microcents_per_second;
        let runtime_label = self.runtime_label;
        let principal = self.principal.clone();
        let mounted_skills = self.mounted_skills.clone();
        #[cfg(feature = "concurrency")]
        let usage_buffer = self.usage_buffer.clone();

        let (event_tx, event_rx) = mpsc::channel::<Bytes>(32);
        let (result_tx, result_rx) =
            tokio::sync::oneshot::channel::<Result<ToolCallResult, ToolError>>();

        // Emit initial progress events before doing any I/O.
        let _ = event_tx.send(format_in_progress(&id, 0)).await;
        let _ = event_tx
            .send(format_command_started(&id, 0, &command))
            .await;

        // Spawn the actual session work so the orchestrator can start
        // consuming events while we boot the container.
        let id_for_task = id.clone();
        let command_for_task = command.clone();
        crate::compat::spawn_detached(async move {
            let start = Instant::now();
            let spec = SessionSpec {
                mounted_skills,
                ..SessionSpec::default()
            };
            let session = match runtime.start_session(spec).await {
                Ok(s) => s,
                Err(RuntimeError::Passthrough) => {
                    // The orchestrator shouldn't have invoked us for a
                    // passthrough runtime; warn and stop without
                    // hanging the request.
                    warn!(
                        stage = "passthrough_invoked",
                        call_id = %id_for_task,
                        "Passthrough runtime received an execute() call; \
                         this indicates a misconfiguration in chat.rs registration"
                    );
                    let _ = event_tx.send(format_completed(&id_for_task, 0, -1)).await;
                    let _ = result_tx.send(Err(ToolError::ExecutionFailed(
                        "shell runtime is configured for passthrough but executor was invoked"
                            .into(),
                    )));
                    return;
                }
                Err(e) => {
                    error!(
                        stage = "session_start_failed",
                        call_id = %id_for_task,
                        error = %e,
                        "Failed to start shell session"
                    );
                    let _ = event_tx.send(format_completed(&id_for_task, 0, -1)).await;
                    let _ = result_tx.send(Err(ToolError::ExecutionFailed(e.to_string())));
                    return;
                }
            };

            let exec = match session
                .exec(ExecRequest {
                    command: command_for_task.clone(),
                    stdin,
                    timeout: None,
                })
                .await
            {
                Ok(e) => e,
                Err(e) => {
                    error!(
                        stage = "exec_failed",
                        call_id = %id_for_task,
                        error = %e,
                        "Failed to exec shell command"
                    );
                    let _ = event_tx.send(format_completed(&id_for_task, 0, -1)).await;
                    let _ = session.terminate().await;
                    let _ = result_tx.send(Err(ToolError::ExecutionFailed(e.to_string())));
                    return;
                }
            };

            // Stream output, accumulating for the continuation payload.
            // We race two futures:
            //   - `output.next()`: the next ExecEvent from the runtime.
            //   - `event_tx.closed()`: resolves when the orchestrator has
            //     dropped its receiver, which happens when the HTTP
            //     client disconnects upstream.
            //
            // This catches disconnect even for commands that produce no
            // output, which the previous send-error-only check missed.
            let mut stdout_buf = String::new();
            let mut stderr_buf = String::new();
            let mut final_exit: i32 = 0;
            let mut output = exec.output;
            let mut client_disconnected = false;
            loop {
                tokio::select! {
                    _ = event_tx.closed() => {
                        warn!(
                            stage = "client_disconnected",
                            call_id = %id_for_task,
                            "Client disconnected (channel closed); terminating session"
                        );
                        client_disconnected = true;
                        break;
                    }
                    maybe_ev = output.next() => {
                        let Some(ev) = maybe_ev else { break };
                        let send_result = match ev {
                            ExecEvent::Stdout(bytes) => {
                                stdout_buf.push_str(&String::from_utf8_lossy(&bytes));
                                event_tx
                                    .send(format_output_chunk(&id_for_task, 0, "stdout", &bytes))
                                    .await
                            }
                            ExecEvent::Stderr(bytes) => {
                                stderr_buf.push_str(&String::from_utf8_lossy(&bytes));
                                event_tx
                                    .send(format_output_chunk(&id_for_task, 0, "stderr", &bytes))
                                    .await
                            }
                            ExecEvent::Exit { code, .. } => {
                                final_exit = code;
                                Ok(())
                            }
                        };
                        if send_result.is_err() {
                            warn!(
                                stage = "client_disconnected",
                                call_id = %id_for_task,
                                "Client disconnected mid-output; terminating session"
                            );
                            client_disconnected = true;
                            break;
                        }
                    }
                }
            }

            let duration_secs = start.elapsed().as_secs_f64();
            // Always tear the session down whether we completed normally
            // or aborted on client disconnect.
            let _ = session.terminate().await;

            // Cost is billable regardless of how the session ended — we
            // ran the VM, the operator pays for the time.
            let cost_microcents = (duration_secs * cost_per_sec as f64).round() as i64;

            // Push the per-principal usage record. We do this on every
            // exit path (completion + disconnect) so the principal is
            // billed for what they consumed.
            #[cfg(feature = "concurrency")]
            if let Some(ref buf) = usage_buffer {
                buf.push(UsageLogEntry {
                    request_id: Uuid::new_v4().to_string(),
                    api_key_id: principal.api_key_id,
                    user_id: principal.user_id,
                    org_id: principal.org_id,
                    project_id: principal.project_id,
                    team_id: principal.team_id,
                    service_account_id: principal.service_account_id,
                    model: "shell".to_string(),
                    provider: runtime_label.to_string(),
                    http_referer: None,
                    input_tokens: 0,
                    output_tokens: 0,
                    cost_microcents: Some(cost_microcents),
                    request_at: Utc::now(),
                    streamed: true,
                    cached_tokens: 0,
                    reasoning_tokens: 0,
                    finish_reason: Some(
                        if client_disconnected {
                            "client_disconnected"
                        } else {
                            "completed"
                        }
                        .to_string(),
                    ),
                    latency_ms: Some((duration_secs * 1000.0) as i32),
                    cancelled: client_disconnected,
                    status_code: Some(200),
                    pricing_source: CostPricingSource::PricingConfig,
                    image_count: None,
                    audio_seconds: None,
                    character_count: None,
                    provider_source: None,
                    record_type: "tool".to_string(),
                    tool_name: Some("shell".to_string()),
                    tool_query: Some(command_for_task.clone()),
                    tool_url: None,
                    tool_bytes_fetched: None,
                    tool_results_count: None,
                    tool_runtime_seconds: Some(duration_secs),
                });
            }
            #[cfg(not(feature = "concurrency"))]
            let _ = (&principal, command_for_task.clone());

            if client_disconnected {
                crate::observability::metrics::record_shell_execution(
                    duration_secs,
                    final_exit,
                    "client_disconnected",
                    runtime_label,
                    cost_microcents,
                );
                // Drop both channels without sending — the orchestrator
                // is gone, no one is listening.
                return;
            }

            let _ = event_tx
                .send(format_completed(&id_for_task, 0, final_exit))
                .await;
            info!(
                stage = "shell_completed",
                call_id = %id_for_task,
                exit_code = final_exit,
                duration_ms = (duration_secs * 1000.0) as u64,
                cost_microcents,
                runtime = runtime_label,
                "Shell command completed"
            );
            crate::observability::metrics::record_shell_execution(
                duration_secs,
                final_exit,
                "completed",
                runtime_label,
                cost_microcents,
            );

            // Build the continuation item — the model sees a single
            // text blob with combined stdout/stderr summary, head+tail
            // truncated.
            let combined = format!(
                "exit_code: {}\nstdout:\n{}\nstderr:\n{}",
                final_exit,
                trim_output(stdout_buf),
                trim_output(stderr_buf)
            );

            let cont_item = ResponsesInputItem::FunctionCallOutput(FunctionCallOutput {
                type_: FunctionCallOutputType::FunctionCallOutput,
                id: Some(id_for_task.clone()),
                call_id: id_for_task.clone(),
                output: combined,
                status: None,
            });

            let _ = result_tx.send(Ok(ToolCallResult {
                call_id: id_for_task,
                continuation_items: vec![cont_item],
            }));
            drop(event_tx);
        });

        Ok(ToolExecutionHandle {
            events: Box::pin(futures_util::stream::unfold(
                event_rx,
                |mut rx| async move { rx.recv().await.map(|item| (item, rx)) },
            )),
            result: Box::pin(async move {
                result_rx
                    .await
                    .map_err(|_| ToolError::ExecutionFailed("shell result channel closed".into()))?
            }),
        })
    }

    fn apply_to_continuation(
        &self,
        payload: &mut CreateResponsesPayload,
        results: &[ToolCallResult],
        is_final_iteration: bool,
    ) {
        let function_outputs: Vec<ResponsesInputItem> = results
            .iter()
            .flat_map(|r| r.continuation_items.clone())
            .collect();
        if function_outputs.is_empty() {
            return;
        }

        match payload.input {
            Some(ResponsesInput::Items(ref mut items)) => {
                items.extend(function_outputs);
            }
            Some(ResponsesInput::Text(ref text)) => {
                let text = text.clone();
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

        if is_final_iteration && let Some(ref mut tools) = payload.tools {
            let before = tools.len();
            tools.retain(|t| !t.is_shell());
            tools.retain(|t| {
                if let ResponsesToolDefinition::Function(v) = t {
                    v.get("name").and_then(|n| n.as_str())
                        != Some(ShellToolArguments::FUNCTION_NAME)
                } else {
                    true
                }
            });
            if tools.len() < before {
                info!(
                    stage = "tools_removed",
                    removed = before - tools.len(),
                    "Removed shell tools on final iteration to force completion"
                );
            }
            if tools.is_empty() {
                payload.tools = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_function_call_arguments() {
        let v = serde_json::json!({
            "type": "function_call",
            "name": "shell",
            "call_id": "call_abc",
            "arguments": "{\"command\": \"echo hi\"}"
        });
        let tc = parse_shell_tool_call(&v).unwrap();
        assert_eq!(tc.id, "call_abc");
        assert_eq!(tc.command, "echo hi");
        assert!(tc.stdin.is_none());
    }

    #[test]
    fn ignores_non_shell_function_calls() {
        let v = serde_json::json!({
            "type": "function_call",
            "name": "web_search",
            "call_id": "call_xyz",
            "arguments": "{\"query\": \"hi\"}"
        });
        assert!(parse_shell_tool_call(&v).is_none());
    }

    #[test]
    fn preprocess_rewrites_shell_tool_to_function() {
        let payload_json = serde_json::json!({
            "tools": [{"type": "shell"}],
            "stream": false,
        });
        let mut payload: CreateResponsesPayload = serde_json::from_value(payload_json).unwrap();
        preprocess_shell_tools(&mut payload);
        let tools = payload.tools.unwrap();
        assert_eq!(tools.len(), 1);
        assert!(matches!(tools[0], ResponsesToolDefinition::Function(_)));
    }

    #[test]
    fn trim_output_preserves_head_and_tail() {
        let big = "a".repeat(MAX_OUTPUT_CHARS + 100);
        let trimmed = trim_output(big);
        assert!(trimmed.contains("chars truncated"));
        assert!(trimmed.starts_with("aaa"));
        assert!(trimmed.ends_with("aaa"));
    }
}
