#[cfg(test)]
mod tests {
    use hc_homie5::device::build_bridge_controller_description;
    use homie5::device_description::HomiePropertyFormat;
    use homie5::{HomieDomain, HomieID};

    #[test]
    fn test_build_bridge_controller_description() {
        let controller_id: HomieID = "test-bridge".try_into().unwrap();
        let domain: HomieDomain = "homie".try_into().unwrap();
        let (action_prop, desc) = build_bridge_controller_description(
            &controller_id,
            "Test Bridge",
            &domain,
            &["refresh", "update"],
        );

        // Check that description has a control node
        assert!(desc.nodes.contains_key(&HomieID::new_const("control")));
        let node = desc.nodes.get(&HomieID::new_const("control")).unwrap();

        // Check the action property exists
        assert!(node.properties.contains_key(&HomieID::new_const("action")));
        let prop = node.properties.get(&HomieID::new_const("action")).unwrap();
        assert!(prop.settable);
        assert!(!prop.retained);

        // Check action_prop references correct path
        assert_eq!(action_prop.prop_id(), &HomieID::new_const("action"));
        assert_eq!(action_prop.device_id(), &controller_id);

        // Check name
        assert_eq!(desc.name.as_deref(), Some("Test Bridge"));

        // Check children are initially empty
        assert!(desc.children.is_empty());
    }

    #[test]
    fn test_build_controller_description_custom_variants() {
        let controller_id: HomieID = "my-ctrl".try_into().unwrap();
        let domain: HomieDomain = "homie".try_into().unwrap();
        let (_, desc) = build_bridge_controller_description(
            &controller_id,
            "My Controller",
            &domain,
            &["start", "stop", "restart"],
        );

        let node = desc.nodes.get(&HomieID::new_const("control")).unwrap();
        let prop = node.properties.get(&HomieID::new_const("action")).unwrap();

        // The format should be Enum with all three variants
        match &prop.format {
            HomiePropertyFormat::Enum(values) => {
                assert!(values.contains(&"start".to_string()));
                assert!(values.contains(&"stop".to_string()));
                assert!(values.contains(&"restart".to_string()));
                assert_eq!(values.len(), 3);
            }
            _ => panic!("Expected Enum format"),
        }
    }

    #[test]
    fn test_bridge_controller_description_structure() {
        let controller_id: HomieID = "test-ctrl".try_into().unwrap();
        let domain: HomieDomain = "homie".try_into().unwrap();
        let (action_prop, desc) = build_bridge_controller_description(
            &controller_id,
            "Test",
            &domain,
            &["refresh"],
        );

        // Verify the description has expected structure
        assert_eq!(desc.children.len(), 0);
        assert_eq!(desc.nodes.len(), 1);
        assert_eq!(action_prop.device_id(), &controller_id);

        // Check the enum has exactly one variant
        let node = desc.nodes.get(&HomieID::new_const("control")).unwrap();
        let prop = node.properties.get(&HomieID::new_const("action")).unwrap();
        match &prop.format {
            HomiePropertyFormat::Enum(values) => {
                assert_eq!(values, &["refresh"]);
            }
            _ => panic!("Expected Enum format"),
        }
    }
}
