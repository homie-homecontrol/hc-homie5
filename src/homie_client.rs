use std::time::Duration;

use homie5::{client::LastWill, extensions::MetaExtMessage, parse_mqtt_message, Homie5Message};
use rand::{distr::Alphanumeric, rng, RngExt};
use rumqttc::{AsyncClient, ClientError, ConnectionError, MqttOptions};
use thiserror::Error;
use tokio::{
    sync::{
        mpsc::{self, error::SendError, Receiver},
        watch,
    },
    task::JoinError,
};

use crate::HomieMQTTClient;

#[derive(Debug, Error)]
pub enum HomieClientError {
    #[error("Mqtt Client error: {0}")]
    MqttClient(#[from] ClientError),
    #[error("Error waiting for homie client task to complete: {0} -- {0:#?}")]
    JoinError(#[from] JoinError),
    #[error("Hhomie client channel is closed. Error sending event via mpsc::channel.")]
    ChannelClosed,
}
impl From<SendError<HomieClientEvent>> for HomieClientError {
    fn from(_: SendError<HomieClientEvent>) -> Self {
        Self::ChannelClosed
    }
}

#[derive(Debug, Clone)]
pub struct MqttClientConfig {
    pub hostname: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub client_id: Option<String>,
    pub last_will: Option<LastWill>,
    pub mqtt_channel_size: usize,
    pub keep_alive: u64,
    pub max_packet_size_incoming: usize,
    pub max_packet_size_outgoing: usize,
    pub clean_session: bool,
}

impl MqttClientConfig {
    // Builder methods

    /// Create a new instance with required fields and default optional fields
    pub fn new(hostname: impl Into<String>) -> Self {
        Self {
            hostname: hostname.into(),
            port: 1883,
            username: String::new(),
            password: String::new(),
            client_id: None,
            last_will: None,
            mqtt_channel_size: 65535,
            keep_alive: 5,
            max_packet_size_incoming: 512 * 1024,
            max_packet_size_outgoing: 512 * 1024,
            clean_session: true, // Default value
        }
    }

    pub fn hostname(mut self, hostname: impl Into<String>) -> Self {
        self.hostname = hostname.into();
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.username = username.into();
        self
    }

    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = password.into();
        self
    }

    pub fn client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_id = Some(client_id.into());
        let id = self.client_id.as_ref().unwrap();
        if id.len() > 23 {
            log::warn!(
                "Warning, client id [{}] exceeds 23 (<{}) character length limit of mqtt spec!",
                id,
                id.len()
            );
        }
        self
    }

    pub fn last_will(mut self, last_will: Option<LastWill>) -> Self {
        self.last_will = last_will;
        self
    }

    pub fn mqtt_channel_size(mut self, mqtt_channel_size: usize) -> Self {
        self.mqtt_channel_size = mqtt_channel_size;
        self
    }

    pub fn keep_alive(mut self, keep_alive: u64) -> Self {
        self.keep_alive = keep_alive;
        self
    }

    pub fn max_packet_size_incoming(mut self, max_packet_size: usize) -> Self {
        self.max_packet_size_incoming = max_packet_size;
        self
    }

    pub fn max_packet_size_outgoing(mut self, max_packet_size: usize) -> Self {
        self.max_packet_size_outgoing = max_packet_size;
        self
    }

    pub fn clean_session(mut self, clean_session: bool) -> Self {
        self.clean_session = clean_session;
        self
    }

    pub fn to_mqtt_options(&self) -> MqttOptions {
        let client_id = if self.client_id.is_none() {
            format!(
                "homie5-{}",
                rng()
                    .sample_iter(&Alphanumeric)
                    .take(12) // Leave space for a prefix if needed
                    .map(char::from)
                    .collect::<String>()
            )
        } else {
            self.client_id.clone().unwrap()
        };
        let mut mqttoptions =
            rumqttc::MqttOptions::new(client_id, self.hostname.to_owned(), self.port.to_owned());
        if !self.username.is_empty() && !self.password.is_empty() {
            mqttoptions.set_credentials(self.username.to_owned(), self.password.to_owned());
        }
        mqttoptions.set_keep_alive(Duration::from_secs(self.keep_alive));
        mqttoptions.set_clean_session(self.clean_session);
        mqttoptions
            .set_max_packet_size(self.max_packet_size_incoming, self.max_packet_size_outgoing);

        if let Some(last_will) = &self.last_will {
            mqttoptions.set_last_will(HomieMQTTClient::map_last_will(last_will.clone()));
        }
        mqttoptions
    }
}

#[derive(Debug)]
pub enum HomieClientEvent {
    Connect,
    Disconnect,
    Stop,
    HomieMessage(Homie5Message),
    Error(ConnectionError),
}

pub struct HomieClientHandle {
    stop_sender: watch::Sender<bool>, // Shutdown signal
    handle: tokio::task::JoinHandle<Result<(), HomieClientError>>,
}

impl HomieClientHandle {
    /// Stops the watcher task.
    pub async fn stop(self) -> Result<(), HomieClientError> {
        let _ = self.stop_sender.send(true); // Send the shutdown signal
        self.handle.await??;
        Ok(())
    }
}

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
    log::trace!("Connecting to mqtt: {}", mqttoptions.client_id());
    let (sender, receiver) = mpsc::channel(channel_size);

    let (mqtt_client, mut eventloop) = AsyncClient::new(mqttoptions, channel_size);
    let (stop_sender, mut stop_receiver) = watch::channel(false);

    let handle = tokio::task::spawn(async move {
        let mut connected = false;
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
                                match MetaExtMessage::from_mqtt_message(&p.topic, &p.payload) {
                                    Ok(meta_event) => {
                                        log::debug!(
                                            "MetaExtMessage (not handled yet): {:#?}",
                                            meta_event
                                        );
                                    }
                                    Err(meta_err) => {
                                        log::error!(
                                            "Error parsing MQTT message.\n  Topic: [{}]\n  Payload: [{:?}]\n  Homie parse error: {}\n  MetaExt parse error: {}",
                                            p.topic,
                                            p.payload,
                                            homie_err,
                                            meta_err
                                        );
                                    }
                                }
                                // log::error!("Error parsing message! Topic: [{}], Payload: [{:?}], Error: {}", p.topic, p.payload, err)
                            }
                        }
                    }
                    rumqttc::Event::Incoming(rumqttc::Incoming::ConnAck(_)) => {
                        log::trace!("HOMIE: Connected");
                        connected = true;
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
