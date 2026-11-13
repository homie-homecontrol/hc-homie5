#[cfg(feature = "ext-meta")]
use homie5::extensions::meta::{merge_device_overlays, MetaDeviceOverlay};
use homie5::{
    device_description::HomieDeviceDescription, DeviceRef, HomieDeviceStatus, HomieDomain, HomieID,
};
#[cfg(feature = "ext-meta")]
use std::collections::HashMap;

use crate::store::{AlertStore, PropertyValueStore};

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
/// A discovered device with its description, property values, alerts and (feature `ext-meta`)
/// `$meta` overlays.
///
/// The overlays are kept per provider; the merged overlay is cached and rebuilt by
/// [`Device::set_meta_overlay`] / [`Device::remove_meta_overlay`], so
/// [`Device::merged_meta_overlay`] and query matching do not allocate.
#[derive(Clone, Debug)]
pub struct Device {
    pub ident: DeviceRef,
    pub state: HomieDeviceStatus,
    pub description: Option<HomieDeviceDescription>,
    pub prop_values: PropertyValueStore,
    pub alerts: AlertStore,
    /// Per-provider meta overlay documents for this device.
    #[cfg(feature = "ext-meta")]
    meta_overlays: HashMap<HomieID, MetaDeviceOverlay>,
    /// Cache: `meta_overlays` merged in provider-id order.
    #[cfg(feature = "ext-meta")]
    merged_meta: MetaDeviceOverlay,
}

impl Device {
    /// Creates a device without description, property values, alerts or meta overlays.
    pub fn new(ident: DeviceRef, state: HomieDeviceStatus) -> Self {
        Self {
            ident,
            state,
            description: None,
            prop_values: PropertyValueStore::new(),
            alerts: AlertStore::new(),
            #[cfg(feature = "ext-meta")]
            meta_overlays: HashMap::new(),
            #[cfg(feature = "ext-meta")]
            merged_meta: merge_device_overlays(std::iter::empty()),
        }
    }

    pub fn homie_domain(&self) -> &HomieDomain {
        self.ident.homie_domain()
    }

    pub fn device_id(&self) -> &HomieID {
        self.ident.device_id()
    }

    /// Per-provider meta overlay documents of this device.
    #[cfg(feature = "ext-meta")]
    pub fn meta_overlays(&self) -> &HashMap<HomieID, MetaDeviceOverlay> {
        &self.meta_overlays
    }

    /// Stores the overlay of `provider` and rebuilds the merged overlay (skipped when the stored
    /// overlay is identical). Returns the previous overlay of that provider.
    #[cfg(feature = "ext-meta")]
    pub fn set_meta_overlay(
        &mut self,
        provider: HomieID,
        overlay: MetaDeviceOverlay,
    ) -> Option<MetaDeviceOverlay> {
        if self.meta_overlays.get(&provider) == Some(&overlay) {
            return Some(overlay);
        }
        let previous = self.meta_overlays.insert(provider, overlay);
        self.rebuild_merged_meta();
        previous
    }

    /// Removes the overlay of `provider` and rebuilds the merged overlay (skipped when the
    /// provider had none). Returns the removed overlay.
    #[cfg(feature = "ext-meta")]
    pub fn remove_meta_overlay(&mut self, provider: &HomieID) -> Option<MetaDeviceOverlay> {
        let removed = self.meta_overlays.remove(provider)?;
        self.rebuild_merged_meta();
        Some(removed)
    }

    /// The consolidated overlay merging all providers' annotations (cached).
    ///
    /// For keys that appear in multiple providers, values are collected into
    /// a deduplicated list in provider-id order. Single-provider keys pass through unchanged.
    #[cfg(feature = "ext-meta")]
    pub fn merged_meta_overlay(&self) -> &MetaDeviceOverlay {
        &self.merged_meta
    }

    #[cfg(feature = "ext-meta")]
    fn rebuild_merged_meta(&mut self) {
        let mut providers: Vec<(&HomieID, &MetaDeviceOverlay)> =
            self.meta_overlays.iter().collect();
        providers.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
        self.merged_meta = merge_device_overlays(providers.into_iter().map(|(_, overlay)| overlay));
    }
}
