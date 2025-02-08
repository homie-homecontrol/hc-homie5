use serde::{de, Serialize};
use serde::{Deserialize, Deserializer};
use std::str::FromStr;

// ValueOperatorCondition
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ValueOperatorCondition<T>
where
    T: PartialEq + PartialOrd + std::fmt::Debug,
{
    pub operator: ConditionOperator,
    #[serde(default = "default_value")]
    pub value: Option<ValueSet<T>>,
}

fn default_value<T>() -> Option<ValueSet<T>>
where
    T: PartialEq + PartialOrd + std::fmt::Debug,
{
    None
}

impl<T> ValueOperatorCondition<T>
where
    T: PartialEq + PartialOrd + std::fmt::Debug,
{
    pub fn evaluate(&self, check_value: &T) -> bool {
        Self::evaluate_value(&self.value, &self.operator, check_value)
    }

    fn evaluate_value(value: &Option<ValueSet<T>>, operator: &ConditionOperator, check_value: &T) -> bool {
        match operator {
            ConditionOperator::Equal => match value {
                Some(ValueSet::Single(value)) => check_value == value,
                Some(ValueSet::Multiple(values)) => values.contains(check_value),
                _ => false,
            },
            ConditionOperator::Greater => match value {
                Some(ValueSet::Single(value)) => check_value > value,
                _ => false,
            },
            ConditionOperator::Less => match value {
                Some(ValueSet::Single(value)) => check_value < value,
                _ => false,
            },
            ConditionOperator::GreaterOrEqual => match value {
                Some(ValueSet::Single(value)) => check_value >= value,
                _ => false,
            },
            ConditionOperator::LessOrEqual => match value {
                Some(ValueSet::Single(value)) => check_value <= value,
                _ => false,
            },
            ConditionOperator::NotEqual => match value {
                Some(ValueSet::Single(value)) => check_value != value,
                Some(ValueSet::Multiple(values)) => !values.contains(check_value),
                _ => false,
            },
            ConditionOperator::IncludesAny => match value {
                Some(ValueSet::Single(value)) => check_value == value,
                Some(ValueSet::Multiple(values)) => values.contains(check_value),
                _ => false,
            },
            ConditionOperator::IncludesNone => match value {
                Some(ValueSet::Single(value)) => check_value != value,
                Some(ValueSet::Multiple(values)) => !values.contains(check_value),
                _ => false,
            },
            ConditionOperator::MatchAlways => true,
            ConditionOperator::IsEmpty => false,
            ConditionOperator::Exists => true,
        }
    }

    pub fn evaluate_option(&self, check_value: Option<&T>) -> bool {
        match self.operator {
            ConditionOperator::IsEmpty => check_value.is_none(),
            ConditionOperator::Exists => check_value.is_some(),
            ConditionOperator::MatchAlways => true,
            _ => match check_value {
                Some(check_value) => self.evaluate(check_value),
                None => false,
            },
        }
    }
}

// ConditionOperators
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

// Implement string conversion for ConditionOperator
impl std::str::FromStr for ConditionOperator {
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

impl<'de> serde::Deserialize<'de> for ConditionOperator {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserialize_condition_operator(deserializer)
    }
}

fn deserialize_condition_operator<'de, D>(deserializer: D) -> Result<ConditionOperator, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    ConditionOperator::from_str(s).map_err(|_| de::Error::custom(format!("Invalid ConditionOperator: {}", s)))
}

// ValueCondition
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ValueSet<T>
where
    T: PartialEq + PartialOrd + std::fmt::Debug,
{
    Single(T),
    Multiple(Vec<T>),
}

