use std::time::Duration;
use tokio::{sync::mpsc, task::JoinHandle};

#[derive(Default, Debug)]
pub struct DelayedSender {
    handle: Option<JoinHandle<()>>,
}

impl DelayedSender {
    pub fn new() -> Self {
        Self { handle: None }
    }

    pub async fn from_schedule<T>(sender: mpsc::Sender<T>, task: T, delay: Duration) -> Self
    where
        T: Send + Sync + 'static,
    {
        let handle = Some(tokio::task::spawn(async move {
            tokio::time::sleep(delay).await;
            if let Err(err) = sender.send(task).await {
                log::warn!("Error sending scheduled event: {}", err)
            }
        }));
        Self { handle }
    }

    pub async fn schedule<T>(&mut self, sender: mpsc::Sender<T>, task: T, delay: Duration)
    where
        T: Send + Sync + 'static,
    {
        self.abort();
        self.handle = Some(tokio::task::spawn(async move {
            tokio::time::sleep(delay).await;
            if let Err(err) = sender.send(task).await {
                log::warn!("Error sending scheduled event: {}", err)
            }
        }));
    }

    /// Return true if the task was aborted, false if it was not running
    pub fn abort(&mut self) -> bool {
        if let Some(handle) = self.handle.take() {
            handle.abort();
            return true;
        }
        false
    }

    pub fn is_finished(&self) -> bool {
        self.handle.as_ref().map(|h| h.is_finished()).unwrap_or(true)
    }
}
