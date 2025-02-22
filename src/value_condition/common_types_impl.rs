use homie5::{HomieDataType, HomieDomain, HomieID, HomieValue};

use crate::impl_value_matcher_for_vec;

use super::ValueMatcher;

// Implement ValueMatcher for Common and Homie Types
impl ValueMatcher for String {
    fn as_match_str(&self) -> &str {
        self.as_str()
    }
}

impl ValueMatcher for HomieValue {
    fn as_match_str(&self) -> &str {
        match self {
            HomieValue::String(ref s) => s.as_str(),
            HomieValue::Enum(ref s) => s.as_str(),
            _ => "",
        }
    }
}

impl ValueMatcher for HomieID {
    fn as_match_str(&self) -> &str {
        self.as_str()
    }
}

// impl_value_matcher_for!(HomieID);
impl_value_matcher_for_vec!(HomieID);

impl ValueMatcher for HomieDataType {
    fn as_match_str(&self) -> &str {
        match self {
            HomieDataType::Integer => "integer",
            HomieDataType::Float => "float",
            HomieDataType::Boolean => "boolean",
            HomieDataType::String => "string",
            HomieDataType::Enum => "enum",
            HomieDataType::Color => "color",
            HomieDataType::Datetime => "datetime",
            HomieDataType::Duration => "duration",
            HomieDataType::JSON => "json",
        }
    }
}

impl ValueMatcher for bool {
    fn as_match_str(&self) -> &str {
        ""
    }
    fn matches_regex(&self, _: &str) -> bool {
        false
    }
}

impl ValueMatcher for i64 {
    fn as_match_str(&self) -> &str {
        ""
    }
    fn matches_regex(&self, _: &str) -> bool {
        false
    }
}
impl ValueMatcher for HomieDomain {
    fn as_match_str(&self) -> &str {
        ""
    }
    fn matches_regex(&self, _: &str) -> bool {
        false
    }
}

impl_value_matcher_for_vec!(String);
