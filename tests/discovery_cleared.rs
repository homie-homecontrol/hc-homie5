#![cfg(feature = "framework")]

use hc_homie5::{controller::HomieDiscovery, model::DiscoveryAction, store::DeviceStore};
use homie5::{DeviceRef, Homie5Message, HomieDeviceStatus, HomieDomain, HomieValue, PropertyRef};

fn fixture() -> (HomieDiscovery, DeviceStore, PropertyRef) {
    let (client, _events) =
        rumqttc::AsyncClient::new(rumqttc::MqttOptions::new("test", "localhost", 1883), 10);
    let (tracker, _) = hc_homie5::client::PendingPublishTracker::new();
    let discovery = HomieDiscovery::new(hc_homie5::client::HomieMQTTClient::new(
        client,
        tracker.queued_counter(),
    ));
    let device = DeviceRef::new(HomieDomain::Default, "device".try_into().unwrap());
    let prop = PropertyRef::new(
        HomieDomain::Default,
        "device".try_into().unwrap(),
        "node".try_into().unwrap(),
        "value".try_into().unwrap(),
    );
    let mut store = DeviceStore::new();
    store.add(&device, HomieDeviceStatus::Ready);
    store.get_device_mut(&device).unwrap().description = Some(serde_yaml_ng::from_str(
        "homie: '5.0'\nversion: 1\nnodes:\n  node:\n    properties:\n      value:\n        datatype: integer\n",
    ).unwrap());
    (discovery, store, prop)
}

#[tokio::test]
async fn clears_stored_value_without_clearing_target() {
    let (discovery, mut store, prop) = fixture();
    let values = &mut store.get_device_mut(prop.device_ref()).unwrap().prop_values;
    values.store_value(prop.prop_pointer(), HomieValue::Integer(1));
    values.store_target(prop.prop_pointer(), HomieValue::Integer(2));
    let action = discovery
        .handle_event(
            Homie5Message::PropertyValueCleared {
                property: prop.clone(),
            },
            &mut store,
        )
        .await
        .unwrap();
    assert!(
        matches!(action, Some(DiscoveryAction::DevicePropertyValueCleared { prop: cleared, from: HomieValue::Integer(1) }) if cleared == prop)
    );
    let entry = store
        .get_device(prop.device_ref())
        .unwrap()
        .prop_values
        .get_value_entry(prop.prop_pointer())
        .unwrap();
    assert_eq!(entry.value, None);
    assert_eq!(entry.value_last_received, None);
    assert_eq!(entry.value_last_changed, None);
    assert!(entry.target_last_received.is_some());
    assert_eq!(entry.target, Some(HomieValue::Integer(2)));
}

#[tokio::test]
async fn clears_stored_target_without_clearing_value() {
    let (discovery, mut store, prop) = fixture();
    let values = &mut store.get_device_mut(prop.device_ref()).unwrap().prop_values;
    values.store_value(prop.prop_pointer(), HomieValue::Integer(1));
    values.store_target(prop.prop_pointer(), HomieValue::Integer(2));
    let action = discovery
        .handle_event(
            Homie5Message::PropertyTargetCleared {
                property: prop.clone(),
            },
            &mut store,
        )
        .await
        .unwrap();
    assert!(
        matches!(action, Some(DiscoveryAction::DevicePropertyTargetCleared { prop: cleared, from: HomieValue::Integer(2) }) if cleared == prop)
    );
    let entry = store
        .get_device(prop.device_ref())
        .unwrap()
        .prop_values
        .get_value_entry(prop.prop_pointer())
        .unwrap();
    assert_eq!(entry.target, None);
    assert_eq!(entry.target_last_received, None);
    assert_eq!(entry.target_last_changed, None);
    assert!(entry.value_last_received.is_some());
    assert_eq!(entry.value, Some(HomieValue::Integer(1)));
}

#[tokio::test]
async fn absent_repeated_and_unknown_clears_are_noops() {
    for target in [false, true] {
        let (discovery, mut store, prop) = fixture();
        let message = |property| {
            if target {
                Homie5Message::PropertyTargetCleared { property }
            } else {
                Homie5Message::PropertyValueCleared { property }
            }
        };
        assert!(discovery
            .handle_event(message(prop.clone()), &mut store)
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            store
                .get_device(prop.device_ref())
                .unwrap()
                .prop_values
                .count(),
            0
        );
        let values = &mut store.get_device_mut(prop.device_ref()).unwrap().prop_values;
        values.store_value(prop.prop_pointer(), HomieValue::Integer(1));
        values.store_target(prop.prop_pointer(), HomieValue::Integer(2));
        assert!(discovery
            .handle_event(message(prop.clone()), &mut store)
            .await
            .unwrap()
            .is_some());
        assert!(discovery
            .handle_event(message(prop.clone()), &mut store)
            .await
            .unwrap()
            .is_none());
        // Even an orphaned cached value is not a known property without a description.
        store.get_device_mut(prop.device_ref()).unwrap().description = None;
        assert!(discovery
            .handle_event(message(prop.clone()), &mut store)
            .await
            .unwrap()
            .is_none());
        store.clear();
        assert!(discovery
            .handle_event(message(prop), &mut store)
            .await
            .unwrap()
            .is_none());
    }
}
