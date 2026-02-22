//! Fixture recording and validation tool for provider testing.
//!
//! This tool manages test fixtures for LLM provider testing:
//! - Record real API responses as JSON fixtures
//! - Validate that all fixture definitions have corresponding files
//! - Scaffold fixture definitions for new providers
//!
//! # Usage
//!
//! ```bash
//! # List available fixtures
//! cargo run --bin record_fixtures -- list
//! cargo run --bin record_fixtures -- list --provider openai --show-enum
//!
//! # Record fixtures (requires API key environment variable)
//! cargo run --bin record_fixtures -- record --provider openai
//! cargo run --bin record_fixtures -- record --provider openai --fixture chat_completion
//!
//! # Validate fixtures (check files exist, find orphans)
//! cargo run --bin record_fixtures -- validate
//! cargo run --bin record_fixtures -- validate --check-orphans
//!
//! # Scaffold fixture definitions for a new provider
//! cargo run --bin record_fixtures -- scaffold --provider anthropic --endpoint https://api.anthropic.com/v1
//! ```
//!
//! # Adding a New Provider
//!
//! 1. Run `scaffold` to generate template fixture definitions
//! 2. Add the definitions to `get_fixture_definitions()` in this file
//! 3. Add `FixtureId` variants to `src/providers/test_utils.rs`
//! 4. Add `ProviderTestSpec` to `src/tests/provider_e2e.rs`
//! 5. Set the API key and run `record --provider <name>`
//! 6. Run `validate` to verify all fixtures exist

use std::{collections::HashMap, fs, path::PathBuf};

#[cfg(feature = "provider-bedrock")]
use aws_credential_types::Credentials;
#[cfg(feature = "provider-bedrock")]
use aws_sigv4::{
    http_request::{SignableBody, SignableRequest, SigningSettings},
    sign::v4::SigningParams,
};
#[cfg(feature = "provider-bedrock")]
use aws_smithy_eventstream::frame::{DecodedFrame, MessageFrameDecoder};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Parser)]
#[command(name = "record_fixtures")]
#[command(about = "Record real API responses as test fixtures")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Record fixtures from a provider
    Record {
        /// Provider to record from (openai, anthropic, etc.)
        #[arg(short, long)]
        provider: String,

        /// Specific fixture to record (optional, records all if not specified)
        #[arg(short, long)]
        fixture: Option<String>,

        /// Output directory for fixtures
        #[arg(short, long, default_value = "tests/fixtures/providers")]
        output: PathBuf,
    },
    /// List available fixture definitions
    List {
        /// Provider to list fixtures for (optional, lists all if not specified)
        #[arg(short, long)]
        provider: Option<String>,

        /// Show the corresponding FixtureId enum variant names
        #[arg(long)]
        show_enum: bool,
    },
    /// Validate that all fixture definitions have corresponding files
    Validate {
        /// Fixtures directory to validate
        #[arg(short, long, default_value = "tests/fixtures/providers")]
        fixtures_dir: PathBuf,

        /// Also check for orphan files (files without definitions)
        #[arg(long)]
        check_orphans: bool,
    },
    /// Generate fixture definition templates for a new provider
    Scaffold {
        /// Provider name (e.g., "anthropic", "bedrock")
        #[arg(short, long)]
        provider: String,

        /// Base URL for the provider API
        #[arg(short, long)]
        endpoint: String,

        /// Output format (rust or json)
        #[arg(long, default_value = "rust")]
        format: String,
    },
}

/// Fixture definition for recording
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FixtureDefinition {
    id: String,
    description: String,
    provider: String,
    endpoint: String,
    method: String,
    path: String,
    request_body: Option<Value>,
    #[serde(default)]
    streaming: bool,
    /// If true, use invalid credentials to trigger auth errors (401/403)
    #[serde(default)]
    use_invalid_credentials: bool,
}

/// Recorded fixture format (matches what tests expect)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecordedFixture {
    id: String,
    description: String,
    request: FixtureRequest,
    response: FixtureResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FixtureRequest {
    method: String,
    path: String,
    /// Optional regex pattern for path matching (used when path contains dynamic segments like model IDs)
    #[serde(skip_serializing_if = "Option::is_none")]
    path_pattern: Option<String>,
}

/// Streaming format for fixtures
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StreamingFormat {
    #[default]
    Sse,
    AwsEventstream,
}

fn is_default_streaming_format(format: &StreamingFormat) -> bool {
    *format == StreamingFormat::Sse
}

/// AWS EventStream event for Bedrock streaming fixtures
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EventStreamEvent {
    event_type: String,
    payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FixtureResponse {
    status: u16,
    headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<Value>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    streaming: bool,
    /// Streaming format: "sse" (default) or "aws_eventstream"
    #[serde(default, skip_serializing_if = "is_default_streaming_format")]
    streaming_format: StreamingFormat,
    /// SSE chunks (for streaming_format: sse)
    #[serde(skip_serializing_if = "Option::is_none")]
    chunks: Option<Vec<Value>>,
    /// AWS EventStream events (for streaming_format: aws_eventstream)
    #[serde(skip_serializing_if = "Option::is_none")]
    events: Option<Vec<EventStreamEvent>>,
}

/// Deterministic prompts for reproducible fixture recording.
/// These prompts ask for specific, predictable content to minimize variation between recordings.
mod prompts {
    /// Chat/Responses API: Ask for a specific sequence that's verifiable
    pub const CHAT_DETERMINISTIC: &str =
        "List the first 10 prime numbers, one per line, with no additional text.";

    /// Chat/Responses API streaming: Same prompt for consistency
    pub const CHAT_STREAMING_DETERMINISTIC: &str =
        "Count from 1 to 20, one number per line, with no additional text.";

    /// Completions API: A prompt that leads to predictable continuation
    pub const COMPLETION_DETERMINISTIC: &str = "The first 10 letters of the English alphabet are: a, b, c, d, e, f, g, h, i, j.\n\nThe first 10 numbers are: 1, 2, 3,";

    /// Completions API streaming: Same approach
    pub const COMPLETION_STREAMING_DETERMINISTIC: &str =
        "Complete this sequence with the next 15 numbers:\n1, 2, 3, 4, 5,";

    /// Tool calling: Single tool call prompt
    pub const TOOL_CALL_SINGLE: &str = "What is the current weather in San Francisco?";

    /// Tool calling: Parallel tool calls prompt
    pub const TOOL_CALL_PARALLEL: &str = "What is the weather in Tokyo and London?";

    /// Reasoning models: A problem that benefits from step-by-step reasoning
    pub const REASONING_DETERMINISTIC: &str =
        "What is the 15th prime number? Think through this step by step.";

    /// Responses API tool calling: Single tool call prompt
    pub const RESPONSES_TOOL_CALL_SINGLE: &str = "What is the current weather in San Francisco?";

    /// Responses API tool calling: Parallel tool calls prompt
    pub const RESPONSES_TOOL_CALL_PARALLEL: &str = "What is the weather in Tokyo and London?";

    /// Responses API reasoning: A problem that benefits from step-by-step reasoning
    pub const RESPONSES_REASONING_DETERMINISTIC: &str =
        "What is the 15th prime number? Think through this step by step.";

    /// Vision: Prompt for describing an image (deterministic request for specific details)
    pub const VISION_DESCRIBE: &str = "Describe this image in exactly 3 bullet points. Focus on: 1) main subject, 2) colors, 3) composition.";

    /// Responses API Vision: Same as VISION_DESCRIBE but for Responses API
    pub const RESPONSES_VISION_DESCRIBE: &str = "Describe this image in exactly 3 bullet points. Focus on: 1) main subject, 2) colors, 3) composition.";
}

/// Standard weather tool definition for Chat Completions API tool calling fixtures.
fn weather_tool() -> serde_json::Value {
    json!({
        "type": "function",
        "function": {
            "name": "get_weather",
            "description": "Get the current weather in a location",
            "parameters": {
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "The city and country, e.g. San Francisco, USA"
                    },
                    "unit": {
                        "type": "string",
                        "enum": ["celsius", "fahrenheit"],
                        "description": "The temperature unit"
                    }
                },
                "required": ["location"]
            }
        }
    })
}

/// Weather tool definition for Anthropic Messages API.
/// Anthropic uses a different format: name/description/input_schema at top level.
fn anthropic_weather_tool() -> serde_json::Value {
    json!({
        "name": "get_weather",
        "description": "Get the current weather in a location",
        "input_schema": {
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "The city and country, e.g. San Francisco, USA"
                },
                "unit": {
                    "type": "string",
                    "enum": ["celsius", "fahrenheit"],
                    "description": "The temperature unit"
                }
            },
            "required": ["location"]
        }
    })
}

/// Weather tool definition for Responses API (uses different format).
fn responses_weather_tool() -> serde_json::Value {
    json!({
        "type": "function",
        "name": "get_weather",
        "description": "Get the current weather in a location",
        "parameters": {
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "The city and country, e.g. San Francisco, USA"
                },
                "unit": {
                    "type": "string",
                    "enum": ["celsius", "fahrenheit"],
                    "description": "The temperature unit"
                }
            },
            "required": ["location"]
        }
    })
}

