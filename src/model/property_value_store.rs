use chrono::{DateTime, Utc};
use homie5::HomieValue;
use serde::{Deserialize, Serialize};
pub enum ValueUpdate<T> {
    Equal,
    Changed { old: Option<T>, new: T },
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PropertyValueEntry {
    pub value: Option<HomieValue>,
    pub target: Option<HomieValue>,
    pub value_last_received: Option<DateTime<Utc>>,
    pub value_last_changed: Option<DateTime<Utc>>,
    pub target_last_received: Option<DateTime<Utc>>,
    pub target_last_changed: Option<DateTime<Utc>>,
}
