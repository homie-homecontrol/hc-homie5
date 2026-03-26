# hc-homie5

`hc-homie5` is a higher-level Rust framework for building Homie v5 applications on top of MQTT.

It builds on the protocol crate [`homie5`](https://crates.io/crates/homie5) and adds:

- a concrete MQTT runtime integration via `rumqttc`
- device-side traits and macros for publishing Homie devices
- controller-side discovery and in-memory state stores
- reusable query, value-condition, and value-mapping utilities
- async helpers used by bridge and controller applications

This crate is used by other Homecontrol applications such as bridges, automation, dashboard, API, and logger services.

## What this crate provides

### Device-side building blocks

- `HomieDeviceCore` and `HomieDevice` traits for implementing Homie devices
- lifecycle helpers for publishing, disconnecting, and unpublishing devices
- optional proc-macro support (`#[homie_device]`, `#[homie_device_enum]`) via `hc-homie5-macros`

### Controller-side building blocks

- `run_homie_client(...)` for running a Homie-aware MQTT event loop
- `HomieDiscovery` for handling incoming Homie messages and tracking lifecycle changes
- `DeviceStore`, `PropertyValueStore`, and `AlertStore` for in-memory state
- `DiscoveryAction` events for reacting to discovered changes
- `HomieControllerClient` for sending set commands

### Shared utilities

- `MqttClientConfig` and `HomieSettings` (env-driven config)
- `ConnectionState` finite state machine
- `QueryDefinition` and `MaterializedQuery` for filtering properties
- `ValueCondition`, `ValueMapping`, and `ValueMappingIO` for rule-like matching/mapping
- `define_event_multiplexer!` macro for multi-source async event loops
- `DebouncedSender` and `DelayedSender`

## Features

Default features are enabled: `base`, `macros`, `homie_client`, `tokio`.

- `base`: stores, models, query, value-condition/mapping, connection state
- `macros`: re-exports `hc-homie5-macros`
- `homie_client`: MQTT client integration (`rumqttc`), discovery, settings, device traits
- `tokio`: async utilities and signal handling
- `ext-meta`: enables Homie meta extension integration (forwarded from `homie5/ext-meta`)

Use minimal features when needed, for example:

```toml
[dependencies]
hc-homie5 = { version = "0.7", default-features = false, features = ["base"] }
```

## Quick start

### 1) Add dependency

```toml
[dependencies]
hc-homie5 = "0.7"
homie5 = "0.10"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

### 2) Build MQTT options and run client loop

```rust,no_run
use hc_homie5::{run_homie_client, HomieSettings};
use homie5::HomieDomain;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = HomieSettings::from_env("HC", "hc-", HomieDomain::Default);
    let mqtt_options = settings.to_mqtt_client_config().to_mqtt_options()?;

    let (_handle, _mqtt_client, mut _events) = run_homie_client(mqtt_options, 1024)?;

    // Consume events and route them into discovery / application logic.
    Ok(())
}
```

### 3) Controller example with `DeviceManager`

```rust,no_run
use hc_homie5::{DeviceManager, HomieClientEvent, HomieSettings};
use homie5::HomieDomain;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = HomieSettings::from_env("HC", "hc-", HomieDomain::Default);
    let config = settings.to_mqtt_client_config();

    let (manager, _handle, mut events) = DeviceManager::new(settings.homie_domain.clone(), &config)?;

    manager.discover().await?;

    while let Some(event) = events.recv().await {
        match event {
            HomieClientEvent::Connect => {
                // Connected to MQTT broker
            }
            HomieClientEvent::HomieMessage(msg) => {
                if let Some(action) = manager.discovery_handle_event(msg).await? {
                    // React to discovery changes (new device, value updates, removals, ...)
                    println!("discovery action: {action:?}");
                }
            }
            HomieClientEvent::Disconnect | HomieClientEvent::Stop => break,
            HomieClientEvent::Error(err) => {
                eprintln!("homie client error: {err}");
                break;
            }
            #[cfg(feature = "ext-meta")]
            HomieClientEvent::MetaMessage(_msg) => {
                // Optional: process meta extension events
            }
        }
    }

    Ok(())
}
```

## Environment variables

`HomieSettings::from_env(prefix, ...)` reads these variables:

- `{PREFIX}_HOMIE_HOST` (default: `localhost`)
- `{PREFIX}_HOMIE_PORT` (default: `1883`)
- `{PREFIX}_HOMIE_USERNAME`
- `{PREFIX}_HOMIE_PASSWORD`
- `{PREFIX}_HOMIE_CLIENT_ID` (auto-generated when missing)
- `{PREFIX}_HOMIE_DOMAIN` (default passed to `from_env`)
- `{PREFIX}_HOMIE_CTRL_ID` (optional)
- `{PREFIX}_HOMIE_CTRL_NAME` (optional)
- `{PREFIX}_HOMIE_USE_TLS` (`true/1/yes`)
- `{PREFIX}_HOMIE_CA_PATH` (optional)
- `{PREFIX}_HOMIE_CLIENT_CERT` (optional)
- `{PREFIX}_HOMIE_CLIENT_KEY` (optional)

## Typical architecture

1. Start `run_homie_client(...)` to receive `HomieClientEvent` values.
2. Feed incoming `HomieMessage` values to `HomieDiscovery::handle_event(...)`.
3. Update/read `DeviceStore` and react to emitted `DiscoveryAction` variants.
4. Use `HomieControllerClient::set_command(...)` to control devices.

## Development

From this crate folder:

```bash
cargo build --verbose
cargo test --verbose
cargo clippy
cargo fmt
```

## License

MIT, see [`LICENSE`](./LICENSE).
