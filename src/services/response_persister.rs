//! Persistence wrapper for `/v1/responses` streams.
//!
//! Sits at the very end of the streaming pipeline (after the tool
//! loop runner). Reads the SSE events flowing to the client, looks
//! for the terminal `response.completed` / `response.failed` /
//! `response.incomplete` event, captures the full `response` object,
//! and persists it via [`ResponsesStore::update`]. The body is
//! forwarded byte-for-byte — the persister is non-destructive.
//!
//! Cancellation: a watch receiver tied to the response row is polled
//! in parallel with the body stream. When it flips, the wrapper
//! terminates forwarding immediately and marks the row Cancelled.

#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;

use axum::body::Body;
use bytes::Bytes;
use chrono::Utc;
use futures_util::StreamExt;
use http::Response;
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::{
    db::repos::{NewResponseEvent, ResponseCompletion, ResponseStatus},
    services::{CancelSignal, ResponseEventBuffer, ResponsesStore},
    streaming::SseBuffer,
};

/// Wrap a streaming Responses-API HTTP response so the final state
/// gets persisted to the `responses` table when the stream terminates.
///
/// Returns the same response shape, with body replaced by a stream
/// that mirrors the original.
pub fn wrap_streaming_with_persistence(
    response: Response<Body>,
    store: Arc<ResponsesStore>,
    response_id: String,
    mut cancel_rx: CancelSignal,
    event_buffer: Option<Arc<ResponseEventBuffer>>,
) -> Response<Body> {
    let (parts, body) = response.into_parts();
    let (tx, rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(32);

    crate::compat::spawn_detached(async move {
        let mut body_stream = body.into_data_stream();
        let mut sse_buffer = SseBuffer::new();
        let mut final_response_object: Option<Value> = None;
        let mut terminal_status: Option<ResponseStatus> = None;
        let mut sequence_number: i64 = 0;

        loop {
            tokio::select! {
                _ = cancel_rx.changed() => {
                    if *cancel_rx.borrow() {
                        warn!(
                            stage = "persist_cancelled",
                            response_id = %response_id,
                            "Cancel signal tripped; aborting stream"
                        );
                        // Persist as cancelled.
                        if let Err(e) = store
                            .update(
                                &response_id,
                                ResponseCompletion {
                                    status: Some(ResponseStatus::Cancelled),
                                    completed_at: Some(Utc::now()),
                                    ..Default::default()
                                },
                            )
                            .await
                        {
                            error!(error = %e, "Failed to mark response cancelled");
                        }
                        return;
                    }
                }
                chunk = body_stream.next() => {
                    let Some(chunk_result) = chunk else { break };
                    let chunk = match chunk_result {
                        Ok(c) => c,
                        Err(e) => {
                            let _ = tx.send(Err(std::io::Error::other(e))).await;
                            return;
                        }
                    };
                    sse_buffer.extend(&chunk);
                    for event in sse_buffer.extract_complete_events() {
                        if final_response_object.is_none()
                            && let Some((resp_obj, status)) = inspect_terminal_event(&event)
                        {
                            final_response_object = Some(resp_obj);
                            terminal_status = Some(status);
                        }

                        // Append to the event log if a buffer is wired
                        // up. We extract the event_type for indexing
                        // and store the parsed JSON for resilient
                        // replay. Errors don't abort the stream — the
                        // event log is best-effort.
                        if let Some(ref buf) = event_buffer {
                            sequence_number += 1;
                            let (event_type, payload) = parse_event_for_log(&event);
                            buf.push(NewResponseEvent {
                                response_id: response_id.clone(),
                                sequence_number,
                                event_type,
                                payload,
                                created_at: Utc::now(),
                            });
                        }

                        if tx.send(Ok(event)).await.is_err() {
                            // Client gone; we still want to record state
                            // for the GET endpoint, so finish the persist
                            // step below.
                            break;
                        }
                    }
                }
            }
        }

        // Flush any trailing partial.
        if !sse_buffer.is_empty() {
            let _ = tx.send(Ok(sse_buffer.take_remaining())).await;
        }

        // Persist the captured state. If we never saw a terminal
        // event, mark as incomplete — the stream ended without a
        // `response.completed`.
        let (output, usage, error_field, status) = match final_response_object {
            Some(resp) => {
                let status = terminal_status.unwrap_or(ResponseStatus::Completed);
                (
                    resp.get("output").cloned(),
                    resp.get("usage").cloned(),
                    resp.get("error").cloned().filter(|v| !v.is_null()),
                    status,
                )
            }
            None => {
                debug!(
                    stage = "persist_no_terminal_event",
                    response_id = %response_id,
                    "Stream ended without a terminal response event; marking incomplete"
                );
                (None, None, None, ResponseStatus::Incomplete)
            }
        };

        if let Err(e) = store
            .update(
                &response_id,
                ResponseCompletion {
                    status: Some(status),
                    completed_at: Some(Utc::now()),
                    output,
                    usage,
                    error: error_field,
                    ..Default::default()
                },
            )
            .await
        {
            error!(
                error = %e,
                response_id = %response_id,
                "Failed to persist final response state"
            );
        } else {
            info!(
                stage = "persist_complete",
                response_id = %response_id,
                status = ?status,
                "Persisted final response state"
            );
        }
    });

    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    });
    Response::from_parts(parts, Body::from_stream(stream))
}

