use std::ops::{Deref, DerefMut};

use homie5::client::{Publish, Subscription, Unsubscribe};
use rumqttc::AsyncClient;

#[derive(Debug, Clone)]
pub struct HomieMQTTClient(AsyncClient);

impl Deref for HomieMQTTClient {
    type Target = AsyncClient;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for HomieMQTTClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl HomieMQTTClient {
    pub fn new(mqtt_client: AsyncClient) -> Self {
        Self(mqtt_client)
    }

    pub fn map_qos(qos: &homie5::client::QoS) -> rumqttc::QoS {
        match qos {
            homie5::client::QoS::AtLeastOnce => rumqttc::QoS::AtLeastOnce,
            homie5::client::QoS::AtMostOnce => rumqttc::QoS::AtMostOnce,
            homie5::client::QoS::ExactlyOnce => rumqttc::QoS::ExactlyOnce,
        }
    }
    pub fn map_last_will(last_will: homie5::client::LastWill) -> rumqttc::LastWill {
        rumqttc::LastWill {
            topic: last_will.topic,
            message: last_will.message.into(),
            qos: Self::map_qos(&last_will.qos),
            retain: last_will.retain,
        }
    }
    // Implementation for publishing messages
    pub async fn homie_publish(&self, p: Publish) -> Result<(), rumqttc::ClientError> {
        self.0
            .publish(p.topic, Self::map_qos(&p.qos), p.retain, p.payload)
            .await?;
        Ok(())
    }

    // Implementation for subscribing to topics
    pub async fn homie_subscribe(
        &self,
        subs: impl Iterator<Item = Subscription> + Send,
    ) -> Result<(), rumqttc::ClientError> {
        for sub in subs {
            self.0.subscribe(sub.topic, Self::map_qos(&sub.qos)).await?;
        }
        Ok(())
    }

    // Implementation for unsubscribing from topics
    pub async fn homie_unsubscribe(
        &self,
        subs: impl Iterator<Item = Unsubscribe> + Send,
    ) -> Result<(), rumqttc::ClientError> {
        for sub in subs {
            self.0.unsubscribe(sub.topic).await?;
        }
        Ok(())
    }
}
