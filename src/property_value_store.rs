use homie5::{HomieValue, PropertyPointer};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

pub enum ValueUpdate<T> {
    Equal,
    Changed { old: Option<T>, new: T },
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PropertyValueEntry {
    pub value: Option<HomieValue>,
    pub target: Option<HomieValue>,
}

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
        if let Some(entry) = self.0.get_mut(prop) {
            if entry.value.as_ref() != Some(&value) {
                let old = entry.value.clone();
                entry.value = Some(value.clone());
                ValueUpdate::Changed { old, new: value }
            } else {
                ValueUpdate::Equal
            }
        } else {
            self.0.insert(
                prop.clone(),
                PropertyValueEntry {
                    value: Some(value.clone()),
                    ..Default::default()
                },
            );
            ValueUpdate::Changed {
                old: None,
                new: value,
            }
        }
    }

    pub fn store_target(
        &mut self,
        prop: &PropertyPointer,
        target: HomieValue,
    ) -> ValueUpdate<HomieValue> {
        if let Some(entry) = self.0.get_mut(prop) {
            if entry.target.as_ref() != Some(&target) {
                let old = entry.target.clone();
                entry.target = Some(target.clone());
                ValueUpdate::Changed { old, new: target }
            } else {
                ValueUpdate::Equal
            }
        } else {
            self.0.insert(
                prop.clone(),
                PropertyValueEntry {
                    target: Some(target.clone()),
                    ..Default::default()
                },
            );
            ValueUpdate::Changed {
                old: None,
                new: target,
            }
        }
    }

    pub fn get_value_entry(&self, prop: &PropertyPointer) -> Option<&PropertyValueEntry> {
        self.0.get(prop)
    }
}
