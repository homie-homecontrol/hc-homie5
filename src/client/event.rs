use homie5::Homie5Message;
use rumqttc::ConnectionError;

#[derive(Debug)]
pub enum HomieClientEvent {
    Connect,
    Disconnect,
    Stop,
    HomieMessage(Homie5Message),
    #[cfg(feature = "ext-meta")]
    MetaMessage(homie5::extensions::meta::MetaMessage),
    Error(ConnectionError),
}
