//! Bounded-channel writer for `response_events`.
//!
//! Each call to `push` enqueues one event onto a bounded channel; a
//! single drainer task collects events and flushes them in batches
//! either when `max_batch` accumulates or after `flush_interval`,
//! whichever comes first. Buffering trades 100ms of tail latency for
//! ~10–100x fewer DB round-trips during high-throughput streaming.

#![cfg(not(target_arch = "wasm32"))]

use std::{collections::HashMap, sync::Arc, time::Duration};

use tokio::{
    sync::{Mutex, mpsc, oneshot, watch},
    time::Instant,
};
use tracing::{debug, warn};

use crate::db::repos::{NewResponseEvent, ResponseEventsRepo};

/// Handle to enqueue events. Cheap to clone; backed by a single
/// underlying channel + drainer.
#[derive(Clone)]
pub struct ResponseEventBuffer {
    tx: mpsc::Sender<NewResponseEvent>,
    /// A direct repo handle for synchronous insertion. The persister
    /// uses this for the terminal event so the row's status update
    /// can't observe a state where status=Completed but the terminal
    /// event hasn't yet been committed to the log.
    repo: Arc<dyn ResponseEventsRepo>,
    /// Watch channel for shutdown coordination. The drainer awaits
    /// `changed()`; `shutdown()` flips `true` and then awaits the
    /// `drainer_done` watch.
    shutdown_tx: watch::Sender<bool>,
    drainer_done_rx: Arc<Mutex<watch::Receiver<bool>>>,
    /// Flush-barrier channel. `flush()` sends a oneshot ack sender; the
    /// drainer drains everything currently queued, commits it, then
    /// acks. Used by the persister to ensure all buffered events are
    /// durably committed *before* the synchronous terminal-event insert.
    flush_tx: mpsc::Sender<oneshot::Sender<()>>,
}

impl ResponseEventBuffer {
    /// Spawn the drainer task and return a handle for `push`. The task
    /// runs until `shutdown()` is called or all senders are dropped,
    /// flushes pending events, and signals completion.
    pub fn spawn(
        repo: Arc<dyn ResponseEventsRepo>,
        max_batch: usize,
        flush_interval: Duration,
        channel_capacity: usize,
    ) -> Self {
        let (tx, rx) = mpsc::channel(channel_capacity);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (done_tx, done_rx) = watch::channel(false);
        let (flush_tx, flush_rx) = mpsc::channel(8);
        let drainer_repo = repo.clone();
        crate::compat::spawn_detached(async move {
            drain_events(
                rx,
                flush_rx,
                drainer_repo,
                max_batch,
                flush_interval,
                shutdown_rx,
            )
            .await;
            let _ = done_tx.send(true);
        });
        Self {
            tx,
            repo,
            shutdown_tx,
            drainer_done_rx: Arc::new(Mutex::new(done_rx)),
            flush_tx,
        }
    }

    /// Synchronously drain and commit every event the drainer currently
    /// holds — both its in-memory batch and anything queued on the
    /// channel at the time of the call. Returns once those events are
    /// durably committed (and `last_sequence_number` ratcheted).
    ///
    /// The persister calls this before `insert_sync` for a terminal
    /// event: every prior non-terminal event was already `push`ed
    /// (synchronous `try_send`), so they are guaranteed to land in the
    /// log before the terminal event. Without it, a reconnecting
    /// `?stream=true` reader could see the terminal event (and emit
    /// `[DONE]`) while earlier events are still buffered, dropping the
    /// tail of the stream.
    pub async fn flush(&self) {
        let (ack_tx, ack_rx) = oneshot::channel();
        if self.flush_tx.send(ack_tx).await.is_err() {
            // Drainer has exited (shutting down); nothing to flush.
            return;
        }
        let _ = ack_rx.await;
    }

