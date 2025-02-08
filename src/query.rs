use crate::value_condition::{ValueCondition, ValueConditionVec};
use homie5::{
    device_description::{
        HomieDeviceDescription, HomieNodeDescription, HomiePropertyDescription, HomiePropertyFormat,
    },
    HomieDataType, HomieDomain, HomieID, PropertyRef,
};
use serde::{Deserialize, Deserializer};
use std::collections::HashSet;

#[derive(Default, Debug, Clone, Deserialize)]
pub struct PropertyQuery {
    pub id: Option<ValueCondition<HomieID>>,
    pub name: Option<ValueCondition<String>>,
    pub datatype: Option<ValueCondition<HomieDataType>>,
    pub format: Option<ValueCondition<String>>,
    pub settable: Option<ValueCondition<bool>>,
    pub retained: Option<ValueCondition<bool>>,
    pub unit: Option<ValueCondition<String>>,
}

impl PropertyQuery {
    pub fn match_query(&self, id: &HomieID, property_desc: &HomiePropertyDescription) -> bool {
        self.id.as_ref().map_or(true, |cond| cond.evaluate(id))
            && self.name.as_ref().map_or(true, |cond| {
                cond.evaluate_option(property_desc.name.as_ref())
            })
            && self
                .datatype
                .as_ref()
                .map_or(true, |cond| cond.evaluate(&property_desc.datatype))
            && self.format.as_ref().map_or(true, |cond| {
                // Treat `Empty` as no value
                if let HomiePropertyFormat::Empty = property_desc.format {
                    false
                } else {
                    cond.evaluate(&property_desc.format.to_string())
                }
            })
            && self
                .settable
                .as_ref()
                .map_or(true, |cond| cond.evaluate(&property_desc.settable))
            && self
                .retained
                .as_ref()
                .map_or(true, |cond| cond.evaluate(&property_desc.retained))
            && self.unit.as_ref().map_or(true, |cond| {
                cond.evaluate_option(property_desc.unit.as_ref())
            })
    }
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct NodeQuery {
    pub id: Option<ValueCondition<HomieID>>,
    pub name: Option<ValueCondition<String>>,
    pub r#type: Option<ValueCondition<String>>,
}

impl NodeQuery {
    pub fn match_query(&self, id: &HomieID, node_desc: &HomieNodeDescription) -> bool {
        self.id.as_ref().map_or(true, |cond| cond.evaluate(id))
            && self
                .name
                .as_ref()
                .map_or(true, |cond| cond.evaluate_option(node_desc.name.as_ref()))
            && self
                .r#type
                .as_ref()
                .map_or(true, |cond| cond.evaluate_option(node_desc.r#type.as_ref()))
    }
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct DeviceQuery {
    pub id: Option<ValueCondition<HomieID>>,
    pub name: Option<ValueCondition<String>>,
    pub version: Option<ValueCondition<i64>>,
    pub homie: Option<ValueCondition<String>>,
    pub children: Option<ValueConditionVec<HomieID>>,
    pub root: Option<ValueCondition<HomieID>>,
    pub parent: Option<ValueCondition<HomieID>>,
    pub extensions: Option<ValueConditionVec<String>>,
}

impl DeviceQuery {
    pub fn match_query(&self, id: &HomieID, device_desc: &HomieDeviceDescription) -> bool {
        // Check each condition in sequence and short-circuit if any condition evaluates to `false`
        self.id.as_ref().map_or(true, |cond| cond.evaluate(id))
            && self
                .name
                .as_ref()
                .map_or(true, |cond| cond.evaluate_option(device_desc.name.as_ref()))
            && self
                .root
                .as_ref()
                .map_or(true, |cond| cond.evaluate_option(device_desc.root.as_ref()))
            && self
                .homie
                .as_ref()
                .map_or(true, |cond| cond.evaluate(&device_desc.homie))
            && self.parent.as_ref().map_or(true, |cond| {
                cond.evaluate_option(device_desc.parent.as_ref())
            })
            && self
                .version
                .as_ref()
                .map_or(true, |cond| cond.evaluate(&device_desc.version))
            && self
                .children
                .as_ref()
                .map_or(true, |cond| cond.evaluate(&device_desc.children))
            && self
                .extensions
                .as_ref()
                .map_or(true, |cond| cond.evaluate(&device_desc.extensions))
    }
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct QueryDefinition {
    #[serde(default)]
    pub domain: Option<ValueCondition<HomieDomain>>,
    #[serde(default)]
    pub device: Option<DeviceQuery>,
    #[serde(default)]
    pub node: Option<NodeQuery>,
    #[serde(default)]
    pub property: Option<PropertyQuery>,
}

impl QueryDefinition {
    pub fn match_query(
        &self,
        domain: &HomieDomain,
        id: &HomieID,
        device_desc: &HomieDeviceDescription,
    ) -> Vec<PropertyRef> {
        let mut matched_properties = Vec::new();

        // Check if the device matches the domain and device-level queries
        if self.domain.as_ref().map_or(true, |cond| {
            if let Some(v) = cond.value() {
                if matches!(v, HomieDomain::All) {
                    return true;
                }
            }
            cond.evaluate(domain)
        }) && self.device.as_ref().map_or(true, |device_query| {
            device_query.match_query(id, device_desc)
        }) {
            // Iterate through all nodes and their properties
            for (node_id, node_desc) in &device_desc.nodes {
                // Check if the node matches the node-level query
                if self.node.as_ref().map_or(true, |node_query| {
                    node_query.match_query(node_id, node_desc)
                }) {
                    for (prop_id, prop_desc) in &node_desc.properties {
                        // Check if the property matches the property-level query
                        if self.property.as_ref().map_or(true, |property_query| {
                            property_query.match_query(prop_id, prop_desc)
                        }) {
                            // Create a PropertyRef for the matched property
                            let property_ref = PropertyRef::new(
                                domain.clone(), // Use the passed domain
                                id.clone(),     // use the passed device id
                                node_id.clone(),
                                prop_id.clone(),
                            );
                            matched_properties.push(property_ref);
                        }
                    }
                }
            }
        }

        matched_properties
    }
}

#[derive(Clone, Debug)]
pub struct MaterializedQuery {
    query: QueryDefinition,
    mat_refs: HashSet<PropertyRef>, // Use HashSet for efficient lookups and removal
}

impl MaterializedQuery {
    pub fn new(query: QueryDefinition) -> Self {
        Self {
            query,
            mat_refs: HashSet::new(),
        }
    }

    pub fn add_materialized(
        &mut self,
        domain: &HomieDomain,
        id: &HomieID,
        device_desc: &HomieDeviceDescription,
    ) {
        // Remove all refs belonging to the given device ID
        self.mat_refs.retain(|prop_ref| prop_ref.device_id() != id);

        let new_refs = self.query.match_query(domain, id, device_desc);
        self.mat_refs.extend(new_refs); // Add new PropertyRefs to the HashSet
    }

    pub fn remove_materialized(
        &mut self,
        domain: &HomieDomain,
        id: &HomieID,
        device_desc: &HomieDeviceDescription,
    ) {
        let to_remove = self.query.match_query(domain, id, device_desc);
        for prop_ref in to_remove {
            self.mat_refs.remove(&prop_ref); // Remove matching PropertyRefs from the HashSet
        }
    }

    pub fn match_query(&self, prop_ref: &PropertyRef) -> bool {
        self.mat_refs.contains(prop_ref)
    }
}

// Implement custom deserialization for MaterializedQuery
impl<'de> Deserialize<'de> for MaterializedQuery {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let query = QueryDefinition::deserialize(deserializer)?;
        Ok(MaterializedQuery::new(query))
    }
}

#[cfg(test)]
mod tests {

    use homie5::device_description::{
        DeviceDescriptionBuilder, IntegerRange, NodeDescriptionBuilder, PropertyDescriptionBuilder,
    };

    use super::*;

    #[test]
    fn test_query_def() {
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
        println!("Matched Property Refs: {:#?}\n\n", refs);
        assert_eq!(refs, cmp_refs);

        // let device_id = "other-device".try_into().unwrap();
        // assert!(!&query.id.as_ref().unwrap().evaluate(&device_id));
    }

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
}
