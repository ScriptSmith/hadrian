//! Anthropic SSE stream transformers.
//!
//! Transforms Anthropic SSE streams to OpenAI-compatible formats.

use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use bytes::Bytes;
use futures_util::Stream;
use serde::{Deserialize, Serialize};

use crate::config::StreamingBufferConfig;

// ============================================================================
// Anthropic Streaming Event Types
// ============================================================================

/// Anthropic streaming event types
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicStreamEvent {
    MessageStart {
        message: MessageStartData,
    },
    ContentBlockStart {
        index: usize,
        content_block: StreamContentBlockType,
    },
    ContentBlockDelta {
        index: usize,
        delta: ContentDelta,
    },
    /// Marks end of a content block
    ContentBlockStop {},
    MessageDelta {
        delta: MessageDeltaData,
        usage: Option<MessageDeltaUsage>,
    },
    MessageStop,
    Ping,
    Error {
        error: AnthropicStreamError,
    },
}

#[derive(Debug, Deserialize)]
pub struct MessageStartData {
    pub id: String,
    pub model: String,
    #[serde(default)]
    pub usage: Option<MessageStartUsage>,
}

#[derive(Debug, Deserialize)]
pub struct MessageStartUsage {
    pub input_tokens: i64,
    #[serde(default)]
    #[allow(dead_code)] // Deserialized but only input_tokens is used from message_start
    pub output_tokens: i64,
    /// Tokens read from the prompt cache (cache hit)
    #[serde(default)]
    pub cache_read_input_tokens: i64,
    /// Tokens written to the prompt cache (cache miss, will be cached).
    #[serde(default)]
    pub cache_creation_input_tokens: i64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamContentBlockType {
    /// Text content block (text field present but unused - we only emit on deltas)
    Text {
        #[allow(dead_code)] // Deserialization field
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
    },
    /// Extended thinking block (thinking field present but unused - we only emit on deltas)
    Thinking {
        #[allow(dead_code)] // Deserialization field
        thinking: String,
    },
}

/// Content delta types from Anthropic streaming.
/// Note: Variant names match Anthropic's API format (text_delta, input_json_delta, etc.)
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::enum_variant_names)]
pub enum ContentDelta {
    TextDelta {
        text: String,
    },
    InputJsonDelta {
        partial_json: String,
    },
    /// Extended thinking delta - emitted as `reasoning` field in OpenAI format
    ThinkingDelta {
        thinking: String,
    },
    /// Extended thinking signature delta (sent at end of thinking block)
    SignatureDelta {
        signature: String,
    },
}

#[derive(Debug, Deserialize)]
pub struct MessageDeltaData {
    pub stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MessageDeltaUsage {
    pub output_tokens: i64,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicStreamError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

// ============================================================================
// OpenAI-compatible Streaming Response Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct OpenAIStreamChunk {
    pub id: String,
    pub object: &'static str,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAIStreamChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<OpenAIStreamUsage>,
}

#[derive(Debug, Serialize)]
pub struct OpenAIStreamChoice {
    pub index: i32,
    pub delta: OpenAIDelta,
    /// Required per OpenAI spec (can be null until stream completes)
    pub finish_reason: Option<String>,
    pub logprobs: Option<()>,
}

#[derive(Debug, Default, Serialize)]
pub struct OpenAIDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAIStreamToolCall>>,
    /// Reasoning/thinking content from extended thinking (Anthropic extension)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OpenAIStreamToolCall {
    pub index: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<&'static str>,
    pub function: OpenAIStreamFunction,
}

#[derive(Debug, Serialize)]
pub struct OpenAIStreamFunction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

/// Breakdown of prompt tokens (OpenAI-compatible)
#[derive(Debug, Serialize)]
pub struct PromptTokensDetails {
    /// Cached tokens read from prompt cache
    pub cached_tokens: i64,
    /// Tokens written to the prompt cache (cache miss, will be cached)
    #[serde(skip_serializing_if = "super::types::is_zero")]
    pub cache_creation_input_tokens: i64,
}

#[derive(Debug, Serialize)]
pub struct OpenAIStreamUsage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    /// Breakdown of prompt tokens including cache information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_details: Option<PromptTokensDetails>,
}

// ============================================================================
// Chat Completion Stream Transformer
// ============================================================================

/// State for tracking the stream transformation
#[derive(Debug, Default)]
struct StreamState {
    message_id: String,
    model: String,
    input_tokens: i64,
    output_tokens: i64,
    /// Cached tokens read from prompt cache
    cache_read_input_tokens: i64,
    /// Tokens written to the prompt cache
    cache_creation_input_tokens: i64,
    /// Tracks which content blocks are tool calls (by index)
    tool_call_indices: Vec<(usize, String, String)>, // (anthropic_index, tool_id, tool_name)
    /// Tracks which content blocks are thinking blocks (by Anthropic index)
    thinking_block_indices: Vec<usize>,
    /// Buffer for incomplete SSE data
    buffer: String,
    /// Whether we've sent the initial role delta
    #[allow(dead_code)] // Deserialization field
    sent_role: bool,
    /// Error state for buffer overflow
    buffer_overflow: bool,
}

/// Stream transformer that converts Anthropic SSE to OpenAI SSE format
pub struct AnthropicToOpenAIStream<S> {
    inner: S,
    state: StreamState,
    /// Output buffer for generated SSE chunks
    output_buffer: Vec<Bytes>,
    /// Maximum input buffer size in bytes
    max_input_buffer_bytes: usize,
    /// Maximum output buffer chunks
    max_output_buffer_chunks: usize,
}

