use std::time::Duration;

use tokio::sync::watch;

use super::{await_publishes_flushed, FlushTimeout, HomieClientError};

pub struct HomieClientHandle {
    pub(super) stop_sender: watch::Sender<bool>, // Shutdown signal
    pub(super) handle: tokio::task::JoinHandle<Result<(), HomieClientError>>,
    /// Observes the number of in-flight (unacknowledged) QoS>0 publishes.
    pub(super) pending_publishes: watch::Receiver<usize>,
}

impl HomieClientHandle {
    /// Stops the watcher task.
    pub async fn stop(self) -> Result<(), HomieClientError> {
        let _ = self.stop_sender.send(true); // Send the shutdown signal
        self.handle.await??;
        Ok(())
    }

    /// Waits until all in-flight QoS>0 publishes have been acknowledged by
    /// the broker.
    ///
    /// Returns immediately when nothing is pending; `max_wait` is only an
    /// upper bound for the dead-broker case. Call this before disconnecting
    /// to guarantee that final messages (e.g. `$state` updates) reached the
    /// broker.
    pub async fn flush(&self, max_wait: Duration) -> Result<(), FlushTimeout> {
        await_publishes_flushed(self.pending_publishes.clone(), max_wait).await
    }
}