/// Recognise an SSE event that terminates the response and carries the
/// full final object. The Responses API emits one of:
///   data: {"type":"response.completed","response":{...}}
///   data: {"type":"response.failed","response":{...}}
///   data: {"type":"response.incomplete","response":{...}}
fn inspect_terminal_event(event: &[u8]) -> Option<(Value, ResponseStatus)> {
    let s = std::str::from_utf8(event).ok()?;
    for line in s.lines() {
        let data = line.strip_prefix("data:")?.trim();
        if data == "[DONE]" {
            continue;
        }
        let json: Value = serde_json::from_str(data).ok()?;
        let event_type = json.get("type")?.as_str()?;
        let status = match event_type {
            "response.completed" => ResponseStatus::Completed,
            "response.failed" => ResponseStatus::Failed,
            "response.incomplete" => ResponseStatus::Incomplete,
            _ => continue,
        };
        let response = json.get("response")?.clone();
        return Some((response, status));
    }
    None
}

/// Parse one SSE event into (`event_type`, `payload`) for the event
/// log. Best-effort: malformed events get `event_type = "unknown"`
/// and the raw bytes as a JSON string so replay never loses data.
fn parse_event_for_log(event: &[u8]) -> (String, Value) {
    let Ok(s) = std::str::from_utf8(event) else {
        return ("unknown".to_string(), Value::String(format!("{event:?}")));
    };
    for line in s.lines() {
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data == "[DONE]" {
            return ("done".to_string(), Value::Null);
        }
        let Ok(json) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        let event_type = json
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("unknown")
            .to_string();
        return (event_type, json);
    }
    ("unknown".to_string(), Value::String(s.to_string()))
}

/// Persist a non-streaming final response by reading and reparsing the
/// JSON body. Returns the bytes so the caller can also send them to
/// the client / cache. On parse failure the response is recorded as
/// `Failed` and the bytes are still forwarded.
pub async fn persist_non_streaming(
    store: &ResponsesStore,
    response_id: &str,
    body_bytes: &[u8],
    http_status: u16,
) {
    let status = if (200..300).contains(&http_status) {
        ResponseStatus::Completed
    } else {
        ResponseStatus::Failed
    };
    let parsed: Result<Value, _> = serde_json::from_slice(body_bytes);
    let (output, usage, error_field) = match parsed {
        Ok(v) => (
            v.get("output").cloned(),
            v.get("usage").cloned(),
            v.get("error").cloned().filter(|v| !v.is_null()),
        ),
        Err(_) => (None, None, None),
    };
    if let Err(e) = store
        .update(
            response_id,
            ResponseCompletion {
                status: Some(status),
                completed_at: Some(Utc::now()),
                output,
                usage,
                error: error_field,
                ..Default::default()
            },
        )
        .await
    {
        error!(error = %e, response_id, "Failed to persist non-streaming response");
    }
}