impl<S> AnthropicToOpenAIStream<S> {
    pub fn new(inner: S, streaming_buffer: &StreamingBufferConfig) -> Self {
        Self {
            inner,
            state: StreamState::default(),
            output_buffer: Vec::new(),
            max_input_buffer_bytes: streaming_buffer.max_input_buffer_bytes,
            max_output_buffer_chunks: streaming_buffer.max_output_buffer_chunks,
        }
    }

    fn created_timestamp() -> i64 {
        chrono::Utc::now().timestamp()
    }

    /// Parse an Anthropic SSE line and generate OpenAI SSE chunks
    fn process_sse_line(&mut self, line: &str) {
        // Handle event type lines (we mostly ignore these)
        if line.starts_with("event:") {
            return;
        }

        // Handle data lines
        if let Some(json_str) = line.strip_prefix("data: ") {
            let json_str = json_str.trim();
            if json_str.is_empty() {
                return;
            }

            // Skip [DONE] marker (OpenAI format, not used by Anthropic but may appear in fixtures)
            if json_str == "[DONE]" {
                return;
            }

            // Parse Anthropic event
            match serde_json::from_str::<AnthropicStreamEvent>(json_str) {
                Ok(event) => self.handle_event(event),
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse Anthropic SSE event: {}, json: {}",
                        e,
                        json_str
                    );
                }
            }
        }
    }

    fn handle_event(&mut self, event: AnthropicStreamEvent) {
        match event {
            AnthropicStreamEvent::MessageStart { message } => {
                self.state.message_id = message.id;
                self.state.model = message.model;
                if let Some(usage) = message.usage {
                    self.state.input_tokens = usage.input_tokens;
                    self.state.cache_read_input_tokens = usage.cache_read_input_tokens;
                    self.state.cache_creation_input_tokens = usage.cache_creation_input_tokens;
                }

                // Send initial chunk with role
                let chunk = OpenAIStreamChunk {
                    id: self.state.message_id.clone(),
                    object: "chat.completion.chunk",
                    created: Self::created_timestamp(),
                    model: self.state.model.clone(),
                    choices: vec![OpenAIStreamChoice {
                        index: 0,
                        delta: OpenAIDelta {
                            role: Some("assistant"),
                            content: None,
                            tool_calls: None,
                            reasoning: None,
                        },
                        finish_reason: None,
                        logprobs: None,
                    }],
                    usage: None,
                };
                self.emit_chunk(&chunk);
                self.state.sent_role = true;
            }

            AnthropicStreamEvent::ContentBlockStart {
                index,
                content_block,
            } => match content_block {
                StreamContentBlockType::ToolUse { id, name } => {
                    // Track this as a tool call
                    let tool_index = self.state.tool_call_indices.len();
                    self.state
                        .tool_call_indices
                        .push((index, id.clone(), name.clone()));

                    // Emit tool call start
                    let chunk = OpenAIStreamChunk {
                        id: self.state.message_id.clone(),
                        object: "chat.completion.chunk",
                        created: Self::created_timestamp(),
                        model: self.state.model.clone(),
                        choices: vec![OpenAIStreamChoice {
                            index: 0,
                            delta: OpenAIDelta {
                                role: None,
                                content: None,
                                tool_calls: Some(vec![OpenAIStreamToolCall {
                                    index: tool_index as i32,
                                    id: Some(id),
                                    type_: Some("function"),
                                    function: OpenAIStreamFunction {
                                        name: Some(name),
                                        arguments: Some(String::new()),
                                    },
                                }]),
                                reasoning: None,
                            },
                            finish_reason: None,
                            logprobs: None,
                        }],
                        usage: None,
                    };
                    self.emit_chunk(&chunk);
                }
                StreamContentBlockType::Text { .. } => {
                    // Nothing special needed for text block start
                }
                StreamContentBlockType::Thinking { .. } => {
                    // Track this as a thinking block for later delta handling
                    self.state.thinking_block_indices.push(index);
                }
            },

            AnthropicStreamEvent::ContentBlockDelta { index, delta } => match delta {
                ContentDelta::TextDelta { text } => {
                    let chunk = OpenAIStreamChunk {
                        id: self.state.message_id.clone(),
                        object: "chat.completion.chunk",
                        created: Self::created_timestamp(),
                        model: self.state.model.clone(),
                        choices: vec![OpenAIStreamChoice {
                            index: 0,
                            delta: OpenAIDelta {
                                role: None,
                                content: Some(text),
                                tool_calls: None,
                                reasoning: None,
                            },
                            finish_reason: None,
                            logprobs: None,
                        }],
                        usage: None,
                    };
                    self.emit_chunk(&chunk);
                }
                ContentDelta::InputJsonDelta { partial_json } => {
                    // Find the tool call index for this content block
                    if let Some((tool_index, _)) = self
                        .state
                        .tool_call_indices
                        .iter()
                        .enumerate()
                        .find(|(_, (anthropic_idx, _, _))| *anthropic_idx == index)
                    {
                        let chunk = OpenAIStreamChunk {
                            id: self.state.message_id.clone(),
                            object: "chat.completion.chunk",
                            created: Self::created_timestamp(),
                            model: self.state.model.clone(),
                            choices: vec![OpenAIStreamChoice {
                                index: 0,
                                delta: OpenAIDelta {
                                    role: None,
                                    content: None,
                                    tool_calls: Some(vec![OpenAIStreamToolCall {
                                        index: tool_index as i32,
                                        id: None,
                                        type_: None,
                                        function: OpenAIStreamFunction {
                                            name: None,
                                            arguments: Some(partial_json),
                                        },
                                    }]),
                                    reasoning: None,
                                },
                                finish_reason: None,
                                logprobs: None,
                            }],
                            usage: None,
                        };
                        self.emit_chunk(&chunk);
                    }
                }
                ContentDelta::ThinkingDelta { thinking } => {
                    // Emit thinking delta as reasoning content
                    // Only emit if this is a tracked thinking block
                    if self.state.thinking_block_indices.contains(&index) {
                        let chunk = OpenAIStreamChunk {
                            id: self.state.message_id.clone(),
                            object: "chat.completion.chunk",
                            created: Self::created_timestamp(),
                            model: self.state.model.clone(),
                            choices: vec![OpenAIStreamChoice {
                                index: 0,
                                delta: OpenAIDelta {
                                    role: None,
                                    content: None,
                                    tool_calls: None,
                                    reasoning: Some(thinking),
                                },
                                finish_reason: None,
                                logprobs: None,
                            }],
                            usage: None,
                        };
                        self.emit_chunk(&chunk);
                    }
                }
                ContentDelta::SignatureDelta { .. } => {
                    // Signature delta marks the end of thinking - we don't expose this
                    // as it's an internal verification mechanism
                }
            },

            AnthropicStreamEvent::ContentBlockStop { .. } => {
                // Nothing to emit for content block stop
            }

            AnthropicStreamEvent::MessageDelta { delta, usage } => {
                // Update output tokens
                if let Some(u) = usage {
                    self.state.output_tokens = u.output_tokens;
                }

                // Emit finish reason if present
                if let Some(stop_reason) = delta.stop_reason {
                    let finish_reason = match stop_reason.as_str() {
                        "end_turn" => "stop",
                        "max_tokens" => "length",
                        "stop_sequence" => "stop",
                        "tool_use" => "tool_calls",
                        "pause_turn" => "stop",
                        "refusal" => "stop",
                        other => other,
                    };

                    let chunk = OpenAIStreamChunk {
                        id: self.state.message_id.clone(),
                        object: "chat.completion.chunk",
                        created: Self::created_timestamp(),
                        model: self.state.model.clone(),
                        choices: vec![OpenAIStreamChoice {
                            index: 0,
                            delta: OpenAIDelta::default(),
                            finish_reason: Some(finish_reason.to_string()),
                            logprobs: None,
                        }],
                        usage: None,
                    };
                    self.emit_chunk(&chunk);
                }
            }

            AnthropicStreamEvent::MessageStop => {
                // Emit final chunk with usage
                let has_cache_details = self.state.cache_read_input_tokens > 0
                    || self.state.cache_creation_input_tokens > 0;
                let chunk = OpenAIStreamChunk {
                    id: self.state.message_id.clone(),
                    object: "chat.completion.chunk",
                    created: Self::created_timestamp(),
                    model: self.state.model.clone(),
                    choices: vec![],
                    usage: Some(OpenAIStreamUsage {
                        prompt_tokens: self.state.input_tokens,
                        completion_tokens: self.state.output_tokens,
                        total_tokens: self.state.input_tokens + self.state.output_tokens,
                        prompt_tokens_details: if has_cache_details {
                            Some(PromptTokensDetails {
                                cached_tokens: self.state.cache_read_input_tokens,
                                cache_creation_input_tokens: self.state.cache_creation_input_tokens,
                            })
                        } else {
                            None
                        },
                    }),
                };
                self.emit_chunk(&chunk);

                // Emit [DONE]
                self.output_buffer.push(Bytes::from("data: [DONE]\n\n"));
            }

            AnthropicStreamEvent::Ping => {
                // Ignore ping events
            }

            AnthropicStreamEvent::Error { error } => {
                tracing::error!(
                    "Anthropic streaming error: {} - {}",
                    error.error_type,
                    error.message
                );
                // Could emit an error chunk here, but OpenAI doesn't have a standard format
            }
        }
    }

    fn emit_chunk(&mut self, chunk: &OpenAIStreamChunk) {
        if let Ok(json) = serde_json::to_string(chunk) {
            let sse = format!("data: {}\n\n", json);
            self.output_buffer.push(Bytes::from(sse));
        }
    }

    /// Process a chunk of bytes, potentially containing multiple SSE events
    fn process_bytes(&mut self, bytes: &[u8]) {
        // Check if we're already in error state
        if self.state.buffer_overflow {
            return;
        }

        if let Ok(text) = std::str::from_utf8(bytes) {
            // Check input buffer limit before adding
            if self.state.buffer.len() + text.len() > self.max_input_buffer_bytes {
                tracing::error!(
                    buffer_size = self.state.buffer.len(),
                    incoming_size = text.len(),
                    max_size = self.max_input_buffer_bytes,
                    "SSE input buffer overflow - possible DoS or malformed response"
                );
                self.state.buffer_overflow = true;
                return;
            }

            self.state.buffer.push_str(text);

            // Process complete lines
            while let Some(pos) = self.state.buffer.find('\n') {
                // Extract and process line, avoiding allocation for empty lines
                let trimmed = self.state.buffer[..pos].trim();
                if !trimmed.is_empty() {
                    // Only allocate when we need to process the line
                    let line = trimmed.to_string();
                    self.process_sse_line(&line);
                }
                // Remove processed bytes in-place (no allocation, reuses buffer capacity)
                self.state.buffer.drain(..=pos);

                // Check output buffer limit
                if self.output_buffer.len() > self.max_output_buffer_chunks {
                    tracing::error!(
                        buffer_size = self.output_buffer.len(),
                        max_size = self.max_output_buffer_chunks,
                        "SSE output buffer overflow - slow consumer or producer overload"
                    );
                    self.state.buffer_overflow = true;
                    return;
                }
            }
        }
    }
}