impl<T> ValueSet<T>
where
    T: PartialEq + PartialOrd + std::fmt::Debug,
{
    pub fn value(&self) -> Option<&T> {
        match self {
            ValueSet::Single(value) => Some(value),
            ValueSet::Multiple(_) => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ValueCondition<T>
where
    T: PartialEq + PartialOrd + std::fmt::Debug,
{
    Value(T),
    Operator(ValueOperatorCondition<T>),
}

impl<T> ValueCondition<T>
where
    T: PartialEq + PartialOrd + std::fmt::Debug,
{
    pub fn evaluate(&self, value: &T) -> bool {
        match self {
            ValueCondition::Value(homie_value) => value == homie_value,
            ValueCondition::Operator(operator_condition) => operator_condition.evaluate(value),
        }
    }
    pub fn evaluate_option(&self, value: Option<&T>) -> bool {
        match self {
            ValueCondition::Value(homie_value) => value == Some(homie_value),
            ValueCondition::Operator(operator_condition) => operator_condition.evaluate_option(value),
        }
    }
    pub fn value(&self) -> Option<&T> {
        match self {
            ValueCondition::Value(homie_value) => Some(homie_value),
            ValueCondition::Operator(operator_condition) => operator_condition.value.as_ref()?.value(),
        }
    }
}

// ================================================================================================
// ================================================================================================
//
//

// ValueOperatorCondition
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ValueOperatorConditionVec<T>
where
    T: PartialEq + PartialOrd + std::fmt::Debug,
{
    pub operator: ConditionOperator,
    #[serde(default = "default_value_vec")]
    pub value: Option<ValueSetVec<T>>,
}

fn default_value_vec<T>() -> Option<ValueSetVec<T>>
where
    T: PartialEq + PartialOrd + std::fmt::Debug,
{
    None
}

impl<T> ValueOperatorConditionVec<T>
where
    T: PartialEq + PartialOrd + std::fmt::Debug,
{
    pub fn evaluate(&self, check_value: &[T]) -> bool {
        match self.operator {
            ConditionOperator::Equal => match &self.value {
                Some(ValueSetVec::Single(value)) => {
                    value.len() == check_value.len() && value.iter().all(|v| check_value.contains(v))
                }
                Some(ValueSetVec::Multiple(values)) => values
                    .iter()
                    .any(|va| va.len() == check_value.len() && va.iter().all(|v| check_value.contains(v))),
                _ => false,
            },
            ConditionOperator::NotEqual => match &self.value {
                Some(ValueSetVec::Single(value)) => {
                    value.len() != check_value.len() || value.iter().any(|v| !check_value.contains(v))
                }
                Some(ValueSetVec::Multiple(values)) => {
                    // Return true if no matching vector is found in `values`
                    values
                        .iter()
                        .all(|va| va.len() != check_value.len() || va.iter().any(|v| !check_value.contains(v)))
                }
                _ => true, // If no value is specified, treat as "not equal"
            },
            ConditionOperator::IncludesAny => match &self.value {
                Some(ValueSetVec::Single(value)) => value.iter().any(|v| check_value.contains(v)),
                Some(ValueSetVec::Multiple(values)) => {
                    values.iter().any(|va| va.iter().any(|v| check_value.contains(v)))
                }
                _ => false,
            },
            ConditionOperator::IncludesNone => match &self.value {
                Some(ValueSetVec::Single(value)) => value.iter().all(|v| !check_value.contains(v)),
                Some(ValueSetVec::Multiple(values)) => {
                    values.iter().all(|va| va.iter().all(|v| !check_value.contains(v)))
                }
                _ => false,
            },
            ConditionOperator::MatchAlways => true,
            _ => false,
        }
    }
}

// ValueCondition
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ValueSetVec<T>
where
    T: PartialEq + PartialOrd + std::fmt::Debug,
{
    Single(Vec<T>),
    Multiple(Vec<Vec<T>>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ValueConditionVec<T>
where
    T: PartialEq + PartialOrd + std::fmt::Debug,
{
    Value(Vec<T>),
    Operator(ValueOperatorConditionVec<T>),
}

impl<T> ValueConditionVec<T>
where
    T: PartialEq + PartialOrd + std::fmt::Debug,
{
    pub fn evaluate(&self, value: &Vec<T>) -> bool {
        match self {
            ValueConditionVec::Value(homie_value) => value == homie_value,
            ValueConditionVec::Operator(operator_condition) => operator_condition.evaluate(value),
        }
    }
}
