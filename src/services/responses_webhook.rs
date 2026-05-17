//! Webhook delivery for Responses API terminal-state transitions.
//!
//! When `[features.responses.webhook]` is configured, a `POST` is
//! fired at the URL each time a stored response transitions to a
//! terminal status (`completed`, `failed`, `cancelled`, `incomplete`).
//! The body is a small JSON envelope; the receiver fetches the full
//! object via `GET /v1/responses/{id}` for any further detail.
//!
//! Delivery is fire-and-forget with bounded retries (3 attempts,
//! exponential backoff). Permanent failures log but don't block the
//! state-transition write path. Future work: route permanent failures
//! through the existing DLQ instead of logging.

#![cfg(not(target_arch = "wasm32"))]

use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Serialize;
use serde_json::json;
use tracing::{debug, info, warn};

use crate::{config::ResponsesWebhookConfig, db::repos::ResponseStatus};

/// Payload sent to the configured webhook endpoint.
///
/// Mirrors OpenAI's webhook event shape so existing handlers work
/// with minimal porting: `type` distinguishes which terminal state
/// fired, `data.id` carries the response id for follow-up fetches.
#[derive(Debug, Serialize)]
pub struct WebhookEvent {
    /// e.g. `"response.completed"`, `"response.failed"`,
    /// `"response.cancelled"`, `"response.incomplete"`.
    #[serde(rename = "type")]
    pub event_type: String,
    /// ISO-8601 timestamp.
    pub created_at: DateTime<Utc>,
    pub data: WebhookEventData,
}

#[derive(Debug, Serialize)]
pub struct WebhookEventData {
    pub id: String,
    pub status: &'static str,
    pub background: bool,
}

/// Dispatcher held in AppState. Cheap to clone (everything inside is
/// already `Arc`-friendly).
#[derive(Clone)]
pub struct ResponsesWebhookDispatcher {
    inner: Arc<DispatcherInner>,
}

struct DispatcherInner {
    config: ResponsesWebhookConfig,
    http: Client,
}

impl ResponsesWebhookDispatcher {
    pub fn new(config: ResponsesWebhookConfig, http: Client) -> Self {
        Self {
            inner: Arc::new(DispatcherInner { config, http }),
        }
    }

    /// Fire-and-forget. Spawns a detached task that retries up to 3
    /// times with 250ms / 1s / 4s backoff. Drops after final failure
    /// with a warning. Best-effort by design — operators who need
    /// stronger delivery guarantees should consume the persisted
    /// `responses` rows directly.
    pub fn enqueue(&self, response_id: String, status: ResponseStatus, background: bool) {
        let Some(event_type) = terminal_event_name(status) else {
            // Non-terminal status — nothing to deliver.
            return;
        };
        let event = WebhookEvent {
            event_type: event_type.to_string(),
            created_at: Utc::now(),
            data: WebhookEventData {
                id: response_id.clone(),
                status: status.as_str(),
                background,
            },
        };
        let dispatcher = self.inner.clone();
        crate::compat::spawn_detached(async move {
            deliver_with_retry(&dispatcher, &event).await;
        });
    }
}

fn terminal_event_name(status: ResponseStatus) -> Option<&'static str> {
    match status {
        ResponseStatus::Completed => Some("response.completed"),
        ResponseStatus::Failed => Some("response.failed"),
        ResponseStatus::Cancelled => Some("response.cancelled"),
        ResponseStatus::Incomplete => Some("response.incomplete"),
        ResponseStatus::Queued | ResponseStatus::InProgress => None,
    }
}

async fn deliver_with_retry(dispatcher: &DispatcherInner, event: &WebhookEvent) {
    let body = match serde_json::to_vec(event) {
        Ok(b) => b,
        Err(e) => {
            warn!(error = %e, "Webhook serialization failed; dropping");
            return;
        }
    };

    const BACKOFFS_MS: [u64; 3] = [250, 1_000, 4_000];
    for (attempt, backoff) in BACKOFFS_MS.iter().enumerate() {
        let mut req = dispatcher
            .http
            .post(&dispatcher.config.url)
            .header("Content-Type", "application/json")
            .header("User-Agent", "hadrian-responses-webhook/1")
            .timeout(Duration::from_secs(dispatcher.config.timeout_secs))
            .body(body.clone());
        if let Some(ref token) = dispatcher.config.bearer_token {
            req = req.bearer_auth(token);
        }
        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                debug!(
                    response_id = %event.data.id,
                    event_type = %event.event_type,
                    attempt = attempt + 1,
                    status = resp.status().as_u16(),
                    "Webhook delivered"
                );
                return;
            }
            Ok(resp) => {
                warn!(
                    response_id = %event.data.id,
                    event_type = %event.event_type,
                    attempt = attempt + 1,
                    status = resp.status().as_u16(),
                    "Webhook responded non-2xx; retrying"
                );
            }
            Err(e) => {
                warn!(
                    response_id = %event.data.id,
                    event_type = %event.event_type,
                    attempt = attempt + 1,
                    error = %e,
                    "Webhook delivery failed; retrying"
                );
            }
        }
        // Last attempt — don't sleep, just give up after the loop.
        if attempt + 1 < BACKOFFS_MS.len() {
            tokio::time::sleep(Duration::from_millis(*backoff)).await;
        }
    }
    info!(
        response_id = %event.data.id,
        event_type = %event.event_type,
        "Webhook delivery permanently failed after {} attempts; dropping",
        BACKOFFS_MS.len()
    );
    // Operators wanting durable retry should poll `responses` directly.
    // The persisted record carries all the same data.
    let _ = json!({}); // suppress unused-import noise on `json!`
}
