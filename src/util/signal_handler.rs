use tokio::sync::mpsc;

/// Waits for a termination signal and then sends `exit_value`.
///
/// On Unix this is SIGINT, SIGTERM or SIGQUIT; on other targets (e.g. Windows) it is Ctrl+C.
pub async fn signal_handler<T: Send + 'static>(sender: mpsc::Sender<T>, exit_value: T) {
    wait_for_termination().await;

    if let Err(err) = sender.send(exit_value).await {
        log::error!("Error sending exit event: {:#?}", err);
    }
}

#[cfg(unix)]
async fn wait_for_termination() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut sigint = signal(SignalKind::interrupt()).expect("Failed to register SIGINT handler");
    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
    let mut sigquit = signal(SignalKind::quit()).expect("Failed to register SIGQUIT handler");

    tokio::select! {
        _ = sigint.recv() => log::info!("Received SIGINT"),
        _ = sigterm.recv() => log::info!("Received SIGTERM"),
        _ = sigquit.recv() => log::info!("Received SIGQUIT"),
    }
}

#[cfg(not(unix))]
async fn wait_for_termination() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to register Ctrl+C handler");
    log::info!("Received Ctrl+C");
}
