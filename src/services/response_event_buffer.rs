//! Bounded-channel writer for `response_events`.
//!
//! Each call to `push` enqueues one event onto a bounded channel; a
//! single drainer task collects events and flushes them in batches
//! either when `max_batch` accumulates or after `flush_interval`,
//! whichever comes first. Buffering trades 100ms of tail latency for
//! ~10–100x fewer DB round-trips during high-throughput streaming.

#![cfg(not(target_arch = "wasm32"))]

use std::{collections::HashMap, sync::Arc, time::Duration};

use tokio::{sync::mpsc, time::Instant};
use tracing::{debug, warn};

use crate::db::repos::{NewResponseEvent, ResponseEventsRepo};

/// Handle to enqueue events. Cheap to clone; backed by a single
/// underlying channel + drainer.
#[derive(Clone)]
pub struct ResponseEventBuffer {
    tx: mpsc::Sender<NewResponseEvent>,
}

impl ResponseEventBuffer {
    /// Spawn the drainer task and return a handle for `push`. The task
    /// runs forever; when the channel is closed (all senders dropped)
    /// it drains pending events and exits.
    pub fn spawn(
        repo: Arc<dyn ResponseEventsRepo>,
        max_batch: usize,
        flush_interval: Duration,
        channel_capacity: usize,
    ) -> Self {
        let (tx, rx) = mpsc::channel(channel_capacity);
        crate::compat::spawn_detached(async move {
            drain_events(rx, repo, max_batch, flush_interval).await;
        });
        Self { tx }
    }

    /// Non-blocking enqueue. Drops on overflow with a warning — we
    /// favor responsiveness over completeness for the event log.
    pub fn push(&self, event: NewResponseEvent) {
        match self.tx.try_send(event) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!("response_event_buffer: channel full; event dropped");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                // Drainer has exited; system is shutting down.
            }
        }
    }
}

async fn drain_events(
    mut rx: mpsc::Receiver<NewResponseEvent>,
    repo: Arc<dyn ResponseEventsRepo>,
    max_batch: usize,
    flush_interval: Duration,
) {
    let mut buffer: Vec<NewResponseEvent> = Vec::with_capacity(max_batch);
    let mut next_flush = Instant::now() + flush_interval;

    loop {
        let now = Instant::now();
        let timeout = if next_flush > now {
            next_flush - now
        } else {
            Duration::ZERO
        };

        let result = tokio::time::timeout(timeout, rx.recv()).await;
        match result {
            Ok(Some(ev)) => {
                buffer.push(ev);
                if buffer.len() >= max_batch {
                    flush(&repo, &mut buffer).await;
                    next_flush = Instant::now() + flush_interval;
                }
            }
            Ok(None) => {
                // Channel closed — flush remainder and exit.
                if !buffer.is_empty() {
                    flush(&repo, &mut buffer).await;
                }
                debug!("response_event_buffer drainer exiting");
                return;
            }
            Err(_) => {
                // Timeout — flush whatever we have.
                if !buffer.is_empty() {
                    flush(&repo, &mut buffer).await;
                }
                next_flush = Instant::now() + flush_interval;
            }
        }
    }
}

async fn flush(repo: &Arc<dyn ResponseEventsRepo>, buffer: &mut Vec<NewResponseEvent>) {
    if buffer.is_empty() {
        return;
    }
    // Compute max sequence per response in this batch so we can
    // ratchet `responses.last_sequence_number` after the insert.
    let mut max_seq: HashMap<String, i64> = HashMap::new();
    for ev in buffer.iter() {
        let entry = max_seq.entry(ev.response_id.clone()).or_insert(0);
        if ev.sequence_number > *entry {
            *entry = ev.sequence_number;
        }
    }
    let drained: Vec<NewResponseEvent> = std::mem::take(buffer);
    let count = drained.len();
    match repo.insert_batch(drained).await {
        Ok(n) => {
            debug!(inserted = n, batch = count, "response_events batch flushed");
        }
        Err(e) => {
            warn!(error = %e, "response_events batch insert failed");
            return;
        }
    }
    for (response_id, seq) in max_seq {
        if let Err(e) = repo.set_last_sequence(&response_id, seq).await {
            warn!(error = %e, %response_id, "set_last_sequence failed");
        }
    }
}
