//! Tracking of in-flight (unacknowledged) QoS>0 MQTT publishes.
//!
//! The homie client run loop records every outgoing publish packet id and
//! removes it again when the broker acknowledges it (PubAck for QoS 1,
//! PubComp for QoS 2). The current pending count is exposed through a
//! [`tokio::sync::watch`] channel so [`HomieClientHandle::flush`] can wait
//! deterministically until all publishes have been acknowledged — e.g. before
//! disconnecting during shutdown.
//!
//! [`HomieClientHandle::flush`]: super::HomieClientHandle::flush

use std::collections::HashSet;
use std::time::Duration;

use thiserror::Error;
use tokio::sync::watch;

/// Error returned by flush operations when pending publishes were not
/// acknowledged within the allowed wait time (e.g. the broker is dead).
#[derive(Debug, Error)]
#[error("timed out waiting for pending MQTT publish acknowledgements")]
pub struct FlushTimeout;

/// Tracks packet ids of in-flight QoS>0 publishes and publishes the pending
/// count via a [`watch`] channel so flush callers can await it reaching zero.
///
/// This is a pure bookkeeping struct: the owning event loop feeds it from the
/// already-polled rumqttc event stream.
#[derive(Debug)]
pub struct PendingPublishTracker {
    pending: HashSet<u16>,
    count_tx: watch::Sender<usize>,
}

impl PendingPublishTracker {
    /// Creates a tracker and the receiver side used to observe the pending
    /// publish count (initially 0).
    pub fn new() -> (Self, watch::Receiver<usize>) {
        let (count_tx, count_rx) = watch::channel(0);
        (
            Self {
                pending: HashSet::new(),
                count_tx,
            },
            count_rx,
        )
    }

    /// Records an outgoing publish. Packet id 0 (QoS 0 — never acknowledged
    /// by the broker) is ignored.
    pub fn record_publish(&mut self, pkid: u16) {
        if pkid == 0 {
            return;
        }
        if self.pending.insert(pkid) {
            self.notify();
        }
    }

    /// Records a broker acknowledgement (PubAck for QoS 1, PubComp for
    /// QoS 2). Unknown packet ids are ignored.
    pub fn record_ack(&mut self, pkid: u16) {
        if self.pending.remove(&pkid) {
            self.notify();
        }
    }

    /// Clears all pending entries.
    ///
    /// Must be called on connection loss: rumqttc retransmits in-flight
    /// QoS>0 publishes itself after reconnecting (re-emitting them as
    /// `Outgoing::Publish` events), so stale packet ids from the old
    /// connection must not wedge `flush`.
    pub fn clear(&mut self) {
        if !self.pending.is_empty() {
            self.pending.clear();
            self.notify();
        }
    }

    /// Number of publishes currently awaiting acknowledgement.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    fn notify(&self) {
        // Receivers may all be gone (nobody flushing) — ignore the error.
        let _ = self.count_tx.send(self.pending.len());
    }
}

/// Waits until the pending publish count observed via `rx` reaches 0.
///
/// Returns immediately when the count is already 0; `max_wait` is only an
/// upper bound for the dead-broker case. If the sender side is dropped (the
/// client event loop has ended) there is nothing left that could be
/// acknowledged, so the wait resolves successfully.
pub async fn await_publishes_flushed(
    mut rx: watch::Receiver<usize>,
    max_wait: Duration,
) -> Result<(), FlushTimeout> {
    if *rx.borrow_and_update() == 0 {
        return Ok(());
    }
    tokio::time::timeout(max_wait, async move {
        loop {
            if rx.changed().await.is_err() {
                // Sender dropped: client task ended, no more acks can arrive.
                return;
            }
            if *rx.borrow_and_update() == 0 {
                return;
            }
        }
    })
    .await
    .map_err(|_| FlushTimeout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_publish_ignores_pkid_zero() {
        let (mut tracker, rx) = PendingPublishTracker::new();
        tracker.record_publish(0);
        assert_eq!(tracker.pending_count(), 0);
        assert_eq!(*rx.borrow(), 0);
    }

    #[test]
    fn publish_and_ack_transitions_watch_value() {
        let (mut tracker, rx) = PendingPublishTracker::new();
        tracker.record_publish(1);
        tracker.record_publish(2);
        assert_eq!(*rx.borrow(), 2);
        // Duplicate publish of an in-flight pkid does not double-count.
        tracker.record_publish(1);
        assert_eq!(*rx.borrow(), 2);
        tracker.record_ack(1);
        assert_eq!(*rx.borrow(), 1);
        tracker.record_ack(2);
        assert_eq!(*rx.borrow(), 0);
    }

    #[test]
    fn ack_for_unknown_pkid_is_noop() {
        let (mut tracker, rx) = PendingPublishTracker::new();
        tracker.record_publish(1);
        tracker.record_ack(42);
        assert_eq!(tracker.pending_count(), 1);
        assert_eq!(*rx.borrow(), 1);
    }

    #[test]
    fn clear_resets_pending() {
        let (mut tracker, rx) = PendingPublishTracker::new();
        tracker.record_publish(1);
        tracker.record_publish(2);
        tracker.clear();
        assert_eq!(tracker.pending_count(), 0);
        assert_eq!(*rx.borrow(), 0);
    }

    #[tokio::test]
    async fn flush_returns_immediately_when_no_pending() {
        let (_tracker, rx) = PendingPublishTracker::new();
        // Even a zero max_wait must succeed: nothing is pending.
        await_publishes_flushed(rx, Duration::ZERO).await.unwrap();
    }

    #[tokio::test]
    async fn flush_resolves_when_pending_drops_to_zero() {
        let (mut tracker, rx) = PendingPublishTracker::new();
        tracker.record_publish(1);
        let flush = tokio::spawn(await_publishes_flushed(rx, Duration::from_secs(5)));
        tokio::task::yield_now().await;
        tracker.record_ack(1);
        flush.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn flush_times_out_when_acks_never_arrive() {
        let (mut tracker, rx) = PendingPublishTracker::new();
        tracker.record_publish(1);
        let res = await_publishes_flushed(rx, Duration::from_millis(50)).await;
        assert!(res.is_err());
        // Keep tracker alive so the timeout (not sender drop) is what fires.
        assert_eq!(tracker.pending_count(), 1);
    }

    #[tokio::test]
    async fn flush_resolves_when_tracker_dropped() {
        let (mut tracker, rx) = PendingPublishTracker::new();
        tracker.record_publish(1);
        let flush = tokio::spawn(await_publishes_flushed(rx, Duration::from_secs(5)));
        tokio::task::yield_now().await;
        drop(tracker);
        flush.await.unwrap().unwrap();
    }
}
