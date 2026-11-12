#[cfg(test)]
mod tests {
    use hc_homie5::query::*;
    use hc_homie5::value::*;
    use homie5::device_description::{
        DeviceDescriptionBuilder, IntegerRange, NodeDescriptionBuilder, PropertyDescriptionBuilder,
    };
    use homie5::{HomieDomain, HomieID, PropertyRef};

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
                        PropertyDescriptionBuilder::integer()
                            .integer_range(IntegerRange {
                                min: Some(1),
                                max: Some(20),
                                step: None,
                            })
                            .build(),
                    )
                    .add_property(
                        HomieID::new_const("prop-2"),
                        PropertyDescriptionBuilder::float().build(),
                    )
                    .add_property(
                        HomieID::new_const("prop-3"),
                        PropertyDescriptionBuilder::boolean().build(),
                    )
                    .build(),
            )
            .add_node(
                HomieID::new_const("node-2"),
                NodeDescriptionBuilder::new()
                    .name("Testnode no 2")
                    .add_property(
                        HomieID::new_const("state"),
                        PropertyDescriptionBuilder::integer()
                            .integer_range(IntegerRange {
                                min: Some(1),
                                max: Some(20),
                                step: None,
                            })
                            .build(),
                    )
                    .build(),
            )
            .build();
        let query: QueryDefinition = serde_yaml_ng::from_str(yaml).unwrap();
        let refs: Vec<PropertyRef> = query.match_query(
            &HomieDomain::Default,
            &HomieID::new_const("device-1"),
            &desc,
            None,
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
        let query: DeviceQuery = serde_yaml_ng::from_str(yaml).unwrap();
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
        let query: DeviceQuery = serde_yaml_ng::from_str(yaml).unwrap();
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
        let query: DeviceQuery = serde_yaml_ng::from_str(yaml).unwrap();
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
        let query: QueryDefinition = serde_yaml_ng::from_str(yaml).unwrap();

        // Build a device description with one node that has two properties.
        // One property has the name "temperature" and the other "humidity".
        let device_desc = DeviceDescriptionBuilder::new()
            .add_node(
                HomieID::new_const("node-1"),
                NodeDescriptionBuilder::new()
                    .name("Test Node")
                    .add_property(
                        HomieID::new_const("temp"),
                        PropertyDescriptionBuilder::float()
                            .name("temperature")
                            .build(),
                    )
                    .add_property(
                        HomieID::new_const("humidity"),
                        PropertyDescriptionBuilder::float().name("humidity").build(),
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
        mat_query.add_materialized(&HomieDomain::Default, &device_id, &device_desc, None);

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

    #[test]
    fn query_definition_rejects_unknown_field() {
        // `propertty` used to be ignored, matching every property of every device
        let res = serde_yaml_ng::from_str::<QueryDefinition>("propertty:\n  id: state\n");
        assert!(res.is_err(), "{res:?}");
    }

    #[test]
    fn device_node_property_queries_reject_unknown_fields() {
        assert!(serde_yaml_ng::from_str::<DeviceQuery>("idd: device-1\n").is_err());
        assert!(serde_yaml_ng::from_str::<NodeQuery>("typ: test-type\n").is_err());
        assert!(serde_yaml_ng::from_str::<PropertyQuery>("datatyp: integer\n").is_err());
        assert!(serde_yaml_ng::from_str::<QueryDefinition>("node:\n  typ: test-type\n").is_err());
    }

    #[test]
    fn query_schema_disallows_additional_properties() {
        let schema = schemars::schema_for!(QueryDefinition);
        let additional = schema.get("additionalProperties").and_then(|v| v.as_bool());
        assert_eq!(additional, Some(false), "{schema:?}");
    }
}

#[cfg(feature = "ext-meta")]
mod meta {
    use hc_homie5::query::*;
    use homie5::device_description::{
        DeviceDescriptionBuilder, HomieDeviceDescription, NodeDescriptionBuilder,
        PropertyDescriptionBuilder,
    };
    use homie5::extensions::meta::{MetaDeviceLevel, MetaDeviceOverlay, MetaEntries, MetaValue};
    use homie5::{HomieDomain, HomieID, PropertyRef};

    fn desc() -> HomieDeviceDescription {
        DeviceDescriptionBuilder::new()
            .add_node(
                HomieID::new_const("thermostat"),
                NodeDescriptionBuilder::new()
                    .r#type("thermostat")
                    .add_property(
                        HomieID::new_const("set-temperature"),
                        PropertyDescriptionBuilder::float().build(),
                    )
                    .build(),
            )
            .build()
    }

    fn overlay(entries: &[(&str, MetaValue)]) -> MetaDeviceOverlay {
        let annotations: MetaEntries = entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        MetaDeviceOverlay {
            schema: 2,
            device: Some(MetaDeviceLevel {
                annotations: Some(annotations),
                nodes: None,
            }),
        }
    }

    fn text(s: &str) -> MetaValue {
        MetaValue::Text(s.to_string())
    }

    fn list(items: &[&str]) -> MetaValue {
        MetaValue::List(items.iter().map(|s| s.to_string()).collect())
    }

    fn matches(yaml: &str, meta: Option<&MetaDeviceOverlay>) -> bool {
        let query: QueryDefinition = serde_yaml_ng::from_str(yaml).unwrap();
        let refs = query.match_query(
            &HomieDomain::Default,
            &HomieID::new_const("dev-1"),
            &desc(),
            meta.map(|m| m as &dyn MetaAnnotations),
        );
        !refs.is_empty()
    }

    #[test]
    fn list_annotation_matches_any_element() {
        let meta = overlay(&[("roles", list(&["heating", "smart-thermostat"]))]);
        assert!(matches(
            "meta:\n  annotations:\n    roles: smart-thermostat\n",
            Some(&meta)
        ));
        assert!(matches(
            "meta:\n  annotations:\n    roles: heating\n",
            Some(&meta)
        ));
        assert!(!matches(
            "meta:\n  annotations:\n    roles: lighting\n",
            Some(&meta)
        ));
        assert!(matches(
            "meta:\n  annotations:\n    roles:\n      operator: includesAny\n      value: [lighting, heating]\n",
            Some(&meta)
        ));
        assert!(matches(
            "meta:\n  annotations:\n    roles:\n      pattern: '^smart-'\n",
            Some(&meta)
        ));
    }

    #[test]
    fn negative_operators_require_every_list_element() {
        let meta = overlay(&[("roles", list(&["heating", "smart-thermostat"]))]);
        assert!(!matches(
            "meta:\n  annotations:\n    roles:\n      operator: includesNone\n      value: [heating]\n",
            Some(&meta)
        ));
        assert!(matches(
            "meta:\n  annotations:\n    roles:\n      operator: includesNone\n      value: [lighting]\n",
            Some(&meta)
        ));
        assert!(!matches(
            "meta:\n  annotations:\n    roles:\n      operator: \"<>\"\n      value: heating\n",
            Some(&meta)
        ));
    }

    #[test]
    fn scalar_annotation_is_a_one_element_list() {
        let meta = overlay(&[("room", text("bathroom"))]);
        assert!(matches(
            "meta:\n  annotations:\n    room: bathroom\n",
            Some(&meta)
        ));
        assert!(!matches(
            "meta:\n  annotations:\n    room: kitchen\n",
            Some(&meta)
        ));
        assert!(matches(
            "meta:\n  annotations:\n    room:\n      first: bathroom\n",
            Some(&meta)
        ));
    }

    #[test]
    fn missing_annotation_does_not_match_except_absence_operators() {
        let meta = overlay(&[("room", text("bathroom"))]);
        for meta in [Some(&meta), None] {
            assert!(!matches("meta:\n  annotations:\n    floor: first\n", meta));
            assert!(!matches(
                "meta:\n  annotations:\n    floor:\n      operator: exists\n",
                meta
            ));
            assert!(!matches(
                "meta:\n  annotations:\n    floor:\n      operator: includesNone\n      value: [x]\n",
                meta
            ));
            assert!(!matches(
                "meta:\n  annotations:\n    floor:\n      first: first\n",
                meta
            ));
            assert!(matches(
                "meta:\n  annotations:\n    floor:\n      operator: isEmpty\n",
                meta
            ));
            assert!(matches(
                "meta:\n  annotations:\n    floor:\n      operator: matchAlways\n",
                meta
            ));
        }
        let empty = overlay(&[("floor", list(&[]))]);
        assert!(!matches(
            "meta:\n  annotations:\n    floor:\n      operator: exists\n",
            Some(&empty)
        ));
        assert!(matches(
            "meta:\n  annotations:\n    floor:\n      operator: isEmpty\n",
            Some(&empty)
        ));
        // An overlay without a device level has no annotations.
        let bare = MetaDeviceOverlay {
            schema: 2,
            device: None,
        };
        assert!(!matches(
            "meta:\n  annotations:\n    room: bathroom\n",
            Some(&bare)
        ));
    }

    #[test]
    fn first_matches_only_the_first_list_element() {
        let meta = overlay(&[("rooms", list(&["bathroom", "hall"]))]);
        assert!(matches(
            "meta:\n  annotations:\n    rooms:\n      first: bathroom\n",
            Some(&meta)
        ));
        assert!(!matches(
            "meta:\n  annotations:\n    rooms:\n      first: hall\n",
            Some(&meta)
        ));
        assert!(matches(
            "meta:\n  annotations:\n    rooms:\n      first:\n        operator: \"=\"\n        value: [kitchen, bathroom]\n",
            Some(&meta)
        ));
    }

    #[test]
    fn all_annotations_and_other_conditions_must_match() {
        let meta = overlay(&[
            ("room", text("bathroom")),
            ("roles", list(&["smart-thermostat"])),
        ]);
        assert!(matches(
            "meta:\n  annotations:\n    room: bathroom\n    roles: smart-thermostat\nnode:\n  type: thermostat\n",
            Some(&meta)
        ));
        assert!(!matches(
            "meta:\n  annotations:\n    room: bathroom\n    roles: lighting\n",
            Some(&meta)
        ));
        assert!(!matches(
            "meta:\n  annotations:\n    room: bathroom\nnode:\n  type: switch\n",
            Some(&meta)
        ));
        // An empty meta query matches regardless of the overlay.
        assert!(matches("meta: {}\n", None));
    }

    #[test]
    fn plain_annotation_maps_are_a_source_too() {
        let entries: MetaEntries = [("room".to_string(), text("bathroom"))]
            .into_iter()
            .collect();
        let query: QueryDefinition =
            serde_yaml_ng::from_str("meta:\n  annotations:\n    room: bathroom\n").unwrap();
        let refs = query.match_query(
            &HomieDomain::Default,
            &HomieID::new_const("dev-1"),
            &desc(),
            Some(&entries),
        );
        assert_eq!(refs.len(), 1);
    }

    const TREE_QUERY: &str = r#"
domain: homie
device:
  id:
    operator: "<>"
    value: other
meta:
  annotations:
    roles:
      operator: includesAny
      value: [smart-thermostat]
    room:
      first:
        operator: "="
        value: [bathroom, hall]
node:
  type: thermostat
property:
  id: set-temperature
"#;

    #[test]
    fn query_definition_with_meta_deserializes_from_text_and_from_tree() {
        let meta = overlay(&[
            ("room", list(&["bathroom", "x"])),
            ("roles", list(&["smart-thermostat"])),
        ]);
        let from_text: QueryDefinition = serde_yaml_ng::from_str(TREE_QUERY).unwrap();
        let tree: serde_yaml_ng::Value = serde_yaml_ng::from_str(TREE_QUERY).unwrap();
        let from_tree: QueryDefinition = serde_yaml_ng::from_value(tree).unwrap();
        let expected = vec![PropertyRef::new(
            HomieDomain::Default,
            HomieID::new_const("dev-1"),
            HomieID::new_const("thermostat"),
            HomieID::new_const("set-temperature"),
        )];
        for query in [from_text, from_tree] {
            let refs = query.match_query(
                &HomieDomain::Default,
                &HomieID::new_const("dev-1"),
                &desc(),
                Some(&meta),
            );
            assert_eq!(refs, expected);
            let refs = query.match_query(
                &HomieDomain::Default,
                &HomieID::new_const("dev-1"),
                &desc(),
                None,
            );
            assert!(refs.is_empty());
        }
    }

    #[test]
    fn meta_query_rejects_unknown_fields() {
        assert!(
            serde_yaml_ng::from_str::<QueryDefinition>("meta:\n  annotation:\n    room: x\n")
                .is_err()
        );
        assert!(serde_yaml_ng::from_str::<MetaCondition>("first: x\nlast: y\n").is_err());
    }

    #[test]
    fn materialized_query_follows_meta_and_removes_by_device() {
        let mut query: MaterializedQuery =
            serde_yaml_ng::from_str("meta:\n  annotations:\n    room: bathroom\n").unwrap();
        let id = HomieID::new_const("dev-1");
        let prop = PropertyRef::new(
            HomieDomain::Default,
            id.clone(),
            HomieID::new_const("thermostat"),
            HomieID::new_const("set-temperature"),
        );
        query.add_materialized(&HomieDomain::Default, &id, &desc(), None);
        assert!(!query.match_query(&prop));
        let meta = overlay(&[("room", text("bathroom"))]);
        query.add_materialized(&HomieDomain::Default, &id, &desc(), Some(&meta));
        assert!(query.match_query(&prop));
        // Removal does not need the overlay that produced the match.
        query.remove_materialized(&HomieDomain::Default, &id, &desc());
        assert!(!query.match_query(&prop));
    }
}
