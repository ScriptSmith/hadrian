//! Executor for background responses claimed by the worker.
//!
//! The worker (`jobs::background_responses`) calls
//! [`execute_persisted_response`] once it claims a queued row. The
//! function reconstructs the request, routes it through the same
//! provider plumbing the synchronous handler uses, then wraps the
//! streaming response with the shared `apply_streaming_pipeline`
//! (output guardrails + server-executed tool loop + persister) and
//! drains the body. Skills referenced on the request are resolved at
//! claim time from the persisted `org_id` so each shell session boots
//! with the same mounts the foreground caller asked for.

#![cfg(not(target_arch = "wasm32"))]

use chrono::Utc;
use futures_util::StreamExt;
use thiserror::Error;
use tracing::{error, info, warn};

use crate::{
    AppState,
    api_types::CreateResponsesPayload,
    db::repos::{ResponseCompletion, ResponseRecord, ResponseStatus},
    routes::execution::{ExecutionResult, ResponsesExecutor, execute_with_fallback},
    routing::{resolver, route_models_extended},
    services::{
        ResponsesStore,
        responses_pipeline::{
            PipelinePrincipal, apply_streaming_pipeline, resolve_and_inject_skills,
        },
    },
};

#[derive(Debug, Error)]
pub enum BackgroundExecuteError {
    #[error("payload deserialization failed: {0}")]
    BadPayload(String),
    #[error("model routing failed: {0}")]
    Routing(String),
    #[error("provider resolution failed: {0}")]
    Resolution(String),
    #[error("provider execution failed: {0}")]
    Execution(String),
    #[error("response store missing — background mode requires persistence")]
    NoStore,
}

