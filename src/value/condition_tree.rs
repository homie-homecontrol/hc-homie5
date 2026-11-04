//! And/or/not combinations of conditions.
//!
//! [`ConditionTree`] combines leaf conditions of any type `L`. It knows nothing about what a leaf
//! checks or how: each consumer supplies the leaf type (for example a property condition, or a
//! time window) and how one leaf evaluates.
//!
//! In YAML/JSON a tree is written as:
//!
//! ```yaml
//! all:                      # every entry must hold (an empty list holds)
//!   - <leaf>
//!   - any:                  # at least one entry must hold (an empty list does not)
//!       - <leaf>
//!       - not: <leaf>       # the entry must not hold
//! ```
//!
//! A plain list is shorthand for `all`, and anything else is a leaf. A leaf must be written as a
//! map whose keys are not `all`, `any` or `not`.

use std::borrow::Cow;
use std::fmt;
use std::marker::PhantomData;

use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::de::value::{MapAccessDeserializer, StringDeserializer};
use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};

const ALL: &str = "all";
const ANY: &str = "any";
const NOT: &str = "not";

/// An and/or/not combination of leaf conditions of type `L`.
#[derive(Debug, Clone, PartialEq)]
pub enum ConditionTree<L> {
    /// Holds when every entry holds; an empty list holds.
    All(Vec<ConditionTree<L>>),
    /// Holds when at least one entry holds; an empty list does not.
    Any(Vec<ConditionTree<L>>),
    /// Holds when the entry does not hold.
    Not(Box<ConditionTree<L>>),
    /// A single condition, evaluated by the caller.
    Leaf(L),
}

impl<L> ConditionTree<L> {
    /// Evaluates the tree, asking `leaf` for each leaf it needs. Stops as soon as the result is
    /// known, so `leaf` may not be called for every leaf.
    pub fn evaluate(&self, leaf: impl Fn(&L) -> bool) -> bool {
        self.evaluate_with(&leaf)
    }

    fn evaluate_with(&self, leaf: &impl Fn(&L) -> bool) -> bool {
        match self {
            Self::All(entries) => entries.iter().all(|entry| entry.evaluate_with(leaf)),
            Self::Any(entries) => entries.iter().any(|entry| entry.evaluate_with(leaf)),
            Self::Not(entry) => !entry.evaluate_with(leaf),
            Self::Leaf(condition) => leaf(condition),
        }
    }

    /// Every leaf of the tree, depth first.
    pub fn leaves(&self) -> Vec<&L> {
        let mut leaves = Vec::new();
        self.collect_leaves(&mut leaves);
        leaves
    }

    fn collect_leaves<'a>(&'a self, leaves: &mut Vec<&'a L>) {
        match self {
            Self::All(entries) | Self::Any(entries) => entries
                .iter()
                .for_each(|entry| entry.collect_leaves(leaves)),
            Self::Not(entry) => entry.collect_leaves(leaves),
            Self::Leaf(condition) => leaves.push(condition),
        }
    }

    /// The same tree with each leaf converted by `f`.
    pub fn map<M>(self, mut f: impl FnMut(L) -> M) -> ConditionTree<M> {
        self.map_with(&mut f)
    }

    fn map_with<M>(self, f: &mut impl FnMut(L) -> M) -> ConditionTree<M> {
        match self {
            Self::All(entries) => {
                ConditionTree::All(entries.into_iter().map(|e| e.map_with(f)).collect())
            }
            Self::Any(entries) => {
                ConditionTree::Any(entries.into_iter().map(|e| e.map_with(f)).collect())
            }
            Self::Not(entry) => ConditionTree::Not(Box::new(entry.map_with(f))),
            Self::Leaf(condition) => ConditionTree::Leaf(f(condition)),
        }
    }

    /// The same tree with each leaf converted by `f`, or the first error `f` returns.
    pub fn try_map<M, E>(
        self,
        mut f: impl FnMut(L) -> Result<M, E>,
    ) -> Result<ConditionTree<M>, E> {
        self.try_map_with(&mut f)
    }

    fn try_map_with<M, E>(
        self,
        f: &mut impl FnMut(L) -> Result<M, E>,
    ) -> Result<ConditionTree<M>, E> {
        Ok(match self {
            Self::All(entries) => ConditionTree::All(
                entries
                    .into_iter()
                    .map(|e| e.try_map_with(f))
                    .collect::<Result<_, _>>()?,
            ),
            Self::Any(entries) => ConditionTree::Any(
                entries
                    .into_iter()
                    .map(|e| e.try_map_with(f))
                    .collect::<Result<_, _>>()?,
            ),
            Self::Not(entry) => ConditionTree::Not(Box::new(entry.try_map_with(f)?)),
            Self::Leaf(condition) => ConditionTree::Leaf(f(condition)?),
        })
    }
}

