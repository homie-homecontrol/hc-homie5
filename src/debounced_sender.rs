use std::pin::Pin;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task;
use tokio::time::{sleep, Instant, Sleep};

/// DebouncedSender: Sends messages after a debounce period.
/// Each new `send` resets the timer again
#[derive(Debug, Clone)]
pub struct DebouncedSender<T>
where
    T: Send + Sync + 'static,
{
    debounce_tx: mpsc::Sender<T>,
}

impl<T> DebouncedSender<T>
where
    T: Send + Sync + 'static,
{
    /// Creates a new `DebouncedSender`.
    ///
    /// - `debounce_duration`: The fixed inactivity delay before sending an event.
    /// - `target`: An `mpsc::Sender` where the debounced (last) event is delivered.
    pub fn new(debounce_duration: Duration, target: mpsc::Sender<T>) -> Self {
        // Create a channel on which events will be received for debouncing.
        let (tx, mut rx) = mpsc::channel::<T>(100);

        // Spawn a background task that runs the debouncing logic.
        task::spawn(async move {
            // Outer loop: wait for the start of a new burst of events.
            while let Some(first_event) = rx.recv().await {
                // Store the first event as the pending event.
                let mut pending_event = first_event;
                // Create a pinned sleep future for the debounce duration.
                let mut timer: Pin<Box<Sleep>> = Box::pin(sleep(debounce_duration));

                // Inner loop: wait for either a new event or the timer to expire.
                loop {
                    tokio::select! {
                        maybe_new = rx.recv() => {
                            match maybe_new {
                                Some(new_event) => {
                                    // Update the pending event to the latest event.
                                    pending_event = new_event;
                                    // Reset the timer to fire after the full debounce period from now.
                                    timer.as_mut().reset(Instant::now() + debounce_duration);
                                }
                                None => {
                                    // The channel closed; exit the task.
                                    return;
                                }
                            }
                        }
                        // When the timer expires, break out of the inner loop.
                        _ = &mut timer => {
                            break;
                        }
                    }
                }
                // After the debounce period, send the last pending event to the target.
                if let Err(e) = target.send(pending_event).await {
                    eprintln!("Failed to send debounced event: {:?}", e);
                    // Optionally, you could decide to break out of the loop here if the target is gone.
                }
            }
        });

        Self { debounce_tx: tx }
    }

    /// Triggers a new event. This call is async and resets the debounce timer.
    pub async fn send(&self, event: T) {
        // Send the event to the debouncer (ignore errors if the debouncer task has stopped).
        let _ = self.debounce_tx.send(event).await;
    }
}
