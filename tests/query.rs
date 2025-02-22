#[cfg(test)]
mod tests {
    use hc_homie5::*;
    use homie5::device_description::{
        DeviceDescriptionBuilder, HomiePropertyFormat, IntegerRange, NodeDescriptionBuilder,
        PropertyDescriptionBuilder,
    };
    use homie5::{HomieDataType, HomieDomain, HomieID, PropertyRef};

    // A basic QueryDefinition test that exercises the full matching logic.
    #[test]
    fn test_query_def() {
        // This query matches devices with id "device-1", nodes of type "test-type",
        // and properties with a datatype equal to either "integer" or "float".
        let yaml = r#"
domain: homie
device:
    id: device-1
node:
    type: test-type
property:
  datatype:
    operator: "="
    value: ["integer", "float"]
"#;
        let desc = DeviceDescriptionBuilder::new()
            .add_node(
                HomieID::new_const("node-1"),
                NodeDescriptionBuilder::new()
                    .name("Testnode")
                    .r#type("test-type")
                    .add_property(
                        HomieID::new_const("prop-1"),
                        PropertyDescriptionBuilder::new(HomieDataType::Integer)
                            .format(HomiePropertyFormat::IntegerRange(IntegerRange {
                                min: Some(1),
                                max: Some(20),
                                step: None,
                            }))
                            .build(),
                    )
                    .add_property(
                        HomieID::new_const("prop-2"),
                        PropertyDescriptionBuilder::new(HomieDataType::Float).build(),
                    )
                    .add_property(
                        HomieID::new_const("prop-3"),
                        PropertyDescriptionBuilder::new(HomieDataType::Boolean).build(),
                    )
                    .build(),
            )
            .add_node(
                HomieID::new_const("node-2"),
                NodeDescriptionBuilder::new()
                    .name("Testnode no 2")
                    .add_property(
                        HomieID::new_const("state"),
                        PropertyDescriptionBuilder::new(HomieDataType::Integer)
                            .format(HomiePropertyFormat::IntegerRange(IntegerRange {
                                min: Some(1),
                                max: Some(20),
                                step: None,
                            }))
                            .build(),
                    )
                    .build(),
            )
            .build();
        let query: QueryDefinition = serde_yml::from_str(yaml).unwrap();
        let refs: Vec<PropertyRef> = query.match_query(
            &HomieDomain::Default,
            &HomieID::new_const("device-1"),
            &desc,
        );
        let cmp_refs: Vec<PropertyRef> = vec![
            PropertyRef::new(
                HomieDomain::Default,
                HomieID::new_const("device-1"),
                HomieID::new_const("node-1"),
                HomieID::new_const("prop-1"),
            ),
            PropertyRef::new(
                HomieDomain::Default,
                HomieID::new_const("device-1"),
                HomieID::new_const("node-1"),
                HomieID::new_const("prop-2"),
            ),
        ];
        // Debug output may help during development:
        println!("Matched Property Refs: {:#?}", refs);
        assert_eq!(refs, cmp_refs);
    }

    // Test deserialization and evaluation of vector operators on DeviceQuery.
    #[test]
    fn test_deserialize_and_evaluate_vector_includes_any() {
        let yaml = r#"
children:
    operator: "includesAny"
    value:
      - ["child1", "child2"]
      - ["child3"]
"#;
        let query: DeviceQuery = serde_yml::from_str(yaml).unwrap();
        let children = vec!["child1".try_into().unwrap(), "child4".try_into().unwrap()];
        assert!(query.children.as_ref().unwrap().evaluate(&children));

        let children = vec!["child5".try_into().unwrap()];
        assert!(!query.children.as_ref().unwrap().evaluate(&children));
    }

    #[test]
    fn test_deserialize_and_evaluate_vector_includes_all() {
        let yaml = r#"
children:
    operator: "="
    value:
      - ["child1", "child2", "child3"]
"#;
        let query: DeviceQuery = serde_yml::from_str(yaml).unwrap();
        let children = vec![
            "child1".try_into().unwrap(),
            "child2".try_into().unwrap(),
            "child3".try_into().unwrap(),
        ];
        assert!(query.children.as_ref().unwrap().evaluate(&children));

        let children = vec!["child5".try_into().unwrap()];
        assert!(!query.children.as_ref().unwrap().evaluate(&children));
    }

