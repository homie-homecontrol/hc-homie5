use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;

use super::ValueMatcher;

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
// --- Condition Operators, extended with pattern matching variants ---
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
impl std::fmt::Display for ConditionOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ConditionOperator::Equal => "=",
            ConditionOperator::Greater => ">",
            ConditionOperator::Less => "<",
            ConditionOperator::GreaterOrEqual => ">=",
            ConditionOperator::LessOrEqual => "<=",
            ConditionOperator::NotEqual => "<>",
            ConditionOperator::IncludesAny => "includesAny",
            ConditionOperator::IncludesNone => "includesNone",
            ConditionOperator::MatchAlways => "matchAlways",
            ConditionOperator::IsEmpty => "isEmpty",
            ConditionOperator::Exists => "exists",
        };
        f.write_str(s)
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

impl Serialize for ConditionOperator {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

// --- Generic ValueSet and ValueCondition types ---

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged, deny_unknown_fields)]
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