/// Run a claimed response to completion.
///
/// `record` must already be in `in_progress` status (claimed via
/// `ResponsesRepo::claim_queued`). The function returns once the
/// streaming response has been fully consumed; the persister updates
/// the row to its terminal status in its own spawned task before this
/// function exits.
pub async fn execute_persisted_response(
    state: AppState,
    record: ResponseRecord,
) -> Result<(), BackgroundExecuteError> {
    let store = state
        .responses_store
        .clone()
        .ok_or(BackgroundExecuteError::NoStore)?;

    info!(
        response_id = %record.id,
        model = %record.model,
        "Background worker executing claimed response"
    );

    // Reconstruct the payload. We force `stream = true` so the
    // persister captures events; the client uses
    // GET /v1/responses/{id}/events for live updates anyway.
    let mut payload: CreateResponsesPayload =
        serde_json::from_value(record.request_payload.clone()).map_err(|e| {
            BackgroundExecuteError::BadPayload(format!("invalid request_payload: {e}"))
        })?;
    payload.stream = true;
    // `background` flag stays — the executor inspects it nowhere in
    // the inner pipeline, but downstream tooling can read it.

    // Route the model.
    let routed = route_models_extended(
        payload.model.as_deref(),
        payload.models.as_deref(),
        &state.config.providers,
    )
    .map_err(|e| BackgroundExecuteError::Routing(e.to_string()))?;

    let resolved = resolver::resolve_to_provider(
        routed,
        state.db.as_ref(),
        state.cache.as_ref(),
        state.secrets.as_ref(),
        None, // background runs without an auth extension; principal already on the row
    )
    .await
    .map_err(|e| BackgroundExecuteError::Resolution(e.to_string()))?;

    let provider_name = resolved.provider_name;
    let provider_config = resolved.provider_config;
    let model_name = resolved.model;
    payload.model = Some(model_name.clone());

    // Resolve skills using the org from the persisted row. Mirrors the
    // foreground path: SKILL.md is prepended to instructions and the
    // returned mounts are threaded into apply_streaming_pipeline so
    // the shell runtime materializes the files when a shell call boots
    // a session.
    let mounted_skills = resolve_and_inject_skills(&state, &mut payload, record.org_id)
        .await
        .map_err(|e| BackgroundExecuteError::BadPayload(format!("skill resolution failed: {e}")))?;

    // Sovereignty requirements are checked at request-creation time
    // for the foreground path; in the background we trust the row.
    let exec_result = execute_with_fallback::<ResponsesExecutor>(
        &state,
        provider_name.clone(),
        provider_config.clone(),
        model_name.clone(),
        payload.clone(),
        None,
    )
    .await
    .map_err(|e| BackgroundExecuteError::Execution(format!("{e:?}")))?;

    let ExecutionResult { response, .. } = exec_result;

    // Cancellation: merge two sources into a single watch channel.
    //   1. In-process: `ResponsesStore::subscribe_cancel` for the same
    //      process — flips immediately when `POST /cancel` is handled
    //      on this replica.
    //   2. Cross-process: poll the row's `status` field every 5s; if
    //      it's `cancelled`, trip the merged channel. Lets cancels
    //      issued from another replica reach an executor that
    //      claimed the row.
    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    if let Some(mut from_store) = store.subscribe_cancel(&record.id).await {
        let tx = cancel_tx.clone();
        crate::compat::spawn_detached(async move {
            if from_store.changed().await.is_ok() && *from_store.borrow() {
                let _ = tx.send(true);
            }
        });
    }
    {
        let store_for_poll = store.clone();
        let id_for_poll = record.id.clone();
        let org_for_poll = record.org_id;
        let tx = cancel_tx;
        crate::compat::spawn_detached(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                match store_for_poll.get(&id_for_poll, org_for_poll).await {
                    Ok(rec) => {
                        if rec.status == ResponseStatus::Cancelled {
                            let _ = tx.send(true);
                            return;
                        }
                        if rec.status.is_terminal() {
                            // Completed/failed/incomplete — no more polling.
                            return;
                        }
                    }
                    Err(_) => return,
                }
            }
        });
    }

    // Reconstruct principal from the persisted row so the shared
    // pipeline applies guardrails / file_search ACLs / shell usage
    // attribution using the same identity that submitted the request.
    let principal = PipelinePrincipal {
        api_key_id: record.api_key_id,
        user_id: record.user_id,
        org_id: record.org_id,
        project_id: record.project_id,
        team_id: None, // teams aren't currently stored on the row
        service_account_id: record.service_account_id,
    };

    let wrapped = apply_streaming_pipeline(
        &state,
        &payload,
        provider_name,
        provider_config,
        model_name,
        principal,
        mounted_skills,
        // Background has no HTTP request_id; use the response_id for
        // audit-log correlation so events tied to this run can be
        // grouped consistently.
        Some(record.id.clone()),
        response,
        Some((record.id.clone(), cancel_rx)),
    );

    // Drain the body silently. The persister's internal spawned task
    // handles event log writes + the terminal row update.
    let (_parts, body) = wrapped.into_parts();
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        if let Err(e) = chunk {
            warn!(
                response_id = %record.id,
                error = %e,
                "Stream error during background drain"
            );
            // Persister still owns the final-state update; if it
            // received zero terminal events it'll mark the row
            // `incomplete`. Best-effort patch to `failed` here:
            let _ = store
                .update(
                    &record.id,
                    ResponseCompletion {
                        status: Some(ResponseStatus::Failed),
                        completed_at: Some(Utc::now()),
                        error: Some(serde_json::json!({
                            "code": "stream_error",
                            "message": e.to_string(),
                        })),
                        ..Default::default()
                    },
                )
                .await;
            return Err(BackgroundExecuteError::Execution(e.to_string()));
        }
    }

    info!(response_id = %record.id, "Background response drain complete");
    Ok(())
}

/// Mark a claimed row as `failed` with a structured error payload.
/// Called by the worker when execute_persisted_response returns Err.
pub async fn mark_background_failure(
    store: &ResponsesStore,
    response_id: &str,
    err: &BackgroundExecuteError,
) {
    let error_payload = serde_json::json!({
        "code": match err {
            BackgroundExecuteError::BadPayload(_) => "bad_payload",
            BackgroundExecuteError::Routing(_) => "routing_failed",
            BackgroundExecuteError::Resolution(_) => "provider_resolution_failed",
            BackgroundExecuteError::Execution(_) => "execution_failed",
            BackgroundExecuteError::NoStore => "internal_error",
        },
        "message": err.to_string(),
    });
    if let Err(e) = store
        .update(
            response_id,
            ResponseCompletion {
                status: Some(ResponseStatus::Failed),
                completed_at: Some(Utc::now()),
                error: Some(error_payload),
                ..Default::default()
            },
        )
        .await
    {
        error!(error = %e, response_id, "Failed to mark background row as failed");
    }
}
