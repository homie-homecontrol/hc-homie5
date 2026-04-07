use std::time::{Duration, Instant};

use homie5::device_description::{
    DeviceDescriptionBuilder, HomieDeviceDescription, NodeDescriptionBuilder,
    PropertyDescriptionBuilder,
};
use homie5::{
    DeviceRef, Homie5DeviceProtocol, HomieDeviceStatus, HomieDomain, HomieID, NodeRef, PropertyRef,
};

use crate::client::HomieMQTTClient;

/// Manages the root bridge/controller device in a bridge application.
///
/// Handles the device lifecycle (publish, disconnect) and child device
/// registration with proper state transitions (init → description → ready).
pub struct BridgeController {
    device_ref: DeviceRef,
    device_desc: HomieDeviceDescription,
    status: HomieDeviceStatus,
    homie_proto: Homie5DeviceProtocol,
    mqtt_client: HomieMQTTClient,
    action_prop: PropertyRef,
    #[cfg(feature = "ext-meta")]
    meta_provider: Option<homie5::extensions::meta::MetaProviderProtocol>,
    /// When set, child updates are staged and published only after the
    /// debounce period elapses with no further changes.
    children_debounce: Option<Duration>,
    children_dirty: bool,
    last_child_change: Option<Instant>,
}

impl BridgeController {
    /// Create a new bridge controller with a standard `control` node and
    /// `action` enum property with the given variants (e.g., `["refresh", "update"]`).
    pub fn new(
        controller_id: HomieID,
        controller_name: &str,
        domain: HomieDomain,
        mqtt_client: HomieMQTTClient,
        action_variants: &[&str],
    ) -> Self {
        let device_ref = DeviceRef::new(domain.clone(), controller_id.clone());
        let (action_prop, device_desc) =
            build_bridge_controller_description(&controller_id, controller_name, &domain, action_variants);

        Self {
            device_ref,
            device_desc,
            status: HomieDeviceStatus::Init,
            homie_proto: Homie5DeviceProtocol::new(controller_id, domain).0,
            mqtt_client,
            action_prop,
            #[cfg(feature = "ext-meta")]
            meta_provider: None,
            children_debounce: None,
            children_dirty: false,
            last_child_change: None,
        }
    }

    /// Create with a custom device description (no auto-generated control node).
    pub fn with_description(
        device_ref: DeviceRef,
        device_desc: HomieDeviceDescription,
        action_prop: PropertyRef,
        mqtt_client: HomieMQTTClient,
    ) -> Self {
        let homie_proto =
            Homie5DeviceProtocol::new(device_ref.device_id().clone(), device_ref.homie_domain().clone()).0;
        Self {
            device_ref,
            device_desc,
            status: HomieDeviceStatus::Init,
            homie_proto,
            mqtt_client,
            action_prop,
            #[cfg(feature = "ext-meta")]
            meta_provider: None,
            children_debounce: None,
            children_dirty: false,
            last_child_change: None,
        }
    }

    // ── Accessors ─────────────────────────────────────

    pub fn device_ref(&self) -> &DeviceRef {
        &self.device_ref
    }

    pub fn homie_proto(&self) -> &Homie5DeviceProtocol {
        &self.homie_proto
    }

    pub fn mqtt_client(&self) -> &HomieMQTTClient {
        &self.mqtt_client
    }

    pub fn action_property(&self) -> &PropertyRef {
        &self.action_prop
    }

    pub fn description(&self) -> &HomieDeviceDescription {
        &self.device_desc
    }

    pub fn description_mut(&mut self) -> &mut HomieDeviceDescription {
        &mut self.device_desc
    }

    pub fn status(&self) -> HomieDeviceStatus {
        self.status
    }

    #[cfg(feature = "ext-meta")]
    pub fn meta_provider(&self) -> Option<&homie5::extensions::meta::MetaProviderProtocol> {
        self.meta_provider.as_ref()
    }

