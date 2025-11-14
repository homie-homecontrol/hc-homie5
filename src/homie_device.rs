use homie5::device_description::HomieDeviceDescription;
use homie5::{
    homie_device_disconnect_steps, homie_device_publish_steps, DevicePublishStep, DeviceRef,
    Homie5DeviceProtocol, HomieDeviceStatus, HomieDomain, HomieID, PropertyRef,
};

use super::HomieMQTTClient;

pub trait HomieDeviceCore {
    fn homie_domain(&self) -> &HomieDomain;
    fn homie_id(&self) -> &HomieID;
    fn device_ref(&self) -> &DeviceRef;
    fn description(&self) -> &HomieDeviceDescription;
    fn client(&self) -> &HomieMQTTClient;
    fn homie_proto(&self) -> &Homie5DeviceProtocol;
    fn state(&self) -> HomieDeviceStatus;
    fn set_state(&mut self, state: HomieDeviceStatus);
}

#[cfg_attr(feature = "enum-dispatch", enum_dispatch::enum_dispatch)]
pub trait HomieDevice: HomieDeviceCore
where
    Self: Send + Sync,
    Self::ResultError: From<homie5::Homie5ProtocolError> + From<rumqttc::ClientError> + Send + Sync,
{
    type ResultError;

    fn publish_property_values(
        &mut self,
    ) -> impl std::future::Future<Output = Result<(), Self::ResultError>> + Send {
        async { Ok(()) }
    }

    fn handle_set_command(
        &mut self,
        property: &PropertyRef,
        set_value: &str,
    ) -> impl std::future::Future<Output = Result<(), Self::ResultError>> + Send;

    fn publish_description(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Self::ResultError>> + Send {
        async {
            let p = self.homie_proto().publish_description(self.description())?;
            self.client().homie_publish(p).await?;
            Ok(())
        }
    }

    fn publish_state(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Self::ResultError>> + Send {
        async {
            let p = self.homie_proto().publish_state(self.state());
            self.client().homie_publish(p).await?;
            Ok(())
        }
    }

    fn subscribe_props(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Self::ResultError>> + Send {
        async {
            let desc = self.description();
            let p = self.homie_proto().subscribe_props(desc)?;
            self.client().homie_subscribe(p).await?;
            Ok(())
        }
    }

    fn unsubscribe_props(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Self::ResultError>> + Send {
        async {
            let desc = self.description();
            let p = self.homie_proto().unsubscribe_props(desc)?;
            self.client().homie_unsubscribe(p).await?;
            Ok(())
        }
    }

    fn publish_meta(
        &mut self,
    ) -> impl std::future::Future<Output = Result<(), Self::ResultError>> + Send {
        async { Ok(()) }
    }
    fn publish_device(
        &mut self,
    ) -> impl std::future::Future<Output = Result<(), Self::ResultError>> + Send {
        async {
            log::debug!("[{}/{}] publishing", self.homie_domain(), self.homie_id());

            let steps = homie_device_publish_steps();

            for step in steps {
                match step {
                    DevicePublishStep::DeviceStateInit => {
                        // set the device into init state
                        self.set_state(HomieDeviceStatus::Init);

                        // publish init state
                        self.publish_state().await?;
                    }
                    DevicePublishStep::DeviceDescription => {
                        // publish description first
                        self.publish_description().await?;
                    }
                    DevicePublishStep::PropertyValues => {
                        // publish any property values the devices exposes
                        self.publish_property_values().await?;
                    }
                    DevicePublishStep::SubscribeProperties => {
                        // subscribe to all properties
                        self.subscribe_props().await?;
                    }
                    DevicePublishStep::DeviceStateReady => {
                        self.publish_meta().await?;

                        // set the device into ready state
                        self.set_state(HomieDeviceStatus::Ready);

                        // publish ready state
                        self.publish_state().await?;
                    }
                }
            }

            Ok(())
        }
    }

    fn unpublish_device(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Self::ResultError>> + Send {
        async {
            let p = self.homie_proto().remove_device(self.description())?;

            for entry in p {
                self.client().homie_publish(entry).await?;
            }
            Ok(())
        }
    }

    fn disconnect_device(
        &mut self,
    ) -> impl std::future::Future<Output = Result<(), Self::ResultError>> + Send {
        async {
            log::debug!("[{}] disconnect", self.homie_proto().id());
            for step in homie_device_disconnect_steps() {
                match step {
                    homie5::DeviceDisconnectStep::DeviceStateDisconnect => {
                        self.set_state(HomieDeviceStatus::Disconnected);
                        self.publish_state().await?;
                    }
                    homie5::DeviceDisconnectStep::UnsubscribeProperties => {
                        self.unsubscribe_props().await?;
                    }
                }
            }
            Ok(())
        }
    }
}