impl<S> Stream for AnthropicToOpenAIStream<S>
where
    S: Stream<Item = Result<Bytes, io::Error>> + Unpin,
{
    type Item = Result<Bytes, io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Check for buffer overflow error
        if self.state.buffer_overflow {
            return Poll::Ready(Some(Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "SSE buffer overflow",
            ))));
        }

        // First, return any buffered output
        if !self.output_buffer.is_empty() {
            return Poll::Ready(Some(Ok(self.output_buffer.remove(0))));
        }

        // Poll the inner stream
        let inner = Pin::new(&mut self.inner);
        match inner.poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                // Process the Anthropic SSE bytes
                self.process_bytes(&bytes);

                // Check for buffer overflow after processing
                if self.state.buffer_overflow {
                    return Poll::Ready(Some(Err(io::Error::new(
                        io::ErrorKind::OutOfMemory,
                        "SSE buffer overflow",
                    ))));
                }

                // Return first buffered output if any
                if !self.output_buffer.is_empty() {
                    Poll::Ready(Some(Ok(self.output_buffer.remove(0))))
                } else {
                    // No output yet, need to poll again
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => {
                // Stream ended - flush any remaining buffer
                if !self.output_buffer.is_empty() {
                    Poll::Ready(Some(Ok(self.output_buffer.remove(0))))
                } else {
                    Poll::Ready(None)
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

// ============================================================================
// Responses API Stream Transformer
// ============================================================================

/// State for tracking the Responses API stream transformation
#[derive(Debug, Default)]
struct ResponsesStreamState {
    response_id: String,
    message_id: String,
    model: String,
    input_tokens: i64,
    output_tokens: i64,
    /// Cached tokens read from prompt cache
    cache_read_input_tokens: i64,
    /// Tokens written to the prompt cache
    cache_creation_input_tokens: i64,
    /// Accumulated text content
    text_content: String,
    /// Accumulated reasoning/thinking content
    reasoning_content: String,
    /// Accumulated thinking signature for multi-turn verification
    signature: String,
    /// Tracks tool calls: (anthropic_index, tool_id, tool_name, arguments)
    tool_calls: Vec<(usize, String, String, String)>,
    /// Tracks thinking block indices (by Anthropic index)
    thinking_block_indices: Vec<usize>,
    /// Buffer for incomplete SSE data
    buffer: String,
    /// Whether we've emitted the response.created event
    emitted_response_created: bool,
    /// Whether we've emitted the output_item.added for message
    emitted_message_added: bool,
    /// Whether we've emitted the content_part.added for text
    emitted_content_part_added: bool,
    /// Whether we've emitted the output_item.added for reasoning
    emitted_reasoning_added: bool,
    /// Error state for buffer overflow
    buffer_overflow: bool,
    /// Stop reason from Anthropic
    stop_reason: Option<String>,
    /// Sequence number for Responses API events
    sequence_number: i32,
}

/// Stream transformer that converts Anthropic SSE to OpenAI Responses API SSE format
pub struct AnthropicToResponsesStream<S> {
    inner: S,
    state: ResponsesStreamState,
    /// Output buffer for generated SSE chunks
    output_buffer: Vec<Bytes>,
    /// Maximum input buffer size in bytes
    max_input_buffer_bytes: usize,
    /// Maximum output buffer chunks
    max_output_buffer_chunks: usize,
}

impl<S> AnthropicToResponsesStream<S> {
    pub fn new(inner: S, streaming_buffer: &StreamingBufferConfig) -> Self {
        Self {
            inner,
            state: ResponsesStreamState::default(),
            output_buffer: Vec::new(),
            max_input_buffer_bytes: streaming_buffer.max_input_buffer_bytes,
            max_output_buffer_chunks: streaming_buffer.max_output_buffer_chunks,
        }
    }

    fn created_timestamp() -> f64 {
        chrono::Utc::now().timestamp() as f64
    }

    fn next_sequence(&mut self) -> i32 {
        let seq = self.state.sequence_number;
        self.state.sequence_number += 1;
        seq
    }

    /// Calculate the output index for message (accounting for reasoning if present)
    fn message_output_index(&self) -> usize {
        if self.state.emitted_reasoning_added {
            1 // Message comes after reasoning
        } else {
            0
        }
    }

    /// Calculate the output index for a tool call (accounting for reasoning and message)
    fn tool_output_index(&self, tool_index: usize) -> usize {
        let mut base = tool_index;
        if self.state.emitted_reasoning_added {
            base += 1;
        }
        if self.state.emitted_message_added {
            base += 1;
        }
        base
    }

    /// Parse an Anthropic SSE line and generate Responses API SSE chunks
    fn process_sse_line(&mut self, line: &str) {
        // Handle event type lines (we mostly ignore these)
        if line.starts_with("event:") {
            return;
        }

        // Handle data lines
        if let Some(json_str) = line.strip_prefix("data: ") {
            let json_str = json_str.trim();
            if json_str.is_empty() {
                return;
            }

            // Skip [DONE] marker (OpenAI format, not used by Anthropic but may appear in fixtures)
            if json_str == "[DONE]" {
                return;
            }

            // Parse Anthropic event
            match serde_json::from_str::<AnthropicStreamEvent>(json_str) {
                Ok(event) => self.handle_event(event),
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse Anthropic SSE event for Responses API: {}, json: {}",
                        e,
                        json_str
                    );
                }
            }
        }
    }

    fn handle_event(&mut self, event: AnthropicStreamEvent) {
        match event {
            AnthropicStreamEvent::MessageStart { message } => {
                self.state.response_id = message.id.clone();
                self.state.message_id = format!(
                    "msg_{}",
                    &message.id[4..].chars().take(24).collect::<String>()
                );
                self.state.model = message.model;
                if let Some(usage) = message.usage {
                    self.state.input_tokens = usage.input_tokens;
                    self.state.cache_read_input_tokens = usage.cache_read_input_tokens;
                    self.state.cache_creation_input_tokens = usage.cache_creation_input_tokens;
                }

                // Emit response.created
                if !self.state.emitted_response_created {
                    self.state.emitted_response_created = true;
                    self.emit_event(
                        "response.created",
                        serde_json::json!({
                            "response": {
                                "id": self.state.response_id,
                                "object": "response",
                                "created_at": Self::created_timestamp(),
                                "model": self.state.model,
                                "status": "in_progress",
                                "output": []
                            }
                        }),
                    );
                }
            }

            AnthropicStreamEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                match content_block {
                    StreamContentBlockType::Text { .. } => {
                        // Emit output_item.added for the message (only once)
                        if !self.state.emitted_message_added {
                            self.state.emitted_message_added = true;
                            let msg_output_index = self.message_output_index();
                            self.emit_event(
                                "response.output_item.added",
                                serde_json::json!({
                                    "output_index": msg_output_index,
                                    "item": {
                                        "type": "message",
                                        "id": self.state.message_id,
                                        "role": "assistant",
                                        "content": [],
                                        "status": "in_progress"
                                    }
                                }),
                            );
                        }

                        // Emit content_part.added for text (only once)
                        if !self.state.emitted_content_part_added {
                            self.state.emitted_content_part_added = true;
                            let msg_output_index = self.message_output_index();
                            self.emit_event(
                                "response.content_part.added",
                                serde_json::json!({
                                    "item_id": self.state.message_id,
                                    "output_index": msg_output_index,
                                    "content_index": 0,
                                    "part": {
                                        "type": "output_text",
                                        "text": "",
                                        "annotations": [],
                                        "logprobs": []
                                    }
                                }),
                            );
                        }
                    }
                    StreamContentBlockType::ToolUse { id, name } => {
                        // Track this tool call
                        let tool_index = self.state.tool_calls.len();
                        self.state.tool_calls.push((
                            index,
                            id.clone(),
                            name.clone(),
                            String::new(),
                        ));

                        // Calculate output index (after reasoning and message if present)
                        let output_index = self.tool_output_index(tool_index);

                        // Emit output_item.added for the function call
                        self.emit_event(
                            "response.output_item.added",
                            serde_json::json!({
                                "output_index": output_index,
                                "item": {
                                    "type": "function_call",
                                    "id": format!("fc_{}", &id[6..].chars().take(24).collect::<String>()),
                                    "call_id": id,
                                    "name": name,
                                    "arguments": "",
                                    "status": "in_progress"
                                }
                            }),
                        );
                    }
                    StreamContentBlockType::Thinking { .. } => {
                        // Track this as a thinking block
                        self.state.thinking_block_indices.push(index);

                        // Emit output_item.added for reasoning (only once)
                        if !self.state.emitted_reasoning_added {
                            self.state.emitted_reasoning_added = true;
                            // Reasoning output comes before message in output array
                            self.emit_event(
                                "response.output_item.added",
                                serde_json::json!({
                                    "output_index": 0,
                                    "item": {
                                        "type": "reasoning",
                                        "id": format!("rs_{}", &self.state.response_id[4..].chars().take(24).collect::<String>()),
                                        "summary": []
                                    }
                                }),
                            );
                        }
                    }
                }
            }

            AnthropicStreamEvent::ContentBlockDelta { index, delta } => match delta {
                ContentDelta::TextDelta { text } => {
                    self.state.text_content.push_str(&text);

                    // Emit text delta
                    let msg_output_index = self.message_output_index();
                    self.emit_event(
                        "response.output_text.delta",
                        serde_json::json!({
                            "item_id": self.state.message_id,
                            "output_index": msg_output_index,
                            "content_index": 0,
                            "delta": text
                        }),
                    );
                }
                ContentDelta::InputJsonDelta { partial_json } => {
                    // Find the tool call by anthropic index and extract needed info
                    let tool_info: Option<(usize, String)> = {
                        let pos = self
                            .state
                            .tool_calls
                            .iter()
                            .position(|(anthropic_idx, _, _, _)| *anthropic_idx == index);
                        if let Some(tool_index) = pos {
                            let tool_id = self.state.tool_calls[tool_index].1.clone();
                            Some((tool_index, tool_id))
                        } else {
                            None
                        }
                    };

                    if let Some((tool_index, tool_id)) = tool_info {
                        // Update the arguments
                        self.state.tool_calls[tool_index].3.push_str(&partial_json);

                        let output_index = self.tool_output_index(tool_index);

                        // Emit function call arguments delta
                        let fc_id =
                            format!("fc_{}", &tool_id[6..].chars().take(24).collect::<String>());
                        self.emit_event(
                            "response.function_call_arguments.delta",
                            serde_json::json!({
                                "item_id": fc_id,
                                "output_index": output_index,
                                "delta": partial_json
                            }),
                        );
                    }
                }
                ContentDelta::ThinkingDelta { thinking } => {
                    // Emit thinking delta as reasoning content
                    if self.state.thinking_block_indices.contains(&index) {
                        self.state.reasoning_content.push_str(&thinking);

                        // Emit reasoning summary delta
                        let reasoning_id = format!(
                            "rs_{}",
                            &self.state.response_id[4..]
                                .chars()
                                .take(24)
                                .collect::<String>()
                        );
                        self.emit_event(
                            "response.reasoning_summary_text.delta",
                            serde_json::json!({
                                "item_id": reasoning_id,
                                "output_index": 0,
                                "summary_index": 0,
                                "delta": thinking
                            }),
                        );
                    }
                }
                ContentDelta::SignatureDelta { signature } => {
                    // Accumulate signature for multi-turn reasoning verification
                    self.state.signature.push_str(&signature);
                }
            },

            AnthropicStreamEvent::ContentBlockStop {} => {
                // Nothing special needed on content block stop
            }

            AnthropicStreamEvent::MessageDelta { delta, usage } => {
                if let Some(u) = usage {
                    self.state.output_tokens = u.output_tokens;
                }
                self.state.stop_reason = delta.stop_reason;
            }

            AnthropicStreamEvent::MessageStop => {
                // Emit completion events

                // Emit reasoning done if we have reasoning content
                if self.state.emitted_reasoning_added {
                    let reasoning_id = format!(
                        "rs_{}",
                        &self.state.response_id[4..]
                            .chars()
                            .take(24)
                            .collect::<String>()
                    );

                    // Emit reasoning summary done
                    self.emit_event(
                        "response.reasoning_summary_text.done",
                        serde_json::json!({
                            "item_id": reasoning_id,
                            "output_index": 0,
                            "summary_index": 0,
                            "text": self.state.reasoning_content
                        }),
                    );

                    // Emit reasoning output_item.done (include signature for multi-turn)
                    let mut reasoning_item = serde_json::json!({
                        "type": "reasoning",
                        "id": reasoning_id,
                        "summary": [{
                            "type": "summary_text",
                            "text": self.state.reasoning_content
                        }]
                    });
                    if !self.state.signature.is_empty() {
                        reasoning_item["signature"] =
                            serde_json::Value::String(self.state.signature.clone());
                    }
                    self.emit_event(
                        "response.output_item.done",
                        serde_json::json!({
                            "output_index": 0,
                            "item": reasoning_item
                        }),
                    );
                }

                // Emit text done if we have text content
                let msg_output_index = self.message_output_index();
                if !self.state.text_content.is_empty() {
                    self.emit_event(
                        "response.output_text.done",
                        serde_json::json!({
                            "item_id": self.state.message_id,
                            "output_index": msg_output_index,
                            "content_index": 0,
                            "text": self.state.text_content
                        }),
                    );

                    // Emit content_part.done
                    self.emit_event(
                        "response.content_part.done",
                        serde_json::json!({
                            "item_id": self.state.message_id,
                            "output_index": msg_output_index,
                            "content_index": 0,
                            "part": {
                                "type": "output_text",
                                "text": self.state.text_content,
                                "annotations": [],
                                "logprobs": []
                            }
                        }),
                    );
                }

                // Emit output_item.done for message
                if self.state.emitted_message_added {
                    self.emit_event(
                        "response.output_item.done",
                        serde_json::json!({
                            "output_index": msg_output_index,
                            "item": {
                                "type": "message",
                                "id": self.state.message_id,
                                "role": "assistant",
                                "status": "completed",
                                "content": [{
                                    "type": "output_text",
                                    "text": self.state.text_content,
                                    "annotations": [],
                                    "logprobs": []
                                }]
                            }
                        }),
                    );
                }

                // Emit function_call_arguments.done and output_item.done for each tool call
                // Clone data to avoid borrow issues
                let tool_calls: Vec<_> = self
                    .state
                    .tool_calls
                    .iter()
                    .enumerate()
                    .map(|(i, (_, tool_id, tool_name, arguments))| {
                        (i, tool_id.clone(), tool_name.clone(), arguments.clone())
                    })
                    .collect();

                for (i, tool_id, tool_name, arguments) in tool_calls {
                    let output_index = self.tool_output_index(i);
                    let fc_id =
                        format!("fc_{}", &tool_id[6..].chars().take(24).collect::<String>());

                    self.emit_event(
                        "response.function_call_arguments.done",
                        serde_json::json!({
                            "item_id": fc_id,
                            "output_index": output_index,
                            "arguments": arguments
                        }),
                    );

                    self.emit_event(
                        "response.output_item.done",
                        serde_json::json!({
                            "output_index": output_index,
                            "item": {
                                "type": "function_call",
                                "id": fc_id,
                                "call_id": tool_id,
                                "name": tool_name,
                                "arguments": arguments,
                                "status": "completed"
                            }
                        }),
                    );
                }

                // Build final output array
                let mut output = Vec::new();

                // Reasoning comes first (if present)
                if self.state.emitted_reasoning_added {
                    let reasoning_id = format!(
                        "rs_{}",
                        &self.state.response_id[4..]
                            .chars()
                            .take(24)
                            .collect::<String>()
                    );
                    let mut reasoning_item = serde_json::json!({
                        "type": "reasoning",
                        "id": reasoning_id,
                        "summary": [{
                            "type": "summary_text",
                            "text": self.state.reasoning_content
                        }]
                    });
                    if !self.state.signature.is_empty() {
                        reasoning_item["signature"] =
                            serde_json::Value::String(self.state.signature.clone());
                    }
                    output.push(reasoning_item);
                }

                // Message comes after reasoning
                if self.state.emitted_message_added {
                    output.push(serde_json::json!({
                        "type": "message",
                        "id": self.state.message_id,
                        "role": "assistant",
                        "status": "completed",
                        "content": [{
                            "type": "output_text",
                            "text": self.state.text_content,
                            "annotations": [],
                            "logprobs": []
                        }]
                    }));
                }

                // Tool calls come last
                for (_, tool_id, tool_name, arguments) in &self.state.tool_calls {
                    let fc_id =
                        format!("fc_{}", &tool_id[6..].chars().take(24).collect::<String>());
                    output.push(serde_json::json!({
                        "type": "function_call",
                        "id": fc_id,
                        "call_id": tool_id,
                        "name": tool_name,
                        "arguments": arguments,
                        "status": "completed"
                    }));
                }

                // Determine status
                let status = match self.state.stop_reason.as_deref() {
                    Some("max_tokens") => "incomplete",
                    _ => "completed",
                };

                // Emit response.completed
                self.emit_event(
                    "response.completed",
                    serde_json::json!({
                        "response": {
                            "id": self.state.response_id,
                            "object": "response",
                            "created_at": Self::created_timestamp(),
                            "model": self.state.model,
                            "status": status,
                            "output": output,
                            "usage": {
                                "input_tokens": self.state.input_tokens,
                                "input_tokens_details": { "cached_tokens": self.state.cache_read_input_tokens },
                                "output_tokens": self.state.output_tokens,
                                "output_tokens_details": { "reasoning_tokens": 0 },
                                "total_tokens": self.state.input_tokens + self.state.output_tokens
                            }
                        }
                    }),
                );

                // Emit [DONE] to signal end of stream (OpenAI Responses API convention)
                self.output_buffer.push(Bytes::from("data: [DONE]\n\n"));
            }

            AnthropicStreamEvent::Ping => {
                // Ignore ping events
            }

            AnthropicStreamEvent::Error { error } => {
                tracing::error!(
                    "Anthropic streaming error: {} - {}",
                    error.error_type,
                    error.message
                );
                // Emit error event
                self.emit_event(
                    "error",
                    serde_json::json!({
                        "type": "server_error",
                        "message": error.message
                    }),
                );
            }
        }
    }

    fn emit_event(&mut self, event_type: &str, data: serde_json::Value) {
        let seq = self.next_sequence();
        let event = serde_json::json!({
            "type": event_type,
            "sequence_number": seq,
        });
        // Merge the data into the event
        let mut event_obj = event.as_object().unwrap().clone();
        if let serde_json::Value::Object(data_obj) = data {
            for (k, v) in data_obj {
                event_obj.insert(k, v);
            }
        }
        // Add logprobs for text-related events (required per OpenAI spec)
        if (event_type == "response.output_text.delta" || event_type == "response.output_text.done")
            && !event_obj.contains_key("logprobs")
        {
            event_obj.insert("logprobs".to_string(), serde_json::Value::Array(vec![]));
        }
        if let Ok(json) = serde_json::to_string(&serde_json::Value::Object(event_obj)) {
            let sse = format!("data: {}\n\n", json);
            self.output_buffer.push(Bytes::from(sse));
        }
    }

    /// Process a chunk of bytes, potentially containing multiple SSE events
    fn process_bytes(&mut self, bytes: &[u8]) {
        // Check if we're already in error state
        if self.state.buffer_overflow {
            return;
        }

        if let Ok(text) = std::str::from_utf8(bytes) {
            // Check input buffer limit before adding
            if self.state.buffer.len() + text.len() > self.max_input_buffer_bytes {
                tracing::error!(
                    buffer_size = self.state.buffer.len(),
                    incoming_size = text.len(),
                    max_size = self.max_input_buffer_bytes,
                    "Responses API SSE input buffer overflow"
                );
                self.state.buffer_overflow = true;
                return;
            }

            self.state.buffer.push_str(text);

            // Process complete lines
            while let Some(pos) = self.state.buffer.find('\n') {
                let trimmed = self.state.buffer[..pos].trim();
                if !trimmed.is_empty() {
                    let line = trimmed.to_string();
                    self.process_sse_line(&line);
                }
                self.state.buffer.drain(..=pos);

                // Check output buffer limit
                if self.output_buffer.len() > self.max_output_buffer_chunks {
                    tracing::error!(
                        buffer_size = self.output_buffer.len(),
                        max_size = self.max_output_buffer_chunks,
                        "Responses API SSE output buffer overflow"
                    );
                    self.state.buffer_overflow = true;
                    return;
                }
            }
        }
    }
}

