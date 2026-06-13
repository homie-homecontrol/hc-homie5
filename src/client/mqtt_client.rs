use std::ops::{Deref, DerefMut};

use homie5::client::{Publish, Subscription, Unsubscribe};
use rumqttc::AsyncClient;

use super::QueuedPublishCounter;

/// Wrapper around [`rumqttc::AsyncClient`] that counts every publish it
/// enqueues so [`HomieClientHandle::flush`](super::HomieClientHandle::flush)
/// can wait for requests the event loop has not even seen yet.
///
/// Publishes issued through the [`Deref`] escape hatch bypass the queued
/// counting (they are still flush-tracked from the moment the event loop
/// emits `Outgoing::Publish`); prefer [`homie_publish`](Self::homie_publish).
#[derive(Debug, Clone)]
pub struct HomieMQTTClient {
    client: AsyncClient,
    queued_publishes: QueuedPublishCounter,
}

impl Deref for HomieMQTTClient {
    type Target = AsyncClient;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl DerefMut for HomieMQTTClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.client
    }
}

impl HomieMQTTClient {
    pub fn new(mqtt_client: AsyncClient, queued_publishes: QueuedPublishCounter) -> Self {
        Self {
            client: mqtt_client,
            queued_publishes,
        }
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
        // Count before enqueueing so the event loop can never observe the
        // request ahead of the counter increment.
        self.queued_publishes.increment();
        if let Err(err) = self
            .client
            .publish(p.topic, Self::map_qos(&p.qos), p.retain, p.payload)
            .await
        {
            self.queued_publishes.decrement();
            return Err(err);
        }
        Ok(())
    }

    // Implementation for subscribing to topics
    pub async fn homie_subscribe(
        &self,
        subs: impl Iterator<Item = Subscription> + Send,
    ) -> Result<(), rumqttc::ClientError> {
        for sub in subs {
            self.client.subscribe(sub.topic, Self::map_qos(&sub.qos)).await?;
        }
        Ok(())
    }

    // Implementation for unsubscribing from topics
    pub async fn homie_unsubscribe(
        &self,
        subs: impl Iterator<Item = Unsubscribe> + Send,
    ) -> Result<(), rumqttc::ClientError> {
        for sub in subs {
            self.client.unsubscribe(sub.topic).await?;
        }
        Ok(())
    }
}