    #[cfg(feature = "ext-meta")]
    pub fn set_meta_provider(&mut self, provider: homie5::extensions::meta::MetaProviderProtocol) {
        self.meta_provider = Some(provider);
    }

    // ── Lifecycle ─────────────────────────────────────

    /// Publish the controller device (init → description → subscribe → ready).
    pub async fn publish(&mut self) -> Result<(), BridgeControllerError> {
        self.status = HomieDeviceStatus::Init;
        self.publish_state().await?;
        self.publish_description_inner().await?;
        self.subscribe_props().await?;
        self.status = HomieDeviceStatus::Ready;
        self.publish_state().await?;
        Ok(())
    }

    /// Disconnect the controller device (lost → unsubscribe).
    pub async fn disconnect(&mut self) -> Result<(), BridgeControllerError> {
        self.status = HomieDeviceStatus::Disconnected;
        self.publish_state().await?;
        self.unsubscribe_props().await?;
        Ok(())
    }

    // ── Children debounce ──────────────────────────────

    /// Set a debounce duration for child updates. When set, `add_child()`,
    /// `remove_child()`, and `clear_children()` stage changes without
    /// publishing. Call [`flush_children`] periodically (e.g. from your
    /// event loop) to publish once the debounce period has elapsed.
    ///
    /// When `None` (default), child changes are published immediately.
    pub fn set_children_debounce(&mut self, debounce: Option<Duration>) {
        self.children_debounce = debounce;
    }

    /// Returns the remaining duration until pending children changes should
    /// be flushed. Returns `None` if there are no pending changes.
    pub fn pending_flush_delay(&self) -> Option<Duration> {
        if !self.children_dirty {
            return None;
        }
        let debounce = self.children_debounce.unwrap_or(Duration::ZERO);
        match self.last_child_change {
            Some(last) => {
                let elapsed = last.elapsed();
                if elapsed >= debounce {
                    Some(Duration::ZERO)
                } else {
                    Some(debounce - elapsed)
                }
            }
            None => Some(Duration::ZERO),
        }
    }

    /// If children are dirty and the debounce period has elapsed, republish
    /// the device description (init → description → ready). Returns `true`
    /// if a republish occurred.
    pub async fn flush_children(&mut self) -> Result<bool, BridgeControllerError> {
        if !self.children_dirty {
            return Ok(false);
        }
        let debounce = self.children_debounce.unwrap_or(Duration::ZERO);
        if let Some(last_change) = self.last_child_change {
            if last_change.elapsed() < debounce {
                return Ok(false);
            }
        }
        self.children_dirty = false;
        self.last_child_change = None;
        self.republish_description().await?;
        Ok(true)
    }

    // ── Child device management ───────────────────────

    /// Add a child device ID. When debounce is configured, the change is
    /// staged and published by the next [`flush_children`] call. Otherwise
    /// transitions immediately: init → publish description → ready.
    pub async fn add_child(&mut self, child_id: HomieID) -> Result<(), BridgeControllerError> {
        self.device_desc.add_child(child_id);
        self.stage_or_publish_children().await
    }

    /// Remove a child device ID. When debounce is configured, the change is
    /// staged and published by the next [`flush_children`] call. Otherwise
    /// transitions immediately: init → publish description → ready.
    pub async fn remove_child(&mut self, child_id: &HomieID) -> Result<(), BridgeControllerError> {
        self.device_desc.remove_child(child_id);
        self.stage_or_publish_children().await
    }

    /// Clear all child device IDs. When debounce is configured, the change is
    /// staged and published by the next [`flush_children`] call. Otherwise
    /// transitions immediately: init → publish description → ready.
    pub async fn clear_children(&mut self) -> Result<(), BridgeControllerError> {
        self.device_desc.children = vec![];
        self.stage_or_publish_children().await
    }

    // ── Meta provider ─────────────────────────────────

