use homie5::{Homie5DeviceProtocol, HomieDomain, HomieID};
use tokio::sync::mpsc::Receiver;

use super::{
    HomieClientError, HomieClientEvent, HomieClientHandle, HomieMQTTClient, MqttClientConfig,
};

/// Result of preparing a bridge MQTT setup.
///
/// Contains the device protocol, MQTT options, and optionally a meta provider protocol,
/// ready to be launched with [`BridgeMqttSetup::run`].
pub struct BridgeMqttSetup {
    pub homie_proto: Homie5DeviceProtocol,
    pub mqtt_options: rumqttc::MqttOptions,
    pub mqtt_channel_size: usize,
    pub max_disconnect: Option<std::time::Duration>,
    #[cfg(feature = "ext-meta")]
    pub meta_provider: homie5::extensions::meta::MetaProviderProtocol,
}

impl MqttClientConfig {
    /// Create MQTT options with a Homie device protocol and last will pre-configured.
    ///
    /// This is a convenience method that eliminates the boilerplate of creating
    /// `Homie5DeviceProtocol`, setting the last will, and configuring MQTT options
    /// that every bridge duplicates.
    pub fn into_bridge_setup(
        self,
        controller_id: HomieID,
        domain: HomieDomain,
    ) -> Result<BridgeMqttSetup, HomieClientError> {
        let (homie_proto, last_will) =
            Homie5DeviceProtocol::new(controller_id.clone(), domain.clone());

        let mqtt_options = self
            .clone()
            .last_will(Some(last_will))
            .to_mqtt_options()?;

        #[cfg(feature = "ext-meta")]
        let meta_provider =
            homie5::extensions::meta::MetaProviderProtocol::new(controller_id, domain);

        Ok(BridgeMqttSetup {
            homie_proto,
            mqtt_options,
            mqtt_channel_size: self.mqtt_channel_size,
            max_disconnect: self.max_disconnect,
            #[cfg(feature = "ext-meta")]
            meta_provider,
        })
    }
}

impl BridgeMqttSetup {
    /// Launch the MQTT client from this setup.
    ///
    /// Returns the client handle, MQTT client wrapper, and event receiver.
    pub fn run(
        self,
    ) -> Result<
        (
            HomieClientHandle,
            HomieMQTTClient,
            Receiver<HomieClientEvent>,
        ),
        HomieClientError,
    > {
        super::run_homie_client_with_options(
            self.mqtt_options,
            self.mqtt_channel_size,
            self.max_disconnect,
        )
    }
}
