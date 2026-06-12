use std::time::Duration;

use tokio::sync::watch;

use super::{FlushTimeout, HomieClientError, PendingPublishObserver};

pub struct HomieClientHandle {
    pub(super) stop_sender: watch::Sender<bool>, // Shutdown signal
    pub(super) handle: tokio::task::JoinHandle<Result<(), HomieClientError>>,
    /// Observes queued plus in-flight (unacknowledged) publishes.
    pub(super) pending_publishes: PendingPublishObserver,
}

impl HomieClientHandle {
    /// Stops the watcher task.
    pub async fn stop(self) -> Result<(), HomieClientError> {
        let _ = self.stop_sender.send(true); // Send the shutdown signal
        self.handle.await??;
        Ok(())
    }

    /// Like [`stop`](Self::stop), but first gives the client task up to
    /// `grace` to exit on its own.
    ///
    /// Use this after requesting a disconnect: the event loop exits naturally
    /// once it processes `Outgoing::Disconnect`, sending a clean DISCONNECT
    /// packet (so the broker discards the LWT). Signalling stop immediately
    /// instead can win the race against the still-queued disconnect request
    /// and tear down the TCP connection uncleanly, firing the LWT.
    pub async fn stop_graceful(mut self, grace: Duration) -> Result<(), HomieClientError> {
        match tokio::time::timeout(grace, &mut self.handle).await {
            Ok(res) => {
                res??;
                Ok(())
            }
            Err(_elapsed) => {
                let _ = self.stop_sender.send(true);
                self.handle.await??;
                Ok(())
            }
        }
    }

    /// Returns an observer of the queued plus in-flight publish count.
    ///
    /// Useful for components that need to flush without owning the handle —
    /// see [`PendingPublishObserver::flushed`].
    pub fn pending_publishes(&self) -> PendingPublishObserver {
        self.pending_publishes.clone()
    }

    /// Waits until all publishes issued before this call have been
    /// acknowledged by the broker.
    ///
    /// Returns immediately when nothing is pending; `max_wait` is only an
    /// upper bound for the dead-broker case. Call this before disconnecting
    /// to guarantee that final messages (e.g. `$state` updates) reached the
    /// broker.
    pub async fn flush(&self, max_wait: Duration) -> Result<(), FlushTimeout> {
        self.pending_publishes.flushed(max_wait).await
    }
}
