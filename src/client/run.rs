use std::time::Duration;

use homie5::parse_mqtt_message;
#[cfg(feature = "ext-meta")]
use homie5::extensions::meta::parse_meta_message;
use rumqttc::{AsyncClient, MqttOptions};
use tokio::sync::{
    mpsc::{self, Receiver},
    watch,
};

use super::{HomieClientError, HomieClientEvent, HomieClientHandle, HomieMQTTClient};

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
    log::trace!("Connecting to mqtt: {}", mqttoptions.client_id());
    let (sender, receiver) = mpsc::channel(channel_size);

    let (mqtt_client, mut eventloop) = AsyncClient::new(mqttoptions, channel_size);
    let (stop_sender, mut stop_receiver) = watch::channel(false);

    let handle = tokio::task::spawn(async move {
        let mut connected = false;
        let mut first_disconnect_at: Option<tokio::time::Instant> = None;
        loop {
            let poll_res = tokio::select! {
                poll_res = eventloop.poll() => poll_res,
                _exit = stop_receiver.changed() => {
                    if *stop_receiver.borrow() {
                        log::trace!("Received stop signal. Exiting...");
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
                                            sender.send(HomieClientEvent::MetaMessage(meta_msg)).await?;
                                        }
                                        Ok(None) => {
                                            log::error!(
                                                "Error parsing MQTT message.\n  Topic: [{}]\n  Payload: [{:?}]\n  Homie parse error: {}",
                                                p.topic,
                                                p.payload,
                                                homie_err,
                                            );
                                        }
                                        Err(meta_err) => {
                                            log::error!(
                                                "Error parsing MQTT message.\n  Topic: [{}]\n  Payload: [{:?}]\n  Homie parse error: {}\n  Meta parse error: {}",
                                                p.topic,
                                                p.payload,
                                                homie_err,
                                                meta_err
                                            );
                                        }
                                    }
                                }
                                #[cfg(not(feature = "ext-meta"))]
                                {
                                    log::error!(
                                        "Error parsing MQTT message.\n  Topic: [{}]\n  Payload: [{:?}]\n  Homie parse error: {}",
                                        p.topic,
                                        p.payload,
                                        homie_err,
                                    );
                                }
                            }
                        }
                    }
                    rumqttc::Event::Incoming(rumqttc::Incoming::ConnAck(_)) => {
                        log::trace!("HOMIE: Connected");
                        connected = true;
                        first_disconnect_at = None;
                        sender.send(HomieClientEvent::Connect).await?;
                    }
                    rumqttc::Event::Outgoing(rumqttc::Outgoing::Disconnect) => {
                        log::trace!("HOMIE: Connection closed from our side.",);
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

                    if first_disconnect_at.is_none() {
                        first_disconnect_at = Some(tokio::time::Instant::now());
                    }
                    if let (Some(max_dur), Some(since)) = (max_disconnect, first_disconnect_at) {
                        if since.elapsed() > max_dur {
                            log::error!(
                                "MQTT broker unreachable for {:?}, giving up",
                                since.elapsed()
                            );
                            break;
                        }
                    }

                    log::error!("HomieClient: Error connecting mqtt. {:#?}", err);
                    sender.send(HomieClientEvent::Error(err)).await?;
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            };
        }
        sender.send(HomieClientEvent::Stop).await?;
        log::trace!("Exiting homie client eventloop...");
        Ok(())
    });
    Ok((
        HomieClientHandle {
            handle,
            stop_sender,
        },
        HomieMQTTClient::new(mqtt_client),
        receiver,
    ))
}
