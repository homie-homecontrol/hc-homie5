#[derive(Clone, Copy, Debug)]
pub enum ConnectionState {
    Init,
    Connected,
    Disconnected,
}

#[derive(Clone, Copy, Debug)]
pub enum ConnectionEvent {
    Connect,
    Disconnect,
    Reconnect,
}

impl ConnectionState {
    pub fn change_state(&mut self, new_state: ConnectionState) -> Option<ConnectionEvent> {
        let event = match (&self, &new_state) {
            (ConnectionState::Init, ConnectionState::Connected) => Some(ConnectionEvent::Connect),
            (ConnectionState::Connected, ConnectionState::Disconnected) => {
                Some(ConnectionEvent::Disconnect)
            }
            (ConnectionState::Disconnected, ConnectionState::Connected) => {
                Some(ConnectionEvent::Reconnect)
            }
            _ => None,
        };

        *self = new_state;
        event
    }
}