    #[test]
    fn test_deserialize_and_evaluate_extensions_includes_none() {
        let yaml = r#"
extensions:
    operator: "includesNone"
    value:
      - "deprecated"
      - "legacy"
"#;
        let query: DeviceQuery = serde_yml::from_str(yaml).unwrap();
        let extensions = vec!["mqtt".to_string(), "homie5".to_string()];
        assert!(query.extensions.as_ref().unwrap().evaluate(&extensions));

        let extensions = vec!["legacy".to_string(), "mqtt".to_string()];
        assert!(!query.extensions.as_ref().unwrap().evaluate(&extensions));
    }

    // --- New tests for NodeQuery ---

    #[test]
    fn test_node_query_match() {
        // Build a NodeQuery that requires:
        // - id equal to "node-1"
        // - name equal to "Test Node"
        // - type equal to "sensor"
        let node_query = NodeQuery {
            id: Some(ValueCondition::Value(HomieID::new_const("node-1"))),
            name: Some(ValueCondition::Value("Test Node".to_string())),
            r#type: Some(ValueCondition::Value("sensor".to_string())),
        };

        // Create a matching node description.
        let node_desc = NodeDescriptionBuilder::new()
            .name("Test Node")
            .r#type("sensor")
            .build();

        // The query should match when the id and node description match.
        assert!(node_query.match_query(&HomieID::new_const("node-1"), &node_desc));

        // Create a node description with a mismatched name.
        let node_desc_bad = NodeDescriptionBuilder::new()
            .name("Wrong Name")
            .r#type("sensor")
            .build();

        assert!(!node_query.match_query(&HomieID::new_const("node-1"), &node_desc_bad));
    }

    // --- New tests for MaterializedQuery ---

    #[test]
    fn test_materialized_query_property_name() {
        // This query looks for a property whose name is "temperature".
        let yaml = r#"
domain: homie
device:
    id: device-1
node:
    name: "Test Node"
property:
  name: "temperature"
"#;
        let query: QueryDefinition = serde_yml::from_str(yaml).unwrap();

        // Build a device description with one node that has two properties.
        // One property has the name "temperature" and the other "humidity".
        let device_desc = DeviceDescriptionBuilder::new()
            .add_node(
                HomieID::new_const("node-1"),
                NodeDescriptionBuilder::new()
                    .name("Test Node")
                    .add_property(
                        HomieID::new_const("temp"),
                        PropertyDescriptionBuilder::new(HomieDataType::Float)
                            .name("temperature")
                            .build(),
                    )
                    .add_property(
                        HomieID::new_const("humidity"),
                        PropertyDescriptionBuilder::new(HomieDataType::Float)
                            .name("humidity")
                            .build(),
                    )
                    .build(),
            )
            .build();

        let device_id = HomieID::new_const("device-1");
        let mut mat_query = MaterializedQuery::new(query);

        // Initially the materialized query holds no references.
        let expected_ref = PropertyRef::new(
            HomieDomain::Default,
            device_id.clone(),
            HomieID::new_const("node-1"),
            HomieID::new_const("temp"),
        );
        assert!(!mat_query.match_query(&expected_ref));

        // Add materialized properties based on the query.
        mat_query.add_materialized(&HomieDomain::Default, &device_id, &device_desc);

        // Now only the property with id "temp" (name "temperature") should match.
        assert!(mat_query.match_query(&expected_ref));

        let non_expected_ref = PropertyRef::new(
            HomieDomain::Default,
            device_id.clone(),
            HomieID::new_const("node-1"),
            HomieID::new_const("humidity"),
        );
        assert!(!mat_query.match_query(&non_expected_ref));

        // Remove materialized properties and verify that the reference is no longer matched.
        mat_query.remove_materialized(&HomieDomain::Default, &device_id, &device_desc);
        assert!(!mat_query.match_query(&expected_ref));
    }
}
