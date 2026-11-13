#![cfg(feature = "ext-meta")]

use hc_homie5::model::Device;
use hc_homie5::query::{MetaAnnotations, QueryDefinition};
use homie5::device_description::{
    DeviceDescriptionBuilder, NodeDescriptionBuilder, PropertyDescriptionBuilder,
};
use homie5::extensions::meta::{
    merge_device_overlays, MetaDeviceLevel, MetaDeviceOverlay, MetaEntries, MetaValue,
};
use homie5::{DeviceRef, HomieDeviceStatus, HomieDomain, HomieID};

fn id(s: &str) -> HomieID {
    HomieID::try_from(s.to_string()).unwrap()
}

fn overlay(entries: &[(&str, &str)]) -> MetaDeviceOverlay {
    let annotations: MetaEntries = entries
        .iter()
        .map(|(k, v)| (k.to_string(), MetaValue::Text(v.to_string())))
        .collect();
    MetaDeviceOverlay {
        schema: 2,
        device: Some(MetaDeviceLevel {
            annotations: Some(annotations),
            nodes: None,
        }),
    }
}

fn device() -> Device {
    let mut device = Device::new(
        DeviceRef::new(HomieDomain::Default, id("dev-1")),
        HomieDeviceStatus::Ready,
    );
    device.description = Some(
        DeviceDescriptionBuilder::new()
            .add_node(
                id("node"),
                NodeDescriptionBuilder::new()
                    .add_property(id("prop"), PropertyDescriptionBuilder::boolean().build())
                    .build(),
            )
            .build(),
    );
    device
}

/// The cache equals a fresh merge of the stored overlays in provider-id order.
fn assert_cache_consistent(device: &Device) {
    let mut providers: Vec<_> = device.meta_overlays().iter().collect();
    providers.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
    let expected = merge_device_overlays(providers.into_iter().map(|(_, o)| o));
    assert_eq!(device.merged_meta_overlay(), &expected);
    // With at most one provider the order cannot matter.
    if device.meta_overlays().len() <= 1 {
        assert_eq!(
            device.merged_meta_overlay(),
            &merge_device_overlays(device.meta_overlays().values())
        );
    }
}

fn room(device: &Device) -> Option<Vec<String>> {
    device.device_annotation("room").map(<[String]>::to_vec)
}

#[test]
fn new_device_has_no_overlays() {
    let device = device();
    assert!(device.meta_overlays().is_empty());
    assert_eq!(device.merged_meta_overlay().device, None);
    assert_cache_consistent(&device);
}

#[test]
fn cache_follows_set_and_remove() {
    let mut device = device();
    assert_eq!(
        device.set_meta_overlay(id("p-b"), overlay(&[("room", "hall")])),
        None
    );
    assert_cache_consistent(&device);
    assert_eq!(room(&device), Some(vec!["hall".to_string()]));

    device.set_meta_overlay(id("p-a"), overlay(&[("room", "bathroom"), ("floor", "1")]));
    assert_cache_consistent(&device);
    // Same key in two providers: merged list, in provider-id order.
    assert_eq!(
        room(&device),
        Some(vec!["bathroom".to_string(), "hall".to_string()])
    );
    assert_eq!(
        device
            .merged_meta_overlay()
            .device
            .as_ref()
            .unwrap()
            .annotations
            .as_ref()
            .unwrap()["room"],
        MetaValue::List(vec!["bathroom".into(), "hall".into()])
    );

    // Removing one provider leaves the other as the single source (scalar again).
    let removed = device.remove_meta_overlay(&id("p-a"));
    assert!(removed.is_some());
    assert_cache_consistent(&device);
    assert_eq!(room(&device), Some(vec!["hall".to_string()]));
    assert_eq!(
        device
            .merged_meta_overlay()
            .device
            .as_ref()
            .unwrap()
            .annotations
            .as_ref()
            .unwrap()["room"],
        MetaValue::Text("hall".into())
    );
    assert!(device.device_annotation("floor").is_none());

    // Removing an absent provider is a no-op.
    assert!(device.remove_meta_overlay(&id("p-x")).is_none());
    assert_cache_consistent(&device);

    // Replacing a provider's overlay updates the cache; an identical overlay changes nothing.
    let previous = device.set_meta_overlay(id("p-b"), overlay(&[("room", "kitchen")]));
    assert_eq!(previous, Some(overlay(&[("room", "hall")])));
    assert_eq!(room(&device), Some(vec!["kitchen".to_string()]));
    device.set_meta_overlay(id("p-b"), overlay(&[("room", "kitchen")]));
    assert_cache_consistent(&device);

    device.remove_meta_overlay(&id("p-b"));
    assert_cache_consistent(&device);
    assert_eq!(room(&device), None);
}

#[test]
fn cloned_device_keeps_the_cache() {
    let mut device = device();
    device.set_meta_overlay(id("p-a"), overlay(&[("room", "bathroom")]));
    let clone = device.clone();
    assert_eq!(clone.merged_meta_overlay(), device.merged_meta_overlay());
}

#[test]
fn match_query_reads_the_device_cache() {
    let query: QueryDefinition =
        serde_yaml_ng::from_str("meta:\n  annotations:\n    room: { any: hall }\n").unwrap();
    let mut device = device();
    let matches = |device: &Device| {
        !query
            .match_query(
                device.homie_domain(),
                device.device_id(),
                device.description.as_ref().unwrap(),
                Some(device),
            )
            .is_empty()
    };
    assert!(!matches(&device));
    device.set_meta_overlay(id("p-a"), overlay(&[("room", "bathroom")]));
    assert!(!matches(&device));
    device.set_meta_overlay(id("p-b"), overlay(&[("room", "hall")]));
    assert!(matches(&device));
    device.remove_meta_overlay(&id("p-b"));
    assert!(!matches(&device));
}
