#[cfg(all(test, feature = "ext-meta", feature = "framework"))]
mod tests {
    use hc_homie5::controller::MetaOverlayHandler;
    use hc_homie5::store::DeviceStore;
    use homie5::extensions::meta::{MetaDeviceOverlay, MetaMessage, MetaProviderInfo};
    use homie5::{DeviceRef, HomieDeviceStatus, HomieDomain, HomieID};

    fn test_domain() -> HomieDomain {
        "homie".try_into().unwrap()
    }

    fn device_ref(id: &'static str) -> DeviceRef {
        DeviceRef::new(test_domain(), id.try_into().unwrap())
    }

    fn test_overlay() -> MetaDeviceOverlay {
        MetaDeviceOverlay {
            schema: 1,
            device: None,
        }
    }

    fn provider_id(id: &'static str) -> HomieID {
        id.try_into().unwrap()
    }

    fn device_id(id: &'static str) -> HomieID {
        id.try_into().unwrap()
    }

    #[test]
    fn test_handle_device_overlay_existing_device() {
        let mut handler = MetaOverlayHandler::new(test_domain());
        let mut store = DeviceStore::new();

        // Add a device to the store
        let dref = device_ref("dev-1");
        store.add(&dref, HomieDeviceStatus::Ready);

        // Send overlay message
        let msg = MetaMessage::DeviceOverlay {
            homie_domain: test_domain(),
            provider_id: provider_id("provider-1"),
            device_id: device_id("dev-1"),
            overlay: test_overlay(),
        };

        let handled = handler.handle_meta_message(msg, &mut store);
        assert!(handled);

        // Overlay should be applied directly to device
        let device = store.get_device(&dref).unwrap();
        assert!(device.meta_overlays.contains_key(&provider_id("provider-1")));
    }

    #[test]
    fn test_handle_device_overlay_pending() {
        let mut handler = MetaOverlayHandler::new(test_domain());
        let mut store = DeviceStore::new();

        // Send overlay for device that doesn't exist yet
        let msg = MetaMessage::DeviceOverlay {
            homie_domain: test_domain(),
            provider_id: provider_id("provider-1"),
            device_id: device_id("dev-2"),
            overlay: test_overlay(),
        };

        let handled = handler.handle_meta_message(msg, &mut store);
        assert!(handled);

        // Device doesn't exist yet, so overlay should be pending
        let dref = device_ref("dev-2");
        assert!(store.get_device(&dref).is_none());

        // Now add the device and apply pending
        store.add(&dref, HomieDeviceStatus::Ready);
        handler.apply_pending_for_device(&dref, &mut store);

        // Overlay should now be on the device
        let device = store.get_device(&dref).unwrap();
        assert!(device.meta_overlays.contains_key(&provider_id("provider-1")));
    }

    #[test]
    fn test_remove_provider() {
        let mut handler = MetaOverlayHandler::new(test_domain());
        let mut store = DeviceStore::new();

        // Add devices and overlays
        let dref1 = device_ref("dev-1");
        let dref2 = device_ref("dev-2");
        store.add(&dref1, HomieDeviceStatus::Ready);
        store.add(&dref2, HomieDeviceStatus::Ready);

        // Add overlays from provider-1 to both devices
        let msg1 = MetaMessage::DeviceOverlay {
            homie_domain: test_domain(),
            provider_id: provider_id("provider-1"),
            device_id: device_id("dev-1"),
            overlay: test_overlay(),
        };
        let msg2 = MetaMessage::DeviceOverlay {
            homie_domain: test_domain(),
            provider_id: provider_id("provider-1"),
            device_id: device_id("dev-2"),
            overlay: test_overlay(),
        };
        handler.handle_meta_message(msg1, &mut store);
        handler.handle_meta_message(msg2, &mut store);

        // Both devices should have overlays
        assert!(store.get_device(&dref1).unwrap().meta_overlays.contains_key(&provider_id("provider-1")));
        assert!(store.get_device(&dref2).unwrap().meta_overlays.contains_key(&provider_id("provider-1")));

        // Remove provider
        let msg = MetaMessage::ProviderRemoval {
            homie_domain: test_domain(),
            provider_id: provider_id("provider-1"),
        };
        handler.handle_meta_message(msg, &mut store);

        // Both devices should have no overlays
        assert!(store.get_device(&dref1).unwrap().meta_overlays.is_empty());
        assert!(store.get_device(&dref2).unwrap().meta_overlays.is_empty());
    }

