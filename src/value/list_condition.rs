//! Conditions over a list of values.
//!
//! A [`ListCondition`] states how the elements of a list relate to a scalar [`ValueCondition`]:
//!
//! ```yaml
//! any: smart-thermostat                       # at least one element matches
//! any: { operator: "=", value: [a, b] }       # at least one element is a or b
//! all: { pattern: "^org\\.homie\\." }          # the list is non-empty and every element matches
//! none: legacy                                # no element matches
//! first: bathroom                             # the first element matches
//! empty: true                                 # the list is empty (false: not empty)
//! ```
//!
//! Each form is a mapping with exactly one of these keys. A bare [`ValueCondition`] is not
//! accepted: the quantifier is always explicit.
//!
//! Empty lists: `any`, `all` and `first` do not match; `none` matches.
//! [`ListCondition::evaluate_option`] treats a missing list as an empty one.
//!
//! Changed in 0.19.1: `all` used to hold on an empty list (vacuous truth). It is now false, so
//! "every room is Büro" does not hold for a device without rooms.

use std::fmt;
use std::marker::PhantomData;

use schemars::JsonSchema;
use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};

use super::{ValueCondition, ValueMatcher};

const ANY: &str = "any";
const ALL: &str = "all";
const NONE: &str = "none";
const FIRST: &str = "first";
const EMPTY: &str = "empty";
const KEYS: &[&str] = &[ANY, ALL, NONE, FIRST, EMPTY];

// (De)serialization is hand-written as a single-key mapping: serde's externally tagged enum
// form maps to YAML tags (`!any`) in serde_yaml_ng. The schema derive below documents the same
// shape (one object per variant with exactly one required key).
#[derive(Debug, Clone, JsonSchema)]
#[schemars(rename_all = "lowercase", deny_unknown_fields, bound = "T: JsonSchema")]
pub enum ListCondition<T>
where
    T: ValueMatcher + PartialEq + PartialOrd + std::fmt::Debug,
{
    /// At least one element matches.
    Any(ValueCondition<T>),
    /// The list is non-empty and every element matches; never holds for an empty list
    /// (until 0.19.0 it did).
    All(ValueCondition<T>),
    /// No element matches; holds for an empty or missing list.
    None(ValueCondition<T>),
    /// The first element matches; never holds for an empty list.
    First(ValueCondition<T>),
    /// `true`: the list is empty; `false`: the list has at least one element.
    Empty(bool),
}

impl<T> ListCondition<T>
where
    T: ValueMatcher + PartialEq + PartialOrd + std::fmt::Debug,
{
    /// Evaluates the condition against the list `values`.
    pub fn evaluate(&self, values: &[T]) -> bool {
        match self {
            ListCondition::Any(cond) => values.iter().any(|value| cond.evaluate(value)),
            ListCondition::All(cond) => {
                !values.is_empty() && values.iter().all(|value| cond.evaluate(value))
            }
            ListCondition::None(cond) => !values.iter().any(|value| cond.evaluate(value)),
            ListCondition::First(cond) => values.first().is_some_and(|value| cond.evaluate(value)),
            ListCondition::Empty(empty) => values.is_empty() == *empty,
        }
    }

    /// Evaluates the condition against an optional list; a missing list counts as empty.
    pub fn evaluate_option(&self, values: Option<&[T]>) -> bool {
        self.evaluate(values.unwrap_or(&[]))
    }
}

impl<T> Serialize for ListCondition<T>
where
    T: ValueMatcher + PartialEq + PartialOrd + std::fmt::Debug + Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            ListCondition::Any(cond) => map.serialize_entry(ANY, cond)?,
            ListCondition::All(cond) => map.serialize_entry(ALL, cond)?,
            ListCondition::None(cond) => map.serialize_entry(NONE, cond)?,
            ListCondition::First(cond) => map.serialize_entry(FIRST, cond)?,
            ListCondition::Empty(empty) => map.serialize_entry(EMPTY, empty)?,
        }
        map.end()
    }
}

impl<'de, T> Deserialize<'de> for ListCondition<T>
where
    T: ValueMatcher + PartialEq + PartialOrd + std::fmt::Debug + Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(ListConditionVisitor(PhantomData))
    }
}

struct ListConditionVisitor<T>(PhantomData<T>);

impl<'de, T> Visitor<'de> for ListConditionVisitor<T>
where
    T: ValueMatcher + PartialEq + PartialOrd + std::fmt::Debug + Deserialize<'de>,
{
    type Value = ListCondition<T>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a list condition: a mapping with exactly one of `any`, `all`, `none`, `first` or `empty`")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let Some(key) = map.next_key::<String>()? else {
            return Err(de::Error::custom(
                "empty list condition: expected one of `any`, `all`, `none`, `first` or `empty`",
            ));
        };
        let cond = match key.as_str() {
            ANY => ListCondition::Any(map.next_value()?),
            ALL => ListCondition::All(map.next_value()?),
            NONE => ListCondition::None(map.next_value()?),
            FIRST => ListCondition::First(map.next_value()?),
            EMPTY => ListCondition::Empty(map.next_value()?),
            bare @ ("operator" | "value" | "pattern") => {
                return Err(de::Error::custom(format!(
                    "`{bare}` is a bare value condition; a list condition wraps it in one of \
                     `any`, `all`, `none` or `first` (e.g. `any: {{ operator: ..., value: ... }}`)"
                )))
            }
            other => return Err(de::Error::unknown_field(other, KEYS)),
        };
        if let Some(extra) = map.next_key::<String>()? {
            return Err(de::Error::custom(format!(
                "a list condition has exactly one key, found `{key}` and `{extra}`"
            )));
        }
        Ok(cond)
    }
}
