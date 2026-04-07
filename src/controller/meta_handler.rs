use std::collections::HashMap;

use homie5::extensions::meta::{MetaDeviceOverlay, MetaMessage};
use homie5::{DeviceRef, HomieDomain, HomieID};

use crate::store::DeviceStore;

/// Handles meta overlay messages for controller applications.
///
/// Manages pending overlays for undiscovered devices and applies overlays
/// to the `DeviceStore` when devices become available.
pub struct MetaOverlayHandler {
    homie_domain: HomieDomain,
    /// provider_id → (device_id → overlay)
    pending: HashMap<HomieID, HashMap<HomieID, MetaDeviceOverlay>>,
}

impl MetaOverlayHandler {
    pub fn new(domain: HomieDomain) -> Self {
        Self {
            homie_domain: domain,
            pending: HashMap::new(),
        }
    }

    /// Process a `MetaMessage` event. Applies overlay to device if present
    /// in the store, otherwise buffers it as pending.
    ///
    /// Returns `true` if the message was handled (domain matched), `false` if ignored.
    pub fn handle_meta_message(&mut self, msg: MetaMessage, devices: &mut DeviceStore) -> bool {
        match msg {
            MetaMessage::ProviderInfo {
                homie_domain,
                provider_id,
                info,
            } => {
                if homie_domain != self.homie_domain {
                    return false;
                }
                log::debug!(
                    "Meta provider discovered: {} (schema={})",
                    provider_id,
                    info.schema
                );
                true
            }
            MetaMessage::ProviderRemoval {
                homie_domain,
                provider_id,
            } => {
                if homie_domain != self.homie_domain {
                    return false;
                }
                self.remove_provider(&provider_id, devices);
                log::debug!("Meta provider removed: {}", provider_id);
                true
            }
            MetaMessage::DeviceOverlay {
                homie_domain,
                provider_id,
                device_id,
                overlay,
            } => {
                if homie_domain != self.homie_domain {
                    return false;
                }
                self.upsert_overlay(device_id, provider_id, overlay, devices);
                true
            }
            MetaMessage::DeviceOverlayRemoval {
                homie_domain,
                provider_id,
                device_id,
            } => {
                if homie_domain != self.homie_domain {
                    return false;
                }
                self.remove_overlay(&device_id, &provider_id, devices);
                true
            }
        }
    }

    /// Apply any pending overlays for a specific device (call after device discovery).
    pub fn apply_pending_for_device(&mut self, device_ref: &DeviceRef, devices: &mut DeviceStore) {
        // Collect all pending overlays for this device_id across all providers
        let mut overlays_to_apply: Vec<(HomieID, MetaDeviceOverlay)> = Vec::new();
        for (provider_id, per_provider) in &mut self.pending {
            if let Some(overlay) = per_provider.remove(device_ref.device_id()) {
                overlays_to_apply.push((provider_id.clone(), overlay));
            }
        }

        // Remove empty provider entries
        self.pending.retain(|_, v| !v.is_empty());

        if overlays_to_apply.is_empty() {
            return;
        }

        if let Some(device) = devices.get_device_mut(device_ref) {
            for (provider_id, overlay) in overlays_to_apply {
                log::trace!(
                    "Applying pending meta overlay: provider={} device={}",
                    provider_id,
                    device_ref.device_id()
                );
                device.meta_overlays.insert(provider_id, overlay);
            }
        }
    }

    /// Remove all overlays from a specific provider (call when provider disconnects).
    pub fn remove_provider(&mut self, provider_id: &HomieID, devices: &mut DeviceStore) {
        // Remove from pending
        self.pending.remove(provider_id);

        // Collect device refs first, then mutate
        let device_refs: Vec<DeviceRef> = devices
            .iter()
            .map(|(domain, device_id, _)| DeviceRef::new(domain.clone(), device_id.clone()))
            .collect();

        for device_ref in device_refs {
            if let Some(device) = devices.get_device_mut(&device_ref) {
                device.meta_overlays.remove(provider_id);
            }
        }
    }

    /// Clear all pending overlays (call on full reconnect).
    pub fn clear(&mut self) {
        self.pending.clear();
    }

    // ── Internal helpers ──────────────────────────────

    fn upsert_overlay(
        &mut self,
        device_id: HomieID,
        provider_id: HomieID,
        overlay: MetaDeviceOverlay,
        devices: &mut DeviceStore,
    ) {
        let device_ref = DeviceRef::new(self.homie_domain.clone(), device_id.clone());

        if let Some(device) = devices.get_device_mut(&device_ref) {
            device.meta_overlays.insert(provider_id, overlay);
            return;
        }

        // Device not yet discovered — buffer as pending
        self.pending
            .entry(provider_id)
            .or_default()
            .insert(device_id, overlay);
    }

    fn remove_overlay(
        &mut self,
        device_id: &HomieID,
        provider_id: &HomieID,
        devices: &mut DeviceStore,
    ) {
        let device_ref = DeviceRef::new(self.homie_domain.clone(), device_id.clone());

        if let Some(device) = devices.get_device_mut(&device_ref) {
            device.meta_overlays.remove(provider_id);
        }

        // Also remove from pending
        if let Some(per_provider) = self.pending.get_mut(provider_id) {
            per_provider.remove(device_id);
            if per_provider.is_empty() {
                self.pending.remove(provider_id);
            }
        }
    }
}
