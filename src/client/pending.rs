//! Tracking of queued and in-flight (unacknowledged) MQTT publishes.
//!
//! Two stages are tracked so a flush can deterministically wait until the
//! broker has acknowledged everything:
//!
//! 1. **Queued** — `AsyncClient::publish()` only enqueues a request; the
//!    rumqttc event loop has not seen it yet. The client wrapper increments a
//!    [`QueuedPublishCounter`] before enqueueing; the count is decremented
//!    when the event loop emits `Outgoing::Publish` for the request.
//! 2. **In-flight** — QoS>0 publishes transmitted to the broker but not yet
//!    acknowledged (PubAck for QoS 1, PubComp for QoS 2), tracked by packet
//!    id in [`PendingPublishTracker`].
//!
//! [`PendingPublishObserver::flushed`] waits until both counts reach zero.
//! Without the queued stage, a flush issued right after `publish().await`
//! could observe an empty in-flight set before the event loop ever polled
//! the request and return too early.
//!
//! Publishes issued through the raw [`rumqttc::AsyncClient`] (bypassing the
//! counting wrapper) are still tracked from the moment the event loop emits
//! `Outgoing::Publish`; only their queued (pre-event-loop) stage is
//! invisible to flush.

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;
use tokio::sync::watch;

/// Error returned by flush operations when pending publishes were not
/// acknowledged within the allowed wait time (e.g. the broker is dead).
#[derive(Debug, Error)]
#[error("timed out waiting for pending MQTT publish acknowledgements")]
pub struct FlushTimeout;

/// Counts publishes that have been handed to the [`rumqttc::AsyncClient`]
/// but not yet processed by its event loop. Incremented by the publishing
/// client wrapper, decremented by [`PendingPublishTracker::record_publish`].
#[derive(Debug, Clone)]
pub struct QueuedPublishCounter(Arc<AtomicUsize>);

impl QueuedPublishCounter {
    /// Records a publish about to be enqueued. Call **before** awaiting the
    /// enqueue so the event loop can never observe the request first.
    pub fn increment(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }

    /// Reverts an [`increment`](Self::increment) whose enqueue failed.
    /// Saturates at zero.
    pub fn decrement(&self) {
        let _ = self
            .0
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| v.checked_sub(1));
    }
}

/// Tracks packet ids of in-flight QoS>0 publishes (plus the shared queued
/// counter) and notifies a [`watch`] channel on every change so
/// [`PendingPublishObserver`] can await full drain.
///
/// This is a pure bookkeeping struct: the owning event loop feeds it from the
/// already-polled rumqttc event stream.
#[derive(Debug)]
pub struct PendingPublishTracker {
    pending: HashSet<u16>,
    queued: Arc<AtomicUsize>,
    count_tx: watch::Sender<usize>,
}

impl PendingPublishTracker {
    /// Creates a tracker and the observer used to await flush completion.
    pub fn new() -> (Self, PendingPublishObserver) {
        let (count_tx, count_rx) = watch::channel(0);
        let queued = Arc::new(AtomicUsize::new(0));
        (
            Self {
                pending: HashSet::new(),
                queued: Arc::clone(&queued),
                count_tx,
            },
            PendingPublishObserver {
                rx: count_rx,
                queued,
            },
        )
    }

    /// Returns the counter the publishing client wrapper must increment for
    /// every publish it enqueues.
    pub fn queued_counter(&self) -> QueuedPublishCounter {
        QueuedPublishCounter(Arc::clone(&self.queued))
    }

