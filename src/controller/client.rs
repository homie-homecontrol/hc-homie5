use homie5::{
    device_description::HomiePropertyDescription, Homie5ControllerProtocol, HomieValue, PropertyRef,
};

use crate::client::HomieMQTTClient;

#[derive(Clone)]
pub struct HomieControllerClient {
    protocol: Homie5ControllerProtocol,
    homie_client: HomieMQTTClient,
}

impl HomieControllerClient {
    pub fn new(protocol: Homie5ControllerProtocol, homie_client: HomieMQTTClient) -> Self {
        Self {
            protocol,
            homie_client,
        }
    }

    /// Sends a set command with QoS 2. Prefer [`Self::set_command_for_property`] when the
    /// property description is known.
    pub async fn set_command(
        &self,
        prop: &PropertyRef,
        value: &HomieValue,
    ) -> Result<(), rumqttc::ClientError> {
        self.homie_client
            .homie_publish(self.protocol.set_command(prop, value))
            .await?;
        Ok(())
    }

    /// Sends a set command with the QoS the convention recommends for the property
    /// (QoS 0 for non-retained properties).
    pub async fn set_command_for_property(
        &self,
        prop: &PropertyRef,
        prop_desc: &HomiePropertyDescription,
        value: &HomieValue,
    ) -> Result<(), rumqttc::ClientError> {
        self.homie_client
            .homie_publish(
                self.protocol
                    .set_command_for_property(prop, prop_desc, value),
            )
            .await?;
        Ok(())
    }

    pub fn protocol(&self) -> &Homie5ControllerProtocol {
        &self.protocol
    }

    pub fn homie_client(&self) -> &HomieMQTTClient {
        &self.homie_client
    }
}
