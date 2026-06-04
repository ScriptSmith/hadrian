//! Shared history-rewriting for server-executed tools.
//!
//! Hadrian self-executes its server tools by rewriting each to a function tool
//! (`web_search` / `file_search` / `mcp_<label>__<tool>`), so the upstream
//! provider never produces the corresponding native hosted item. The
//! spec-shaped hosted items Hadrian synthesizes for the client
//! (`web_search_call`, `file_search_call`, `mcp_call`) are persisted in the
//! stored response, so on a follow-up turn they come back as input — rebuilt
//! from a `previous_response_id` chain (`services/responses_chain.rs`) or
//! resent by a client doing manual multi-turn.
//!
//! Forwarded verbatim, no upstream accepts them: OpenAI-compatible providers
//! (e.g. OpenRouter) reject the turn with `invalid_prompt`, and
//! Anthropic/Bedrock/Vertex silently drop them. Rewriting each hosted item back
//! into the `function_call` + `function_call_output` pair every provider
//! understands — with the issued query as the call arguments and the retained
//! result text as the output — keeps multi-turn coherent behind any provider
//! and lets the model draw on the earlier results.
//!
//! All three 1→2 rewrites share this driver; each tool supplies only the
//! per-item conversion (`web_search_tool::rewrite_web_search_history`,
//! `file_search_tool::rewrite_file_search_history`,
//! `mcp::preprocess::rewrite_mcp_history`). The shell tool is the one server
//! tool that does *not* use this: its history is already two items
//! (`shell_call` plus `shell_call_output`), so it rewrites them 1:1 in place
//! (`shell_tool::rewrite_shell_history_to_function_calls`) and carries its own
//! output-ordering fixups.

use crate::api_types::responses::{
    CreateResponsesPayload, FunctionCallOutput, FunctionToolCall, ResponsesInput,
    ResponsesInputItem,
};

/// Expand every hosted server-tool item in `payload.input` into the
/// `function_call` + `function_call_output` pair every provider understands.
///
/// `expand` inspects each item and returns `Some((call, output))` for the ones
/// its tool owns, or `None` to leave the item untouched. The call and its output
/// share a `call_id` so the per-provider conversion pairs them. A no-op when the
/// input isn't an item list.
pub fn rewrite_hosted_calls_to_function_pairs(
    payload: &mut CreateResponsesPayload,
    expand: impl Fn(&ResponsesInputItem) -> Option<(FunctionToolCall, FunctionCallOutput)>,
) {
    let Some(ResponsesInput::Items(items)) = payload.input.as_mut() else {
        return;
    };
    // Each owned call expands to two items; over-reserve by a small amount
    // rather than reallocate mid-rewrite.
    let mut rewritten = Vec::with_capacity(items.len() + 1);
    for item in std::mem::take(items) {
        match expand(&item) {
            Some((call, output)) => {
                rewritten.push(ResponsesInputItem::FunctionCall(call));
                rewritten.push(ResponsesInputItem::FunctionCallOutput(output));
            }
            None => rewritten.push(item),
        }
    }
    *items = rewritten;
}
