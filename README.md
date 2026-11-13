# hc-homie5

`hc-homie5` is a higher-level Rust framework for building Homie v5 applications on top of MQTT.

It builds on the protocol crate [`homie5`](https://crates.io/crates/homie5) and adds:

- a concrete MQTT runtime integration via `rumqttc`
- device-side traits and macros for publishing Homie devices
- controller-side discovery and in-memory state stores
- reusable query, value-condition, and value-mapping utilities
- an alert engine for health monitoring
- async helpers used by bridge and controller applications

This crate is used by other Homecontrol applications such as bridges, automation, dashboard, API, and logger services.

## Module structure

The public API is organized into domain-oriented submodules:

| Module | Feature | Description |
|--------|---------|-------------|
| `store` | base | `DeviceStore`, `PropertyValueStore`, `AlertStore` — in-memory state |
| `model` | base | `Device`, `PropertyValueEntry`, `DiscoveryAction` — data types |
| `query` | base | `QueryDefinition`, `MaterializedQuery`, `MetaQuery`, `MetaAnnotations` — property filtering, incl. `$meta` annotations |
| `value` | base | `ValueCondition`, `ListCondition` (any/all/none/first/empty), `ConditionTree` (all/any/not), `ValueMapping`, `ValueMappingIO` — matching/mapping |
| `connection` | base | `ConnectionState`, `ConnectionEvent` — connection lifecycle FSM |
| `alerts` | base | `AlertSpec`, `AlertEngine`, `AlertState` — alert engine |
| `util` | base | `UniqueByIter` and other helpers |
| `client` | framework | `run_homie_client()`, `MqttClientConfig`, `HomieClientEvent` — MQTT integration |
| `device` | framework | `HomieDeviceCore`, `HomieDevice` traits — device-side building blocks |
| `controller` | framework | `DeviceManager`, `HomieDiscovery`, `HomieControllerClient` — controller-side |
| `settings` | framework | `HomieSettings` — env-driven configuration |
| `logging` | base / logging | `truncate`, `chain` (base); `init`, `LogConfig`, `Outage` (logging) — shared service logging |
| `template` | template | `Engine`, `TemplateRegistry`, `ExpandContext`, `check`, `expand` — structural YAML config templates (`use:`/`with:`, `for_each`, `when:`, `{{ }}` expressions) |

Async utilities (`DebouncedSender`, `DelayedSender`) and the `define_event_multiplexer!` macro require the `tokio` feature.

## Features

Default features: `base`, `macros`, `framework`, `tokio`.

- `base`: stores, models, query, value-condition/mapping, connection state, alerts (WASM-safe)
- `macros`: re-exports `hc-homie5-macros` (`#[homie_device]`, `#[homie_device_enum]`)
- `framework`: MQTT client integration (`rumqttc`), discovery, settings, device/controller traits
- `tokio`: async utilities (`DebouncedSender`, `DelayedSender`) and signal handling
- `ext-meta`: enables Homie meta extension integration (forwarded from `homie5/ext-meta`)
- `logging`: service logging setup (`logging::init`, `log_config!`, panic hook, `Outage`) via `tracing-subscriber`
- `template`: YAML config template engine (`template` module) with `minijinja` expressions; not in default features

Use minimal features when needed, for example:

```toml
[dependencies]
hc-homie5 = { version = "0.19", default-features = false, features = ["base"] }
```

## Quick start

### 1) Add dependency

```toml
[dependencies]
hc-homie5 = "0.19"
homie5 = "0.13"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

### 2) Build MQTT options and run client loop

```rust,no_run
use hc_homie5::client::run_homie_client;
use hc_homie5::settings::HomieSettings;
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
use hc_homie5::client::HomieClientEvent;
use hc_homie5::controller::DeviceManager;
use hc_homie5::settings::HomieSettings;
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

## Queries and conditions

`QueryDefinition` (YAML) selects properties by `domain`, `device`, `node`, `property` and `meta`;
every present section must match. Queries deserialize from YAML text and from parsed
`serde_yaml_ng::Value` trees alike.

- Scalar fields take a `ValueCondition`: a literal (`thermostat`), `{ pattern: "^re" }`, or
  `{ operator: <op>, value: <v | [v, ...]> }` with `=`, `<>`, `>`, `<`, `>=`, `<=`, `between`,
  `includesAny` / `includesNone` ("is one of" / "is none of" a value list), `exists`, `isEmpty`,
  `matchAlways`.
- List fields take a `ListCondition`, a mapping with exactly one quantifier key; bare value
  conditions are rejected:

  | Form | Holds when | Empty or missing list |
  |------|------------|-----------------------|
  | `any: <cond>` | at least one element matches | no |
  | `all: <cond>` | the list is non-empty and every element matches | no |
  | `none: <cond>` | no element matches | yes |
  | `first: <cond>` | the first element matches | no |
  | `empty: true` / `empty: false` | the list is empty / not empty | `true` / `false` |

  `none:` holds on an empty or missing list. Since 0.19.1 `all:` no longer does (it used to
  hold vacuously): "every room is Büro" is false for a device without rooms.

  List fields are `device.children`, `device.extensions` and each `$meta` annotation in
  `meta.annotations`.
- `meta.annotations` matches device-level `$meta` annotations. A scalar annotation is a
  one-element list, a missing annotation an empty list. Annotation values are always strings,
  so compare with `"true"`, not `true`. Pass the device itself (`Some(&device)`, feature
  `ext-meta`; it reads the cached merged overlay) or any other `MetaAnnotations` source to
  `match_query` / `add_materialized`; with `None` every annotation counts as missing.

## Config templates (`template`)

The `template` module expands parsed `serde_yaml_ng::Value` trees before they are deserialized
into typed config. It knows nothing about Homie: apps supply device data through an
`ExpandContext` (query results, globals, scope variables) and register functions on the
minijinja environment (`Engine::env_mut()`).

```yaml
# heating-card.tpl.yaml
template: heating-card
params: [item, title]
defaults: { title: "{{ item.name }}" }
body:
  type: card
  title: "{{ title }}"
  device: "{{ item.id }}"
```

```yaml
cards:
  - for_each: { query: { node: { type: thermostat } }, sort_by: meta.room }
    use: heating-card
  - for_each: { items: [thermo-office, thermo-bath] }   # or items: "{{ expr }}"
    item: { type: value, device: "{{ item }}" }
  - when: "show_extra"
    use: extra-card
    with: { title: Extra }
```

- `{{ expr }}` in any scalar; a string that is exactly one placeholder yields a typed value.
- `use:`/`with:` replaces a mapping by a template body; a list body is spliced into a list.
- `for_each` takes `query:` (device query through the context) or `items:` (literal list or one
  expression returning a list); the element is `item`; optional `sort_by` field path.
- `when:` keeps a list item only if the expression is truthy.
- `check()` reports unknown templates, parameter errors, syntax and recursion at load time;
  `expand()` / `expand_documents()` (root `for_each`) produce the final tree.

See the module documentation of `hc_homie5::template` for scopes, ordering and limits.

## Device meta overlays (`ext-meta`)

`Device` keeps one `$meta` overlay per provider and caches their merge (providers in id order;
a key set by several providers becomes a deduplicated list):

- `Device::new(ident, state)` creates a device; the other public fields can be set afterwards.
- `meta_overlays()` returns the per-provider overlays (read-only).
- `set_meta_overlay(provider, overlay)` / `remove_meta_overlay(&provider)` change them and
  rebuild the cache; an identical overlay or an absent provider skips the rebuild.
- `merged_meta_overlay()` returns the cached merge by reference, without allocating.

`controller::MetaOverlayHandler` applies `MetaMessage`s through these methods, buffering
overlays for devices that are not discovered yet.

```yaml
device:
  extensions: { any: { pattern: "^org\\.homie\\.meta:" } }
node:
  type: thermostat
meta:
  annotations:
    roles: { any: smart-thermostat }
    room: { first: { operator: "=", value: [bathroom, hall] } }
    heating: { any: "true" }
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

## Logging

The crate logs through `tracing`. Services set up logging with the `logging` feature as the
first statement of `main`:

```rust
hc_homie5::logging::init(hc_homie5::log_config!("HC_MYBRIDGE"))?;
```

| Variable | Default | Meaning |
|----------|---------|---------|
| `{PREFIX}_LOG_LEVEL` | `info` | Level of the service's own crate (`off`, `error`, `warn`, `info`, `debug`, `trace`); all other crates stay at `warn` |
| `RUST_LOG` | – | Full `tracing` filter; overrides `{PREFIX}_LOG_LEVEL` |
| `{PREFIX}_LOG_FORMAT` | `text` | `text` or `json` (one object per line) |
| `NO_COLOR` | – | Disables ANSI colors; colors are only used when stdout is a terminal |

Every line carries the target and `file:line`. `init` also installs a panic hook that logs the
panic as one ERROR event and exits with code 1 (`LogConfig::exit_on_panic(false)` keeps the
process running). `logging::Outage` logs a recurring failure once when it starts and once when it
ends; `logging::truncate` and `logging::chain` turn large payloads and error chains into one-line field values.

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
