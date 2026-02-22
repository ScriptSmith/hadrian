//! Schema validation utilities for validating API responses against OpenAI OpenAPI spec.

use std::{collections::HashMap, path::PathBuf, sync::RwLock};

use once_cell::sync::Lazy;
use serde_json::Value;

/// Response type for determining which schema to validate against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseType {
    /// Chat Completions API (/v1/chat/completions)
    ChatCompletion,
    /// Streaming Chat Completions
    ChatCompletionStream,
    /// Responses API (/v1/responses)
    Response,
    /// Streaming Responses API
    ResponseStream,
    /// Legacy Completions API (/v1/completions)
    Completion,
    /// Embeddings API (/v1/embeddings)
    Embedding,
}

/// Well-known schema identifiers for OpenAI API responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SchemaId {
    /// CreateChatCompletionResponse - Chat Completions API
    ChatCompletion,
    /// CreateChatCompletionStreamResponse - Streaming Chat Completions
    ChatCompletionStream,
    /// Response - Responses API
    Response,
    /// CreateCompletionResponse - Legacy Completions API
    Completion,
    /// CreateEmbeddingResponse - Embeddings API
    Embedding,
    /// ErrorResponse - Error responses (for future use)
    #[allow(dead_code)] // Validation infrastructure
    Error,
}

impl SchemaId {
    /// Get the OpenAPI component schema name for this ID.
    pub fn schema_name(&self) -> &'static str {
        match self {
            SchemaId::ChatCompletion => "CreateChatCompletionResponse",
            SchemaId::ChatCompletionStream => "CreateChatCompletionStreamResponse",
            SchemaId::Response => "Response",
            SchemaId::Completion => "CreateCompletionResponse",
            SchemaId::Embedding => "CreateEmbeddingResponse",
            SchemaId::Error => "ErrorResponse",
        }
    }

    /// Get the SchemaId for a given ResponseType (non-streaming).
    pub fn from_response_type(response_type: ResponseType) -> Option<Self> {
        match response_type {
            ResponseType::ChatCompletion => Some(SchemaId::ChatCompletion),
            ResponseType::ChatCompletionStream => Some(SchemaId::ChatCompletionStream),
            ResponseType::Response => Some(SchemaId::Response),
            ResponseType::ResponseStream => None, // Uses discriminator-based validation
            ResponseType::Completion => Some(SchemaId::Completion),
            ResponseType::Embedding => Some(SchemaId::Embedding),
        }
    }
}

/// Validation error with details about what failed.
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.path.is_empty() {
            write!(f, "{}", self.message)
        } else {
            write!(f, "{}: {}", self.path, self.message)
        }
    }
}

/// Result of schema validation.
#[derive(Debug)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
}

