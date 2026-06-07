//! Small text-extraction helpers shared by the provider request converters.
//!
//! Anthropic, Bedrock, and Vertex all need to pull the plain text out of a
//! Responses-API message so it can be folded into their native system prompt.
//! The logic is provider-agnostic (it only touches shared `api_types`), so it
//! lives here rather than being duplicated per provider.

use crate::api_types::responses::{EasyInputMessageContent, ResponseInputContentItem};

/// Extract the concatenated text from an easy-input message content value.
pub(crate) fn easy_content_text(content: &EasyInputMessageContent) -> String {
    match content {
        EasyInputMessageContent::Text(text) => text.clone(),
        EasyInputMessageContent::Parts(parts) => input_content_text(parts),
    }
}

/// Extract the concatenated `input_text` from a list of input content items.
pub(crate) fn input_content_text(parts: &[ResponseInputContentItem]) -> String {
    parts
        .iter()
        .filter_map(|part| match part {
            ResponseInputContentItem::InputText { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
