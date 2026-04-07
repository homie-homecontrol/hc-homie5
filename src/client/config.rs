use std::path::PathBuf;
use std::time::Duration;

use homie5::client::LastWill;
use rand::{distr::Alphanumeric, rng, RngExt};
use rumqttc::{ClientError, MqttOptions, Transport};
use thiserror::Error;
use tokio::{
    sync::mpsc::error::SendError,
    task::JoinError,
};

use super::{HomieClientEvent, HomieMQTTClient};

#[derive(Debug, Error)]
pub enum HomieClientError {
    #[error("Mqtt Client error: {0}")]
    MqttClient(#[from] ClientError),
    #[error("Error waiting for homie client task to complete: {0} -- {0:#?}")]
    JoinError(#[from] JoinError),
    #[error("Hhomie client channel is closed. Error sending event via mpsc::channel.")]
    ChannelClosed,
    #[error("TLS configuration error: {0}")]
    TlsConfig(String),
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
    pub use_tls: bool,
    pub ca_path: Option<PathBuf>,
    pub client_cert_path: Option<PathBuf>,
    pub client_key_path: Option<PathBuf>,
    /// Maximum time the client will retry after disconnect before giving up.
    /// When exceeded, `HomieClientEvent::Stop` is sent.
    /// Default: `None` (retry forever).
    pub max_disconnect: Option<Duration>,
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
            clean_session: true,
            use_tls: false,
            ca_path: None,
            client_cert_path: None,
            client_key_path: None,
            max_disconnect: None,
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

    pub fn use_tls(mut self, use_tls: bool) -> Self {
        self.use_tls = use_tls;
        self
    }

    pub fn ca_path(mut self, ca_path: Option<impl Into<PathBuf>>) -> Self {
        self.ca_path = ca_path.map(|p| p.into());
        self
    }

    pub fn client_cert_path(mut self, client_cert_path: Option<impl Into<PathBuf>>) -> Self {
        self.client_cert_path = client_cert_path.map(|p| p.into());
        self
    }

    pub fn client_key_path(mut self, client_key_path: Option<impl Into<PathBuf>>) -> Self {
        self.client_key_path = client_key_path.map(|p| p.into());
        self
    }

    /// Set maximum time the client will retry after disconnect before giving up.
    /// When exceeded, `HomieClientEvent::Stop` is sent with a log message.
    /// Default: `None` (retry forever).
    pub fn max_disconnect(mut self, duration: Option<Duration>) -> Self {
        self.max_disconnect = duration;
        self
    }

    pub fn to_mqtt_options(&self) -> Result<MqttOptions, HomieClientError> {
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

        if self.use_tls {
            let ca = match &self.ca_path {
                Some(path) => std::fs::read(path).map_err(|e| {
                    HomieClientError::TlsConfig(format!(
                        "failed to read CA certificate '{}': {e}",
                        path.display()
                    ))
                })?,
                None => Vec::new(),
            };

            let client_auth = match (&self.client_cert_path, &self.client_key_path) {
                (Some(cert_path), Some(key_path)) => {
                    let cert = std::fs::read(cert_path).map_err(|e| {
                        HomieClientError::TlsConfig(format!(
                            "failed to read client certificate '{}': {e}",
                            cert_path.display()
                        ))
                    })?;
                    let key = std::fs::read(key_path).map_err(|e| {
                        HomieClientError::TlsConfig(format!(
                            "failed to read client key '{}': {e}",
                            key_path.display()
                        ))
                    })?;
                    Some((cert, key))
                }
                (Some(_), None) => {
                    return Err(HomieClientError::TlsConfig(
                        "client_cert_path requires client_key_path".to_string(),
                    ));
                }
                (None, Some(_)) => {
                    return Err(HomieClientError::TlsConfig(
                        "client_key_path requires client_cert_path".to_string(),
                    ));
                }
                (None, None) => None,
            };

            let transport = if ca.is_empty() {
                Transport::tls_with_default_config()
            } else {
                Transport::tls(ca, client_auth, None)
            };

            mqttoptions.set_transport(transport);
        }

        Ok(mqttoptions)
    }
}