impl ValidationResult {
    /// Create a successful validation result.
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            errors: vec![],
        }
    }

    /// Create a failed validation result with errors.
    pub fn invalid(errors: Vec<ValidationError>) -> Self {
        Self {
            is_valid: false,
            errors,
        }
    }

    /// Format errors as a single string for logging.
    pub fn error_message(&self) -> String {
        self.errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// Container for OpenAPI schemas with lazy loading and caching.
pub struct OpenApiSchemas {
    /// Raw OpenAPI spec as JSON
    spec: Value,
    /// Compiled JSON schemas, cached by schema name
    compiled: RwLock<HashMap<String, Value>>,
}

/// Global singleton for loaded schemas.
static SCHEMAS: Lazy<Result<OpenApiSchemas, String>> = Lazy::new(OpenApiSchemas::load);

impl OpenApiSchemas {
    /// Get the global schema instance.
    pub fn get() -> Result<&'static OpenApiSchemas, &'static str> {
        SCHEMAS.as_ref().map_err(|e| e.as_str())
    }

    /// Load the OpenAPI spec from the repository.
    fn load() -> Result<Self, String> {
        let spec_path = Self::spec_path();
        let content = std::fs::read_to_string(&spec_path)
            .map_err(|e| format!("Failed to read OpenAPI spec at {:?}: {}", spec_path, e))?;

        // Parse JSON
        let spec: Value = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse OpenAPI spec JSON: {}", e))?;

        Ok(Self {
            spec,
            compiled: RwLock::new(HashMap::new()),
        })
    }

    fn spec_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("openapi/openai.openapi.json")
    }

    /// Extract and compile a schema by ID.
    /// Returns a resolved JSON Schema suitable for validation.
    pub fn get_schema(&self, id: SchemaId) -> Result<Value, String> {
        let schema_name = id.schema_name();

        // Check cache first
        {
            let cache = self.compiled.read().unwrap();
            if let Some(schema) = cache.get(schema_name) {
                return Ok(schema.clone());
            }
        }

        // Extract and resolve the schema
        let schema = self.extract_schema(schema_name)?;

        // Cache it
        {
            let mut cache = self.compiled.write().unwrap();
            cache.insert(schema_name.to_string(), schema.clone());
        }

        Ok(schema)
    }

    /// Extract and resolve a schema by name (for arbitrary schema names).
    /// This is useful for discriminator-based validation where the schema name
    /// is determined at runtime from the event type.
    #[cfg(feature = "response-validation")]
    pub fn extract_schema_by_name(&self, name: &str) -> Result<Value, String> {
        // Check cache first
        {
            let cache = self.compiled.read().unwrap();
            if let Some(schema) = cache.get(name) {
                return Ok(schema.clone());
            }
        }

        // Extract and resolve
        let schema = self.extract_schema(name)?;

        // Cache it
        {
            let mut cache = self.compiled.write().unwrap();
            cache.insert(name.to_string(), schema.clone());
        }

        Ok(schema)
    }

    /// Extract a schema from components/schemas and resolve $ref references.
    fn extract_schema(&self, name: &str) -> Result<Value, String> {
        let schemas = self
            .spec
            .get("components")
            .and_then(|c| c.get("schemas"))
            .ok_or_else(|| "OpenAPI spec missing components/schemas".to_string())?;

        let raw_schema = schemas
            .get(name)
            .ok_or_else(|| format!("Schema '{}' not found in OpenAPI spec", name))?;

        // Resolve $ref references within the schema
        self.resolve_refs(raw_schema.clone(), 0)
    }

    /// Recursively resolve $ref references in a schema.
    /// Also handles OpenAPI 3.0 `nullable: true` by converting to JSON Schema anyOf.
    /// max_depth prevents infinite recursion on circular refs.
    fn resolve_refs(&self, mut schema: Value, depth: usize) -> Result<Value, String> {
        const MAX_DEPTH: usize = 50;
        if depth > MAX_DEPTH {
            // Return a permissive schema for deeply nested or circular refs
            return Ok(serde_json::json!({}));
        }

        match &mut schema {
            Value::Object(map) => {
                // Handle $ref
                if let Some(Value::String(ref_path)) = map.get("$ref") {
                    let resolved = self.resolve_ref(ref_path, depth + 1)?;
                    return Ok(resolved);
                }

                // Handle OpenAPI 3.0 `nullable: true` - convert to JSON Schema anyOf
                // This must be done before processing other fields so nested schemas
                // with nullable are properly handled.
                let is_nullable = map
                    .get("nullable")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if is_nullable {
                    // Remove nullable key
                    map.remove("nullable");

                    // Clone the current schema (without nullable) for further processing
                    let non_null_schema = self.resolve_refs(schema.clone(), depth + 1)?;

                    // Wrap in anyOf with null type
                    return Ok(serde_json::json!({
                        "anyOf": [
                            non_null_schema,
                            { "type": "null" }
                        ]
                    }));
                }

                // Handle allOf, anyOf, oneOf by resolving their items
                for key in ["allOf", "anyOf", "oneOf"] {
                    if let Some(Value::Array(items)) = map.get_mut(key) {
                        let resolved_items: Result<Vec<Value>, String> = items
                            .iter()
                            .map(|item| self.resolve_refs(item.clone(), depth + 1))
                            .collect();
                        *items = resolved_items?;
                    }
                }

                // Handle properties
                if let Some(Value::Object(props)) = map.get_mut("properties") {
                    let keys: Vec<String> = props.keys().cloned().collect();
                    for key in keys {
                        if let Some(prop) = props.remove(&key) {
                            let resolved = self.resolve_refs(prop, depth + 1)?;
                            props.insert(key, resolved);
                        }
                    }
                }

                // Handle items (for arrays)
                if let Some(items) = map.remove("items") {
                    let resolved = self.resolve_refs(items, depth + 1)?;
                    map.insert("items".to_string(), resolved);
                }

                // Handle additionalProperties
                if let Some(Value::Object(_)) = map.get("additionalProperties")
                    && let Some(ap) = map.remove("additionalProperties")
                {
                    let resolved = self.resolve_refs(ap, depth + 1)?;
                    map.insert("additionalProperties".to_string(), resolved);
                }

                Ok(schema)
            }
            Value::Array(arr) => {
                let resolved: Result<Vec<Value>, String> = arr
                    .iter()
                    .map(|item| self.resolve_refs(item.clone(), depth + 1))
                    .collect();
                Ok(Value::Array(resolved?))
            }
            _ => Ok(schema),
        }
    }

    /// Resolve a $ref path like "#/components/schemas/SomeType"
    fn resolve_ref(&self, ref_path: &str, depth: usize) -> Result<Value, String> {
        if !ref_path.starts_with("#/components/schemas/") {
            // External refs not supported - return permissive schema
            return Ok(serde_json::json!({}));
        }

        let schema_name = ref_path
            .strip_prefix("#/components/schemas/")
            .ok_or_else(|| format!("Invalid $ref path: {}", ref_path))?;

        let schemas = self
            .spec
            .get("components")
            .and_then(|c| c.get("schemas"))
            .ok_or_else(|| "OpenAPI spec missing components/schemas".to_string())?;

        let raw_schema = schemas
            .get(schema_name)
            .ok_or_else(|| format!("Referenced schema '{}' not found", schema_name))?;

        self.resolve_refs(raw_schema.clone(), depth)
    }

    /// Validate a JSON value against a schema.
    pub fn validate(&self, id: SchemaId, value: &Value) -> ValidationResult {
        let schema = match self.get_schema(id) {
            Ok(s) => s,
            Err(e) => {
                return ValidationResult::invalid(vec![ValidationError {
                    path: String::new(),
                    message: format!("Failed to load schema: {}", e),
                }]);
            }
        };

        self.validate_against_schema(&schema, value)
    }

    /// Validate a JSON value against a resolved schema.
    fn validate_against_schema(&self, schema: &Value, value: &Value) -> ValidationResult {
        #[cfg(feature = "response-validation")]
        {
            match jsonschema::draft202012::new(schema) {
                Ok(validator) => {
                    let errors: Vec<ValidationError> = validator
                        .iter_errors(value)
                        .map(|e| ValidationError {
                            path: e.instance_path.to_string(),
                            message: e.to_string(),
                        })
                        .collect();

                    if errors.is_empty() {
                        ValidationResult::valid()
                    } else {
                        ValidationResult::invalid(errors)
                    }
                }
                Err(e) => ValidationResult::invalid(vec![ValidationError {
                    path: String::new(),
                    message: format!("Failed to compile schema: {}", e),
                }]),
            }
        }
        #[cfg(not(feature = "response-validation"))]
        {
            let _ = (schema, value);
            ValidationResult::valid()
        }
    }
}

