use homie5::HomieValue;
use serde::{de, Deserialize, Deserializer, Serialize};
use std::str::FromStr;

pub trait AsMatchStr {
    /// Returns a string slice representation used for matching.
    fn as_match_str(&self) -> &str;
}

/// A trait that defines value matching behavior.
pub trait ValueMatcher: AsMatchStr
where
    Self: std::fmt::Debug + Sized,
{
    /// Checks if the value matches the given regular expression.
    fn matches_regex(&self, pattern: &str) -> bool;

    fn matches(&self, operator: ConditionOperator, operand: Option<&ValueSet<Self>>) -> bool;

    fn matches_literal(&self, other: &Self) -> bool;
}

// --- Condition Operators, extended with pattern matching variants ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ConditionOperator {
    Equal,
    Greater,
    Less,
    GreaterOrEqual,
    LessOrEqual,
    NotEqual,
    IncludesAny,
    IncludesNone,
    MatchAlways,
    IsEmpty,
    Exists,
}

impl FromStr for ConditionOperator {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "=" => Ok(ConditionOperator::Equal),
            ">" => Ok(ConditionOperator::Greater),
            "<" => Ok(ConditionOperator::Less),
            ">=" => Ok(ConditionOperator::GreaterOrEqual),
            "<=" => Ok(ConditionOperator::LessOrEqual),
            "<>" => Ok(ConditionOperator::NotEqual),
            "includesAny" => Ok(ConditionOperator::IncludesAny),
            "includesNone" => Ok(ConditionOperator::IncludesNone),
            "matchAlways" => Ok(ConditionOperator::MatchAlways),
            "isEmpty" => Ok(ConditionOperator::IsEmpty),
            "exists" => Ok(ConditionOperator::Exists),
            _ => Err(()),
        }
    }
}

impl<'de> Deserialize<'de> for ConditionOperator {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        ConditionOperator::from_str(s)
            .map_err(|_| de::Error::custom(format!("Invalid ConditionOperator: {}", s)))
    }
}

// --- Generic ValueSet and ValueCondition types ---

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub enum ValueSet<T>
where
    T: ValueMatcher + std::fmt::Debug,
{
    Single(T),
    Multiple(Vec<T>),
}

