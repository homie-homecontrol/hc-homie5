use chrono::{DateTime, Utc};
use homie5::{DeviceRef, Homie5Message, HomieDeviceStatus, HomieID, HomieValue, PropertyRef};
#[cfg(feature = "ext-meta")]
use homie5::HomieDomain;

use super::Device;

#[derive(Debug, Clone)]
pub enum DiscoveryAction {
    NewDevice {
        device: DeviceRef,
        status: HomieDeviceStatus,
    },
    DeviceRemoved(Device),
    StateChanged {
        device: DeviceRef,
        from: HomieDeviceStatus,
        to: HomieDeviceStatus,
    },
    DeviceDescriptionChanged(DeviceRef),
    DevicePropertyValueChanged {
        prop: PropertyRef,
        from: Option<HomieValue>,
        to: HomieValue,
        value_last_received: Option<DateTime<Utc>>,
        value_last_changed: Option<DateTime<Utc>>,
    },
    DevicePropertyTargetChanged {
        prop: PropertyRef,
        from: Option<HomieValue>,
        to: HomieValue,
        target_last_received: Option<DateTime<Utc>>,
        target_last_changed: Option<DateTime<Utc>>,
    },
    DevicePropertyValueTriggered {
        prop: PropertyRef,
        value: HomieValue,
    },
    DeviceAlert {
        device: DeviceRef,
        alert_id: HomieID,
        alert: String,
    },
    DeviceAlertChanged {
        device: DeviceRef,
        alert_id: HomieID,
        from_alert: String,
        to_alert: String,
    },
    DeviceAlertCleared {
        device: DeviceRef,
        alert_id: HomieID,
    },
    #[cfg(feature = "ext-meta")]
    MetaProviderDiscovered {
        homie_domain: HomieDomain,
        provider_id: HomieID,
        info: homie5::extensions::meta::MetaProviderInfo,
    },
    #[cfg(feature = "ext-meta")]
    MetaProviderRemoved {
        homie_domain: HomieDomain,
        provider_id: HomieID,
    },
    #[cfg(feature = "ext-meta")]
    MetaDeviceOverlayChanged {
        provider_id: HomieID,
        device_id: HomieID,
        overlay: homie5::extensions::meta::MetaDeviceOverlay,
    },
    #[cfg(feature = "ext-meta")]
    MetaDeviceOverlayRemoved {
        provider_id: HomieID,
        device_id: HomieID,
    },
    Unhandled(Homie5Message),
}