    #[test]
    fn test_remove_device_overlay() {
        let mut handler = MetaOverlayHandler::new(test_domain());
        let mut store = DeviceStore::new();

        let dref = device_ref("dev-1");
        store.add(&dref, HomieDeviceStatus::Ready);

        // Add overlay
        let msg = MetaMessage::DeviceOverlay {
            homie_domain: test_domain(),
            provider_id: provider_id("provider-1"),
            device_id: device_id("dev-1"),
            overlay: test_overlay(),
        };
        handler.handle_meta_message(msg, &mut store);
        assert!(store.get_device(&dref).unwrap().meta_overlays.contains_key(&provider_id("provider-1")));

        // Remove overlay
        let msg = MetaMessage::DeviceOverlayRemoval {
            homie_domain: test_domain(),
            provider_id: provider_id("provider-1"),
            device_id: device_id("dev-1"),
        };
        handler.handle_meta_message(msg, &mut store);
        assert!(store.get_device(&dref).unwrap().meta_overlays.is_empty());
    }

    #[test]
    fn test_ignores_wrong_domain() {
        let mut handler = MetaOverlayHandler::new(test_domain());
        let mut store = DeviceStore::new();

        let other_domain: HomieDomain = "other".try_into().unwrap();
        let msg = MetaMessage::DeviceOverlay {
            homie_domain: other_domain,
            provider_id: provider_id("provider-1"),
            device_id: device_id("dev-1"),
            overlay: test_overlay(),
        };

        let handled = handler.handle_meta_message(msg, &mut store);
        assert!(!handled);
    }

    #[test]
    fn test_clear() {
        let mut handler = MetaOverlayHandler::new(test_domain());
        let mut store = DeviceStore::new();

        // Buffer some pending overlays
        let msg = MetaMessage::DeviceOverlay {
            homie_domain: test_domain(),
            provider_id: provider_id("provider-1"),
            device_id: device_id("dev-1"),
            overlay: test_overlay(),
        };
        handler.handle_meta_message(msg, &mut store);

        handler.clear();

        // After clear, applying pending should do nothing
        let dref = device_ref("dev-1");
        store.add(&dref, HomieDeviceStatus::Ready);
        handler.apply_pending_for_device(&dref, &mut store);
        assert!(store.get_device(&dref).unwrap().meta_overlays.is_empty());
    }

    #[test]
    fn test_provider_info_handled() {
        let mut handler = MetaOverlayHandler::new(test_domain());
        let mut store = DeviceStore::new();

        let msg = MetaMessage::ProviderInfo {
            homie_domain: test_domain(),
            provider_id: provider_id("provider-1"),
            info: MetaProviderInfo {
                schema: 1,
                title: Some("Test Provider".into()),
                description: None,
            },
        };

        let handled = handler.handle_meta_message(msg, &mut store);
        assert!(handled);
    }

    #[test]
    fn test_multiple_providers_same_device() {
        let mut handler = MetaOverlayHandler::new(test_domain());
        let mut store = DeviceStore::new();

        let dref = device_ref("dev-1");
        store.add(&dref, HomieDeviceStatus::Ready);

        // Add overlays from two different providers
        for provider in &["provider-1", "provider-2"] {
            let msg = MetaMessage::DeviceOverlay {
                homie_domain: test_domain(),
                provider_id: provider_id(provider),
                device_id: device_id("dev-1"),
                overlay: test_overlay(),
            };
            handler.handle_meta_message(msg, &mut store);
        }

        let device = store.get_device(&dref).unwrap();
        assert_eq!(device.meta_overlays.len(), 2);

        // Remove only one provider
        let msg = MetaMessage::ProviderRemoval {
            homie_domain: test_domain(),
            provider_id: provider_id("provider-1"),
        };
        handler.handle_meta_message(msg, &mut store);

        let device = store.get_device(&dref).unwrap();
        assert_eq!(device.meta_overlays.len(), 1);
        assert!(device.meta_overlays.contains_key(&provider_id("provider-2")));
    }
}
