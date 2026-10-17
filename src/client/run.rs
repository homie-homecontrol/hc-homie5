use std::time::Duration;

#[cfg(feature = "ext-meta")]
use homie5::extensions::meta::parse_meta_message;
use homie5::parse_mqtt_message;
use rumqttc::{AsyncClient, MqttOptions};
use tokio::sync::{
    mpsc::{self, Receiver},
    watch,
};

use crate::logging::{truncate, PAYLOAD_MAX};

use super::{
    HomieClientError, HomieClientEvent, HomieClientHandle, HomieMQTTClient, PendingPublishTracker,
};

pub fn run_homie_client(
    mqttoptions: MqttOptions,
    channel_size: usize,
) -> Result<
    (
        HomieClientHandle,
        HomieMQTTClient,
        Receiver<HomieClientEvent>,
    ),
    HomieClientError,
> {
    run_homie_client_with_options(mqttoptions, channel_size, None)
}

pub fn run_homie_client_with_options(
    mqttoptions: MqttOptions,
    channel_size: usize,
    max_disconnect: Option<Duration>,
) -> Result<
    (
        HomieClientHandle,
        HomieMQTTClient,
        Receiver<HomieClientEvent>,
    ),
    HomieClientError,
> {
    tracing::trace!(client_id = %mqttoptions.client_id(), "starting MQTT client");
    let (sender, receiver) = mpsc::channel(channel_size);

    let (mqtt_client, mut eventloop) = AsyncClient::new(mqttoptions, channel_size);
    let (stop_sender, mut stop_receiver) = watch::channel(false);
    let (mut pending_publishes, pending_publishes_observer) = PendingPublishTracker::new();
    let queued_counter = pending_publishes.queued_counter();

    let handle = tokio::task::spawn(async move {
        let mut connected = false;
        let mut first_disconnect_at: Option<tokio::time::Instant> = None;
        loop {
            let poll_res = tokio::select! {
                poll_res = eventloop.poll() => poll_res,
                _exit = stop_receiver.changed() => {
                    if *stop_receiver.borrow() {
                        tracing::trace!("MQTT client stop requested");
                        break;
                    }
                    continue;
                }
            };

            match poll_res {
                Ok(event) => match &event {
                    rumqttc::Event::Incoming(rumqttc::Packet::Publish(p)) => {
                        match parse_mqtt_message(&p.topic, &p.payload) {
                            Ok(event) => {
                                sender.send(HomieClientEvent::HomieMessage(event)).await?;
                            }
                            Err(homie_err) => {
                                #[cfg(feature = "ext-meta")]
                                {
                                    match parse_meta_message(&p.topic, &p.payload) {
                                        Ok(Some(meta_msg)) => {
                                            sender
                                                .send(HomieClientEvent::MetaMessage(meta_msg))
                                                .await?;
                                        }
                                        Ok(None) => {
                                            tracing::warn!(
                                                topic = %p.topic,
                                                payload = %truncate(&String::from_utf8_lossy(&p.payload), PAYLOAD_MAX),
                                                error = %homie_err,
                                                "unparsable MQTT message ignored"
                                            );
                                        }
                                        Err(meta_err) => {
                                            tracing::warn!(
                                                topic = %p.topic,
                                                payload = %truncate(&String::from_utf8_lossy(&p.payload), PAYLOAD_MAX),
                                                error = %homie_err,
                                                meta_error = %meta_err,
                                                "unparsable MQTT message ignored"
                                            );
                                        }
                                    }
                                }
                                #[cfg(not(feature = "ext-meta"))]
                                {
                                    tracing::warn!(
                                        topic = %p.topic,
                                        payload = %truncate(&String::from_utf8_lossy(&p.payload), PAYLOAD_MAX),
                                        error = %homie_err,
                                        "unparsable MQTT message ignored"
                                    );
                                }
                            }
                        }
                    }
                    rumqttc::Event::Incoming(rumqttc::Incoming::ConnAck(_)) => {
                        tracing::trace!("MQTT connected");
                        connected = true;
                        first_disconnect_at = None;
                        sender.send(HomieClientEvent::Connect).await?;
                    }
                    // Pending-publish tracking: pkids are recorded on outgoing
                    // publish and released on broker acknowledgement so
                    // `HomieClientHandle::flush` can await an empty in-flight set.
                    rumqttc::Event::Outgoing(rumqttc::Outgoing::Publish(pkid)) => {
                        pending_publishes.record_publish(*pkid);
                    }
                    rumqttc::Event::Incoming(rumqttc::Incoming::PubAck(ack)) => {
                        pending_publishes.record_ack(ack.pkid);
                    }
                    rumqttc::Event::Incoming(rumqttc::Incoming::PubComp(comp)) => {
                        pending_publishes.record_ack(comp.pkid);
                    }
                    rumqttc::Event::Outgoing(rumqttc::Outgoing::Disconnect) => {
                        tracing::trace!("MQTT connection closed by client");
                        // Nothing can be acknowledged after the disconnect —
                        // release any flush waiters instead of letting them
                        // run into their max_wait.
                        pending_publishes.clear_all();
                        sender.send(HomieClientEvent::Disconnect).await?;

                        break;
                    }
                    _ => {}
                },

                Err(err) => {
                    if connected {
                        connected = false;
                        sender.send(HomieClientEvent::Disconnect).await?;
                    }
                    // Connection lost: rumqttc retransmits in-flight QoS>0
                    // publishes itself after reconnecting (re-emitted as
                    // Outgoing::Publish), so drop the stale pkids — they must
                    // not wedge flush(). Queued requests survive in rumqttc's
                    // request channel and are kept.
                    pending_publishes.clear_in_flight();

                    let since = *first_disconnect_at.get_or_insert_with(tokio::time::Instant::now);
                    tracing::debug!(error = %err, "MQTT connection error");
                    sender.send(HomieClientEvent::Error(err)).await?;
                    if let Some(max_dur) = max_disconnect {
                        if since.elapsed() > max_dur {
                            tracing::debug!(
                                outage_secs = since.elapsed().as_secs(),
                                "MQTT broker unreachable too long; stopping client"
                            );
                            break;
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            };
        }
        sender.send(HomieClientEvent::Stop).await?;
        tracing::trace!("MQTT client stopped");
        Ok(())
    });
    Ok((
        HomieClientHandle {
            handle,
            stop_sender,
            pending_publishes: pending_publishes_observer,
        },
        HomieMQTTClient::new(mqtt_client, queued_counter),
        receiver,
    ))
}