impl<T> ValueSet<T>
where
    T: ValueMatcher + std::fmt::Debug,
{
    pub fn value(&self) -> Option<&T> {
        match self {
            ValueSet::Single(value) => Some(value),
            ValueSet::Multiple(_) => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum ValueCondition<T>
where
    T: ValueMatcher + PartialEq + PartialOrd + std::fmt::Debug,
{
    Value(T),
    Operator(ValueOperatorCondition<T>),
    Pattern(Pattern),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Pattern {
    pub pattern: String,
}

impl<T> ValueCondition<T>
where
    T: ValueMatcher + PartialEq + PartialOrd + std::fmt::Debug,
{
    pub fn evaluate(&self, value: &T) -> bool {
        match self {
            ValueCondition::Value(literal) => value.matches_literal(literal),
            ValueCondition::Operator(op_condition) => op_condition.evaluate(value),
            ValueCondition::Pattern(pattern) => value.matches_regex(&pattern.pattern),
        }
    }

    pub fn evaluate_option(&self, value: Option<&T>) -> bool {
        match self {
            ValueCondition::Value(literal) => {
                value.map(|v| v.matches_literal(literal)).unwrap_or(false)
            }
            ValueCondition::Operator(op_condition) => op_condition.evaluate_option(value),
            ValueCondition::Pattern(pattern) => value
                .map(|v| v.matches_regex(&pattern.pattern))
                .unwrap_or(false),
        }
    }

    pub fn value(&self) -> Option<&T> {
        match self {
            ValueCondition::Value(literal) => Some(literal),
            ValueCondition::Operator(op_condition) => op_condition.value.as_ref()?.value(),
            ValueCondition::Pattern(_pattern) => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ValueOperatorCondition<T>
where
    T: ValueMatcher + std::fmt::Debug,
{
    pub operator: ConditionOperator,
    #[serde(default = "default_value")]
    pub value: Option<ValueSet<T>>,
}

fn default_value<T>() -> Option<ValueSet<T>>
where
    T: ValueMatcher + std::fmt::Debug,
{
    None
}

impl<T> ValueOperatorCondition<T>
where
    T: ValueMatcher + std::fmt::Debug,
{
    /// Evaluates using standard (non-pattern) operators.
    pub fn evaluate(&self, check_value: &T) -> bool {
        check_value.matches(self.operator, self.value.as_ref())
    }

    pub fn evaluate_option(&self, check_value: Option<&T>) -> bool {
        match self.operator {
            ConditionOperator::IsEmpty => check_value.is_none(),
            ConditionOperator::Exists => check_value.is_some(),
            ConditionOperator::MatchAlways => true,
            _ => match check_value {
                Some(val) => self.evaluate(val),
                None => false,
            },
        }
    }
}

// The helper macro contains the common implementation logic.
#[doc(hidden)]
#[macro_export]
macro_rules! __impl_value_matcher_for_helper {
    ($t:ty) => {
        impl $crate::ValueMatcher for $t {
            fn matches_regex(&self, pattern: &str) -> bool {
                regex::Regex::new(pattern)
                    .map(|re| re.is_match(self.as_match_str()))
                    .unwrap_or(false)
            }

            fn matches(
                &self,
                operator: $crate::ConditionOperator,
                operand: Option<&$crate::ValueSet<Self>>,
            ) -> bool {
                match operator {
                    $crate::ConditionOperator::Equal => match operand {
                        Some($crate::ValueSet::Single(ref v)) => self == v,
                        Some($crate::ValueSet::Multiple(ref values)) => values.contains(self),
                        _ => false,
                    },
                    $crate::ConditionOperator::Greater => match operand {
                        Some($crate::ValueSet::Single(ref v)) => self > v,
                        _ => false,
                    },
                    $crate::ConditionOperator::Less => match operand {
                        Some($crate::ValueSet::Single(ref v)) => self < v,
                        _ => false,
                    },
                    $crate::ConditionOperator::GreaterOrEqual => match operand {
                        Some($crate::ValueSet::Single(ref v)) => self >= v,
                        _ => false,
                    },
                    $crate::ConditionOperator::LessOrEqual => match operand {
                        Some($crate::ValueSet::Single(ref v)) => self <= v,
                        _ => false,
                    },
                    $crate::ConditionOperator::NotEqual => match operand {
                        Some($crate::ValueSet::Single(ref v)) => self != v,
                        Some($crate::ValueSet::Multiple(ref values)) => !values.contains(self),
                        _ => false,
                    },
                    $crate::ConditionOperator::IncludesAny => match operand {
                        Some($crate::ValueSet::Single(ref v)) => self == v,
                        Some($crate::ValueSet::Multiple(ref values)) => values.contains(self),
                        _ => false,
                    },
                    $crate::ConditionOperator::IncludesNone => match operand {
                        Some($crate::ValueSet::Single(ref v)) => self != v,
                        Some($crate::ValueSet::Multiple(ref values)) => !values.contains(self),
                        _ => false,
                    },
                    $crate::ConditionOperator::MatchAlways => true,
                    $crate::ConditionOperator::IsEmpty => false,
                    $crate::ConditionOperator::Exists => true,
                }
            }

            fn matches_literal(&self, other: &Self) -> bool {
                self == other
            }
        }
    };
}

#[macro_export]
macro_rules! impl_value_matcher_for {
    // Case when the flag is provided and is true.
    ($t:ty, true) => {
        impl $crate::AsMatchStr for $t {
            fn as_match_str(&self) -> &str {
                // Default implementation (you might choose to call self.as_ref() if available)
                self.as_ref()
            }
        }
        $crate::__impl_value_matcher_for_helper!($t);
    };
    // Case when the flag is provided and is false.
    ($t:ty, false) => {
        impl $crate::AsMatchStr for $t {
            fn as_match_str(&self) -> &str {
                ""
            }
        }
        $crate::__impl_value_matcher_for_helper!($t);
    };
    // Case when no flag is provided: default to false.
    ($t:ty) => {
        $crate::__impl_value_matcher_for_helper!($t);
    };
}

// The helper macro contains the common implementation logic.
#[macro_export]
macro_rules! impl_value_matcher_for_vec {
    ($t:ty) => {
        impl $crate::AsMatchStr for Vec<$t> {
            fn as_match_str(&self) -> &str {
                ""
            }
        }
        impl $crate::ValueMatcher for Vec<$t> {
            fn matches_regex(&self, _: &str) -> bool {
                false
            }

            fn matches(
                &self,
                operator: $crate::ConditionOperator,
                operand: Option<&$crate::ValueSet<Self>>,
            ) -> bool {
                match operator {
                    $crate::ConditionOperator::Equal => match operand {
                        Some($crate::ValueSet::Single(value)) => {
                            value.len() == self.len() && value.iter().all(|v| self.contains(v))
                        }
                        Some($crate::ValueSet::Multiple(values)) => values.iter().any(|va| {
                            va.len() == self.len() && va.iter().all(|v| self.contains(v))
                        }),
                        _ => false,
                    },
                    $crate::ConditionOperator::NotEqual => match operand {
                        Some($crate::ValueSet::Single(value)) => {
                            value.len() != self.len() || value.iter().any(|v| !self.contains(v))
                        }
                        Some($crate::ValueSet::Multiple(values)) => {
                            // Return true if no matching vector is found in `values`
                            values.iter().all(|va| {
                                va.len() != self.len() || va.iter().any(|v| !self.contains(v))
                            })
                        }
                        _ => true, // If no value is specified, treat as "not equal"
                    },
                    $crate::ConditionOperator::IncludesAny => match operand {
                        Some($crate::ValueSet::Single(value)) => {
                            value.iter().any(|v| self.contains(v))
                        }
                        Some($crate::ValueSet::Multiple(values)) => {
                            values.iter().any(|va| va.iter().any(|v| self.contains(v)))
                        }
                        _ => false,
                    },
                    $crate::ConditionOperator::IncludesNone => match operand {
                        Some($crate::ValueSet::Single(value)) => {
                            value.iter().all(|v| !self.contains(v))
                        }
                        Some($crate::ValueSet::Multiple(values)) => {
                            values.iter().all(|va| va.iter().all(|v| !self.contains(v)))
                        }
                        _ => false,
                    },
                    $crate::ConditionOperator::MatchAlways => true,
                    _ => false,
                }
            }

            fn matches_literal(&self, other: &Self) -> bool {
                self == other
            }
        }
    };
}

impl_value_matcher_for!(String, true);
impl_value_matcher_for!(&str, true);

impl AsMatchStr for HomieValue {
    fn as_match_str(&self) -> &str {
        match self {
            HomieValue::String(ref s) => s.as_str(),
            HomieValue::Enum(ref s) => s.as_str(),
            _ => "",
        }
    }
}
impl_value_matcher_for!(HomieValue);

// impl AsMatchStr for Vec<String> {
//     fn as_match_str(&self) -> &str {
//         ""
//     }
// }
//
// impl ValueMatcher for Vec<String> {
//     fn matches_regex(&self, pattern: &str) -> bool {
//         false
//     }
//
//     fn matches(&self, operator: ConditionOperator, operand: Option<&ValueSet<Self>>) -> bool {
//         match operator {
//             ConditionOperator::Equal => match operand {
//                 Some(ValueSet::Single(value)) => {
//                     value.len() == self.len() && value.iter().all(|v| self.contains(v))
//                 }
//                 Some(ValueSet::Multiple(values)) => values
//                     .iter()
//                     .any(|va| va.len() == self.len() && va.iter().all(|v| self.contains(v))),
//                 _ => false,
//             },
//             ConditionOperator::NotEqual => match operand {
//                 Some(ValueSet::Single(value)) => {
//                     value.len() != self.len() || value.iter().any(|v| !self.contains(v))
//                 }
//                 Some(ValueSet::Multiple(values)) => {
//                     // Return true if no matching vector is found in `values`
//                     values
//                         .iter()
//                         .all(|va| va.len() != self.len() || va.iter().any(|v| !self.contains(v)))
//                 }
//                 _ => true, // If no value is specified, treat as "not equal"
//             },
//             ConditionOperator::IncludesAny => match operand {
//                 Some(ValueSet::Single(value)) => value.iter().any(|v| self.contains(v)),
//                 Some(ValueSet::Multiple(values)) => {
//                     values.iter().any(|va| va.iter().any(|v| self.contains(v)))
//                 }
//                 _ => false,
//             },
//             ConditionOperator::IncludesNone => match operand {
//                 Some(ValueSet::Single(value)) => value.iter().all(|v| !self.contains(v)),
//                 Some(ValueSet::Multiple(values)) => {
//                     values.iter().all(|va| va.iter().all(|v| !self.contains(v)))
//                 }
//                 _ => false,
//             },
//             ConditionOperator::MatchAlways => true,
//             _ => false,
//         }
//     }
//
//     fn matches_literal(&self, other: &Self) -> bool {
//         self == other
//     }
// }

// //
// // This method is available when T implements ValueMatcher.
// impl<T> ValueOperatorCondition<T>
// where
//     T: ValueMatcher + PartialEq + PartialOrd + std::fmt::Debug,
// {
//     pub fn evaluate_with_pattern(&self, check_value: &T) -> bool {
//         match self.operator {
//             ConditionOperator::Wildcard => {
//                 if let Some(ValueSet::Single(ref pattern_val)) = self.value {
//                     check_value.matches_wildcard(pattern_val.as_match_str())
//                 } else {
//                     false
//                 }
//             }
//             ConditionOperator::Regex => {
//                 if let Some(ValueSet::Single(ref pattern_val)) = self.value {
//                     check_value.matches_regex(pattern_val.as_match_str())
//                 } else {
//                     false
//                 }
//             }
//             _ => self.evaluate(check_value),
//         }
//     }
// }

// ================================================================================================
// ================================================================================================
//
//

// // ValueOperatorCondition
// #[derive(Debug, Clone, Deserialize, Serialize)]
// pub struct ValueOperatorConditionVec<T>
// where
//     T: PartialEq + PartialOrd + std::fmt::Debug,
// {
//     pub operator: ConditionOperator,
//     #[serde(default = "default_value_vec")]
//     pub value: Option<ValueSetVec<T>>,
// }
//
// fn default_value_vec<T>() -> Option<ValueSetVec<T>>
// where
//     T: PartialEq + PartialOrd + std::fmt::Debug,
// {
//     None
// }
//
// impl<T> ValueOperatorConditionVec<T>
// where
//     T: PartialEq + PartialOrd + std::fmt::Debug,
// {
//     pub fn evaluate(&self, check_value: &[T]) -> bool {
//         match self.operator {
//             ConditionOperator::Equal => match &self.value {
//                 Some(ValueSetVec::Single(value)) => {
//                     value.len() == check_value.len()
//                         && value.iter().all(|v| check_value.contains(v))
//                 }
//                 Some(ValueSetVec::Multiple(values)) => values.iter().any(|va| {
//                     va.len() == check_value.len() && va.iter().all(|v| check_value.contains(v))
//                 }),
//                 _ => false,
//             },
//             ConditionOperator::NotEqual => match &self.value {
//                 Some(ValueSetVec::Single(value)) => {
//                     value.len() != check_value.len()
//                         || value.iter().any(|v| !check_value.contains(v))
//                 }
//                 Some(ValueSetVec::Multiple(values)) => {
//                     // Return true if no matching vector is found in `values`
//                     values.iter().all(|va| {
//                         va.len() != check_value.len() || va.iter().any(|v| !check_value.contains(v))
//                     })
//                 }
//                 _ => true, // If no value is specified, treat as "not equal"
//             },
//             ConditionOperator::IncludesAny => match &self.value {
//                 Some(ValueSetVec::Single(value)) => value.iter().any(|v| check_value.contains(v)),
//                 Some(ValueSetVec::Multiple(values)) => values
//                     .iter()
//                     .any(|va| va.iter().any(|v| check_value.contains(v))),
//                 _ => false,
//             },
//             ConditionOperator::IncludesNone => match &self.value {
//                 Some(ValueSetVec::Single(value)) => value.iter().all(|v| !check_value.contains(v)),
//                 Some(ValueSetVec::Multiple(values)) => values
//                     .iter()
//                     .all(|va| va.iter().all(|v| !check_value.contains(v))),
//                 _ => false,
//             },
//             ConditionOperator::MatchAlways => true,
//             _ => false,
//         }
//     }
// }
//
// // ValueCondition
// #[derive(Debug, Clone, Deserialize, Serialize)]
// #[serde(untagged)]
// pub enum ValueSetVec<T>
// where
//     T: PartialEq + PartialOrd + std::fmt::Debug,
// {
//     Single(Vec<T>),
//     Multiple(Vec<Vec<T>>),
// }
//
// #[derive(Debug, Clone, Deserialize, Serialize)]
// #[serde(untagged)]
// pub enum ValueConditionVec<T>
// where
//     T: PartialEq + PartialOrd + std::fmt::Debug,
// {
//     Value(Vec<T>),
//     Operator(ValueOperatorConditionVec<T>),
// }
//
// impl<T> ValueConditionVec<T>
// where
//     T: PartialEq + PartialOrd + std::fmt::Debug,
// {
//     pub fn evaluate(&self, value: &Vec<T>) -> bool {
//         match self {
//             ValueConditionVec::Value(homie_value) => value == homie_value,
//             ValueConditionVec::Operator(operator_condition) => operator_condition.evaluate(value),
//         }
//     }
// }
