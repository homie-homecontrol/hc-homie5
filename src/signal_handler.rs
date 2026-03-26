use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;

pub async fn signal_handler<T: Send + 'static>(sender: mpsc::Sender<T>, exit_value: T) {
    let mut sigint = signal(SignalKind::interrupt()).expect("Failed to register SIGINT handler");
    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
    let mut sigquit = signal(SignalKind::quit()).expect("Failed to register SIGQUIT handler");

    tokio::select! {
        _ = sigint.recv() => log::info!("Received SIGINT"),
        _ = sigterm.recv() => log::info!("Received SIGTERM"),
        _ = sigquit.recv() => log::info!("Received SIGQUIT"),
    }

    if let Err(err) = sender.send(exit_value).await {
        log::error!("Error sending exit event: {:#?}", err);
    }
}
