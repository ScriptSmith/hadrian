//! `GET`/`POST cancel`/`DELETE` handlers for stored Responses API
//! records, matching OpenAI's Responses API spec.
//!
//! Persistence happens during `POST /v1/responses` (see chat.rs); these
//! endpoints surface the resulting rows.

#![cfg(feature = "server")]

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Serialize;
use serde_json::{Map, Value};

use super::ApiError;
use crate::{
    AppState,
    auth::AuthenticatedRequest,
    db::repos::ResponseRecord,
    services::{ResponsesStore, ResponsesStoreError},
};

/// Wire-format response shape. Wraps the stored JSON output and stamps
/// gateway-controlled fields (id, status, created_at). All other
/// fields come from the persisted request_payload / output / usage so
/// the surface matches OpenAI's spec.
#[derive(Serialize)]
pub struct WireResponse {
    id: String,
    object: &'static str,
    status: &'static str,
    background: bool,
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<String>,
    created_at: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    completed_at: Option<f64>,
    #[serde(skip_serializing_if = "Value::is_null")]
    output: Value,
    #[serde(skip_serializing_if = "Value::is_null")]
    usage: Value,
    #[serde(skip_serializing_if = "Value::is_null")]
    error: Value,
    /// Echo selected request-payload fields so the response carries the
    /// instructions, tools, etc. that originally drove it — same as
    /// OpenAI's Retrieve endpoint.
    #[serde(flatten)]
    echoed: Map<String, Value>,
}

fn record_to_wire(record: &ResponseRecord) -> WireResponse {
    // Pull selected request fields back into the top-level shape so
    // clients can introspect what they sent. Anything sensitive
    // (e.g. raw secret values) is omitted because callers only ever
    // stored placeholders.
    const ECHO_FIELDS: &[&str] = &[
        "input",
        "instructions",
        "metadata",
        "tools",
        "tool_choice",
        "parallel_tool_calls",
        "temperature",
        "top_p",
        "max_output_tokens",
        "reasoning",
        "text",
        "include",
        "store",
        "previous_response_id",
    ];
    let mut echoed = Map::new();
    if let Value::Object(obj) = &record.request_payload {
        for k in ECHO_FIELDS {
            if let Some(v) = obj.get(*k) {
                echoed.insert((*k).to_string(), v.clone());
            }
        }
    }
    WireResponse {
        id: record.id.clone(),
        object: "response",
        status: record.status.as_str(),
        background: record.background,
        model: record.model.clone(),
        provider: record.provider.clone(),
        created_at: record.created_at.timestamp() as f64,
        completed_at: record.completed_at.map(|t| t.timestamp() as f64),
        output: record.output.clone().unwrap_or(Value::Null),
        usage: record.usage.clone().unwrap_or(Value::Null),
        error: record.error.clone().unwrap_or(Value::Null),
        echoed,
    }
}

fn resolve_store(state: &AppState) -> Result<&ResponsesStore, ApiError> {
    state.responses_store.as_deref().ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_IMPLEMENTED,
            "responses_persistence_disabled",
            "Response persistence requires a configured database".to_string(),
        )
    })
}

fn caller_org(auth: Option<&Extension<AuthenticatedRequest>>) -> Option<uuid::Uuid> {
    auth.and_then(|Extension(a)| {
        a.api_key()
            .and_then(|k| k.org_id)
            .or_else(|| a.principal().org_id())
    })
}

fn map_store_err(e: ResponsesStoreError) -> ApiError {
    match e {
        ResponsesStoreError::NotFound => ApiError::new(
            StatusCode::NOT_FOUND,
            "response_not_found",
            "No such response".to_string(),
        ),
        ResponsesStoreError::NotBackground => ApiError::new(
            StatusCode::BAD_REQUEST,
            "response_not_background",
            "Only responses created with background=true can be cancelled".to_string(),
        ),
        ResponsesStoreError::Database(e) => ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        ),
        ResponsesStoreError::Internal(s) => {
            ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", s)
        }
    }
}