    #[cfg(feature = "ext-meta")]
    pub async fn publish_meta_provider_info(
        &self,
        info: &homie5::extensions::meta::MetaProviderInfo,
    ) -> Result<(), BridgeControllerError> {
        let provider = self.meta_provider.as_ref().ok_or_else(|| {
            BridgeControllerError::MetaProviderNotSet
        })?;
        let publish = provider.publish_provider_info(info)?;
        self.mqtt_client.homie_publish(publish).await?;
        Ok(())
    }

    // ── Internal helpers ──────────────────────────────

    /// Stage a children change for deferred publish, or publish immediately
    /// when debounce is not configured.
    async fn stage_or_publish_children(&mut self) -> Result<(), BridgeControllerError> {
        self.device_desc.update_version();
        if self.children_debounce.is_some() {
            self.children_dirty = true;
            self.last_child_change = Some(Instant::now());
            Ok(())
        } else {
            self.republish_description().await
        }
    }

    async fn publish_state(&self) -> Result<(), BridgeControllerError> {
        let p = self.homie_proto.publish_state(self.status);
        self.mqtt_client.homie_publish(p).await?;
        Ok(())
    }

    async fn publish_description_inner(&self) -> Result<(), BridgeControllerError> {
        let p = self.homie_proto.publish_description(&self.device_desc)?;
        self.mqtt_client.homie_publish(p).await?;
        Ok(())
    }

    async fn subscribe_props(&self) -> Result<(), BridgeControllerError> {
        let p = self.homie_proto.subscribe_props(&self.device_desc)?;
        self.mqtt_client.homie_subscribe(p).await?;
        Ok(())
    }

    async fn unsubscribe_props(&self) -> Result<(), BridgeControllerError> {
        let p = self.homie_proto.unsubscribe_props(&self.device_desc)?;
        self.mqtt_client.homie_unsubscribe(p).await?;
        Ok(())
    }

    /// Republish description with init→ready state transition.
    async fn republish_description(&mut self) -> Result<(), BridgeControllerError> {
        self.status = HomieDeviceStatus::Init;
        self.publish_state().await?;
        self.publish_description_inner().await?;
        self.status = HomieDeviceStatus::Ready;
        self.publish_state().await?;
        Ok(())
    }
}

/// Build a standard bridge controller description with a `control` node
/// and `action` enum property.
pub fn build_bridge_controller_description(
    controller_id: &HomieID,
    controller_name: &str,
    domain: &HomieDomain,
    action_variants: &[&str],
) -> (PropertyRef, HomieDeviceDescription) {
    let node = NodeRef::new(
        domain.clone(),
        controller_id.clone(),
        "control".try_into().unwrap(),
    );
    let prop = PropertyRef::from_node(node.clone(), "action".try_into().unwrap());
    let desc = DeviceDescriptionBuilder::new()
        .name(controller_name)
        .add_node(
            node.node_id().clone(),
            NodeDescriptionBuilder::new()
                .name("control")
                .add_property(
                    prop.prop_id().clone(),
                    PropertyDescriptionBuilder::enumeration(action_variants.iter().copied())
                        .unwrap()
                        .name("Request action from controller")
                        .retained(false)
                        .settable(true)
                        .build(),
                )
                .build(),
        )
        .build();
    (prop, desc)
}

/// Errors from BridgeController operations.
#[derive(Debug, thiserror::Error)]
pub enum BridgeControllerError {
    #[error("MQTT client error: {0}")]
    MqttClient(#[from] rumqttc::ClientError),
    #[error("Homie protocol error: {0}")]
    HomieProtocol(#[from] homie5::Homie5ProtocolError),
    #[cfg(feature = "ext-meta")]
    #[error("Meta protocol error: {0}")]
    MetaProtocol(#[from] homie5::extensions::meta::MetaError),
    #[cfg(feature = "ext-meta")]
    #[error("Meta provider not set on BridgeController")]
    MetaProviderNotSet,
}
