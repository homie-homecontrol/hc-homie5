//! Declarative device/node/property queries.
//!
//! A [`QueryDefinition`] combines optional `domain`, `device`, `node`, `property` and `meta`
//! sections; every present section must match. Scalar fields take a
//! [`ValueCondition`]; list fields (`device.children`, `device.extensions`, and each `$meta`
//! annotation in `meta.annotations`) take a [`ListCondition`] with an explicit quantifier.
//!
//! ```yaml
//! device:
//!   extensions: { any: { pattern: "^org\\.homie\\.meta:" } }
//!   children: { empty: true }
//! node:
//!   type: thermostat
//! meta:
//!   annotations:
//!     roles: { any: smart-thermostat }
//! ```

use crate::value::{ListCondition, ValueCondition};
use homie5::{
    device_description::{
        HomieDeviceDescription, HomieNodeDescription, HomiePropertyDescription, HomiePropertyFormat,
    },
    HomieDataType, HomieDomain, HomieID, PropertyRef,
};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer};
use std::collections::{BTreeMap, HashSet};

#[derive(Default, Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
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
        self.id.as_ref().is_none_or(|cond| cond.evaluate(id))
            && self
                .name
                .as_ref()
                .is_none_or(|cond| cond.evaluate_option(property_desc.name.as_ref()))
            && self
                .datatype
                .as_ref()
                .is_none_or(|cond| cond.evaluate(&property_desc.datatype))
            && self.format.as_ref().is_none_or(|cond| {
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
                .is_none_or(|cond| cond.evaluate(&property_desc.settable))
            && self
                .retained
                .as_ref()
                .is_none_or(|cond| cond.evaluate(&property_desc.retained))
            && self
                .unit
                .as_ref()
                .is_none_or(|cond| cond.evaluate_option(property_desc.unit.as_ref()))
    }
}

#[derive(Default, Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NodeQuery {
    pub id: Option<ValueCondition<HomieID>>,
    pub name: Option<ValueCondition<String>>,
    pub r#type: Option<ValueCondition<String>>,
}

impl NodeQuery {
    pub fn match_query(&self, id: &HomieID, node_desc: &HomieNodeDescription) -> bool {
        self.id.as_ref().is_none_or(|cond| cond.evaluate(id))
            && self
                .name
                .as_ref()
                .is_none_or(|cond| cond.evaluate_option(node_desc.name.as_ref()))
            && self
                .r#type
                .as_ref()
                .is_none_or(|cond| cond.evaluate_option(node_desc.r#type.as_ref()))
    }
}

#[derive(Default, Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeviceQuery {
    pub id: Option<ValueCondition<HomieID>>,
    pub name: Option<ValueCondition<String>>,
    pub version: Option<ValueCondition<i64>>,
    pub homie: Option<ValueCondition<String>>,
    /// Child device IDs, matched as a list (`any:`, `all:`, `none:`, `first:`, `empty:`).
    pub children: Option<ListCondition<HomieID>>,
    pub root: Option<ValueCondition<HomieID>>,
    pub parent: Option<ValueCondition<HomieID>>,
    /// Extension identifiers, matched as a list (`any:`, `all:`, `none:`, `first:`, `empty:`).
    pub extensions: Option<ListCondition<String>>,
}

impl DeviceQuery {
    pub fn match_query(&self, id: &HomieID, device_desc: &HomieDeviceDescription) -> bool {
        // Check each condition in sequence and short-circuit if any condition evaluates to `false`
        self.id.as_ref().is_none_or(|cond| cond.evaluate(id))
            && self
                .name
                .as_ref()
                .is_none_or(|cond| cond.evaluate_option(device_desc.name.as_ref()))
            && self
                .root
                .as_ref()
                .is_none_or(|cond| cond.evaluate_option(device_desc.root.as_ref()))
            && self
                .homie
                .as_ref()
                .is_none_or(|cond| cond.evaluate(&device_desc.homie))
            && self
                .parent
                .as_ref()
                .is_none_or(|cond| cond.evaluate_option(device_desc.parent.as_ref()))
            && self
                .version
                .as_ref()
                .is_none_or(|cond| cond.evaluate(&device_desc.version))
            && self
                .children
                .as_ref()
                .is_none_or(|cond| cond.evaluate(&device_desc.children))
            && self
                .extensions
                .as_ref()
                .is_none_or(|cond| cond.evaluate(&device_desc.extensions))
    }
}

/// Device-level `$meta` annotations a [`QueryDefinition`] can match against.
///
/// With the `ext-meta` feature it is implemented for
/// [`MetaDeviceOverlay`](homie5::extensions::meta::MetaDeviceOverlay) (pass the merged overlay,
/// see `Device::merged_meta_overlay`) and for plain annotation maps
/// ([`MetaEntries`](homie5::extensions::meta::MetaEntries)).
pub trait MetaAnnotations {
    /// Values of the device annotation `key`, or `None` when it is absent.
    /// A scalar annotation is a one-element slice.
    fn device_annotation(&self, key: &str) -> Option<&[String]>;
}

#[cfg(feature = "ext-meta")]
impl MetaAnnotations for homie5::extensions::meta::MetaEntries {
    fn device_annotation(&self, key: &str) -> Option<&[String]> {
        use homie5::extensions::meta::MetaValue;
        match self.get(key)? {
            MetaValue::Text(value) => Some(std::slice::from_ref(value)),
            MetaValue::List(values) => Some(values.as_slice()),
        }
    }
}

#[cfg(feature = "ext-meta")]
impl MetaAnnotations for homie5::extensions::meta::MetaDeviceOverlay {
    fn device_annotation(&self, key: &str) -> Option<&[String]> {
        self.device
            .as_ref()?
            .annotations
            .as_ref()?
            .device_annotation(key)
    }
}

/// Selects devices by their device-level `$meta` annotations; every listed annotation must match.
///
/// Each annotation is matched as a list of strings with a [`ListCondition`]: a scalar annotation
/// is a one-element list, a missing annotation an empty one. Annotation values are always strings,
/// so compare with `"true"`, not `true`.
///
/// ```yaml
/// meta:
///   annotations:
///     roles: { any: smart-thermostat }
///     room: { first: { operator: "=", value: [bathroom, hall] } }
///     heating: { any: "true" }
/// ```
#[derive(Default, Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MetaQuery {
    #[serde(default)]
    pub annotations: BTreeMap<String, ListCondition<String>>,
}

