#![allow(dead_code)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize, Serializer};

use super::chat_completion::CacheControl;

/// Serialize f64 as i64 when it's a whole number, to satisfy APIs that expect integer types.
fn serialize_as_integer<S>(value: &Option<f64>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(v) if v.fract() == 0.0 => serializer.serialize_i64(*v as i64),
        Some(v) => serializer.serialize_f64(*v),
        None => serializer.serialize_none(),
    }
}
use validator::Validate;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResponseInputImageDetail {
    Auto,
    High,
    Low,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResponseInputAudioFormat {
    Mp3,
    Wav,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EasyInputMessageRole {
    User,
    System,
    Assistant,
    Developer,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InputMessageItemRole {
    User,
    System,
    Developer,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    InProgress,
    Completed,
    Incomplete,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputMessageStatus {
    Completed,
    Incomplete,
    InProgress,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputItemReasoningStatus {
    Completed,
    Incomplete,
    InProgress,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputItemFunctionCallStatus {
    Completed,
    Incomplete,
    InProgress,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WebSearchStatus {
    Completed,
    Searching,
    InProgress,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImageGenerationStatus {
    InProgress,
    Completed,
    Generating,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OpenResponsesReasoningFormat {
    Unknown,
    #[serde(rename = "openai-responses-v1")]
    OpenaiResponsesV1,
    #[serde(rename = "xai-responses-v1")]
    XaiResponsesV1,
    #[serde(rename = "anthropic-claude-v1")]
    AnthropicClaudeV1,
    #[serde(rename = "google-gemini-v1")]
    GoogleGeminiV1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResponsesSearchContextSize {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResponsesReasoningEffort {
    High,
    Medium,
    Low,
    Minimal,
    None,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResponsesReasoningSummary {
    Auto,
    Concise,
    Detailed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResponseTextConfigVerbosity {
    High,
    Low,
    Medium,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DataVectorStore {
    Deny,
    Allow,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Quantization {
    Int4,
    Int8,
    Fp4,
    Fp6,
    Fp8,
    Fp16,
    Bf16,
    Fp32,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProviderSort {
    Price,
    Throughput,
    Latency,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResponsesResponseStatus {
    Completed,
    Incomplete,
    InProgress,
    Failed,
    Cancelled,
    Queued,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResponsesErrorCode {
    ServerError,
    RateLimitExceeded,
    InvalidPrompt,
    VectorStoreTimeout,
    InvalidImage,
    InvalidImageFormat,
    InvalidBase64Image,
    InvalidImageUrl,
    ImageTooLarge,
    ImageTooSmall,
    ImageParseError,
    ImageContentPolicyViolation,
    InvalidImageMode,
    ImageFileTooLarge,
    UnsupportedImageMediaType,
    EmptyImageFile,
    FailedToDownloadImage,
    ImageFileNotFound,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IncompleteDetailsReason {
    MaxOutputTokens,
    ContentFilter,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResponsesIncludable {
    #[serde(rename = "file_search_call.results")]
    FileSearchCallResults,
    #[serde(rename = "message.input_image.image_url")]
    MessageInputImageImageUrl,
    #[serde(rename = "computer_call_output.output.image_url")]
    ComputerCallOutputOutputImageUrl,
    #[serde(rename = "reasoning.encrypted_content")]
    ReasoningEncryptedContent,
    #[serde(rename = "code_interpreter_call.outputs")]
    CodeInterpreterCallOutputs,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResponsesServiceTier {
    Auto,
    Default,
    Flex,
    Priority,
    Scale,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResponsesTruncation {
    Auto,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseInputAudioData {
    pub data: String,
    pub format: ResponseInputAudioFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::enum_variant_names)] // Intentional: matches OpenAI API spec
pub enum ResponseInputContentItem {
    InputText {
        text: String,
        /// **Hadrian Extension:** Cache control for prompt caching (Anthropic/Bedrock)
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    InputImage {
        detail: ResponseInputImageDetail,
        #[serde(skip_serializing_if = "Option::is_none")]
        image_url: Option<String>,
        /// **Hadrian Extension:** Cache control for prompt caching (Anthropic/Bedrock)
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    InputFile {
        #[serde(skip_serializing_if = "Option::is_none")]
        file_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        file_data: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        file_url: Option<String>,
        /// **Hadrian Extension:** Cache control for prompt caching (Anthropic/Bedrock)
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    InputAudio {
        input_audio: ResponseInputAudioData,
        /// **Hadrian Extension:** Cache control for prompt caching (Anthropic/Bedrock)
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EasyInputMessageContent {
    Text(String),
    Parts(Vec<ResponseInputContentItem>),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningTextContentType {
    ReasoningText,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningTextContent {
    #[serde(rename = "type")]
    pub type_: ReasoningTextContentType,
    pub text: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningSummaryTextType {
    SummaryText,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningSummaryText {
    #[serde(rename = "type")]
    pub type_: ReasoningSummaryTextType,
    pub text: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResponsesReasoningType {
    Reasoning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesReasoning {
    #[serde(rename = "type")]
    pub type_: ResponsesReasoningType,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<ReasoningTextContent>>,
    pub summary: Vec<ReasoningSummaryText>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<OutputItemReasoningStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<OpenResponsesReasoningFormat>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Citation & Annotation Types
// ─────────────────────────────────────────────────────────────────────────────
//
// Annotations are metadata attached to response text indicating source citations.
// When file_search finds relevant content, the model references it using markers
// like `[Source 1]`. These markers are then converted to structured annotations
// with byte positions pointing to where the citation appears in the response.
//
// ## Citation Flow
//
// 1. File search returns results numbered as Source 1, Source 2, etc.
// 2. Model generates response text with citation markers: "According to [Source 1]..."
// 3. CitationTracker parses markers and creates FileCitation annotations
// 4. Annotations are injected into `response.content_part.done` SSE events
//
// ## Frontend Rendering
//
// Clients should:
// 1. Parse the `annotations` array from `output_text` content items
// 2. Use `index` to locate citation markers in the text
// 3. Optionally replace markers with interactive citation UI elements
// 4. Link citations to files using `file_id` for navigation/preview
//
// ## Example Response
//
// ```json
// {
//   "type": "output_text",
//   "text": "According to [Source 1], revenue increased by 15%.",
//   "annotations": [
//     {
//       "type": "file_citation",
//       "file_id": "file-abc123",
//       "filename": "q3_report.pdf",
//       "index": 13
//     }
//   ]
// }
// ```
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileCitationType {
    FileCitation,
}

/// A citation pointing to a file used as a source in the response.
///
/// Generated when the model references content from file_search results.
/// The `index` field indicates where in the response text the citation
/// marker (e.g., `[Source 1]`) appears.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCitation {
    #[serde(rename = "type")]
    pub type_: FileCitationType,
    /// The unique identifier of the cited file (prefixed with `file-`).
    pub file_id: String,
    /// The display name of the cited file.
    pub filename: String,
    /// Byte offset in the response text where the citation marker starts.
    pub index: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UrlCitationType {
    UrlCitation,
}

/// A citation pointing to a URL used as a source in the response.
///
/// Generated when the model references content from web search results.
/// The `start_index` and `end_index` fields define the byte range in
/// the response text that should be associated with this citation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlCitation {
    #[serde(rename = "type")]
    pub type_: UrlCitationType,
    /// The source URL.
    pub url: String,
    /// The title of the web page.
    pub title: String,
    /// Byte offset where the cited text range begins.
    pub start_index: u64,
    /// Byte offset where the cited text range ends (exclusive).
    pub end_index: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilePathType {
    FilePath,
}

/// A reference to a file path generated by the model (e.g., from code_interpreter).
///
/// Unlike `FileCitation` which points to source files, `FilePath` references
/// files that were created or modified during response generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePath {
    #[serde(rename = "type")]
    pub type_: FilePathType,
    /// The unique identifier of the referenced file.
    pub file_id: String,
    /// Byte offset in the response text where the file reference appears.
    pub index: u64,
}

/// Annotation types that can be attached to response text.
///
/// Annotations provide structured metadata about citations and references
/// within model-generated text. They enable clients to render interactive
/// citations, link to source materials, and provide transparency about
/// which sources informed the response.
///
/// ## Annotation Types
///
/// - **FileCitation**: References a file from vector store search results.
///   The model marks these with `[Source N]` patterns which are converted
///   to structured annotations with the source file information.
///
/// - **UrlCitation**: References a web page from web search results.
///   Includes the full URL and page title for linking.
///
/// - **FilePath**: References a file generated during response creation
///   (e.g., by code_interpreter). Points to downloadable output files.
///
/// ## Index Fields
///
/// All annotation types include index fields indicating byte positions
/// in the response text:
///
/// - `index`: Single position where a marker like `[Source 1]` starts
/// - `start_index`/`end_index`: Range of text associated with a citation
///
/// These are **byte offsets**, not character offsets. For UTF-8 text with
/// multi-byte characters, clients must account for encoding when mapping
/// indices to display positions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponsesAnnotation {
    /// Citation to a file from vector store search.
    FileCitation {
        /// The unique identifier of the cited file.
        file_id: String,
        /// The display name of the cited file.
        filename: String,
        /// Byte offset where the citation marker (e.g., `[Source 1]`) starts.
        index: u64,
    },
    /// Citation to a URL from web search.
    UrlCitation {
        /// The source URL.
        url: String,
        /// The title of the web page.
        title: String,
        /// Byte offset where the cited text range begins.
        start_index: u64,
        /// Byte offset where the cited text range ends (exclusive).
        end_index: u64,
    },
    /// Reference to a generated file path.
    FilePath {
        /// The unique identifier of the referenced file.
        file_id: String,
        /// Byte offset where the file reference appears.
        index: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputMessageContentItem {
    OutputText {
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        annotations: Vec<ResponsesAnnotation>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        logprobs: Vec<serde_json::Value>,
    },
    Refusal {
        refusal: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    Message,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EasyInputMessage {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<MessageType>,
    pub role: EasyInputMessageRole,
    pub content: EasyInputMessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputMessageItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<MessageType>,
    pub role: InputMessageItemRole,
    pub content: Vec<ResponseInputContentItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputMessage {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: MessageType,
    pub role: String, // "assistant"
    pub content: Vec<OutputMessageContentItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<OutputMessageStatus>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FunctionToolCallType {
    FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionToolCall {
    #[serde(rename = "type")]
    pub type_: FunctionToolCallType,
    pub id: String,
    pub call_id: String,
    pub name: String,
    pub arguments: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ToolCallStatus>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FunctionCallOutputType {
    FunctionCallOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCallOutput {
    #[serde(rename = "type")]
    pub type_: FunctionCallOutputType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub call_id: String,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ToolCallStatus>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputItemFunctionCallType {
    FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputItemFunctionCall {
    #[serde(rename = "type")]
    pub type_: OutputItemFunctionCallType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub arguments: String,
    pub call_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<OutputItemFunctionCallStatus>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WebSearchCallOutputType {
    WebSearchCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchCallOutput {
    #[serde(rename = "type")]
    pub type_: WebSearchCallOutputType,
    pub id: String,
    pub status: WebSearchStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileSearchCallOutputType {
    FileSearchCall,
}

/// Content item within a file search result.
///
/// Matches OpenAI's format where content is an array of typed items.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FileSearchResultContent {
    /// Text content from the search result.
    Text { text: String },
}

/// A single result item from a file search operation.
///
/// This matches OpenAI's file search result schema when `include=["file_search_call.results"]`
/// is specified in the request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchResultItem {
    /// The ID of the file this result came from.
    pub file_id: String,
    /// The filename of the source file.
    pub filename: String,
    /// Relevance score between 0 and 1.
    pub score: f64,
    /// Optional attributes/metadata associated with the file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<HashMap<String, serde_json::Value>>,
    /// The content retrieved from the file.
    /// OpenAI uses an array format with typed content items.
    pub content: Vec<FileSearchResultContent>,
}

/// Output item for a file_search tool call.
///
/// When the model invokes file_search, this output item is included in the response
/// to show the queries that were searched and (optionally) the search results.
///
/// The `results` field is only populated when the request includes
/// `include=["file_search_call.results"]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchCallOutput {
    #[serde(rename = "type")]
    pub type_: FileSearchCallOutputType,
    /// Unique identifier for this file search call.
    pub id: String,
    /// The search queries executed.
    pub queries: Vec<String>,
    /// Status of the file search operation.
    pub status: WebSearchStatus,
    /// Search results, only included when requested via the `include` parameter.
    /// When not included, this field is omitted from the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub results: Option<Vec<FileSearchResultItem>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImageGenerationCallType {
    ImageGenerationCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenerationCall {
    #[serde(rename = "type")]
    pub type_: ImageGenerationCallType,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    pub status: ImageGenerationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponsesInputItem {
    Reasoning(ResponsesReasoning),
    EasyMessage(EasyInputMessage),
    MessageItem(InputMessageItem),
    FunctionCall(FunctionToolCall),
    FunctionCallOutput(FunctionCallOutput),
    OutputMessage(OutputMessage),
    OutputFunctionCall(OutputItemFunctionCall),
    WebSearchCall(WebSearchCallOutput),
    FileSearchCall(FileSearchCallOutput),
    ImageGeneration(ImageGenerationCall),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponsesInput {
    Text(String),
    Items(Vec<ResponsesInputItem>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponsesOutputItem {
    Message(OutputMessage),
    Reasoning(ResponsesReasoning),
    FunctionCall(OutputItemFunctionCall),
    WebSearchCall(WebSearchCallOutput),
    FileSearchCall(FileSearchCallOutput),
    ImageGeneration(ImageGenerationCall),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WebSearchUserLocationType {
    Approximate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchUserLocation {
    #[serde(rename = "type")]
    pub type_: WebSearchUserLocationType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchFilters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_domains: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WebSearchPreviewToolType {
    WebSearchPreview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchPreviewTool {
    #[serde(rename = "type")]
    pub type_: WebSearchPreviewToolType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_context_size: Option<ResponsesSearchContextSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_location: Option<WebSearchUserLocation>,
    /// **Hadrian Extension:** Cache control for prompt caching (Anthropic/Bedrock)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebSearchPreview20250311ToolType {
    #[serde(rename = "web_search_preview_2025_03_11")]
    WebSearchPreview20250311,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchPreview20250311Tool {
    #[serde(rename = "type")]
    pub type_: WebSearchPreview20250311ToolType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_context_size: Option<ResponsesSearchContextSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_location: Option<WebSearchUserLocation>,
    /// **Hadrian Extension:** Cache control for prompt caching (Anthropic/Bedrock)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WebSearchToolType {
    WebSearch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchTool {
    #[serde(rename = "type")]
    pub type_: WebSearchToolType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<WebSearchFilters>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_context_size: Option<ResponsesSearchContextSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_location: Option<WebSearchUserLocation>,
    /// **Hadrian Extension:** Cache control for prompt caching (Anthropic/Bedrock)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebSearch20250826ToolType {
    #[serde(rename = "web_search_2025_08_26")]
    WebSearch20250826,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearch20250826Tool {
    #[serde(rename = "type")]
    pub type_: WebSearch20250826ToolType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<WebSearchFilters>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_context_size: Option<ResponsesSearchContextSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_location: Option<WebSearchUserLocation>,
    /// **Hadrian Extension:** Cache control for prompt caching (Anthropic/Bedrock)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

// ─────────────────────────────────────────────────────────────────────────────
// File Search Tool (for Responses API RAG)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileSearchToolType {
    FileSearch,
}

/// File search ranking options for controlling result relevance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchRankingOptions {
    /// The ranker to use for scoring results.
    /// Values: "auto" or "default-2024-11-15"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ranker: Option<String>,
    /// Minimum score threshold (0.0-1.0) for results to be included.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score_threshold: Option<f64>,
}

/// Filter comparison types for file search metadata filtering.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FileSearchFilterComparison {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
}

/// A single comparison filter for file search.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct FileSearchComparisonFilter {
    #[serde(rename = "type")]
    pub type_: FileSearchFilterComparison,
    pub key: String,
    pub value: serde_json::Value,
}

/// Logical operator types for compound filters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FileSearchFilterLogicalType {
    And,
    Or,
}

/// A compound filter combining multiple filters with a logical operator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct FileSearchCompoundFilter {
    #[serde(rename = "type")]
    pub type_: FileSearchFilterLogicalType,
    pub filters: Vec<FileSearchFilter>,
}

/// File search filter - either a comparison or a compound filter.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum FileSearchFilter {
    Comparison(FileSearchComparisonFilter),
    Compound(FileSearchCompoundFilter),
}

/// File search tool for RAG in the Responses API.
/// Enables semantic search across vector stores.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchTool {
    #[serde(rename = "type")]
    pub type_: FileSearchToolType,
    /// Vector store IDs to search across.
    pub vector_store_ids: Vec<String>,
    /// Maximum number of results to return.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_num_results: Option<usize>,
    /// Ranking options for controlling result relevance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ranking_options: Option<FileSearchRankingOptions>,
    /// Metadata filters to apply to the search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<FileSearchFilter>,
    /// **Hadrian Extension:** Cache control for prompt caching (Anthropic/Bedrock)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

impl FileSearchTool {
    /// Check if this is a file_search tool.
    pub fn is_file_search(&self) -> bool {
        matches!(self.type_, FileSearchToolType::FileSearch)
    }
}

/// Tool definition - can be a function tool, web search tool, or file search tool
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponsesToolDefinition {
    FileSearch(FileSearchTool), // Must be before Function to match type field first
    Function(serde_json::Value), // Generic function tool with any structure
    WebSearchPreview(WebSearchPreviewTool),
    WebSearchPreview20250311(WebSearchPreview20250311Tool),
    WebSearch(WebSearchTool),
    WebSearch20250826(WebSearch20250826Tool),
}

impl ResponsesToolDefinition {
    /// Returns the file search tool if this is a file_search tool definition.
    pub fn as_file_search(&self) -> Option<&FileSearchTool> {
        match self {
            ResponsesToolDefinition::FileSearch(tool) => Some(tool),
            _ => None,
        }
    }

    /// Returns true if this is a file_search tool.
    pub fn is_file_search(&self) -> bool {
        matches!(self, ResponsesToolDefinition::FileSearch(_))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResponsesToolChoiceDefault {
    Auto,
    None,
    Required,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResponsesNamedToolChoiceType {
    Function,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesNamedToolChoice {
    #[serde(rename = "type")]
    pub type_: ResponsesNamedToolChoiceType,
    pub name: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WebSearchToolChoiceType {
    #[serde(rename = "web_search_preview_2025_03_11")]
    WebSearchPreview20250311,
    #[serde(rename = "web_search_preview")]
    WebSearchPreview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesWebSearchToolChoice {
    #[serde(rename = "type")]
    pub type_: WebSearchToolChoiceType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponsesToolChoice {
    String(ResponsesToolChoiceDefault),
    Named(ResponsesNamedToolChoice),
    WebSearch(ResponsesWebSearchToolChoice),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseFormatTextConfig {
    Text,
    JsonObject,
    JsonSchema {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        strict: Option<bool>,
        schema: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ResponseTextConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<ResponseFormatTextConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbosity: Option<ResponseTextConfigVerbosity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ResponsesReasoningConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<ResponsesReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<ResponsesReasoningSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PromptVariable {
    Text(String),
    InputText {
        #[serde(rename = "type")]
        type_: String,
        text: String,
    },
    InputImage {
        #[serde(rename = "type")]
        type_: String,
        detail: ResponseInputImageDetail,
        #[serde(skip_serializing_if = "Option::is_none")]
        image_url: Option<String>,
    },
    InputFile {
        #[serde(rename = "type")]
        type_: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        file_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        file_data: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        file_url: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesPrompt {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<HashMap<String, PromptVariable>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BigNumberUnion {
    Number(f64),
    String(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMaxPrice {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<BigNumberUnion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion: Option<BigNumberUnion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<BigNumberUnion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<BigNumberUnion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<BigNumberUnion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProviderNameOrString {
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesProviderConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_fallbacks: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_parameters: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_vector_store: Option<DataVectorStore>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zdr: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enforce_distillable_text: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<ProviderNameOrString>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<ProviderNameOrString>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore: Option<Vec<ProviderNameOrString>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantizations: Option<Vec<Quantization>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<ProviderSort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_price: Option<ProviderMaxPrice>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WebPluginEngine {
    Native,
    Exa,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FileParserPdfEngine {
    MistralOcr,
    PdfText,
    Native,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileParserPdfConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<FileParserPdfEngine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "id", rename_all = "kebab-case")]
pub enum ResponsesPlugin {
    Moderation,
    Web {
        #[serde(skip_serializing_if = "Option::is_none")]
        max_results: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        search_prompt: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        engine: Option<WebPluginEngine>,
    },
    #[serde(rename = "file-parser")]
    FileParser {
        #[serde(skip_serializing_if = "Option::is_none")]
        max_files: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pdf: Option<FileParserPdfConfig>,
    },
}

/// Create responses request (OpenAI Responses API)
#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateResponsesPayload {
    /// Input messages/items
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub input: Option<ResponsesInput>,

    /// System instructions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,

    /// Request metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub metadata: Option<HashMap<String, String>>,

    /// Available tools
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Vec<Object>))]
    pub tools: Option<Vec<ResponsesToolDefinition>>,

    /// Tool choice configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub tool_choice: Option<ResponsesToolChoice>,

    /// Allow parallel tool calls
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,

    /// Model to use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// **Hadrian Extension:** List of models for multi-model routing (alternative to single model)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,

    /// Text configuration
    #[validate(nested)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub text: Option<ResponseTextConfig>,

    /// Reasoning configuration
    #[validate(nested)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub reasoning: Option<ResponsesReasoningConfig>,

    /// Maximum output tokens
    #[validate(range(min = 1.0))]
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_as_integer"
    )]
    pub max_output_tokens: Option<f64>,

    /// Sampling temperature (0.0 to 2.0)
    #[validate(range(min = 0.0, max = 2.0))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,

    /// Nucleus sampling probability (0.0 to 1.0)
    #[validate(range(min = 0.0, max = 1.0))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,

    /// **Hadrian Extension:** Top-k sampling (supported by some providers like Anthropic)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<f64>,

    /// Prompt cache key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,

    /// Previous response ID for conversation continuation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,

    /// Prompt template reference
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub prompt: Option<ResponsesPrompt>,

    /// Items to include in response
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Vec<String>))]
    pub include: Option<Vec<ResponsesIncludable>>,

    /// Run in background
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,

    /// Safety identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_identifier: Option<String>,

    /// Store response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,

    /// Service tier
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub service_tier: Option<serde_json::Value>,

    /// Truncation strategy
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub truncation: Option<serde_json::Value>,

    /// Enable streaming
    #[serde(default)]
    pub stream: bool,

    /// **Hadrian Extension:** Provider routing configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub provider: Option<ResponsesProviderConfig>,

    /// **Hadrian Extension:** Plugins to enable for this request
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Vec<Object>))]
    pub plugins: Option<Vec<ResponsesPlugin>>,

    /// User identifier for abuse detection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesErrorField {
    pub code: ResponsesErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesIncompleteDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<IncompleteDetailsReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesUsageInputTokensDetails {
    pub cached_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesUsageOutputTokensDetails {
    pub reasoning_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesUsageCostDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream_inference_cost: Option<f64>,
    pub upstream_inference_input_cost: f64,
    pub upstream_inference_output_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesUsage {
    pub input_tokens: i64,
    pub input_tokens_details: ResponsesUsageInputTokensDetails,
    pub output_tokens: i64,
    pub output_tokens_details: ResponsesUsageOutputTokensDetails,
    pub total_tokens: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_byok: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_details: Option<ResponsesUsageCostDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesReasoningConfigOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<ResponsesReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<ResponsesReasoningSummary>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResponseType {
    Response,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResponsesResponse {
    pub id: String,
    pub object: ResponseType,
    pub created_at: f64,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ResponsesResponseStatus>,
    pub output: Vec<ResponsesOutputItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_identifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ResponsesErrorField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incomplete_details: Option<ResponsesIncompleteDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ResponsesUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tool_calls: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_as_integer"
    )]
    pub max_output_tokens: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ResponsesToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ResponsesToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<ResponsesPrompt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ResponsesReasoningConfigOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<ResponsesServiceTier>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation: Option<ResponsesTruncation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<ResponseTextConfig>,
}