    /// Trigger an orderly shutdown of the drainer. Waits until the
    /// drainer has flushed in-flight events and exited. Called by
    /// `cli::server` during graceful shutdown so buffered events
    /// aren't lost when the process is killed.
    pub async fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
        let mut done = self.drainer_done_rx.lock().await;
        // wait_for returns immediately if the value is already true.
        let _ = done.wait_for(|v| *v).await;
    }

    /// Enqueue one event. On overflow we never silently drop: a dropped
    /// event leaves a permanent hole in the replay log while the terminal
    /// event still commits, so a reconnecting `?stream=true` reader
    /// replays a complete-looking response that is missing content.
    /// Instead we `flush()` the drainer (which commits everything
    /// currently queued, in order) and retry once; if the channel is
    /// still full (drainer contended), we commit the event synchronously.
    pub async fn push(&self, event: NewResponseEvent) {
        let event = match self.tx.try_send(event) {
            Ok(()) => return,
            Err(mpsc::error::TrySendError::Full(event)) => event,
            // Drainer has exited; system is shutting down.
            Err(mpsc::error::TrySendError::Closed(_)) => return,
        };
        // Channel full — drain everything queued (preserving order) and
        // retry the fast path once.
        warn!("response_event_buffer: channel full; flushing and retrying");
        self.flush().await;
        let event = match self.tx.try_send(event) {
            Ok(()) => return,
            Err(mpsc::error::TrySendError::Full(event)) => event,
            Err(mpsc::error::TrySendError::Closed(_)) => return,
        };
        // Still full after a flush: commit synchronously rather than drop.
        if let Err(e) = self.insert_sync(event).await {
            warn!(error = %e, "response_event_buffer: synchronous fallback insert failed");
        }
    }

    /// Synchronously insert a single event, bypassing the buffer.
    /// Returns once the row is committed and `last_sequence_number`
    /// is ratcheted.
    ///
    /// The persister uses this for the terminal event so the row's
    /// status update happens-after the terminal event commit. Without
    /// this, `GET /v1/responses/{id}?stream=true` readers can observe
    /// `status=Completed` before the terminal event reaches the log.
    pub async fn insert_sync(&self, event: NewResponseEvent) -> Result<(), crate::db::DbError> {
        let response_id = event.response_id.clone();
        let seq = event.sequence_number;
        self.repo.insert_batch(vec![event]).await?;
        self.repo.set_last_sequence(&response_id, seq).await?;
        Ok(())
    }
}

async fn drain_events(
    mut rx: mpsc::Receiver<NewResponseEvent>,
    mut flush_rx: mpsc::Receiver<oneshot::Sender<()>>,
    repo: Arc<dyn ResponseEventsRepo>,
    max_batch: usize,
    flush_interval: Duration,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut buffer: Vec<NewResponseEvent> = Vec::with_capacity(max_batch);
    let mut next_flush = Instant::now() + flush_interval;

    loop {
        if *shutdown.borrow() {
            break;
        }
        let now = Instant::now();
        let timeout = if next_flush > now {
            next_flush - now
        } else {
            Duration::ZERO
        };
        let result = tokio::select! {
            biased;
            _ = shutdown.changed() => {
                break;
            }
            req = flush_rx.recv() => {
                match req {
                    Some(ack) => {
                        // Pull everything currently queued so all events
                        // sent before this flush request land before we
                        // ack — the caller `push`ed them synchronously,
                        // so they are already in the channel buffer.
                        while let Ok(ev) = rx.try_recv() {
                            buffer.push(ev);
                        }
                        flush(&repo, &mut buffer).await;
                        next_flush = Instant::now() + flush_interval;
                        let _ = ack.send(());
                        continue;
                    }
                    // All flush handles dropped (shutting down). The
                    // data channel lives in the same struct and closes
                    // with it, so exit to the final drain rather than
                    // busy-looping on the closed flush channel.
                    None => break,
                }
            }
            r = tokio::time::timeout(timeout, rx.recv()) => r,
        };
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
                break;
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

    // Final flush: drain anything still in the channel + buffer so
    // shutdown doesn't lose events.
    while let Ok(ev) = rx.try_recv() {
        buffer.push(ev);
    }
    if !buffer.is_empty() {
        flush(&repo, &mut buffer).await;
    }
    debug!("response_event_buffer drainer exiting");
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