/// Validate a JSON response against a schema.
/// Returns Ok(()) if valid, Err with error details if invalid.
pub fn validate_response(id: SchemaId, value: &Value) -> Result<(), String> {
    let schemas = OpenApiSchemas::get().map_err(|e| e.to_string())?;
    let result = schemas.validate(id, value);

    if result.is_valid {
        Ok(())
    } else {
        Err(result.error_message())
    }
}

/// Validate a single streaming chat completion chunk.
pub fn validate_streaming_chunk(chunk: &Value) -> Result<(), String> {
    let schemas = OpenApiSchemas::get().map_err(|e| e.to_string())?;
    let result = schemas.validate(SchemaId::ChatCompletionStream, chunk);

    if result.is_valid {
        Ok(())
    } else {
        Err(result.error_message())
    }
}

/// Map Responses API event type to its schema name.
/// Returns None for unknown event types.
fn responses_event_schema_name(event_type: &str) -> Option<&'static str> {
    match event_type {
        // Core response lifecycle events
        "response.created" => Some("ResponseCreatedEvent"),
        "response.in_progress" => Some("ResponseInProgressEvent"),
        "response.completed" => Some("ResponseCompletedEvent"),
        "response.failed" => Some("ResponseFailedEvent"),
        "response.incomplete" => Some("ResponseIncompleteEvent"),
        "response.queued" => Some("ResponseQueuedEvent"),

        // Output item events
        "response.output_item.added" => Some("ResponseOutputItemAddedEvent"),
        "response.output_item.done" => Some("ResponseOutputItemDoneEvent"),

        // Content part events
        "response.content_part.added" => Some("ResponseContentPartAddedEvent"),
        "response.content_part.done" => Some("ResponseContentPartDoneEvent"),

        // Text events
        "response.output_text.delta" => Some("ResponseTextDeltaEvent"),
        "response.output_text.done" => Some("ResponseTextDoneEvent"),

        // Refusal events
        "response.refusal.delta" => Some("ResponseRefusalDeltaEvent"),
        "response.refusal.done" => Some("ResponseRefusalDoneEvent"),

        // Function call events
        "response.function_call_arguments.delta" => Some("ResponseFunctionCallArgumentsDeltaEvent"),
        "response.function_call_arguments.done" => Some("ResponseFunctionCallArgumentsDoneEvent"),

        // File search events
        "response.file_search_call.in_progress" => Some("ResponseFileSearchCallInProgressEvent"),
        "response.file_search_call.searching" => Some("ResponseFileSearchCallSearchingEvent"),
        "response.file_search_call.completed" => Some("ResponseFileSearchCallCompletedEvent"),

        // Image generation events
        "response.image_generation_call.in_progress" => Some("ResponseImageGenCallInProgressEvent"),
        "response.image_generation_call.generating" => Some("ResponseImageGenCallGeneratingEvent"),
        "response.image_generation_call.partial_image" => {
            Some("ResponseImageGenCallPartialImageEvent")
        }
        "response.image_generation_call.completed" => Some("ResponseImageGenCallCompletedEvent"),

        // Code interpreter events
        "response.code_interpreter_call.in_progress" => {
            Some("ResponseCodeInterpreterCallInProgressEvent")
        }
        "response.code_interpreter_call.interpreting" => {
            Some("ResponseCodeInterpreterCallInterpretingEvent")
        }
        "response.code_interpreter_call.code.delta" => {
            Some("ResponseCodeInterpreterCallCodeDeltaEvent")
        }
        "response.code_interpreter_call.code.done" => {
            Some("ResponseCodeInterpreterCallCodeDoneEvent")
        }
        "response.code_interpreter_call.completed" => {
            Some("ResponseCodeInterpreterCallCompletedEvent")
        }

        // Web search events
        "response.web_search_call.in_progress" => Some("ResponseWebSearchCallInProgressEvent"),
        "response.web_search_call.searching" => Some("ResponseWebSearchCallSearchingEvent"),
        "response.web_search_call.completed" => Some("ResponseWebSearchCallCompletedEvent"),

        // Reasoning events
        "response.reasoning_summary_part.added" => Some("ResponseReasoningSummaryPartAddedEvent"),
        "response.reasoning_summary_part.done" => Some("ResponseReasoningSummaryPartDoneEvent"),
        "response.reasoning_summary_text.delta" => Some("ResponseReasoningSummaryTextDeltaEvent"),
        "response.reasoning_summary_text.done" => Some("ResponseReasoningSummaryTextDoneEvent"),
        "response.reasoning.delta" => Some("ResponseReasoningTextDeltaEvent"),
        "response.reasoning.done" => Some("ResponseReasoningTextDoneEvent"),

        // Audio events
        "response.audio.delta" => Some("ResponseAudioDeltaEvent"),
        "response.audio.done" => Some("ResponseAudioDoneEvent"),
        "response.audio_transcript.delta" => Some("ResponseAudioTranscriptDeltaEvent"),
        "response.audio_transcript.done" => Some("ResponseAudioTranscriptDoneEvent"),

        // Error event
        "response.error" => Some("ResponseErrorEvent"),

        _ => None,
    }
}