    /// Records that the event loop emitted `Outgoing::Publish`: the request
    /// left the queue (queued count decremented, saturating — retransmits
    /// and uncounted raw publishes must not underflow) and, for QoS>0
    /// (packet id != 0), is now awaiting broker acknowledgement.
    pub fn record_publish(&mut self, pkid: u16) {
        let _ = self
            .queued
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| v.checked_sub(1));
        if pkid != 0 {
            self.pending.insert(pkid);
        }
        self.notify();
    }

    /// Records a broker acknowledgement (PubAck for QoS 1, PubComp for
    /// QoS 2). Unknown packet ids are ignored.
    pub fn record_ack(&mut self, pkid: u16) {
        if self.pending.remove(&pkid) {
            self.notify();
        }
    }

    /// Clears all in-flight entries.
    ///
    /// Must be called on connection loss: rumqttc retransmits in-flight
    /// QoS>0 publishes itself after reconnecting (re-emitting them as
    /// `Outgoing::Publish` events), so stale packet ids from the old
    /// connection must not wedge a flush. The queued count is left alone —
    /// queued requests survive in rumqttc's request channel and are
    /// processed after reconnecting.
    pub fn clear_in_flight(&mut self) {
        if !self.pending.is_empty() {
            self.pending.clear();
        }
        self.notify();
    }

    /// Clears everything (in-flight and queued). Call when the connection is
    /// shut down for good (outgoing disconnect): nothing can be acknowledged
    /// anymore, so flush waiters must be released.
    pub fn clear_all(&mut self) {
        self.pending.clear();
        self.queued.store(0, Ordering::SeqCst);
        self.notify();
    }

    /// Number of publishes currently awaiting acknowledgement (in-flight
    /// only, excluding queued).
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    fn notify(&self) {
        // `send` notifies receivers even when the value is unchanged, which
        // flush relies on to re-check the queued count. Receivers may all be
        // gone (nobody flushing) — ignore the error.
        let _ = self.count_tx.send(self.pending.len());
    }
}

/// Observer side of [`PendingPublishTracker`]: awaits the moment when no
/// publish is queued or awaiting broker acknowledgement.
#[derive(Debug, Clone)]
pub struct PendingPublishObserver {
    rx: watch::Receiver<usize>,
    queued: Arc<AtomicUsize>,
}

impl PendingPublishObserver {
    /// Current total of queued plus in-flight publishes.
    pub fn pending_count(&self) -> usize {
        *self.rx.borrow() + self.queued.load(Ordering::SeqCst)
    }