impl<S> Stream for AnthropicToResponsesStream<S>
where
    S: Stream<Item = Result<Bytes, io::Error>> + Unpin,
{
    type Item = Result<Bytes, io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Check for buffer overflow error
        if self.state.buffer_overflow {
            return Poll::Ready(Some(Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "SSE buffer overflow",
            ))));
        }

        // First, return any buffered output
        if !self.output_buffer.is_empty() {
            return Poll::Ready(Some(Ok(self.output_buffer.remove(0))));
        }

        // Poll the inner stream
        let inner = Pin::new(&mut self.inner);
        match inner.poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                // Process the Anthropic SSE bytes
                self.process_bytes(&bytes);

                // Check for buffer overflow after processing
                if self.state.buffer_overflow {
                    return Poll::Ready(Some(Err(io::Error::new(
                        io::ErrorKind::OutOfMemory,
                        "SSE buffer overflow",
                    ))));
                }

                // Return buffered output or wake for more
                if !self.output_buffer.is_empty() {
                    Poll::Ready(Some(Ok(self.output_buffer.remove(0))))
                } else {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => {
                // Stream ended - flush any remaining buffer
                if !self.output_buffer.is_empty() {
                    Poll::Ready(Some(Ok(self.output_buffer.remove(0))))
                } else {
                    Poll::Ready(None)
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_message_start() {
        let json = r#"{"type":"message_start","message":{"id":"msg_123","model":"claude-sonnet-4-5-20250929","usage":{"input_tokens":25,"output_tokens":1}}}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).unwrap();

        match event {
            AnthropicStreamEvent::MessageStart { message } => {
                assert_eq!(message.id, "msg_123");
                assert_eq!(message.model, "claude-sonnet-4-5-20250929");
                assert_eq!(message.usage.unwrap().input_tokens, 25);
            }
            _ => panic!("Expected MessageStart"),
        }
    }

    #[test]
    fn test_parse_content_block_delta() {
        let json = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).unwrap();

        match event {
            AnthropicStreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(index, 0);
                match delta {
                    ContentDelta::TextDelta { text } => assert_eq!(text, "Hello"),
                    _ => panic!("Expected TextDelta"),
                }
            }
            _ => panic!("Expected ContentBlockDelta"),
        }
    }

    #[test]
    fn test_parse_tool_use_start() {
        let json = r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_123","name":"get_weather"}}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).unwrap();

        match event {
            AnthropicStreamEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                assert_eq!(index, 1);
                match content_block {
                    StreamContentBlockType::ToolUse { id, name } => {
                        assert_eq!(id, "toolu_123");
                        assert_eq!(name, "get_weather");
                    }
                    _ => panic!("Expected ToolUse"),
                }
            }
            _ => panic!("Expected ContentBlockStart"),
        }
    }

    #[test]
    fn test_parse_message_delta_with_stop() {
        let json = r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":42}}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).unwrap();

        match event {
            AnthropicStreamEvent::MessageDelta { delta, usage } => {
                assert_eq!(delta.stop_reason, Some("end_turn".to_string()));
                assert_eq!(usage.unwrap().output_tokens, 42);
            }
            _ => panic!("Expected MessageDelta"),
        }
    }

    #[test]
    fn test_parse_message_stop() {
        let json = r#"{"type":"message_stop"}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(event, AnthropicStreamEvent::MessageStop));
    }

    #[test]
    fn test_parse_ping() {
        let json = r#"{"type":"ping"}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(event, AnthropicStreamEvent::Ping));
    }

    #[test]
    fn test_parse_thinking_block_start() {
        let json = r#"{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).unwrap();

        match event {
            AnthropicStreamEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                assert_eq!(index, 0);
                assert!(matches!(
                    content_block,
                    StreamContentBlockType::Thinking { .. }
                ));
            }
            _ => panic!("Expected ContentBlockStart"),
        }
    }

    #[test]
    fn test_parse_thinking_delta() {
        let json = r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Let me think about this..."}}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).unwrap();

        match event {
            AnthropicStreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(index, 0);
                match delta {
                    ContentDelta::ThinkingDelta { thinking } => {
                        assert_eq!(thinking, "Let me think about this...");
                    }
                    _ => panic!("Expected ThinkingDelta"),
                }
            }
            _ => panic!("Expected ContentBlockDelta"),
        }
    }

    #[test]
    fn test_parse_signature_delta() {
        let json = r#"{"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"abc123..."}}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).unwrap();

        match event {
            AnthropicStreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(index, 0);
                assert!(matches!(delta, ContentDelta::SignatureDelta { .. }));
            }
            _ => panic!("Expected ContentBlockDelta"),
        }
    }

    #[test]
    fn test_openai_delta_with_reasoning() {
        let delta = OpenAIDelta {
            role: None,
            content: None,
            tool_calls: None,
            reasoning: Some("thinking content".to_string()),
        };

        let json = serde_json::to_string(&delta).unwrap();
        assert!(json.contains(r#""reasoning":"thinking content""#));
        assert!(!json.contains(r#""content""#)); // content is None, should be skipped
    }

    #[test]
    fn test_openai_delta_without_reasoning() {
        let delta = OpenAIDelta {
            role: Some("assistant"),
            content: Some("Hello".to_string()),
            tool_calls: None,
            reasoning: None,
        };

        let json = serde_json::to_string(&delta).unwrap();
        assert!(json.contains(r#""content":"Hello""#));
        assert!(!json.contains(r#"reasoning"#)); // reasoning is None, should be skipped
    }
}