impl<L> From<L> for ConditionTree<L> {
    fn from(leaf: L) -> Self {
        Self::Leaf(leaf)
    }
}

impl<L: Serialize> Serialize for ConditionTree<L> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::All(entries) => single_entry(serializer, ALL, entries),
            Self::Any(entries) => single_entry(serializer, ANY, entries),
            Self::Not(entry) => single_entry(serializer, NOT, entry),
            Self::Leaf(condition) => condition.serialize(serializer),
        }
    }
}

fn single_entry<S: Serializer, V: Serialize + ?Sized>(
    serializer: S,
    key: &str,
    value: &V,
) -> Result<S::Ok, S::Error> {
    let mut map = serializer.serialize_map(Some(1))?;
    map.serialize_entry(key, value)?;
    map.end()
}

impl<'de, L: Deserialize<'de>> Deserialize<'de> for ConditionTree<L> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(TreeVisitor(PhantomData))
    }
}

struct TreeVisitor<L>(PhantomData<L>);

impl<'de, L: Deserialize<'de>> Visitor<'de> for TreeVisitor<L> {
    type Value = ConditionTree<L>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a condition, a list of conditions, or a map with `all`, `any` or `not`")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut entries = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(entry) = seq.next_element()? {
            entries.push(entry);
        }
        Ok(ConditionTree::All(entries))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let Some(key) = map.next_key::<String>()? else {
            return L::deserialize(MapAccessDeserializer::new(map)).map(ConditionTree::Leaf);
        };
        let tree = match key.as_str() {
            ALL => ConditionTree::All(map.next_value()?),
            ANY => ConditionTree::Any(map.next_value()?),
            NOT => ConditionTree::Not(map.next_value()?),
            _ => {
                // A leaf: hand it the map again, starting with the key read above.
                let rest = Replay {
                    key: Some(key),
                    map,
                };
                return L::deserialize(MapAccessDeserializer::new(rest)).map(ConditionTree::Leaf);
            }
        };
        if let Some(extra) = map.next_key::<String>()? {
            return Err(de::Error::custom(format!(
                "`{key}` cannot be combined with other fields, found `{extra}`"
            )));
        }
        Ok(tree)
    }
}

/// A map whose first key was already read: yields that key again, then the rest of the map.
struct Replay<A> {
    key: Option<String>,
    map: A,
}

impl<'de, A: MapAccess<'de>> MapAccess<'de> for Replay<A> {
    type Error = A::Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error> {
        match self.key.take() {
            Some(key) => seed
                .deserialize(StringDeserializer::<A::Error>::new(key))
                .map(Some),
            None => self.map.next_key_seed(seed),
        }
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, Self::Error> {
        self.map.next_value_seed(seed)
    }

    fn size_hint(&self) -> Option<usize> {
        self.map
            .size_hint()
            .map(|rest| rest + usize::from(self.key.is_some()))
    }
}

impl<L: JsonSchema> JsonSchema for ConditionTree<L> {
    fn schema_name() -> Cow<'static, str> {
        format!("ConditionTree_{}", L::schema_name()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let leaf = generator.subschema_for::<L>();
        let tree = generator.subschema_for::<Self>();
        let list = json_schema!({ "type": "array", "items": tree });
        let keyed = |key: &str, value: &Schema| {
            json_schema!({
                "type": "object",
                "properties": { key: value },
                "required": [key],
                "additionalProperties": false,
            })
        };
        json_schema!({
            "anyOf": [
                keyed(ALL, &list),
                keyed(ANY, &list),
                keyed(NOT, &tree),
                list,
                leaf,
            ]
        })
    }
}