    /// Waits until all publishes issued **before** this call have left the
    /// request queue and been acknowledged by the broker.
    ///
    /// Returns immediately when nothing is pending; `max_wait` is only an
    /// upper bound for the dead-broker case. If the tracker is dropped (the
    /// client event loop has ended) there is nothing left that could be
    /// acknowledged, so the wait resolves successfully.
    pub async fn flushed(&self, max_wait: Duration) -> Result<(), FlushTimeout> {
        let mut rx = self.rx.clone();
        if *rx.borrow_and_update() + self.queued.load(Ordering::SeqCst) == 0 {
            return Ok(());
        }
        tokio::time::timeout(max_wait, async move {
            loop {
                if rx.changed().await.is_err() {
                    // Sender dropped: client task ended, no more acks can arrive.
                    return;
                }
                if *rx.borrow_and_update() + self.queued.load(Ordering::SeqCst) == 0 {
                    return;
                }
            }
        })
        .await
        .map_err(|_| FlushTimeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_publish_with_pkid_zero_tracks_no_in_flight() {
        let (mut tracker, observer) = PendingPublishTracker::new();
        let counter = tracker.queued_counter();
        counter.increment();
        assert_eq!(observer.pending_count(), 1); // queued
        tracker.record_publish(0); // QoS 0: leaves queue, nothing in-flight
        assert_eq!(tracker.pending_count(), 0);
        assert_eq!(observer.pending_count(), 0);
    }

    #[test]
    fn publish_and_ack_transitions_counts() {
        let (mut tracker, observer) = PendingPublishTracker::new();
        let counter = tracker.queued_counter();
        counter.increment();
        counter.increment();
        assert_eq!(observer.pending_count(), 2); // both queued
        tracker.record_publish(1);
        tracker.record_publish(2);
        assert_eq!(observer.pending_count(), 2); // both in-flight now
        tracker.record_ack(1);
        assert_eq!(observer.pending_count(), 1);
        tracker.record_ack(2);
        assert_eq!(observer.pending_count(), 0);
    }

    #[test]
    fn uncounted_raw_publish_does_not_underflow_queue() {
        let (mut tracker, observer) = PendingPublishTracker::new();
        // No queued increment (raw AsyncClient publish or retransmit).
        tracker.record_publish(7);
        assert_eq!(observer.pending_count(), 1); // in-flight only
        tracker.record_ack(7);
        assert_eq!(observer.pending_count(), 0);
    }

    #[test]
    fn ack_for_unknown_pkid_is_noop() {
        let (mut tracker, observer) = PendingPublishTracker::new();
        tracker.record_publish(1);
        tracker.record_ack(42);
        assert_eq!(tracker.pending_count(), 1);
        assert_eq!(observer.pending_count(), 1);
    }

    #[test]
    fn failed_enqueue_decrement_saturates() {
        let (tracker, observer) = PendingPublishTracker::new();
        let counter = tracker.queued_counter();
        counter.decrement(); // saturates, no underflow
        assert_eq!(observer.pending_count(), 0);
        counter.increment();
        counter.decrement(); // failed enqueue rollback
        assert_eq!(observer.pending_count(), 0);
    }

    #[test]
    fn clear_in_flight_keeps_queued() {
        let (mut tracker, observer) = PendingPublishTracker::new();
        let counter = tracker.queued_counter();
        counter.increment();
        counter.increment();
        counter.increment(); // third one stays queued in rumqttc's request channel
        tracker.record_publish(1);
        tracker.record_publish(2);
        tracker.clear_in_flight(); // connection loss
        assert_eq!(tracker.pending_count(), 0);
        assert_eq!(observer.pending_count(), 1); // queued survives reconnect
    }

    #[test]
    fn clear_all_resets_everything() {
        let (mut tracker, observer) = PendingPublishTracker::new();
        let counter = tracker.queued_counter();
        counter.increment();
        tracker.record_publish(1);
        tracker.clear_all();
        assert_eq!(observer.pending_count(), 0);
    }

    #[tokio::test]
    async fn flush_returns_immediately_when_no_pending() {
        let (_tracker, observer) = PendingPublishTracker::new();
        // Even a zero max_wait must succeed: nothing is pending.
        observer.flushed(Duration::ZERO).await.unwrap();
    }

    #[tokio::test]
    async fn flush_resolves_when_in_flight_drains() {
        let (mut tracker, observer) = PendingPublishTracker::new();
        tracker.record_publish(1);
        let flush = tokio::spawn(async move { observer.flushed(Duration::from_secs(5)).await });
        tokio::task::yield_now().await;
        tracker.record_ack(1);
        flush.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn flush_waits_for_queued_publishes_too() {
        let (mut tracker, observer) = PendingPublishTracker::new();
        let counter = tracker.queued_counter();
        counter.increment(); // publish enqueued, event loop has not seen it
        let flush = tokio::spawn(async move { observer.flushed(Duration::from_secs(5)).await });
        tokio::task::yield_now().await;
        tracker.record_publish(3); // event loop processes the request
        tokio::task::yield_now().await;
        tracker.record_ack(3); // broker acks
        flush.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn flush_times_out_when_acks_never_arrive() {
        let (mut tracker, observer) = PendingPublishTracker::new();
        tracker.record_publish(1);
        let res = observer.flushed(Duration::from_millis(50)).await;
        assert!(res.is_err());
        // Keep tracker alive so the timeout (not sender drop) is what fires.
        assert_eq!(tracker.pending_count(), 1);
    }

    #[tokio::test]
    async fn flush_resolves_when_tracker_dropped() {
        let (mut tracker, observer) = PendingPublishTracker::new();
        tracker.record_publish(1);
        let flush = tokio::spawn(async move { observer.flushed(Duration::from_secs(5)).await });
        tokio::task::yield_now().await;
        drop(tracker);
        flush.await.unwrap().unwrap();
    }
}
