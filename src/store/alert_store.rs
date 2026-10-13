use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use homie5::HomieID;

pub enum AlertUpdate {
    New {
        id: HomieID,
        alert: String,
    },
    Changed {
        id: HomieID,
        old_alert: String,
        new_alert: String,
    },
    Cleared {
        id: HomieID,
    },
    Equal,
    NoChange,
}

#[derive(Default, Clone, Debug)]
pub struct AlertStore(HashMap<HomieID, String>);

impl Deref for AlertStore {
    type Target = HashMap<HomieID, String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for AlertStore {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AlertStore {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Stores an active alert. An empty `alert` is a valid alert text, not a removal; use
    /// [`AlertStore::clear_alert`] for cleared alerts.
    pub fn store_alert(&mut self, id: HomieID, alert: String) -> AlertUpdate {
        if let Some(entry) = self.0.get_mut(&id) {
            if *entry != alert {
                let old = entry.clone();
                *entry = alert.clone();
                AlertUpdate::Changed {
                    id,
                    old_alert: old,
                    new_alert: alert,
                }
            } else {
                AlertUpdate::Equal
            }
        } else {
            self.0.insert(id.clone(), alert.clone());
            AlertUpdate::New { id, alert }
        }
    }

    /// Removes a cleared alert.
    pub fn clear_alert(&mut self, id: HomieID) -> AlertUpdate {
        if self.0.remove(&id).is_some() {
            AlertUpdate::Cleared { id }
        } else {
            AlertUpdate::NoChange
        }
    }

    pub fn as_map(&self) -> &HashMap<HomieID, String> {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_alert_text_is_stored_and_clear_removes_it() {
        let id = HomieID::new_const("battery");
        let mut store = AlertStore::new();

        assert!(matches!(
            store.store_alert(id.clone(), String::new()),
            AlertUpdate::New { .. }
        ));
        assert_eq!(store.get(&id).map(String::as_str), Some(""));

        assert!(matches!(
            store.clear_alert(id.clone()),
            AlertUpdate::Cleared { .. }
        ));
        assert!(matches!(store.clear_alert(id), AlertUpdate::NoChange));
    }
}
