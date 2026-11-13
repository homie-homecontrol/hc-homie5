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

    fn ids(items: &[&str]) -> Vec<HomieID> {
        items
            .iter()
            .map(|s| HomieID::try_from(s.to_string()).unwrap())
            .collect()
    }

    // DeviceQuery list fields take a ListCondition.
    #[test]
    fn children_any_and_empty() {
        let query: DeviceQuery = serde_yaml_ng::from_str(
            "children:\n  any:\n    operator: \"=\"\n    value: [child1, child3]\n",
        )
        .unwrap();
        let cond = query.children.as_ref().unwrap();
        assert!(cond.evaluate(&ids(&["child1", "child4"])));
        assert!(!cond.evaluate(&ids(&["child5"])));
        assert!(!cond.evaluate(&[]));

        let query: DeviceQuery = serde_yaml_ng::from_str("children: { empty: true }\n").unwrap();
        let cond = query.children.as_ref().unwrap();
        assert!(cond.evaluate(&[]));
        assert!(!cond.evaluate(&ids(&["child1"])));
    }

    #[test]
    fn extensions_any_pattern_and_none() {
        let query: DeviceQuery = serde_yaml_ng::from_str(
            "extensions:\n  any: { pattern: \"^org\\\\.homie\\\\.meta:\" }\n",
        )
        .unwrap();
        let cond = query.extensions.as_ref().unwrap();
        assert!(cond.evaluate(&[
            "org.homie.legacy-stats:0.1.1:[4.x]".to_string(),
            "org.homie.meta:0.1.0:[5.x]".to_string()
        ]));
        assert!(!cond.evaluate(&["org.homie.legacy-stats:0.1.1:[4.x]".to_string()]));
        assert!(!cond.evaluate(&["xorg.homie.meta:0.1.0".to_string()]));

        let query: DeviceQuery = serde_yaml_ng::from_str(
            "extensions:\n  none: { operator: includesAny, value: [deprecated, legacy] }\n",
        )
        .unwrap();
        let cond = query.extensions.as_ref().unwrap();
        assert!(cond.evaluate(&["mqtt".to_string(), "homie5".to_string()]));
        assert!(!cond.evaluate(&["legacy".to_string(), "mqtt".to_string()]));
    }

    #[test]
    fn device_query_matches_children_and_extensions() {
        let desc = DeviceDescriptionBuilder::new()
            .add_child(HomieID::new_const("child-1"))
            .add_extension("org.homie.meta:0.1.0:[5.x]".to_string())
            .build();
        let id = HomieID::new_const("bridge");
        let query: DeviceQuery = serde_yaml_ng::from_str(
            "children: { any: child-1 }\nextensions: { any: { pattern: \"^org\\\\.homie\\\\.meta:\" } }\n",
        )
        .unwrap();
        assert!(query.match_query(&id, &desc));
        let query: DeviceQuery = serde_yaml_ng::from_str("children: { empty: true }\n").unwrap();
        assert!(!query.match_query(&id, &desc));
        let query: DeviceQuery = serde_yaml_ng::from_str("children: { empty: false }\n").unwrap();
        assert!(query.match_query(&id, &desc));
    }

    #[test]
    fn list_fields_reject_bare_conditions() {
        assert!(serde_yaml_ng::from_str::<DeviceQuery>("children: child1\n").is_err());
        assert!(serde_yaml_ng::from_str::<DeviceQuery>(
            "extensions:\n  operator: includesAny\n  value: [x]\n"
        )
        .is_err());
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

    /// Whether `annotations` (a YAML mapping body for `meta.annotations`) matches `meta`.
    fn matches(annotations: &str, meta: Option<&MetaDeviceOverlay>) -> bool {
        let yaml = format!("meta:\n  annotations: {annotations}\n");
        let query: QueryDefinition = serde_yaml_ng::from_str(&yaml).unwrap();
        let refs = query.match_query(
            &HomieDomain::Default,
            &HomieID::new_const("dev-1"),
            &desc(),
            meta.map(|m| m as &dyn MetaAnnotations),
        );
        !refs.is_empty()
    }

    #[test]
    fn any_with_literal_operator_and_pattern() {
        let meta = overlay(&[("roles", list(&["heating", "smart-thermostat"]))]);
        assert!(matches("{ roles: { any: smart-thermostat } }", Some(&meta)));
        assert!(!matches("{ roles: { any: lighting } }", Some(&meta)));
        assert!(matches(
            "{ roles: { any: { operator: \"=\", value: [lighting, heating] } } }",
            Some(&meta)
        ));
        assert!(!matches(
            "{ roles: { any: { operator: \"=\", value: [lighting, audio] } } }",
            Some(&meta)
        ));
        assert!(matches(
            "{ roles: { any: { pattern: '^smart-' } } }",
            Some(&meta)
        ));
        assert!(!matches(
            "{ roles: { any: { pattern: '^dumb-' } } }",
            Some(&meta)
        ));
    }

    #[test]
    fn all_none_and_first() {
        let meta = overlay(&[("roles", list(&["heating", "smart-thermostat"]))]);
        assert!(matches(
            "{ roles: { all: { pattern: '^(heating|smart-)' } } }",
            Some(&meta)
        ));
        assert!(!matches("{ roles: { all: heating } }", Some(&meta)));
        assert!(matches("{ roles: { none: lighting } }", Some(&meta)));
        assert!(!matches("{ roles: { none: heating } }", Some(&meta)));
        assert!(matches("{ roles: { first: heating } }", Some(&meta)));
        assert!(!matches(
            "{ roles: { first: smart-thermostat } }",
            Some(&meta)
        ));
    }

    #[test]
    fn empty_true_and_false() {
        let meta = overlay(&[("roles", list(&["heating"])), ("tags", list(&[]))]);
        assert!(matches("{ roles: { empty: false } }", Some(&meta)));
        assert!(!matches("{ roles: { empty: true } }", Some(&meta)));
        assert!(matches("{ tags: { empty: true } }", Some(&meta)));
        assert!(!matches("{ tags: { empty: false } }", Some(&meta)));
    }

    #[test]
    fn scalar_annotation_is_a_one_element_list_of_strings() {
        let meta = overlay(&[("room", text("bathroom")), ("heating", text("true"))]);
        assert!(matches("{ room: { any: bathroom } }", Some(&meta)));
        assert!(matches("{ room: { first: bathroom } }", Some(&meta)));
        assert!(matches("{ room: { all: bathroom } }", Some(&meta)));
        assert!(!matches("{ room: { any: kitchen } }", Some(&meta)));
        // Annotation values are strings: compare with "true".
        assert!(matches("{ heating: { any: \"true\" } }", Some(&meta)));
    }

    #[test]
    fn missing_annotation_is_an_empty_list() {
        let meta = overlay(&[("room", text("bathroom"))]);
        let bare = MetaDeviceOverlay {
            schema: 2,
            device: None,
        };
        for meta in [Some(&meta), Some(&bare), None] {
            assert!(!matches("{ floor: { any: first } }", meta));
            assert!(!matches("{ floor: { first: first } }", meta));
            assert!(matches("{ floor: { all: first } }", meta));
            assert!(matches("{ floor: { none: first } }", meta));
            assert!(matches("{ floor: { empty: true } }", meta));
            assert!(!matches("{ floor: { empty: false } }", meta));
        }
    }

    #[test]
    fn all_annotations_and_other_sections_must_match() {
        let meta = overlay(&[
            ("room", text("bathroom")),
            ("roles", list(&["smart-thermostat"])),
        ]);
        let query = |yaml: &str| serde_yaml_ng::from_str::<QueryDefinition>(yaml).unwrap();
        let run = |q: QueryDefinition| {
            !q.match_query(
                &HomieDomain::Default,
                &HomieID::new_const("dev-1"),
                &desc(),
                Some(&meta),
            )
            .is_empty()
        };
        assert!(run(query(
            "meta: { annotations: { room: { any: bathroom }, roles: { any: smart-thermostat } } }\nnode: { type: thermostat }\n"
        )));
        assert!(!run(query(
            "meta: { annotations: { room: { any: bathroom }, roles: { any: lighting } } }\n"
        )));
        assert!(!run(query(
            "meta: { annotations: { room: { any: bathroom } } }\nnode: { type: switch }\n"
        )));
        // An empty meta query matches regardless of the overlay.
        assert!(matches("{}", None));
    }

    #[test]
    fn plain_annotation_maps_are_a_source_too() {
        let entries: MetaEntries = [("room".to_string(), text("bathroom"))]
            .into_iter()
            .collect();
        let query: QueryDefinition =
            serde_yaml_ng::from_str("meta:\n  annotations:\n    room: { any: bathroom }\n")
                .unwrap();
        let refs = query.match_query(
            &HomieDomain::Default,
            &HomieID::new_const("dev-1"),
            &desc(),
            Some(&entries),
        );
        assert_eq!(refs.len(), 1);
    }

    const QUERY: &str = r#"
domain: homie
device:
  id:
    operator: "<>"
    value: other
  extensions: { empty: true }
meta:
  annotations:
    roles:
      any:
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
        let from_text: QueryDefinition = serde_yaml_ng::from_str(QUERY).unwrap();
        let tree: serde_yaml_ng::Value = serde_yaml_ng::from_str(QUERY).unwrap();
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
    fn meta_query_rejects_unknown_fields_and_bare_conditions() {
        assert!(serde_yaml_ng::from_str::<QueryDefinition>(
            "meta:\n  annotation:\n    room: { any: x }\n"
        )
        .is_err());
        assert!(serde_yaml_ng::from_str::<QueryDefinition>(
            "meta:\n  annotations:\n    room: bathroom\n"
        )
        .is_err());
        assert!(serde_yaml_ng::from_str::<QueryDefinition>(
            "meta:\n  annotations:\n    room: { operator: \"=\", value: x }\n"
        )
        .is_err());
    }

    #[test]
    fn materialized_query_follows_meta_and_removes_by_device() {
        let mut query: MaterializedQuery =
            serde_yaml_ng::from_str("meta:\n  annotations:\n    room: { any: bathroom }\n")
                .unwrap();
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

    #[test]
    fn materialized_query_keeps_same_device_id_in_other_domain() {
        let mut query: MaterializedQuery =
            serde_yaml_ng::from_str("node:\n  type: thermostat\n").unwrap();
        let id = HomieID::new_const("dev-1");
        let other: HomieDomain = "other".parse().unwrap();
        let prop = |domain: &HomieDomain| {
            PropertyRef::new(
                domain.clone(),
                id.clone(),
                HomieID::new_const("thermostat"),
                HomieID::new_const("set-temperature"),
            )
        };
        query.add_materialized(&HomieDomain::Default, &id, &desc(), None);
        query.add_materialized(&other, &id, &desc(), None);
        assert!(query.match_query(&prop(&HomieDomain::Default)));
        assert!(query.match_query(&prop(&other)));

        // Re-evaluating one domain's device leaves the other domain's matches alone.
        let no_match = DeviceDescriptionBuilder::new().build();
        query.add_materialized(&other, &id, &no_match, None);
        assert!(query.match_query(&prop(&HomieDomain::Default)));
        assert!(!query.match_query(&prop(&other)));
    }
}