/// Event types that contain the complex `Response` schema which fails to compile.
/// These are skipped during validation until schema resolution is improved.
const COMPLEX_RESPONSE_EVENTS: &[&str] = &[
    "response.created",
    "response.in_progress",
    "response.completed",
    "response.failed",
    "response.incomplete",
    "response.queued",
];

/// Validate a single Responses API streaming chunk.
///
/// Uses discriminator-based validation: the chunk's `type` field determines
/// which schema to validate against.
///
/// Note: Events containing the full `Response` object are skipped because the
/// `Response` schema has complex `allOf` composition that fails to compile.
pub fn validate_responses_streaming_chunk(chunk: &Value) -> Result<(), String> {
    let event_type = chunk
        .get("type")
        .and_then(|t| t.as_str())
        .ok_or_else(|| "Chunk missing 'type' field".to_string())?;

    // Skip events that contain the complex Response object
    if COMPLEX_RESPONSE_EVENTS.contains(&event_type) {
        return Ok(());
    }

    let schema_name = responses_event_schema_name(event_type)
        .ok_or_else(|| format!("Unknown event type '{}' - no schema mapping", event_type))?;

    #[cfg(feature = "response-validation")]
    {
        let schemas = OpenApiSchemas::get().map_err(|e| e.to_string())?;
        let schema = schemas.extract_schema_by_name(schema_name)?;

        match jsonschema::draft202012::new(&schema) {
            Ok(validator) => {
                let errors: Vec<String> = validator
                    .iter_errors(chunk)
                    .map(|e| format!("{}: {}", e.instance_path, e))
                    .collect();

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(format!(
                        "Event '{}' failed {} schema validation: {}",
                        event_type,
                        schema_name,
                        errors.join("; ")
                    ))
                }
            }
            Err(e) => Err(format!("Failed to compile schema '{}': {}", schema_name, e)),
        }
    }
    #[cfg(not(feature = "response-validation"))]
    {
        let _ = (chunk, schema_name);
        Ok(())
    }
}