/// `GET /v1/responses/{response_id}` — retrieve a stored response.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/responses/{response_id}",
    tag = "responses",
    params(("response_id" = String, Path, description = "ID returned by POST /v1/responses")),
    responses(
        (status = 200, description = "The stored response object"),
        (status = 404, description = "Response not found", body = crate::openapi::ErrorResponse),
        (status = 501, description = "Persistence disabled", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
pub async fn api_v1_responses_get(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    Path(response_id): Path<String>,
) -> Result<Json<WireResponse>, ApiError> {
    let store = resolve_store(&state)?;
    let org_id = caller_org(auth.as_ref()).or(state.default_org_id);
    let record = store
        .get(&response_id, org_id)
        .await
        .map_err(map_store_err)?;
    Ok(Json(record_to_wire(&record)))
}

/// `POST /v1/responses/{response_id}/cancel` — cancel an in-progress
/// background response. Per OpenAI's spec, returns 400 if the response
/// is not in background mode. Idempotent for already-terminal rows.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/responses/{response_id}/cancel",
    tag = "responses",
    params(("response_id" = String, Path, description = "ID of the response to cancel")),
    responses(
        (status = 200, description = "The cancelled response object"),
        (status = 400, description = "Response is not in background mode", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Response not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
pub async fn api_v1_responses_cancel(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    Path(response_id): Path<String>,
) -> Result<Json<WireResponse>, ApiError> {
    let store = resolve_store(&state)?;
    let org_id = caller_org(auth.as_ref()).or(state.default_org_id);
    let record = store
        .cancel(&response_id, org_id)
        .await
        .map_err(map_store_err)?;
    Ok(Json(record_to_wire(&record)))
}

#[derive(Serialize)]
pub struct DeleteResponse {
    pub id: String,
    pub object: &'static str,
    pub deleted: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Event log replay
// ─────────────────────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct EventsQuery {
    /// Return events with `sequence_number > starting_after`. Clients
    /// pass the highest `sequence_number` they've already seen so
    /// reconnect resumes without duplicates.
    #[serde(default)]
    pub starting_after: Option<i64>,
    /// Soft cap on the number of events returned per page. Useful for
    /// non-streaming clients that want a single GET. Default 200.
    #[serde(default)]
    pub limit: Option<i64>,
}

/// `GET /v1/responses/{response_id}/events?starting_after=N` —
/// poll-replay reconnect.
///
/// Streams the persisted event log as Server-Sent Events. While the
/// response is still in-progress the handler polls the DB every
/// 250ms for new rows; once the row reaches a terminal status and the
/// caller has caught up to `last_sequence_number`, the stream
/// terminates with a `data: [DONE]` sentinel.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/responses/{response_id}/events",
    tag = "responses",
    params(
        ("response_id" = String, Path, description = "ID of the response"),
        ("starting_after" = Option<i64>, Query, description = "Resume cursor"),
        ("limit" = Option<i64>, Query, description = "Cap on events per page"),
    ),
    responses(
        (status = 200, description = "SSE stream of response events"),
        (status = 404, description = "Response not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
pub async fn api_v1_responses_events(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    Path(response_id): Path<String>,
    axum::extract::Query(query): axum::extract::Query<EventsQuery>,
) -> Result<axum::response::Response, ApiError> {
    use axum::body::Body;
    use bytes::Bytes;
    use http::{Response as HttpResponse, header};

    let store = resolve_store(&state)?;
    let Some(db) = state.db.as_ref().cloned() else {
        return Err(ApiError::new(
            StatusCode::NOT_IMPLEMENTED,
            "responses_persistence_disabled",
            "Response persistence requires a configured database".to_string(),
        ));
    };

    // Verify the response exists and belongs to the caller's org.
    let org_id = caller_org(auth.as_ref()).or(state.default_org_id);
    let _record = store
        .get(&response_id, org_id)
        .await
        .map_err(map_store_err)?;

    let starting_after = query.starting_after.unwrap_or(0);
    let limit = query.limit.unwrap_or(200).clamp(1, 1000);
    let store_clone = state.responses_store.as_ref().cloned();
    let events_repo = db.response_events();
    let response_id_clone = response_id.clone();
    let org_id_for_task = org_id;

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(8);

    crate::compat::spawn_detached(async move {
        let mut cursor = starting_after;
        loop {
            // Drain everything past the cursor.
            let events = match events_repo
                .list_after(&response_id_clone, cursor, limit)
                .await
            {
                Ok(e) => e,
                Err(e) => {
                    let _ = tx.send(Err(std::io::Error::other(e.to_string()))).await;
                    return;
                }
            };

            let batch_max = events.last().map(|e| e.sequence_number);
            for ev in events {
                let payload_str = serde_json::to_string(&ev.payload).unwrap_or_default();
                let sse = format!("data: {payload_str}\n\n");
                if tx.send(Ok(Bytes::from(sse))).await.is_err() {
                    return; // client disconnected
                }
            }

            if let Some(seq) = batch_max {
                cursor = seq;
            }

            // Decide whether to keep polling. Re-fetch the response
            // to see if it's reached a terminal state and we've caught
            // up to last_sequence_number.
            let Some(ref store) = store_clone else { return };
            let record = match store.get(&response_id_clone, org_id_for_task).await {
                Ok(r) => r,
                Err(_) => return,
            };
            if record.status.is_terminal() && cursor >= record.last_sequence_number {
                let _ = tx.send(Ok(Bytes::from_static(b"data: [DONE]\n\n"))).await;
                return;
            }

            // In-progress — poll again shortly.
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    });

    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    });
    Ok(HttpResponse::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from_stream(stream))
        .unwrap())
}

/// `DELETE /v1/responses/{response_id}` — remove a stored response.
/// Returns the OpenAI-spec deletion confirmation shape.
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/api/v1/responses/{response_id}",
    tag = "responses",
    params(("response_id" = String, Path, description = "ID of the response to delete")),
    responses((status = 200, description = "Deletion confirmation")),
    security(("api_key" = []))
))]
pub async fn api_v1_responses_delete(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    Path(response_id): Path<String>,
) -> Result<Json<DeleteResponse>, ApiError> {
    let store = resolve_store(&state)?;
    let org_id = caller_org(auth.as_ref()).or(state.default_org_id);
    let deleted = store
        .delete(&response_id, org_id)
        .await
        .map_err(map_store_err)?;
    Ok(Json(DeleteResponse {
        id: response_id,
        object: "response.deleted",
        deleted,
    }))
}
