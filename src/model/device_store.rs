use homie5::{
    device_description::HomieDeviceDescription, DeviceRef, HomieDeviceStatus, HomieDomain, HomieID,
};
#[cfg(feature = "ext-meta")]
use std::collections::HashMap;

use crate::{AlertStore, PropertyValueStore};

pub enum DeviceUpdate<'a> {
    Added(&'a DeviceRef),
    StateUpdate {
        device: &'a DeviceRef,
        from: HomieDeviceStatus,
        to: HomieDeviceStatus,
    },
    NoChange,
}

#[allow(clippy::large_enum_variant)] // Suppress the Clippy warning for large enum variants - most
                                     // messages will be Update
pub enum DescriptionUpdate<'a> {
    Update {
        device: &'a DeviceRef,
        from: Option<HomieDeviceDescription>,
        to: &'a HomieDeviceDescription,
    },
    NoChange,
    NotFound,
}

#[allow(clippy::large_enum_variant)] // Suppress the Clippy warning for large enum variants - most
                                     // messages will be Remove
pub enum DeviceRemove {
    Removed(Device),
    NotFound,
}
#[derive(Clone, Debug)]
pub struct Device {
    pub ident: DeviceRef,
    pub state: HomieDeviceStatus,
    pub description: Option<HomieDeviceDescription>,
    pub prop_values: PropertyValueStore,
    pub alerts: AlertStore,
    /// Per-provider meta overlay documents for this device.
    #[cfg(feature = "ext-meta")]
    pub meta_overlays: HashMap<HomieID, homie5::extensions::meta::MetaDeviceOverlay>,
}

impl Device {
    pub fn homie_domain(&self) -> &HomieDomain {
        self.ident.homie_domain()
    }

    pub fn device_id(&self) -> &HomieID {
        self.ident.device_id()
    }

    /// Returns a consolidated overlay merging all providers' annotations.
    ///
    /// For keys that appear in multiple providers, values are collected into
    /// a deduplicated list. Single-provider keys pass through unchanged.
    #[cfg(feature = "ext-meta")]
    pub fn merged_meta_overlay(&self) -> homie5::extensions::meta::MetaDeviceOverlay {
        homie5::extensions::meta::merge_device_overlays(self.meta_overlays.values())
    }
}
