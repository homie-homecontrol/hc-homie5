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
            ConditionOperator::Between => match operand {
                Some(ValueSet::Multiple(ref bounds)) => match bounds.as_slice() {
                    [min, max] => self >= min && self <= max,
                    _ => false,
                },
                _ => false,
            },
        }
    }

    fn matches_literal(&self, other: &Self) -> bool {
        self == other
    }
}
