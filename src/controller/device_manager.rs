use std::sync::Arc;
use std::time::Duration;

use homie5::{Homie5ControllerProtocol, Homie5Message, HomieDomain, HomieValue, PropertyRef};
use tokio::sync::{mpsc, RwLock};

use crate::{
    client::{
        run_homie_client, FlushTimeout, HomieClientError, HomieClientEvent, HomieClientHandle,
        MqttClientConfig, PendingPublishObserver,
    },
    model::DiscoveryAction,
    store::DeviceStore,
};

use super::{HomieControllerClient, HomieDiscovery, DiscoveryError};

#[derive(Clone)]
pub struct DeviceManager {
    devices: Arc<RwLock<DeviceStore>>,
    ctrl_client: HomieControllerClient,
    discovery: HomieDiscovery,
    homie_domain: HomieDomain,
    /// Observes queued plus in-flight publishes of the underlying homie
    /// client connection (see [`DeviceManager::flush`]).
    pending_publishes: PendingPublishObserver,
}

impl DeviceManager {
    pub fn new(
        homie_domain: HomieDomain,
        homie_client_options: &MqttClientConfig,
    ) -> Result<
        (
            Self,
            HomieClientHandle,
            mpsc::Receiver<HomieClientEvent>,
        ),
        HomieClientError,
    > {
        let (homie_client_handle, homie_mqtt_client, homie_event_receiver) = run_homie_client(
            homie_client_options.to_mqtt_options()?,
            homie_client_options.mqtt_channel_size,
        )?;

        let devices = Arc::new(RwLock::new(DeviceStore::new()));
        let discovery = HomieDiscovery::new(homie_mqtt_client.clone());
        let ctrl_client =
            HomieControllerClient::new(Homie5ControllerProtocol::new(), homie_mqtt_client);
        let pending_publishes = homie_client_handle.pending_publishes();

        Ok((
            Self {
                devices,
                discovery,
                ctrl_client,
                homie_domain,
                pending_publishes,
            },
            homie_client_handle,
            homie_event_receiver,
        ))
    }

    pub async fn discover(&self) -> Result<(), DiscoveryError> {
        self.discovery.discover(&self.homie_domain).await?;
        Ok(())
    }

    pub async fn stop_discover(&self) -> Result<(), DiscoveryError> {
        self.discovery.stop_discover(&self.homie_domain).await?;
        Ok(())
    }

    pub async fn discovery_handle_event(
        &self,
        message: Homie5Message,
    ) -> Result<Option<DiscoveryAction>, DiscoveryError> {
        let mut devices = self.devices.write().await;
        self.discovery.handle_event(message, &mut devices).await
    }

    pub async fn set_command(
        &self,
        target: &PropertyRef,
        value: &HomieValue,
    ) -> Result<(), rumqttc::ClientError> {
        self.ctrl_client.set_command(target, value).await?;
        Ok(())
    }

    pub async fn disconnect_client(&self) -> Result<(), rumqttc::ClientError> {
        self.ctrl_client.homie_client().disconnect().await?;
        Ok(())
    }

    /// Waits until all in-flight QoS>0 publishes of this manager's client
    /// connection have been acknowledged by the broker.
    ///
    /// Returns immediately when nothing is pending; `max_wait` is only an
    /// upper bound for the dead-broker case. Call this before
    /// [`disconnect_client`](Self::disconnect_client) to guarantee final
    /// messages reached the broker.
    pub async fn flush(&self, max_wait: Duration) -> Result<(), FlushTimeout> {
        self.pending_publishes.flushed(max_wait).await
    }

    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, DeviceStore> {
        self.devices.read().await
    }

    pub async fn write(&self) -> tokio::sync::RwLockWriteGuard<'_, DeviceStore> {
        self.devices.write().await
    }

    pub fn devices(&self) -> &Arc<RwLock<DeviceStore>> {
        &self.devices
    }

    pub fn homie_domain(&self) -> &HomieDomain {
        &self.homie_domain
    }
}
