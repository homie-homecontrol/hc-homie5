use homie5::HomieValue;
use serde::{Deserialize, Serialize};
pub enum ValueUpdate<T> {
    Equal,
    Changed { old: Option<T>, new: T },
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PropertyValueEntry {
    pub value: Option<HomieValue>,
    pub target: Option<HomieValue>,
}
