use tokio::sync::mpsc;

/// Waits for a termination signal and then sends `exit_value`.
///
/// On Unix this is SIGINT, SIGTERM or SIGQUIT; on other targets (e.g. Windows) it is Ctrl+C.
pub async fn signal_handler<T: Send + 'static>(sender: mpsc::Sender<T>, exit_value: T) {
    wait_for_termination().await;

    if let Err(err) = sender.send(exit_value).await {
        tracing::debug!(error = %err, "cannot send exit event; receiver closed");
    }
}

#[cfg(unix)]
async fn wait_for_termination() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut sigint = signal(SignalKind::interrupt()).expect("Failed to register SIGINT handler");
    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
    let mut sigquit = signal(SignalKind::quit()).expect("Failed to register SIGQUIT handler");

    tokio::select! {
        _ = sigint.recv() => tracing::debug!(signal = "SIGINT", "termination signal received"),
        _ = sigterm.recv() => tracing::debug!(signal = "SIGTERM", "termination signal received"),
        _ = sigquit.recv() => tracing::debug!(signal = "SIGQUIT", "termination signal received"),
    }
}

#[cfg(not(unix))]
async fn wait_for_termination() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to register Ctrl+C handler");
    tracing::debug!(signal = "Ctrl+C", "termination signal received");
}