impl MetaQuery {
    pub fn match_query(&self, meta: Option<&dyn MetaAnnotations>) -> bool {
        self.annotations.iter().all(|(key, cond)| {
            cond.evaluate_option(meta.and_then(|meta| meta.device_annotation(key)))
        })
    }
}

#[derive(Default, Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryDefinition {
    #[serde(default)]
    pub domain: Option<ValueCondition<HomieDomain>>,
    #[serde(default)]
    pub device: Option<DeviceQuery>,
    #[serde(default)]
    pub node: Option<NodeQuery>,
    #[serde(default)]
    pub property: Option<PropertyQuery>,
    /// Device-level `$meta` annotation conditions, matched against the overlay passed to
    /// [`QueryDefinition::match_query`].
    #[serde(default)]
    pub meta: Option<MetaQuery>,
}

impl QueryDefinition {
    /// Returns every property of the device that matches the query.
    ///
    /// `meta` holds the device's `$meta` annotations; with `None` a query with `meta` conditions
    /// sees every annotation as missing.
    pub fn match_query(
        &self,
        domain: &HomieDomain,
        id: &HomieID,
        device_desc: &HomieDeviceDescription,
        meta: Option<&dyn MetaAnnotations>,
    ) -> Vec<PropertyRef> {
        let mut matched_properties = Vec::new();

        // Check if the device matches the domain and device-level queries
        if self.domain.as_ref().is_none_or(|cond| {
            if let Some(v) = cond.value() {
                if matches!(v, HomieDomain::All) {
                    return true;
                }
            }
            cond.evaluate(domain)
        }) && self
            .device
            .as_ref()
            .is_none_or(|device_query| device_query.match_query(id, device_desc))
            && self
                .meta
                .as_ref()
                .is_none_or(|meta_query| meta_query.match_query(meta))
        {
            // Iterate through all nodes and their properties
            for (node_id, node_desc) in &device_desc.nodes {
                // Check if the node matches the node-level query
                if self
                    .node
                    .as_ref()
                    .is_none_or(|node_query| node_query.match_query(node_id, node_desc))
                {
                    for (prop_id, prop_desc) in &node_desc.properties {
                        // Check if the property matches the property-level query
                        if self.property.as_ref().is_none_or(|property_query| {
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

    /// Re-evaluates the device and replaces its references; call it again when the device's
    /// description or `$meta` overlay changes.
    pub fn add_materialized(
        &mut self,
        domain: &HomieDomain,
        id: &HomieID,
        device_desc: &HomieDeviceDescription,
        meta: Option<&dyn MetaAnnotations>,
    ) {
        // Remove all refs belonging to the given device ID
        self.mat_refs.retain(|prop_ref| prop_ref.device_id() != id);

        let new_refs = self.query.match_query(domain, id, device_desc, meta);
        self.mat_refs.extend(new_refs); // Add new PropertyRefs to the HashSet
    }

    /// Removes every reference of the device. `_device_desc` is unused and kept for compatibility:
    /// removal no longer re-evaluates the query, so it does not depend on the overlay or
    /// description that produced the match.
    pub fn remove_materialized(
        &mut self,
        domain: &HomieDomain,
        id: &HomieID,
        _device_desc: &HomieDeviceDescription,
    ) {
        self.mat_refs
            .retain(|prop_ref| !(prop_ref.homie_domain() == domain && prop_ref.device_id() == id));
    }

    /// The underlying query definition.
    pub fn query(&self) -> &QueryDefinition {
        &self.query
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
