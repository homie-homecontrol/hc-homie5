use super::{ConditionOperator, ValueSet};

/// A trait that defines value matching behavior.
pub trait ValueMatcher
where
    Self: PartialEq + PartialOrd + std::fmt::Debug + Sized,
{
    /// Returns a string slice representation used for matching.
    fn as_match_str(&self) -> &str;

    /// Checks if the value matches the given regular expression.
    fn matches_regex(&self, pattern: &str) -> bool {
        regex::Regex::new(pattern)
            .map(|re| re.is_match(self.as_match_str()))
            .unwrap_or(false)
    }

    fn matches(&self, operator: ConditionOperator, operand: Option<&ValueSet<Self>>) -> bool {
        match operator {
            ConditionOperator::Equal => match operand {
                Some(ValueSet::Single(ref v)) => self == v,
                Some(ValueSet::Multiple(ref values)) => values.contains(self),
                _ => false,
            },
            ConditionOperator::Greater => match operand {
                Some(ValueSet::Single(ref v)) => self > v,
                _ => false,
            },
            ConditionOperator::Less => match operand {
                Some(ValueSet::Single(ref v)) => self < v,
                _ => false,
            },
            ConditionOperator::GreaterOrEqual => match operand {
                Some(ValueSet::Single(ref v)) => self >= v,
                _ => false,
            },
            ConditionOperator::LessOrEqual => match operand {
                Some(ValueSet::Single(ref v)) => self <= v,
                _ => false,
            },
            ConditionOperator::NotEqual => match operand {
                Some(ValueSet::Single(ref v)) => self != v,
                Some(ValueSet::Multiple(ref values)) => !values.contains(self),
                _ => false,
            },
            ConditionOperator::IncludesAny => match operand {
                Some(ValueSet::Single(ref v)) => self == v,
                Some(ValueSet::Multiple(ref values)) => values.contains(self),
                _ => false,
            },
            ConditionOperator::IncludesNone => match operand {
                Some(ValueSet::Single(ref v)) => self != v,
                Some(ValueSet::Multiple(ref values)) => !values.contains(self),
                _ => false,
            },
            ConditionOperator::MatchAlways => true,
            ConditionOperator::IsEmpty => false,
            ConditionOperator::Exists => true,
        }
    }

    fn matches_literal(&self, other: &Self) -> bool {
        self == other
    }
}

// The helper macro to implement a generic value matcher for Vecs of types with PartialEq + PartialOrd
#[macro_export]
macro_rules! impl_value_matcher_for_vec {
    ($t:ty) => {
        impl $crate::value::ValueMatcher for Vec<$t> {
            fn as_match_str(&self) -> &str {
                ""
            }
            fn matches_regex(&self, _: &str) -> bool {
                false
            }

            fn matches(
                &self,
                operator: $crate::value::ConditionOperator,
                operand: Option<&$crate::value::ValueSet<Self>>,
            ) -> bool {
                match operator {
                    $crate::value::ConditionOperator::Equal => match operand {
                        Some($crate::value::ValueSet::Single(value)) => {
                            value.len() == self.len() && value.iter().all(|v| self.contains(v))
                        }
                        Some($crate::value::ValueSet::Multiple(values)) => values.iter().any(|va| {
                            va.len() == self.len() && va.iter().all(|v| self.contains(v))
                        }),
                        _ => false,
                    },
                    $crate::value::ConditionOperator::NotEqual => match operand {
                        Some($crate::value::ValueSet::Single(value)) => {
                            value.len() != self.len() || value.iter().any(|v| !self.contains(v))
                        }
                        Some($crate::value::ValueSet::Multiple(values)) => {
                            // Return true if no matching vector is found in `values`
                            values.iter().all(|va| {
                                va.len() != self.len() || va.iter().any(|v| !self.contains(v))
                            })
                        }
                        _ => true, // If no value is specified, treat as "not equal"
                    },
                    $crate::value::ConditionOperator::IncludesAny => match operand {
                        Some($crate::value::ValueSet::Single(value)) => {
                            value.iter().any(|v| self.contains(v))
                        }
                        Some($crate::value::ValueSet::Multiple(values)) => {
                            values.iter().any(|va| va.iter().any(|v| self.contains(v)))
                        }
                        _ => false,
                    },
                    $crate::value::ConditionOperator::IncludesNone => match operand {
                        Some($crate::value::ValueSet::Single(value)) => {
                            value.iter().all(|v| !self.contains(v))
                        }
                        Some($crate::value::ValueSet::Multiple(values)) => {
                            values.iter().all(|va| va.iter().all(|v| !self.contains(v)))
                        }
                        _ => false,
                    },
                    $crate::value::ConditionOperator::MatchAlways => true,
                    _ => false,
                }
            }
        }
    };
}
