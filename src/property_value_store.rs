use chrono::Utc;
use homie5::{HomieValue, PropertyPointer};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use crate::{PropertyValueEntry, ValueUpdate};

#[derive(Default, Clone, Debug)]
pub struct PropertyValueStore(HashMap<PropertyPointer, PropertyValueEntry>);

impl Deref for PropertyValueStore {
    type Target = HashMap<PropertyPointer, PropertyValueEntry>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PropertyValueStore {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl PropertyValueStore {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn count(&self) -> usize {
        self.0.keys().count()
    }
    pub fn store_value(
        &mut self,
        prop: &PropertyPointer,
        value: HomieValue,
    ) -> ValueUpdate<HomieValue> {
        let now = Utc::now();
        if let Some(entry) = self.0.get_mut(prop) {
            entry.value_last_received = Some(now);
            if entry.value.as_ref() != Some(&value) {
                let old = entry.value.clone();
                entry.value = Some(value.clone());
                entry.value_last_changed = Some(now);
                ValueUpdate::Changed {
                    old,
                    new: value,
                    last_received: Some(now),
                    last_changed: Some(now),
                }
            } else {
                ValueUpdate::Equal {
                    last_received: Some(now),
                    last_changed: entry.value_last_changed,
                }
            }
        } else {
            self.0.insert(
                prop.clone(),
                PropertyValueEntry {
                    value: Some(value.clone()),
                    value_last_received: Some(now),
                    value_last_changed: Some(now),
                    ..Default::default()
                },
            );
            ValueUpdate::Changed {
                old: None,
                new: value,
                last_received: Some(now),
                last_changed: Some(now),
            }
        }
    }

    pub fn store_target(
        &mut self,
        prop: &PropertyPointer,
        target: HomieValue,
    ) -> ValueUpdate<HomieValue> {
        let now = Utc::now();
        if let Some(entry) = self.0.get_mut(prop) {
            entry.target_last_received = Some(now);
            if entry.target.as_ref() != Some(&target) {
                let old = entry.target.clone();
                entry.target = Some(target.clone());
                entry.target_last_changed = Some(now);
                ValueUpdate::Changed {
                    old,
                    new: target,
                    last_received: Some(now),
                    last_changed: Some(now),
                }
            } else {
                ValueUpdate::Equal {
                    last_received: Some(now),
                    last_changed: entry.target_last_changed,
                }
            }
        } else {
            self.0.insert(
                prop.clone(),
                PropertyValueEntry {
                    target: Some(target.clone()),
                    target_last_received: Some(now),
                    target_last_changed: Some(now),
                    ..Default::default()
                },
            );
            ValueUpdate::Changed {
                old: None,
                new: target,
                last_received: Some(now),
                last_changed: Some(now),
            }
        }
    }

    pub fn get_value_entry(&self, prop: &PropertyPointer) -> Option<&PropertyValueEntry> {
        self.0.get(prop)
    }
}
