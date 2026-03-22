use homie5::{
    DeviceRef, Homie5ControllerProtocol, Homie5Message, HomieDomain, HomieID, HomieValue,
    PropertyRef, ToTopic,
};
#[cfg(feature = "ext-meta")]
use homie5::extensions::meta::{self, MetaMessage};
use rumqttc::ClientError;
use thiserror::Error;

use crate::{
    device_store::DeviceStore, AlertUpdate, DescriptionUpdate, DeviceRemove, DeviceUpdate,
    DiscoveryAction, HomieMQTTClient, ValueUpdate,
};

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("Received a device description message for a non existing device: {0:?}")]
    DescriptionForNonExistingDevice(DeviceRef),
    #[error("Mqtt Client error: {0}")]
    MqttClient(#[from] ClientError),
}
#[derive(Clone)]
pub struct HomieDiscovery {
    client: Homie5ControllerProtocol,
    #[cfg(feature = "ext-meta")]
    meta_client: meta::MetaControllerProtocol,
    mqtt_client: HomieMQTTClient,
}

impl HomieDiscovery {
    pub fn new(mqtt_client: HomieMQTTClient) -> Self {
        Self {
            mqtt_client,
            client: Homie5ControllerProtocol::new(),
            #[cfg(feature = "ext-meta")]
            meta_client: meta::MetaControllerProtocol::new(),
        }
    }

    pub async fn discover(&self, homie_domain: &HomieDomain) -> Result<(), DiscoveryError> {
        self.mqtt_client
            .homie_subscribe(self.client.subscribe_device_discovery(homie_domain))
            .await?;
        self.mqtt_client
            .homie_subscribe(self.client.subscribe_broadcast(homie_domain))
            .await?;
        #[cfg(feature = "ext-meta")]
        {
            self.mqtt_client
                .homie_subscribe(self.meta_client.subscribe_provider_discovery(homie_domain))
                .await?;
            self.mqtt_client
                .homie_subscribe(self.meta_client.subscribe_all_overlays(homie_domain))
                .await?;
        }
        Ok(())
    }

    pub async fn stop_discover(&self, homie_domain: &HomieDomain) -> Result<(), DiscoveryError> {
        self.mqtt_client
            .homie_unsubscribe(self.client.unsubscribe_device_discovery(homie_domain))
            .await?;
        self.mqtt_client
            .homie_unsubscribe(self.client.unsubscribe_broadcast(homie_domain))
            .await?;
        #[cfg(feature = "ext-meta")]
        {
            self.mqtt_client
                .homie_unsubscribe(self.meta_client.unsubscribe_provider_discovery(homie_domain))
                .await?;
            self.mqtt_client
                .homie_unsubscribe(self.meta_client.unsubscribe_all_overlays(homie_domain))
                .await?;
        }
        Ok(())
    }

    pub async fn handle_event(
        &self,
        event: Homie5Message,
        devices: &mut DeviceStore,
    ) -> Result<Option<DiscoveryAction>, DiscoveryError> {
        let action = match event {
            Homie5Message::DeviceState { device, state } => match devices.add(&device, state) {
                DeviceUpdate::Added(device_ref) => {
                    self.mqtt_client
                        .homie_subscribe(self.client.subscribe_device(device_ref))
                        .await?;
                    Some(DiscoveryAction::NewDevice {
                        device,
                        status: state,
                    })
                }
                DeviceUpdate::StateUpdate { from, to, .. } => {
                    Some(DiscoveryAction::StateChanged { device, from, to })
                }
                DeviceUpdate::NoChange => None,
            },
            Homie5Message::DeviceDescription {
                device,
                description,
            } => match devices.store_description(&device, description) {
                DescriptionUpdate::Update {
                    device: device_ref,
                    from,
                    to,
                } => {
                    if let Some(from) = from {
                        if from.version == to.version {
                            return Ok(None);
                        }
                        self.mqtt_client
                            .homie_unsubscribe(self.client.unsubscribe_props(device_ref, &from))
                            .await?;
                    }

                    self.mqtt_client
                        .homie_subscribe(self.client.subscribe_props(device_ref, to))
                        .await?;
                    Some(DiscoveryAction::DeviceDescriptionChanged(device))
                }
                DescriptionUpdate::NoChange => None,
                DescriptionUpdate::NotFound => {
                    log::warn!(
                        "Warning, description update received for non discovered device [{}]",
                        device.to_topic()
                    );
                    return Err(DiscoveryError::DescriptionForNonExistingDevice(device));
                }
            },
            Homie5Message::PropertyValue { property, value } => {
                self.update_prop_value(property, value, devices)
            }
            Homie5Message::PropertyTarget { property, target } => {
                self.update_prop_target(property, target, devices)
            }
            Homie5Message::DeviceAlert {
                device,
                alert_id,
                alert_msg,
            } => self.store_alert(device, alert_id, alert_msg, devices),
            Homie5Message::DeviceRemoval { device } => {
                self.mqtt_client
                    .homie_unsubscribe(self.client.unsubscribe_device(&device))
                    .await?;

                let DeviceRemove::Removed(dev) = devices.remove_device(&device) else {
                    return Ok(None);
                };

                let Some(description) = &dev.description else {
                    return Ok(None);
                };

                self.mqtt_client
                    .homie_unsubscribe(self.client.unsubscribe_props(&device, description))
                    .await?;

                log::info!("============> Removed device {}", dev.device_id());
                Some(DiscoveryAction::DeviceRemoved(dev))
            }
            _ => Some(DiscoveryAction::Unhandled(event)),
        };

        Ok(action)
    }