fn get_fixture_definitions() -> Vec<FixtureDefinition> {
    vec![
        // OpenAI Chat Completion
        FixtureDefinition {
            id: "openai:chat_completion:success".into(),
            description: "Successful non-streaming chat completion response".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "gpt-4o-mini",
                "messages": [{"role": "user", "content": prompts::CHAT_DETERMINISTIC}],
                "max_tokens": 100,
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "openai:chat_completion:streaming".into(),
            description: "Streaming chat completion response with SSE chunks".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "gpt-4o-mini",
                "messages": [{"role": "user", "content": prompts::CHAT_STREAMING_DETERMINISTIC}],
                "max_tokens": 100,
                "temperature": 0,
                "stream": true
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        // OpenAI Embeddings
        FixtureDefinition {
            id: "openai:embedding:success".into(),
            description: "Successful embedding response".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/embeddings".into(),
            request_body: Some(json!({
                "model": "text-embedding-3-small",
                "input": "The quick brown fox jumps over the lazy dog."
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        // OpenAI Models
        FixtureDefinition {
            id: "openai:models:list".into(),
            description: "List models response".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "GET".into(),
            path: "/models".into(),
            request_body: None,
            streaming: false,
            use_invalid_credentials: false,
        },
        // OpenAI Responses API
        FixtureDefinition {
            id: "openai:responses:success".into(),
            description: "Successful non-streaming Responses API response".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/responses".into(),
            request_body: Some(json!({
                "model": "gpt-4o-mini",
                "input": prompts::CHAT_DETERMINISTIC,
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "openai:responses:streaming".into(),
            description: "Streaming Responses API response with SSE events".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/responses".into(),
            request_body: Some(json!({
                "model": "gpt-4o-mini",
                "input": prompts::CHAT_STREAMING_DETERMINISTIC,
                "temperature": 0,
                "stream": true
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        // OpenAI Completions API
        FixtureDefinition {
            id: "openai:completion:success".into(),
            description: "Successful non-streaming text completion response".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/completions".into(),
            request_body: Some(json!({
                "model": "gpt-3.5-turbo-instruct",
                "prompt": prompts::COMPLETION_DETERMINISTIC,
                "max_tokens": 100,
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "openai:completion:streaming".into(),
            description: "Streaming text completion response with SSE chunks".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/completions".into(),
            request_body: Some(json!({
                "model": "gpt-3.5-turbo-instruct",
                "prompt": prompts::COMPLETION_STREAMING_DETERMINISTIC,
                "max_tokens": 100,
                "temperature": 0,
                "stream": true
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        // OpenAI Tool Calling
        FixtureDefinition {
            id: "openai:tool_call:success".into(),
            description: "Chat completion with single tool call response".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "gpt-4o-mini",
                "messages": [{"role": "user", "content": prompts::TOOL_CALL_SINGLE}],
                "tools": [weather_tool()],
                "tool_choice": "auto",
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "openai:tool_call:streaming".into(),
            description: "Streaming chat completion with tool call response".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "gpt-4o-mini",
                "messages": [{"role": "user", "content": prompts::TOOL_CALL_SINGLE}],
                "tools": [weather_tool()],
                "tool_choice": "auto",
                "temperature": 0,
                "stream": true
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "openai:tool_call:parallel".into(),
            description: "Chat completion with parallel tool calls response".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "gpt-4o-mini",
                "messages": [{"role": "user", "content": prompts::TOOL_CALL_PARALLEL}],
                "tools": [weather_tool()],
                "tool_choice": "auto",
                "parallel_tool_calls": true,
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "openai:tool_call:with_result".into(),
            description: "Chat completion response after tool results are provided".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "gpt-4o-mini",
                "messages": [
                    {"role": "user", "content": prompts::TOOL_CALL_SINGLE},
                    {
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [{
                            "id": "call_recorded_001",
                            "type": "function",
                            "function": {
                                "name": "get_weather",
                                "arguments": "{\"location\":\"San Francisco\",\"unit\":\"celsius\"}"
                            }
                        }]
                    },
                    {
                        "role": "tool",
                        "tool_call_id": "call_recorded_001",
                        "content": "{\"temperature\": 18, \"condition\": \"partly cloudy\", \"humidity\": 65}"
                    }
                ],
                "tools": [weather_tool()],
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        // OpenAI Reasoning Models (o3-mini, o1, etc.)
        FixtureDefinition {
            id: "openai:reasoning:success".into(),
            description:
                "Chat completion from o3-mini reasoning model with reasoning_tokens in usage".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "o3-mini",
                "messages": [{"role": "user", "content": prompts::REASONING_DETERMINISTIC}],
                "max_completion_tokens": 2000
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "openai:reasoning:streaming".into(),
            description:
                "Streaming chat completion from o3-mini reasoning model with reasoning_tokens"
                    .into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "o3-mini",
                "messages": [{"role": "user", "content": prompts::REASONING_DETERMINISTIC}],
                "max_completion_tokens": 2000,
                "stream": true,
                "stream_options": {"include_usage": true}
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        // OpenAI Responses API - Tool Calling
        FixtureDefinition {
            id: "openai:responses_tool_call:success".into(),
            description: "Responses API with single tool call (function_call output)".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/responses".into(),
            request_body: Some(json!({
                "model": "gpt-4o-mini",
                "input": prompts::RESPONSES_TOOL_CALL_SINGLE,
                "tools": [responses_weather_tool()],
                "tool_choice": "auto",
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "openai:responses_tool_call:streaming".into(),
            description: "Streaming Responses API with tool call".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/responses".into(),
            request_body: Some(json!({
                "model": "gpt-4o-mini",
                "input": prompts::RESPONSES_TOOL_CALL_SINGLE,
                "tools": [responses_weather_tool()],
                "tool_choice": "auto",
                "temperature": 0,
                "stream": true
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "openai:responses_tool_call:parallel".into(),
            description: "Responses API with parallel tool calls".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/responses".into(),
            request_body: Some(json!({
                "model": "gpt-4o-mini",
                "input": prompts::RESPONSES_TOOL_CALL_PARALLEL,
                "tools": [responses_weather_tool()],
                "tool_choice": "auto",
                "parallel_tool_calls": true,
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "openai:responses_tool_call:with_result".into(),
            description: "Responses API response after function_call_output is provided".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/responses".into(),
            request_body: Some(json!({
                "model": "gpt-4o-mini",
                "input": [
                    {"type": "message", "role": "user", "content": prompts::RESPONSES_TOOL_CALL_SINGLE},
                    {
                        "type": "function_call",
                        "call_id": "call_responses_recorded_001",
                        "name": "get_weather",
                        "arguments": "{\"location\":\"San Francisco\",\"unit\":\"celsius\"}"
                    },
                    {
                        "type": "function_call_output",
                        "call_id": "call_responses_recorded_001",
                        "output": "{\"temperature\": 18, \"condition\": \"partly cloudy\", \"humidity\": 65}"
                    }
                ],
                "tools": [responses_weather_tool()],
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        // OpenAI Responses API - Reasoning Models (o3-mini, o1)
        FixtureDefinition {
            id: "openai:responses_reasoning:success".into(),
            description: "Responses API with o3-mini reasoning model (reasoning_tokens in usage)"
                .into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/responses".into(),
            request_body: Some(json!({
                "model": "o3-mini",
                "input": prompts::RESPONSES_REASONING_DETERMINISTIC,
                "reasoning": {
                    "effort": "medium"
                }
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "openai:responses_reasoning:streaming".into(),
            description: "Streaming Responses API with o3-mini reasoning model".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/responses".into(),
            request_body: Some(json!({
                "model": "o3-mini",
                "input": prompts::RESPONSES_REASONING_DETERMINISTIC,
                "reasoning": {
                    "effort": "medium"
                },
                "stream": true
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        // OpenAI Vision (Chat Completions with image input)
        // Note: These require a real image URL or base64 data to record.
        // The fixtures are created manually with a sample image for testing.
        FixtureDefinition {
            id: "openai:vision:success".into(),
            description: "Chat completion with base64-encoded image input".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "gpt-4o-mini",
                "messages": [{
                    "role": "user",
                    "content": [
                        {"type": "text", "text": prompts::VISION_DESCRIBE},
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
                                "detail": "low"
                            }
                        }
                    ]
                }],
                "max_tokens": 300,
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "openai:vision_url:success".into(),
            description: "Chat completion with image URL input".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "gpt-4o-mini",
                "messages": [{
                    "role": "user",
                    "content": [
                        {"type": "text", "text": prompts::VISION_DESCRIBE},
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": "https://upload.wikimedia.org/wikipedia/commons/thumb/4/47/PNG_transparency_demonstration_1.png/280px-PNG_transparency_demonstration_1.png",
                                "detail": "low"
                            }
                        }
                    ]
                }],
                "max_tokens": 300,
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        // OpenAI Responses API - Vision (image input)
        FixtureDefinition {
            id: "openai:responses_vision:success".into(),
            description: "Responses API with base64-encoded image input".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/responses".into(),
            request_body: Some(json!({
                "model": "gpt-4o-mini",
                "input": [{
                    "role": "user",
                    "content": [
                        {"type": "input_text", "text": prompts::RESPONSES_VISION_DESCRIBE},
                        {
                            "type": "input_image",
                            "detail": "auto",
                            "image_url": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="
                        }
                    ]
                }],
                "max_output_tokens": 300,
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "openai:responses_vision_url:success".into(),
            description: "Responses API with image URL input".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/responses".into(),
            request_body: Some(json!({
                "model": "gpt-4o-mini",
                "input": [{
                    "role": "user",
                    "content": [
                        {"type": "input_text", "text": prompts::RESPONSES_VISION_DESCRIBE},
                        {
                            "type": "input_image",
                            "detail": "low",
                            "image_url": "https://upload.wikimedia.org/wikipedia/commons/thumb/4/47/PNG_transparency_demonstration_1.png/280px-PNG_transparency_demonstration_1.png"
                        }
                    ]
                }],
                "max_output_tokens": 300,
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        // =========================================================================
        // OpenAI Error Responses (manually created, not recordable)
        // These exist for testing error handling - they're not recorded from real APIs
        // Note: These use a simple naming scheme (rate:limit instead of rate_limit:error)
        // to match existing file names like rate_limit.json
        // =========================================================================
        FixtureDefinition {
            id: "openai:rate:limit".into(),
            description: "Rate limit error response (429)".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: None, // Not recordable - manually created
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "openai:server:error".into(),
            description: "Server error response (500)".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: None,
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "openai:bad:request".into(),
            description: "Bad request error response (400)".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: None,
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "openai:unauthorized".into(),
            description: "Unauthorized error response (401)".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: None,
            streaming: false,
            use_invalid_credentials: true,
        },
        // =========================================================================
        // OpenRouter Fixtures (OpenAI-compatible API with cost in usage)
        // =========================================================================
        FixtureDefinition {
            id: "openrouter:chat_completion:success".into(),
            description: "Successful non-streaming chat completion via OpenRouter with cost".into(),
            provider: "openrouter".into(),
            endpoint: "https://openrouter.ai/api/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "openai/gpt-4o-mini",
                "messages": [{"role": "user", "content": prompts::CHAT_DETERMINISTIC}],
                "max_tokens": 100,
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "openrouter:chat_completion:streaming".into(),
            description: "Streaming chat completion via OpenRouter with cost in final chunk".into(),
            provider: "openrouter".into(),
            endpoint: "https://openrouter.ai/api/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "openai/gpt-4o-mini",
                "messages": [{"role": "user", "content": prompts::CHAT_STREAMING_DETERMINISTIC}],
                "max_tokens": 100,
                "temperature": 0,
                "stream": true
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "openrouter:responses:success".into(),
            description: "Successful non-streaming Responses API via OpenRouter with cost".into(),
            provider: "openrouter".into(),
            endpoint: "https://openrouter.ai/api/v1".into(),
            method: "POST".into(),
            path: "/responses".into(),
            request_body: Some(json!({
                "model": "openai/gpt-4o-mini",
                "input": prompts::CHAT_DETERMINISTIC,
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "openrouter:responses:streaming".into(),
            description: "Streaming Responses API via OpenRouter with cost in final event".into(),
            provider: "openrouter".into(),
            endpoint: "https://openrouter.ai/api/v1".into(),
            method: "POST".into(),
            path: "/responses".into(),
            request_body: Some(json!({
                "model": "openai/gpt-4o-mini",
                "input": prompts::CHAT_STREAMING_DETERMINISTIC,
                "temperature": 0,
                "stream": true
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        // =========================================================================
        // Anthropic Fixtures (Native Messages API format)
        // Note: These use Anthropic's native request/response format.
        // The gateway converts these to OpenAI-compatible format.
        // =========================================================================
        FixtureDefinition {
            id: "anthropic:messages:success".into(),
            description: "Successful non-streaming Anthropic Messages API response".into(),
            provider: "anthropic".into(),
            endpoint: "https://api.anthropic.com".into(),
            method: "POST".into(),
            path: "/v1/messages".into(),
            request_body: Some(json!({
                "model": "claude-sonnet-4-20250514",
                "max_tokens": 256,
                "messages": [{"role": "user", "content": prompts::CHAT_DETERMINISTIC}]
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "anthropic:messages:streaming".into(),
            description: "Streaming Anthropic Messages API response with SSE events".into(),
            provider: "anthropic".into(),
            endpoint: "https://api.anthropic.com".into(),
            method: "POST".into(),
            path: "/v1/messages".into(),
            request_body: Some(json!({
                "model": "claude-sonnet-4-20250514",
                "max_tokens": 256,
                "messages": [{"role": "user", "content": prompts::CHAT_STREAMING_DETERMINISTIC}],
                "stream": true
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "anthropic:tool_call:success".into(),
            description: "Anthropic Messages API with single tool_use response".into(),
            provider: "anthropic".into(),
            endpoint: "https://api.anthropic.com".into(),
            method: "POST".into(),
            path: "/v1/messages".into(),
            request_body: Some(json!({
                "model": "claude-sonnet-4-20250514",
                "max_tokens": 256,
                "messages": [{"role": "user", "content": prompts::TOOL_CALL_SINGLE}],
                "tools": [anthropic_weather_tool()],
                "tool_choice": {"type": "auto"}
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "anthropic:tool_call:streaming".into(),
            description: "Streaming Anthropic Messages API with tool_use response".into(),
            provider: "anthropic".into(),
            endpoint: "https://api.anthropic.com".into(),
            method: "POST".into(),
            path: "/v1/messages".into(),
            request_body: Some(json!({
                "model": "claude-sonnet-4-20250514",
                "max_tokens": 256,
                "messages": [{"role": "user", "content": prompts::TOOL_CALL_SINGLE}],
                "tools": [anthropic_weather_tool()],
                "tool_choice": {"type": "auto"},
                "stream": true
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "anthropic:tool_call:parallel".into(),
            description: "Anthropic Messages API with parallel tool_use responses".into(),
            provider: "anthropic".into(),
            endpoint: "https://api.anthropic.com".into(),
            method: "POST".into(),
            path: "/v1/messages".into(),
            request_body: Some(json!({
                "model": "claude-sonnet-4-20250514",
                "max_tokens": 512,
                "messages": [{"role": "user", "content": prompts::TOOL_CALL_PARALLEL}],
                "tools": [anthropic_weather_tool()],
                "tool_choice": {"type": "auto"}
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "anthropic:tool_call:with_result".into(),
            description: "Anthropic Messages API response after tool_result is provided".into(),
            provider: "anthropic".into(),
            endpoint: "https://api.anthropic.com".into(),
            method: "POST".into(),
            path: "/v1/messages".into(),
            request_body: Some(json!({
                "model": "claude-sonnet-4-20250514",
                "max_tokens": 256,
                "messages": [
                    {"role": "user", "content": prompts::TOOL_CALL_SINGLE},
                    {
                        "role": "assistant",
                        "content": [{
                            "type": "tool_use",
                            "id": "toolu_recorded_001",
                            "name": "get_weather",
                            "input": {"location": "San Francisco, USA", "unit": "celsius"}
                        }]
                    },
                    {
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": "toolu_recorded_001",
                            "content": "{\"temperature\": 18, \"condition\": \"partly cloudy\", \"humidity\": 65}"
                        }]
                    }
                ],
                "tools": [anthropic_weather_tool()]
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "anthropic:thinking:success".into(),
            description: "Anthropic Messages API with extended thinking (Claude 4+)".into(),
            provider: "anthropic".into(),
            endpoint: "https://api.anthropic.com".into(),
            method: "POST".into(),
            path: "/v1/messages".into(),
            request_body: Some(json!({
                "model": "claude-sonnet-4-20250514",
                "max_tokens": 16000,
                "messages": [{"role": "user", "content": prompts::REASONING_DETERMINISTIC}],
                "thinking": {
                    "type": "enabled",
                    "budget_tokens": 10000
                }
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "anthropic:thinking:streaming".into(),
            description: "Streaming Anthropic Messages API with extended thinking".into(),
            provider: "anthropic".into(),
            endpoint: "https://api.anthropic.com".into(),
            method: "POST".into(),
            path: "/v1/messages".into(),
            request_body: Some(json!({
                "model": "claude-sonnet-4-20250514",
                "max_tokens": 16000,
                "messages": [{"role": "user", "content": prompts::REASONING_DETERMINISTIC}],
                "thinking": {
                    "type": "enabled",
                    "budget_tokens": 10000
                },
                "stream": true
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "anthropic:vision:success".into(),
            description: "Anthropic Messages API with base64 image input".into(),
            provider: "anthropic".into(),
            endpoint: "https://api.anthropic.com".into(),
            method: "POST".into(),
            path: "/v1/messages".into(),
            request_body: Some(json!({
                "model": "claude-sonnet-4-20250514",
                "max_tokens": 300,
                "messages": [{
                    "role": "user",
                    "content": [
                        {"type": "text", "text": prompts::VISION_DESCRIBE},
                        {
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="
                            }
                        }
                    ]
                }]
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        // =========================================================================
        // Anthropic Error Responses
        // - bad_request: Triggered by invalid model name
        // - unauthorized: Triggered by using invalid API key (use --invalid-key flag)
        // - rate_limit/server_error: Cannot be triggered on demand, manually created
        // =========================================================================
        FixtureDefinition {
            id: "anthropic:bad:request".into(),
            description: "Bad request error response (400) - invalid model".into(),
            provider: "anthropic".into(),
            endpoint: "https://api.anthropic.com".into(),
            method: "POST".into(),
            path: "/v1/messages".into(),
            request_body: Some(json!({
                "model": "invalid-model-that-does-not-exist",
                "max_tokens": 100,
                "messages": [{"role": "user", "content": "Hello"}]
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "anthropic:unauthorized".into(),
            description: "Unauthorized error response (401)".into(),
            provider: "anthropic".into(),
            endpoint: "https://api.anthropic.com".into(),
            method: "POST".into(),
            path: "/v1/messages".into(),
            request_body: Some(json!({
                "model": "claude-sonnet-4-20250514",
                "max_tokens": 100,
                "messages": [{"role": "user", "content": "Hello"}]
            })),
            streaming: false,
            use_invalid_credentials: true,
        },
        // =========================================================================
        // Bedrock Fixtures (AWS Bedrock Converse API)
        // Note: Recording requires AWS credentials (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY)
        // and AWS_REGION environment variables.
        // Uses Nova Lite for basic chat/vision, Claude Sonnet for tool calling.
        // Unauthorized fixture uses use_invalid_credentials: true to auto-use fake creds.
        // =========================================================================
        // Bedrock Converse API (Chat Completions) - using Nova Lite
        FixtureDefinition {
            id: "bedrock:converse:success".into(),
            description: "Successful non-streaming Bedrock Converse API response".into(),
            provider: "bedrock".into(),
            endpoint: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
            method: "POST".into(),
            path: "/model/us.amazon.nova-2-lite-v1:0/converse".into(),
            request_body: Some(json!({
                "messages": [{"role": "user", "content": [{"text": prompts::CHAT_DETERMINISTIC}]}],
                "inferenceConfig": {"maxTokens": 256}
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "bedrock:converse:streaming".into(),
            description: "Streaming Bedrock Converse API response with SSE events".into(),
            provider: "bedrock".into(),
            endpoint: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
            method: "POST".into(),
            path: "/model/us.amazon.nova-2-lite-v1:0/converse-stream".into(),
            request_body: Some(json!({
                "messages": [{"role": "user", "content": [{"text": prompts::CHAT_STREAMING_DETERMINISTIC}]}],
                "inferenceConfig": {"maxTokens": 256}
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        // Bedrock Responses API - uses Converse API internally (same as chat completions)
        // These fixtures are for testing the Responses API endpoint which routes through Converse
        FixtureDefinition {
            id: "bedrock:responses:success".into(),
            description: "Successful non-streaming Bedrock Converse API response for Responses API"
                .into(),
            provider: "bedrock".into(),
            endpoint: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
            method: "POST".into(),
            path: "/model/us.amazon.nova-2-lite-v1:0/converse".into(),
            request_body: Some(json!({
                "messages": [{"role": "user", "content": [{"text": prompts::CHAT_DETERMINISTIC}]}],
                "inferenceConfig": {"maxTokens": 256}
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "bedrock:responses:streaming".into(),
            description: "Streaming Bedrock Converse API response for Responses API".into(),
            provider: "bedrock".into(),
            endpoint: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
            method: "POST".into(),
            path: "/model/us.amazon.nova-2-lite-v1:0/converse-stream".into(),
            request_body: Some(json!({
                "messages": [{"role": "user", "content": [{"text": prompts::CHAT_STREAMING_DETERMINISTIC}]}],
                "inferenceConfig": {"maxTokens": 256}
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        // Bedrock Tool Calling (Converse API) - using Claude Sonnet
        FixtureDefinition {
            id: "bedrock:tool_call:success".into(),
            description: "Bedrock Converse API with single tool use response".into(),
            provider: "bedrock".into(),
            endpoint: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
            method: "POST".into(),
            path: "/model/us.anthropic.claude-sonnet-4-5-20250929-v1:0/converse".into(),
            request_body: Some(json!({
                "messages": [{"role": "user", "content": [{"text": prompts::TOOL_CALL_SINGLE}]}],
                "inferenceConfig": {"maxTokens": 256},
                "toolConfig": {
                    "tools": [{
                        "toolSpec": {
                            "name": "get_weather",
                            "description": "Get the current weather in a location",
                            "inputSchema": {
                                "json": {
                                    "type": "object",
                                    "properties": {
                                        "location": {"type": "string", "description": "The city and country"},
                                        "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]}
                                    },
                                    "required": ["location"]
                                }
                            }
                        }
                    }]
                }
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "bedrock:tool_call:streaming".into(),
            description: "Streaming Bedrock Converse API with tool use response".into(),
            provider: "bedrock".into(),
            endpoint: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
            method: "POST".into(),
            path: "/model/us.anthropic.claude-sonnet-4-5-20250929-v1:0/converse-stream".into(),
            request_body: Some(json!({
                "messages": [{"role": "user", "content": [{"text": prompts::TOOL_CALL_SINGLE}]}],
                "inferenceConfig": {"maxTokens": 256},
                "toolConfig": {
                    "tools": [{
                        "toolSpec": {
                            "name": "get_weather",
                            "description": "Get the current weather in a location",
                            "inputSchema": {
                                "json": {
                                    "type": "object",
                                    "properties": {
                                        "location": {"type": "string", "description": "The city and country"},
                                        "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]}
                                    },
                                    "required": ["location"]
                                }
                            }
                        }
                    }]
                }
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "bedrock:tool_call:parallel".into(),
            description: "Bedrock Converse API with parallel tool use responses".into(),
            provider: "bedrock".into(),
            endpoint: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
            method: "POST".into(),
            path: "/model/us.anthropic.claude-sonnet-4-5-20250929-v1:0/converse".into(),
            request_body: Some(json!({
                "messages": [{"role": "user", "content": [{"text": prompts::TOOL_CALL_PARALLEL}]}],
                "inferenceConfig": {"maxTokens": 512},
                "toolConfig": {
                    "tools": [{
                        "toolSpec": {
                            "name": "get_weather",
                            "description": "Get the current weather in a location",
                            "inputSchema": {
                                "json": {
                                    "type": "object",
                                    "properties": {
                                        "location": {"type": "string", "description": "The city and country"},
                                        "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]}
                                    },
                                    "required": ["location"]
                                }
                            }
                        }
                    }]
                }
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "bedrock:tool_call:with_result".into(),
            description: "Bedrock Converse API response after tool result is provided".into(),
            provider: "bedrock".into(),
            endpoint: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
            method: "POST".into(),
            path: "/model/us.anthropic.claude-sonnet-4-5-20250929-v1:0/converse".into(),
            request_body: Some(json!({
                "messages": [
                    {"role": "user", "content": [{"text": prompts::TOOL_CALL_SINGLE}]},
                    {
                        "role": "assistant",
                        "content": [{
                            "toolUse": {
                                "toolUseId": "tooluse_recorded_001",
                                "name": "get_weather",
                                "input": {"location": "San Francisco, USA", "unit": "celsius"}
                            }
                        }]
                    },
                    {
                        "role": "user",
                        "content": [{
                            "toolResult": {
                                "toolUseId": "tooluse_recorded_001",
                                "content": [{"json": {"temperature": 18, "condition": "partly cloudy", "humidity": 65}}]
                            }
                        }]
                    }
                ],
                "inferenceConfig": {"maxTokens": 256},
                "toolConfig": {
                    "tools": [{
                        "toolSpec": {
                            "name": "get_weather",
                            "description": "Get the current weather in a location",
                            "inputSchema": {
                                "json": {
                                    "type": "object",
                                    "properties": {
                                        "location": {"type": "string", "description": "The city and country"},
                                        "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]}
                                    },
                                    "required": ["location"]
                                }
                            }
                        }
                    }]
                }
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        // Bedrock Vision (Converse API with image) - using Nova Lite
        FixtureDefinition {
            id: "bedrock:vision:success".into(),
            description: "Bedrock Converse API with base64 image input".into(),
            provider: "bedrock".into(),
            endpoint: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
            method: "POST".into(),
            path: "/model/us.amazon.nova-2-lite-v1:0/converse".into(),
            request_body: Some(json!({
                "messages": [{
                    "role": "user",
                    "content": [
                        {"text": prompts::VISION_DESCRIBE},
                        {
                            "image": {
                                "format": "png",
                                "source": {
                                    "bytes": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="
                                }
                            }
                        }
                    ]
                }],
                "inferenceConfig": {"maxTokens": 300}
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        // Bedrock Error Responses
        FixtureDefinition {
            id: "bedrock:bad:request".into(),
            description: "Bad request error response (400) - invalid model".into(),
            provider: "bedrock".into(),
            endpoint: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
            method: "POST".into(),
            path: "/model/invalid-model/converse".into(),
            request_body: Some(json!({
                "messages": [{"role": "user", "content": [{"text": "Hello"}]}],
                "inferenceConfig": {"maxTokens": 100}
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "bedrock:unauthorized".into(),
            description: "Unauthorized error response (403) - invalid AWS credentials".into(),
            provider: "bedrock".into(),
            endpoint: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
            method: "POST".into(),
            path: "/model/us.amazon.nova-2-lite-v1:0/converse".into(),
            request_body: Some(json!({
                "messages": [{"role": "user", "content": [{"text": "Hello"}]}],
                "inferenceConfig": {"maxTokens": 100}
            })),
            streaming: false,
            use_invalid_credentials: true,
        },
        // =========================================================================
        // Vertex AI Fixtures (Google Gemini API via AI Platform)
        // =========================================================================
        // Uses API key authentication with ?key= query parameter.
        // Streaming uses SSE format (same as OpenAI/Anthropic).
        // Endpoint: aiplatform.googleapis.com (NOT generativelanguage.googleapis.com)
        //
        // Vertex Chat Completions (generateContent API) - using Gemini 2.0 Flash
        FixtureDefinition {
            id: "vertex:generate_content:success".into(),
            description: "Successful non-streaming Vertex generateContent response".into(),
            provider: "vertex".into(),
            endpoint: "https://aiplatform.googleapis.com/v1/publishers/google/models".into(),
            method: "POST".into(),
            path: "/gemini-2.0-flash:generateContent".into(),
            request_body: Some(json!({
                "contents": [{"role": "user", "parts": [{"text": prompts::CHAT_DETERMINISTIC}]}],
                "generationConfig": {"maxOutputTokens": 100, "temperature": 0}
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "vertex:generate_content:streaming".into(),
            description: "Streaming Vertex generateContent response with SSE".into(),
            provider: "vertex".into(),
            endpoint: "https://aiplatform.googleapis.com/v1/publishers/google/models".into(),
            method: "POST".into(),
            path: "/gemini-2.0-flash:streamGenerateContent".into(),
            request_body: Some(json!({
                "contents": [{"role": "user", "parts": [{"text": prompts::CHAT_STREAMING_DETERMINISTIC}]}],
                "generationConfig": {"maxOutputTokens": 100, "temperature": 0}
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        // Vertex Responses API - same generateContent endpoint, different request format
        FixtureDefinition {
            id: "vertex:responses:success".into(),
            description: "Successful non-streaming Vertex generateContent for Responses API".into(),
            provider: "vertex".into(),
            endpoint: "https://aiplatform.googleapis.com/v1/publishers/google/models".into(),
            method: "POST".into(),
            path: "/gemini-2.0-flash:generateContent".into(),
            request_body: Some(json!({
                "contents": [{"role": "user", "parts": [{"text": prompts::CHAT_DETERMINISTIC}]}],
                "generationConfig": {"maxOutputTokens": 100, "temperature": 0}
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "vertex:responses:streaming".into(),
            description: "Streaming Vertex generateContent for Responses API".into(),
            provider: "vertex".into(),
            endpoint: "https://aiplatform.googleapis.com/v1/publishers/google/models".into(),
            method: "POST".into(),
            path: "/gemini-2.0-flash:streamGenerateContent".into(),
            request_body: Some(json!({
                "contents": [{"role": "user", "parts": [{"text": prompts::CHAT_STREAMING_DETERMINISTIC}]}],
                "generationConfig": {"maxOutputTokens": 100, "temperature": 0}
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        // Vertex Tool Calling (generateContent with tools)
        FixtureDefinition {
            id: "vertex:tool_call:success".into(),
            description: "Vertex generateContent with single function call response".into(),
            provider: "vertex".into(),
            endpoint: "https://aiplatform.googleapis.com/v1/publishers/google/models".into(),
            method: "POST".into(),
            path: "/gemini-2.0-flash:generateContent".into(),
            request_body: Some(json!({
                "contents": [{"role": "user", "parts": [{"text": prompts::TOOL_CALL_SINGLE}]}],
                "tools": [{"functionDeclarations": [vertex_weather_function()]}],
                "generationConfig": {"maxOutputTokens": 100, "temperature": 0}
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "vertex:tool_call:streaming".into(),
            description: "Streaming Vertex generateContent with function call response".into(),
            provider: "vertex".into(),
            endpoint: "https://aiplatform.googleapis.com/v1/publishers/google/models".into(),
            method: "POST".into(),
            path: "/gemini-2.0-flash:streamGenerateContent".into(),
            request_body: Some(json!({
                "contents": [{"role": "user", "parts": [{"text": prompts::TOOL_CALL_SINGLE}]}],
                "tools": [{"functionDeclarations": [vertex_weather_function()]}],
                "generationConfig": {"maxOutputTokens": 100, "temperature": 0}
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "vertex:tool_call:parallel".into(),
            description: "Vertex generateContent with parallel function calls".into(),
            provider: "vertex".into(),
            endpoint: "https://aiplatform.googleapis.com/v1/publishers/google/models".into(),
            method: "POST".into(),
            path: "/gemini-2.0-flash:generateContent".into(),
            request_body: Some(json!({
                "contents": [{"role": "user", "parts": [{"text": prompts::TOOL_CALL_PARALLEL}]}],
                "tools": [{"functionDeclarations": [vertex_weather_function()]}],
                "generationConfig": {"maxOutputTokens": 100, "temperature": 0}
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "vertex:tool_call:with_result".into(),
            description: "Vertex generateContent after function result is provided".into(),
            provider: "vertex".into(),
            endpoint: "https://aiplatform.googleapis.com/v1/publishers/google/models".into(),
            method: "POST".into(),
            path: "/gemini-2.0-flash:generateContent".into(),
            request_body: Some(json!({
                "contents": [
                    {"role": "user", "parts": [{"text": prompts::TOOL_CALL_SINGLE}]},
                    {"role": "model", "parts": [{"functionCall": {"name": "get_weather", "args": {"location": "San Francisco, USA"}}}]},
                    {"role": "user", "parts": [{"functionResponse": {"name": "get_weather", "response": {"temperature": 72, "conditions": "sunny"}}}]}
                ],
                "tools": [{"functionDeclarations": [vertex_weather_function()]}],
                "generationConfig": {"maxOutputTokens": 100, "temperature": 0}
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        // Vertex Vision (generateContent with image)
        FixtureDefinition {
            id: "vertex:vision:success".into(),
            description: "Vertex generateContent with base64 image input".into(),
            provider: "vertex".into(),
            endpoint: "https://aiplatform.googleapis.com/v1/publishers/google/models".into(),
            method: "POST".into(),
            path: "/gemini-2.0-flash:generateContent".into(),
            request_body: Some(json!({
                "contents": [{
                    "role": "user",
                    "parts": [
                        {"inlineData": {"mimeType": "image/png", "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="}},
                        {"text": prompts::VISION_DESCRIBE}
                    ]
                }],
                "generationConfig": {"maxOutputTokens": 200, "temperature": 0}
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        // Vertex Error Responses
        FixtureDefinition {
            id: "vertex:bad:request".into(),
            description: "Bad request error response (400) - invalid model".into(),
            provider: "vertex".into(),
            endpoint: "https://aiplatform.googleapis.com/v1/publishers/google/models".into(),
            method: "POST".into(),
            path: "/invalid-model:generateContent".into(),
            request_body: Some(json!({
                "contents": [{"role": "user", "parts": [{"text": "Hello"}]}]
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "vertex:unauthorized".into(),
            description: "Unauthorized error response (401) - invalid API key".into(),
            provider: "vertex".into(),
            endpoint: "https://aiplatform.googleapis.com/v1/publishers/google/models".into(),
            method: "POST".into(),
            path: "/gemini-2.0-flash:generateContent".into(),
            request_body: Some(json!({
                "contents": [{"role": "user", "parts": [{"text": "Hello"}]}]
            })),
            streaming: false,
            use_invalid_credentials: true,
        },
        // =========================================================================
        // Ollama Fixtures (OpenAI-compatible API, local server)
        // Uses qwen3:4b for text and gemma3:4b for vision
        // =========================================================================
        FixtureDefinition {
            id: "ollama:chat_completion:success".into(),
            description: "Successful non-streaming chat completion via Ollama".into(),
            provider: "ollama".into(),
            endpoint: "http://localhost:11434/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "qwen3:4b",
                "messages": [{"role": "user", "content": prompts::CHAT_DETERMINISTIC}],
                "max_tokens": 100,
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "ollama:chat_completion:streaming".into(),
            description: "Streaming chat completion via Ollama".into(),
            provider: "ollama".into(),
            endpoint: "http://localhost:11434/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "qwen3:4b",
                "messages": [{"role": "user", "content": prompts::CHAT_STREAMING_DETERMINISTIC}],
                "max_tokens": 100,
                "temperature": 0,
                "stream": true
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "ollama:tool_call:success".into(),
            description: "Chat completion with single tool call via Ollama".into(),
            provider: "ollama".into(),
            endpoint: "http://localhost:11434/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "qwen3:4b",
                "messages": [{"role": "user", "content": prompts::TOOL_CALL_SINGLE}],
                "tools": [weather_tool()],
                "tool_choice": "auto",
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "ollama:tool_call:streaming".into(),
            description: "Streaming chat completion with tool call via Ollama".into(),
            provider: "ollama".into(),
            endpoint: "http://localhost:11434/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "qwen3:4b",
                "messages": [{"role": "user", "content": prompts::TOOL_CALL_SINGLE}],
                "tools": [weather_tool()],
                "tool_choice": "auto",
                "temperature": 0,
                "stream": true
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "ollama:vision:success".into(),
            description: "Vision request with base64 image via Ollama".into(),
            provider: "ollama".into(),
            endpoint: "http://localhost:11434/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "gemma3:4b",
                "messages": [{
                    "role": "user",
                    "content": [
                        {"type": "text", "text": prompts::VISION_DESCRIBE},
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
                                "detail": "low"
                            }
                        }
                    ]
                }],
                "max_tokens": 300,
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: "ollama:bad_request".into(),
            description: "Error response for invalid/missing model".into(),
            provider: "ollama".into(),
            endpoint: "http://localhost:11434/v1".into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "nonexistent-model-xyz",
                "messages": [{"role": "user", "content": "Hello"}]
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
    ]
}

/// Vertex AI function declaration format for weather tool.
fn vertex_weather_function() -> serde_json::Value {
    json!({
        "name": "get_weather",
        "description": "Get the current weather in a location",
        "parameters": {
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "The city and country, e.g. San Francisco, USA"
                },
                "unit": {
                    "type": "string",
                    "enum": ["celsius", "fahrenheit"],
                    "description": "The temperature unit"
                }
            },
            "required": ["location"]
        }
    })
}

/// Get AWS credentials from environment or default credential chain.
async fn get_aws_credentials() -> Result<Credentials, Box<dyn std::error::Error>> {
    // Try static credentials from environment first
    if let (Ok(access_key), Ok(secret_key)) = (
        std::env::var("AWS_ACCESS_KEY_ID"),
        std::env::var("AWS_SECRET_ACCESS_KEY"),
    ) {
        let session_token = std::env::var("AWS_SESSION_TOKEN").ok();
        return Ok(Credentials::new(
            access_key,
            secret_key,
            session_token,
            None,
            "environment",
        ));
    }

    // Fall back to default credential chain
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let provider = config
        .credentials_provider()
        .ok_or("No AWS credentials provider available")?;

    use aws_credential_types::provider::ProvideCredentials;
    let creds = provider
        .provide_credentials()
        .await
        .map_err(|e| format!("Failed to get AWS credentials: {}", e))?;

    Ok(creds)
}

/// Sign an HTTP request using AWS SigV4.
fn sign_aws_request(
    credentials: &Credentials,
    region: &str,
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let identity = credentials.clone().into();

    let signing_settings = SigningSettings::default();
    let signing_params = SigningParams::builder()
        .identity(&identity)
        .region(region)
        .name("bedrock")
        .time(std::time::SystemTime::now())
        .settings(signing_settings)
        .build()
        .map_err(|e| format!("Failed to build signing params: {}", e))?;

    let signable_request = SignableRequest::new(
        method,
        url,
        headers.iter().copied(),
        SignableBody::Bytes(body),
    )
    .map_err(|e| format!("Failed to create signable request: {}", e))?;

    let (signing_instructions, _signature) =
        aws_sigv4::http_request::sign(signable_request, &signing_params.into())
            .map_err(|e| format!("Failed to sign request: {}", e))?
            .into_parts();

    let mut signed_headers = Vec::new();
    for (name, value) in signing_instructions.headers() {
        signed_headers.push((name.to_string(), value.to_string()));
    }

    Ok(signed_headers)
}

async fn record_fixture(
    client: &reqwest::Client,
    def: &FixtureDefinition,
    api_key: &str,
) -> Result<RecordedFixture, Box<dyn std::error::Error>> {
    let url = format!("{}{}", def.endpoint, def.path);

    println!("  Recording: {} -> {}", def.id, url);

    let response = if def.provider == "bedrock" {
        // AWS Bedrock Converse API requires SigV4 signing
        let aws_creds = if def.use_invalid_credentials {
            // Use fake credentials for unauthorized fixture
            Credentials::new(
                "AKIAIOSFODNN7INVALID",
                "wJalrXUtnFEMI/K7MDENG/bPxRfiCYINVALIDKEY",
                None,
                None,
                "invalid_for_testing",
            )
        } else {
            get_aws_credentials().await?
        };
        let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());

        let body_bytes = def
            .request_body
            .as_ref()
            .map(serde_json::to_vec)
            .transpose()?
            .unwrap_or_default();

        let base_headers = vec![
            ("content-type", "application/json"),
            ("host", def.endpoint.trim_start_matches("https://")),
        ];

        let signed_headers = sign_aws_request(
            &aws_creds,
            &region,
            &def.method,
            &url,
            &base_headers,
            &body_bytes,
        )?;

        let mut request = match def.method.as_str() {
            "GET" => client.get(&url),
            "POST" => client.post(&url),
            _ => return Err(format!("Unsupported method: {}", def.method).into()),
        };

        request = request.header("Content-Type", "application/json");
        for (name, value) in signed_headers {
            request = request.header(name, value);
        }

        if !body_bytes.is_empty() {
            request = request.body(body_bytes);
        }

        request.send().await?
    } else if def.provider == "vertex" {
        // Vertex AI uses API key in query parameter
        let effective_api_key = if def.use_invalid_credentials {
            "invalid-api-key-for-testing"
        } else {
            api_key
        };

        // Build URL with API key in query parameter
        let vertex_url = if def.streaming {
            format!(
                "{}{}?alt=sse&key={}",
                def.endpoint, def.path, effective_api_key
            )
        } else {
            format!("{}{}?key={}", def.endpoint, def.path, effective_api_key)
        };

        let mut request = match def.method.as_str() {
            "GET" => client.get(&vertex_url),
            "POST" => client.post(&vertex_url),
            _ => return Err(format!("Unsupported method: {}", def.method).into()),
        };

        request = request.header("Content-Type", "application/json");

        if let Some(body) = &def.request_body {
            request = request.json(body);
        }

        request.send().await?
    } else {
        // Use fake API key for unauthorized fixtures
        let effective_api_key = if def.use_invalid_credentials {
            "sk-invalid-key-for-testing-401-errors"
        } else {
            api_key
        };

        let mut request = match def.method.as_str() {
            "GET" => client.get(&url),
            "POST" => client.post(&url),
            _ => return Err(format!("Unsupported method: {}", def.method).into()),
        };

        // Add provider-specific headers
        if def.provider == "anthropic" {
            // Anthropic uses x-api-key and anthropic-version headers
            request = request
                .header("x-api-key", effective_api_key)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json");
        } else {
            // OpenAI-compatible providers use Bearer token
            request = request
                .header("Authorization", format!("Bearer {}", effective_api_key))
                .header("Content-Type", "application/json");

            // Add OpenRouter-specific headers
            if def.provider == "openrouter" {
                request = request
                    .header("HTTP-Referer", "https://github.com/ScriptSmith/hadrian")
                    .header("X-Title", "Hadrian Gateway Fixture Recording");
            }
        }

        if let Some(body) = &def.request_body {
            request = request.json(body);
        }

        request.send().await?
    };
    let status = response.status().as_u16();

    let mut headers = HashMap::new();
    for (key, value) in response.headers() {
        if let Ok(v) = value.to_str() {
            // Only include relevant headers
            let key_lower = key.as_str().to_lowercase();
            if key_lower == "content-type"
                || key_lower.starts_with("x-")
                || key_lower == "retry-after"
            {
                headers.insert(key.as_str().to_string(), v.to_string());
            }
        }
    }

    let fixture_response = if def.streaming {
        // For streaming, collect chunks based on format
        let body_bytes = response.bytes().await?;

        // Bedrock Converse API uses AWS EventStream binary format, others use SSE
        if def.provider == "bedrock" {
            // Decode AWS EventStream binary format
            let events = decode_eventstream_events(&body_bytes);

            headers.insert("content-type".to_string(), "text/event-stream".to_string());
            headers.insert("transfer-encoding".to_string(), "chunked".to_string());

            FixtureResponse {
                status,
                headers,
                body: None,
                streaming: true,
                streaming_format: StreamingFormat::AwsEventstream,
                chunks: None,
                events: Some(events),
            }
        } else {
            // Standard SSE format (data: {...}\n\n)
            let body_text = String::from_utf8_lossy(&body_bytes);
            let mut chunks = Vec::new();

            for line in body_text.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        continue;
                    }
                    if let Ok(chunk) = serde_json::from_str::<Value>(data) {
                        chunks.push(chunk);
                    }
                }
            }

            headers.insert("content-type".to_string(), "text/event-stream".to_string());
            headers.insert("transfer-encoding".to_string(), "chunked".to_string());

            FixtureResponse {
                status,
                headers,
                body: None,
                streaming: true,
                streaming_format: StreamingFormat::Sse,
                chunks: Some(chunks),
                events: None,
            }
        }
    } else {
        let body: Value = response.json().await?;
        FixtureResponse {
            status,
            headers,
            body: Some(body),
            streaming: false,
            streaming_format: StreamingFormat::default(),
            chunks: None,
            events: None,
        }
    };

    Ok(RecordedFixture {
        id: def.id.clone(),
        description: def.description.clone(),
        request: FixtureRequest {
            method: def.method.clone(),
            path: def.path.clone(),
            path_pattern: generate_path_pattern(&def.path),
        },
        response: fixture_response,
    })
}

fn get_api_key(provider: &str) -> Result<String, String> {
    let env_var = match provider {
        "openai" => "OPENAI_API_KEY",
        "anthropic" => "ANTHROPIC_API_KEY",
        "openrouter" => "OPENROUTER_API_KEY",
        // Vertex uses GEMINI_API_KEY or GOOGLE_API_KEY
        "vertex" => {
            return std::env::var("GEMINI_API_KEY")
                .or_else(|_| std::env::var("GOOGLE_API_KEY"))
                .map_err(|_| {
                    "GEMINI_API_KEY or GOOGLE_API_KEY environment variable not set".to_string()
                });
        }
        // Bedrock uses AWS credentials from environment or config, not API key
        "bedrock" => return Ok("aws-credentials".to_string()),
        // Ollama is a local server and doesn't require an API key
        "ollama" => return Ok("no-key-required".to_string()),
        _ => return Err(format!("Unknown provider: {}", provider)),
    };

    std::env::var(env_var).map_err(|_| format!("{} environment variable not set", env_var))
}

/// Generate a path_pattern for paths that contain dynamic segments (like model IDs).
/// Returns None if the path doesn't need pattern matching.
fn generate_path_pattern(path: &str) -> Option<String> {
    // Bedrock Converse API paths: /model/{model_id}/converse or /model/{model_id}/converse-stream
    if path.starts_with("/model/")
        && (path.ends_with("/converse") || path.ends_with("/converse-stream"))
    {
        if path.ends_with("/converse-stream") {
            Some("/model/.+/converse-stream".to_string())
        } else {
            Some("/model/.+/converse".to_string())
        }
    }
    // Vertex AI paths: /{model}:generateContent or /{model}:streamGenerateContent
    else if path.ends_with(":generateContent") || path.ends_with(":streamGenerateContent") {
        if path.ends_with(":streamGenerateContent") {
            Some("/.+:streamGenerateContent".to_string())
        } else {
            Some("/.+:generateContent".to_string())
        }
    } else {
        None
    }
}

/// Decode AWS EventStream binary format into EventStreamEvent objects.
/// Each EventStream message contains headers (including event type) and a JSON payload.
fn decode_eventstream_events(data: &[u8]) -> Vec<EventStreamEvent> {
    let mut events = Vec::new();
    let mut decoder = MessageFrameDecoder::new();
    let mut buffer = bytes::BytesMut::from(data);

    loop {
        match decoder.decode_frame(&mut buffer) {
            Ok(DecodedFrame::Complete(message)) => {
                // Extract event type from headers
                let event_type = message.headers().iter().find_map(|h| {
                    if h.name().as_str() == ":event-type" {
                        h.value().as_string().ok().map(|s| s.as_str().to_string())
                    } else {
                        None
                    }
                });

                // Skip if it's an exception
                let message_type = message.headers().iter().find_map(|h| {
                    if h.name().as_str() == ":message-type" {
                        h.value().as_string().ok().map(|s| s.as_str().to_string())
                    } else {
                        None
                    }
                });

                if message_type.as_deref() == Some("exception") {
                    eprintln!(
                        "  Warning: EventStream exception: {:?}",
                        String::from_utf8_lossy(message.payload())
                    );
                    continue;
                }

                // Parse payload as JSON and create event
                if let Ok(payload) = serde_json::from_slice::<Value>(message.payload())
                    && let Some(event_type) = event_type
                {
                    events.push(EventStreamEvent {
                        event_type,
                        payload,
                    });
                }
            }
            Ok(DecodedFrame::Incomplete) => {
                // No more complete frames
                break;
            }
            Err(e) => {
                eprintln!("  Warning: Failed to decode EventStream frame: {}", e);
                break;
            }
        }
    }

    events
}

fn fixture_filename(id: &str) -> String {
    // Convert "openai:chat_completion:success" to "chat_completion_success.json"
    // Convert "openai:unauthorized" to "unauthorized.json"
    let parts: Vec<&str> = id.split(':').collect();
    match parts.len() {
        3.. => format!("{}_{}.json", parts[1], parts[2]),
        2 => format!("{}.json", parts[1]),
        _ => format!("{}.json", id.replace(':', "_")),
    }
}

/// Convert fixture ID to the corresponding FixtureId enum variant name.
/// e.g., "openai:chat_completion:success" -> "OpenAiChatCompletionSuccess"
fn fixture_enum_variant(id: &str) -> String {
    let parts: Vec<&str> = id.split(':').collect();
    if parts.len() < 2 {
        return to_pascal_case(id);
    }

    // provider:endpoint:variant -> ProviderEndpointVariant
    parts.iter().map(|p| to_pascal_case(p)).collect()
}

/// Convert snake_case or kebab-case to PascalCase
/// Handles special cases like "openai" -> "OpenAi", "openrouter" -> "OpenRouter"
fn to_pascal_case(s: &str) -> String {
    // Handle special provider names that have internal capitalization
    let s = s
        .replace("openai", "open_ai")
        .replace("openrouter", "open_router");

    s.split(['_', '-'])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

/// Get the expected file path for a fixture definition
fn fixture_file_path(def: &FixtureDefinition, base_dir: &std::path::Path) -> PathBuf {
    let filename = fixture_filename(&def.id);
    base_dir.join(&def.provider).join(filename)
}

/// Validate fixtures: check that all definitions have files and optionally find orphans
fn validate_fixtures(
    fixtures_dir: &std::path::Path,
    check_orphans: bool,
) -> (Vec<String>, Vec<String>) {
    let definitions = get_fixture_definitions();
    let mut missing = Vec::new();
    let mut orphans = Vec::new();

    // Check each definition has a corresponding file
    for def in &definitions {
        let path = fixture_file_path(def, fixtures_dir);
        if !path.exists() {
            missing.push(format!(
                "{} -> {} ({})",
                def.id,
                path.display(),
                fixture_enum_variant(&def.id)
            ));
        }
    }

    // Check for orphan files (files without definitions)
    if check_orphans {
        let expected_files: std::collections::HashSet<_> = definitions
            .iter()
            .map(|d| fixture_file_path(d, fixtures_dir))
            .collect();

        if let Ok(entries) = fs::read_dir(fixtures_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir()
                    && let Ok(subentries) = fs::read_dir(entry.path())
                {
                    for subentry in subentries.flatten() {
                        let path = subentry.path();
                        if path.extension().is_some_and(|e| e == "json")
                            && !expected_files.contains(&path)
                        {
                            orphans.push(path.display().to_string());
                        }
                    }
                }
            }
        }
    }

    (missing, orphans)
}

/// Template for common fixture types that most providers should support
fn scaffold_fixtures(provider: &str, endpoint: &str) -> Vec<FixtureDefinition> {
    vec![
        // Chat Completions
        FixtureDefinition {
            id: format!("{provider}:chat_completion:success"),
            description: "Successful non-streaming chat completion response".into(),
            provider: provider.into(),
            endpoint: endpoint.into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "MODEL_NAME",
                "messages": [{"role": "user", "content": prompts::CHAT_DETERMINISTIC}],
                "max_tokens": 100,
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: format!("{provider}:chat_completion:streaming"),
            description: "Streaming chat completion response with SSE chunks".into(),
            provider: provider.into(),
            endpoint: endpoint.into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "MODEL_NAME",
                "messages": [{"role": "user", "content": prompts::CHAT_STREAMING_DETERMINISTIC}],
                "max_tokens": 100,
                "temperature": 0,
                "stream": true
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        // Responses API
        FixtureDefinition {
            id: format!("{provider}:responses:success"),
            description: "Successful non-streaming Responses API response".into(),
            provider: provider.into(),
            endpoint: endpoint.into(),
            method: "POST".into(),
            path: "/responses".into(),
            request_body: Some(json!({
                "model": "MODEL_NAME",
                "input": prompts::CHAT_DETERMINISTIC,
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: format!("{provider}:responses:streaming"),
            description: "Streaming Responses API response with SSE events".into(),
            provider: provider.into(),
            endpoint: endpoint.into(),
            method: "POST".into(),
            path: "/responses".into(),
            request_body: Some(json!({
                "model": "MODEL_NAME",
                "input": prompts::CHAT_STREAMING_DETERMINISTIC,
                "temperature": 0,
                "stream": true
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
        // Embeddings
        FixtureDefinition {
            id: format!("{provider}:embedding:success"),
            description: "Successful embedding response".into(),
            provider: provider.into(),
            endpoint: endpoint.into(),
            method: "POST".into(),
            path: "/embeddings".into(),
            request_body: Some(json!({
                "model": "EMBEDDING_MODEL_NAME",
                "input": "The quick brown fox jumps over the lazy dog."
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        // Tool Calling
        FixtureDefinition {
            id: format!("{provider}:tool_call:success"),
            description: "Chat completion with single tool call response".into(),
            provider: provider.into(),
            endpoint: endpoint.into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "MODEL_NAME",
                "messages": [{"role": "user", "content": prompts::TOOL_CALL_SINGLE}],
                "tools": [weather_tool()],
                "tool_choice": "auto",
                "temperature": 0
            })),
            streaming: false,
            use_invalid_credentials: false,
        },
        FixtureDefinition {
            id: format!("{provider}:tool_call:streaming"),
            description: "Streaming chat completion with tool call response".into(),
            provider: provider.into(),
            endpoint: endpoint.into(),
            method: "POST".into(),
            path: "/chat/completions".into(),
            request_body: Some(json!({
                "model": "MODEL_NAME",
                "messages": [{"role": "user", "content": prompts::TOOL_CALL_SINGLE}],
                "tools": [weather_tool()],
                "tool_choice": "auto",
                "temperature": 0,
                "stream": true
            })),
            streaming: true,
            use_invalid_credentials: false,
        },
    ]
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::List {
            provider,
            show_enum,
        } => {
            let definitions = get_fixture_definitions();

            println!("Available fixtures:\n");
            for def in definitions {
                if provider.is_none() || provider.as_ref() == Some(&def.provider) {
                    println!(
                        "  {} [{}]",
                        def.id,
                        if def.streaming {
                            "streaming"
                        } else {
                            "non-streaming"
                        }
                    );
                    println!("    {}", def.description);
                    if show_enum {
                        println!("    FixtureId::{}", fixture_enum_variant(&def.id));
                        println!("    File: {}/{}", def.provider, fixture_filename(&def.id));
                    }
                    println!();
                }
            }
        }
        Commands::Record {
            provider,
            fixture,
            output,
        } => {
            let api_key = get_api_key(&provider)?;
            let client = reqwest::Client::new();

            let definitions: Vec<_> = get_fixture_definitions()
                .into_iter()
                .filter(|d| d.provider == provider)
                .filter(|d| fixture.is_none() || d.id.contains(fixture.as_ref().unwrap()))
                .collect();

            if definitions.is_empty() {
                println!("No fixtures found for provider: {}", provider);
                return Ok(());
            }

            let provider_dir = output.join(&provider);
            fs::create_dir_all(&provider_dir)?;

            println!(
                "Recording {} fixtures for {}...\n",
                definitions.len(),
                provider
            );

            for def in &definitions {
                match record_fixture(&client, def, &api_key).await {
                    Ok(recorded) => {
                        let filename = fixture_filename(&def.id);
                        let filepath = provider_dir.join(&filename);
                        let json = serde_json::to_string_pretty(&recorded)?;
                        fs::write(&filepath, json)?;
                        println!("  ✓ Saved: {}", filepath.display());
                    }
                    Err(e) => {
                        println!("  ✗ Failed {}: {}", def.id, e);
                    }
                }
            }

            println!("\nDone!");
        }
        Commands::Validate {
            fixtures_dir,
            check_orphans,
        } => {
            println!("Validating fixtures in {}...\n", fixtures_dir.display());

            let (missing, orphans) = validate_fixtures(&fixtures_dir, check_orphans);

            if missing.is_empty() {
                println!(
                    "✓ All {} fixture definitions have corresponding files.",
                    get_fixture_definitions().len()
                );
            } else {
                println!("✗ Missing {} fixture files:\n", missing.len());
                for m in &missing {
                    println!("  - {}", m);
                }
            }

            if check_orphans {
                println!();
                if orphans.is_empty() {
                    println!("✓ No orphan fixture files found.");
                } else {
                    println!(
                        "⚠ Found {} orphan files (no matching definition):\n",
                        orphans.len()
                    );
                    for o in &orphans {
                        println!("  - {}", o);
                    }
                }
            }

            // Exit with error code if there are missing fixtures
            if !missing.is_empty() {
                std::process::exit(1);
            }
        }
        Commands::Scaffold {
            provider,
            endpoint,
            format,
        } => {
            let fixtures = scaffold_fixtures(&provider, &endpoint);

            println!("Scaffold for provider '{}' ({}):\n", provider, endpoint);

            match format.as_str() {
                "json" => {
                    println!("{}", serde_json::to_string_pretty(&fixtures)?);
                }
                _ => {
                    // Rust format - print code to add to get_fixture_definitions()
                    println!("// Add to get_fixture_definitions() in src/bin/record_fixtures.rs:");
                    println!(
                        "// ========================================================================="
                    );
                    println!("// {} Fixtures", to_pascal_case(&provider));
                    println!(
                        "// ========================================================================="
                    );

                    for def in &fixtures {
                        println!("FixtureDefinition {{");
                        println!("    id: \"{}\".into(),", def.id);
                        println!("    description: \"{}\".into(),", def.description);
                        println!("    provider: \"{}\".into(),", def.provider);
                        println!("    endpoint: \"{}\".into(),", def.endpoint);
                        println!("    method: \"{}\".into(),", def.method);
                        println!("    path: \"{}\".into(),", def.path);
                        if let Some(body) = &def.request_body {
                            println!(
                                "    request_body: Some(json!({})),",
                                serde_json::to_string_pretty(body)?
                                    .lines()
                                    .enumerate()
                                    .map(|(i, l)| if i == 0 {
                                        l.to_string()
                                    } else {
                                        format!("        {}", l)
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            );
                        } else {
                            println!("    request_body: None,");
                        }
                        println!("    streaming: {},", def.streaming);
                        println!("}},");
                        println!();
                    }

                    println!("\n// Add to FixtureId enum in src/providers/test_utils.rs:");
                    for def in &fixtures {
                        println!("    {},", fixture_enum_variant(&def.id));
                    }

                    println!(
                        "\n// Add to FixtureId::file_path() match in src/providers/test_utils.rs:"
                    );
                    for def in &fixtures {
                        println!(
                            "    Self::{} => \"{}/{}\",",
                            fixture_enum_variant(&def.id),
                            provider,
                            fixture_filename(&def.id)
                        );
                    }

                    println!("\n// Add to ProviderFixtures in src/tests/provider_e2e.rs:");
                    println!(
                        "pub static {}_SPEC: ProviderTestSpec = ProviderTestSpec {{",
                        provider.to_uppercase()
                    );
                    println!("    name: \"{}\",", provider);
                    println!("    provider_type: \"open_ai\", // or appropriate type");
                    println!("    default_model: \"MODEL_NAME\",");
                    println!("    extra_config: \"\",");
                    println!("    fixtures: ProviderFixtures {{");
                    println!(
                        "        chat_completion_success: Some(FixtureId::{}ChatCompletionSuccess),",
                        to_pascal_case(&provider)
                    );
                    println!(
                        "        chat_completion_streaming: Some(FixtureId::{}ChatCompletionStreaming),",
                        to_pascal_case(&provider)
                    );
                    println!(
                        "        responses_success: Some(FixtureId::{}ResponsesSuccess),",
                        to_pascal_case(&provider)
                    );
                    println!(
                        "        responses_streaming: Some(FixtureId::{}ResponsesStreaming),",
                        to_pascal_case(&provider)
                    );
                    println!(
                        "        embedding_success: Some(FixtureId::{}EmbeddingSuccess),",
                        to_pascal_case(&provider)
                    );
                    println!(
                        "        tool_call_success: Some(FixtureId::{}ToolCallSuccess),",
                        to_pascal_case(&provider)
                    );
                    println!(
                        "        tool_call_streaming: Some(FixtureId::{}ToolCallStreaming),",
                        to_pascal_case(&provider)
                    );
                    println!("        // Set remaining fields to None or add more fixtures");
                    println!("        ..Default::default()");
                    println!("    }},");
                    println!("}};");
                }
            }
        }
    }

    Ok(())
}
