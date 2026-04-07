use tokio::sync::watch;

use super::HomieClientError;

pub struct HomieClientHandle {
    pub(super) stop_sender: watch::Sender<bool>, // Shutdown signal
    pub(super) handle: tokio::task::JoinHandle<Result<(), HomieClientError>>,
}

impl HomieClientHandle {
    /// Stops the watcher task.
    pub async fn stop(self) -> Result<(), HomieClientError> {
        let _ = self.stop_sender.send(true); // Send the shutdown signal
        self.handle.await??;
        Ok(())
    }
}