    /// Handle a parsed `MetaMessage` from the `$meta` overlay namespace.
    ///
    /// Returns the corresponding `DiscoveryAction` for the caller to process.
    #[cfg(feature = "ext-meta")]
    pub fn handle_meta_event(&self, event: MetaMessage) -> Option<DiscoveryAction> {
        match event {
            MetaMessage::ProviderInfo {
                homie_domain,
                provider_id,
                info,
            } => Some(DiscoveryAction::MetaProviderDiscovered {
                homie_domain,
                provider_id,
                info,
            }),
            MetaMessage::ProviderRemoval {
                homie_domain,
                provider_id,
            } => Some(DiscoveryAction::MetaProviderRemoved {
                homie_domain,
                provider_id,
            }),
            MetaMessage::DeviceOverlay {
                provider_id,
                device_id,
                overlay,
                ..
            } => Some(DiscoveryAction::MetaDeviceOverlayChanged {
                provider_id,
                device_id,
                overlay,
            }),
            MetaMessage::DeviceOverlayRemoval {
                provider_id,
                device_id,
                ..
            } => Some(DiscoveryAction::MetaDeviceOverlayRemoved {
                provider_id,
                device_id,
            }),
        }
    }

    fn update_prop_value(
        &self,
        property: PropertyRef,
        value: String,
        devices: &mut DeviceStore,
    ) -> Option<DiscoveryAction> {
        let device = devices.get_device_mut(property.device_ref())?;
        let Some((Ok(value), retained)) = device.description.as_ref().and_then(|desc| {
            desc.with_property(&property, |prop_desc| {
                //log::debug!("PropertyValue: {} - {:?}", property.to_topic(), prop_desc,);
                if !prop_desc.retained {
                    log::debug!("PropertyValue: {} - {}", property.to_topic(), value,);
                }

                (HomieValue::parse(&value, prop_desc), prop_desc.retained)
            })
        }) else {
            return None;
        };
        if retained {
            match device
                .prop_values
                .store_value(property.prop_pointer(), value)
            {
                ValueUpdate::Equal { .. } => None,
                ValueUpdate::Changed {
                    old,
                    new,
                    last_received,
                    last_changed,
                } => Some(DiscoveryAction::DevicePropertyValueChanged {
                    prop: property,
                    from: old,
                    to: new,
                    value_last_received: last_received,
                    value_last_changed: last_changed,
                }),
            }
        } else {
            Some(DiscoveryAction::DevicePropertyValueTriggered {
                prop: property,
                value,
            })
        }
    }

    fn update_prop_target(
        &self,
        property: PropertyRef,
        target: String,
        devices: &mut DeviceStore,
    ) -> Option<DiscoveryAction> {
        // log::debug!("PropertyTarget: {} - {}", property.to_topic(), target);
        let device = devices.get_device_mut(property.device_ref())?;
        let Some(Ok(value)) = device.description.as_ref().and_then(|desc| {
            desc.with_property(&property, |prop_desc| HomieValue::parse(&target, prop_desc))
        }) else {
            return None;
        };
        match device
            .prop_values
            .store_target(property.prop_pointer(), value)
        {
            ValueUpdate::Equal { .. } => None,
            ValueUpdate::Changed {
                old,
                new,
                last_received,
                last_changed,
            } => Some(DiscoveryAction::DevicePropertyTargetChanged {
                prop: property,
                from: old,
                to: new,
                target_last_received: last_received,
                target_last_changed: last_changed,
            }),
        }
    }
    #[allow(dead_code)]
    fn store_alert(
        &self,
        device_ref: DeviceRef,
        id: HomieID,
        alert: String,
        devices: &mut DeviceStore,
    ) -> Option<DiscoveryAction> {
        let device = devices.get_device_mut(&device_ref)?;
        match device.alerts.store_alert(id, alert) {
            AlertUpdate::Equal | AlertUpdate::NoChange => None,
            AlertUpdate::New { id, alert } => Some(DiscoveryAction::DeviceAlert {
                device: device_ref,
                alert_id: id,
                alert,
            }),
            AlertUpdate::Changed {
                id,
                old_alert,
                new_alert,
            } => Some(DiscoveryAction::DeviceAlertChanged {
                device: device_ref,
                alert_id: id,
                from_alert: old_alert,
                to_alert: new_alert,
            }),
            AlertUpdate::Cleared { id } => Some(DiscoveryAction::DeviceAlertCleared {
                device: device_ref,
                alert_id: id,
            }),
        }
    }
}