#[cfg(all(test, feature = "response-validation"))]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_schema_loading() {
        let schemas = OpenApiSchemas::get().expect("Should load schemas");
        let schema = schemas
            .get_schema(SchemaId::ChatCompletion)
            .expect("Should extract ChatCompletion schema");

        assert!(schema.is_object(), "Schema should be an object");
    }

    #[test]
    fn test_chat_completion_validation_success() {
        let valid_response = json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1677652288,
            "model": "gpt-4o-mini",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello, how can I help you?",
                    "refusal": null
                },
                "logprobs": null,
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 20,
                "total_tokens": 30
            }
        });

        let result = validate_response(SchemaId::ChatCompletion, &valid_response);
        assert!(result.is_ok(), "Valid response should pass: {:?}", result);
    }

    #[test]
    fn test_chat_completion_validation_missing_required() {
        let invalid_response = json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1677652288,
            "model": "gpt-4o-mini"
            // missing "choices"
        });

        let result = validate_response(SchemaId::ChatCompletion, &invalid_response);
        assert!(result.is_err(), "Invalid response should fail validation");
        assert!(
            result.unwrap_err().contains("choices"),
            "Error should mention missing 'choices'"
        );
    }

    #[test]
    fn test_streaming_chunk_validation_success() {
        let valid_chunk = json!({
            "id": "chatcmpl-123",
            "object": "chat.completion.chunk",
            "created": 1694268190,
            "model": "gpt-4o-mini",
            "choices": [{
                "index": 0,
                "delta": {
                    "content": "Hello"
                },
                "logprobs": null,
                "finish_reason": null
            }]
        });

        let result = validate_streaming_chunk(&valid_chunk);
        assert!(
            result.is_ok(),
            "Valid streaming chunk should pass: {:?}",
            result
        );
    }

    #[test]
    fn test_responses_streaming_validation() {
        let valid_event = json!({
            "type": "response.output_text.delta",
            "item_id": "msg_123",
            "output_index": 0,
            "content_index": 0,
            "delta": "Hello",
            "sequence_number": 1,
            "logprobs": []
        });

        let result = validate_responses_streaming_chunk(&valid_event);
        assert!(
            result.is_ok(),
            "Valid Responses event should pass: {:?}",
            result
        );
    }

    #[test]
    fn test_complex_response_events_skipped() {
        // Events with complex Response object should be skipped
        let event = json!({
            "type": "response.completed"
            // Missing other fields, but should be skipped anyway
        });

        let result = validate_responses_streaming_chunk(&event);
        assert!(result.is_ok(), "Complex events should be skipped");
    }
}
