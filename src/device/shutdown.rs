use std::time::Duration;

use crate::client::{HomieClientHandle, HomieClientError, HomieMQTTClient};

use super::HomieDevice;

/// Graceful bridge shutdown: disconnect a list of devices, then tear down MQTT.
///
/// Steps:
/// 1. Call `disconnect_device()` on each device
/// 2. Sleep for `drain_delay` to let outstanding MQTT packets flush
/// 3. Disconnect the MQTT client
/// 4. Stop the `HomieClientHandle` (waits for event loop task to finish)
pub async fn graceful_bridge_shutdown<'a, D, E>(
    devices: impl IntoIterator<Item = &'a mut D>,
    mqtt_client: &HomieMQTTClient,
    handle: HomieClientHandle,
    drain_delay: Duration,
) -> Result<(), GracefulShutdownError<E>>
where
    D: HomieDevice<ResultError = E> + 'a,
    E: From<homie5::Homie5ProtocolError>
        + From<rumqttc::ClientError>
        + Send
        + Sync
        + std::fmt::Debug,
{
    for device in devices {
        device
            .disconnect_device()
            .await
            .map_err(GracefulShutdownError::Device)?;
    }
    tokio::time::sleep(drain_delay).await;
    mqtt_client
        .disconnect()
        .await
        .map_err(|e| GracefulShutdownError::MqttClient(HomieClientError::MqttClient(e)))?;
    handle
        .stop()
        .await
        .map_err(GracefulShutdownError::MqttClient)?;
    log::info!("Disconnected from MQTT");
    Ok(())
}

/// Errors from graceful bridge shutdown.
#[derive(Debug, thiserror::Error)]
pub enum GracefulShutdownError<E: std::fmt::Debug> {
    #[error("Device disconnect error: {0:?}")]
    Device(E),
    #[error("MQTT client error: {0}")]
    MqttClient(HomieClientError),
}
