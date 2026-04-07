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

    pub fn store_alert(&mut self, id: HomieID, alert: String) -> AlertUpdate {
        if alert.is_empty() {
            if self.0.remove(&id).is_some() {
                return AlertUpdate::Cleared { id };
            } else {
                return AlertUpdate::NoChange;
            }
        }
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

    pub fn as_map(&self) -> &HashMap<HomieID, String> {
        &self.0
    }
}
