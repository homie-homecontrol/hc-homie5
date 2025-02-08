use homie5::{
    device_description::HomieDeviceDescription, DeviceRef, HomieDeviceStatus, HomieDomain, HomieID,
    HomieValue, PropertyRef,
};
use std::collections::{
    hash_map::{Entry, Keys},
    HashMap,
};

use crate::{AlertStore, PropertyValueEntry};

use super::PropertyValueStore;

pub enum DeviceUpdate<'a> {
    Added(&'a DeviceRef),
    StateUpdate {
        device: &'a DeviceRef,
        from: HomieDeviceStatus,
        to: HomieDeviceStatus,
    },
    NoChange,
}

pub enum DescriptionUpdate<'a> {
    Update {
        device: &'a DeviceRef,
        from: Option<HomieDeviceDescription>,
        to: &'a HomieDeviceDescription,
    },
    NoChange,
    NotFound,
}
pub enum DeviceRemove {
    Removed(Device),
    NotFound,
}
#[derive(Clone, Debug)]
pub struct Device {
    pub ident: DeviceRef,
    pub state: HomieDeviceStatus,
    pub description: Option<HomieDeviceDescription>,
    pub prop_values: PropertyValueStore,
    pub alerts: AlertStore,
}

impl Device {
    pub fn homie_domain(&self) -> &HomieDomain {
        self.ident.homie_domain()
    }

    pub fn device_id(&self) -> &HomieID {
        self.ident.device_id()
    }
}

pub type DeviceMap = HashMap<HomieID, Device>;
#[derive(Default, Clone)]
pub struct DeviceStore(HashMap<HomieDomain, DeviceMap>);

impl DeviceStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add<'a>(
        &mut self,
        device_ref: &'a DeviceRef,
        status: HomieDeviceStatus,
    ) -> DeviceUpdate<'a> {
        if let Some(device) = self
            .0
            .get_mut(device_ref.homie_domain())
            .and_then(|d| d.get_mut(device_ref.device_id()))
        {
            if device.state != status {
                let update = DeviceUpdate::StateUpdate {
                    device: device_ref,
                    from: device.state,
                    to: status,
                };
                device.state = status;
                update
            } else {
                DeviceUpdate::NoChange
            }
        } else {
            let device = Device {
                ident: device_ref.to_owned(),
                state: status,
                description: None,
                prop_values: PropertyValueStore::new(),
                alerts: AlertStore::new(),
            };
            if let Some(dev_map) = self.0.get_mut(device_ref.homie_domain()) {
                dev_map.insert(device_ref.device_id().to_owned(), device);
            } else {
                let mut dev_map = HashMap::new();
                dev_map.insert(device_ref.device_id().to_owned(), device);
                self.0.insert(device_ref.homie_domain().to_owned(), dev_map);
            };
            DeviceUpdate::Added(device_ref)
        }
    }

    pub fn remove_device(&mut self, devref: &DeviceRef) -> DeviceRemove {
        if let Some(dm) = self.0.get_mut(devref.homie_domain()) {
            if let Some(device) = dm.remove(devref.device_id()) {
                DeviceRemove::Removed(device)
            } else {
                DeviceRemove::NotFound
            }
        } else {
            DeviceRemove::NotFound
        }
    }

    pub fn store_description<'a>(
        &'a mut self,
        device_ref: &'a DeviceRef,
        description: HomieDeviceDescription,
    ) -> DescriptionUpdate<'a> {
        if let Some(device) = self
            .0
            .get_mut(device_ref.homie_domain())
            .and_then(|dm| dm.get_mut(device_ref.device_id()))
        {
            if let Some(current_desc) = &device.description {
                if current_desc.version != description.version {
                    let old_desc = device.description.take().unwrap();
                    device.description = Some(description);
                    DescriptionUpdate::Update {
                        device: device_ref,
                        from: Some(old_desc),
                        to: device.description.as_ref().unwrap(),
                    }
                } else {
                    DescriptionUpdate::NoChange
                }
            } else {
                device.description = Some(description);
                DescriptionUpdate::Update {
                    device: device_ref,
                    from: None,
                    to: device.description.as_ref().unwrap(),
                }
            }
        } else {
            DescriptionUpdate::NotFound
        }
    }

    pub fn device_entry(&mut self, devref: DeviceRef) -> Entry<HomieID, Device> {
        let (homie_domain, id) = devref.into_parts();
        self.0.entry(homie_domain).or_default().entry(id)
    }

    pub fn get_device(&self, devref: &DeviceRef) -> Option<&Device> {
        self.0
            .get(devref.homie_domain())
            .and_then(|tr| tr.get(devref.device_id()))
    }

    pub fn get_value_entry(&self, prop: &PropertyRef) -> Option<&PropertyValueEntry> {
        self.get_device(prop.device_ref())
            .and_then(|device| device.prop_values.get_value_entry(prop.prop_pointer()))
    }

    pub fn get_property_value(&self, prop: &PropertyRef) -> Option<&HomieValue> {
        self.get_device(prop.device_ref()).and_then(|device| {
            device
                .prop_values
                .get_value_entry(prop.prop_pointer())
                .and_then(|entry| entry.value.as_ref())
        })
    }

    pub fn get_device_mut(&mut self, devref: &DeviceRef) -> Option<&mut Device> {
        self.0
            .get_mut(devref.homie_domain())
            .and_then(|tr| tr.get_mut(devref.device_id()))
    }

    pub fn contains_device(&self, devref: &DeviceRef) -> bool {
        self.0
            .get(devref.homie_domain())
            .map(|tr| tr.contains_key(devref.device_id()))
            .unwrap_or(false)
    }

    pub fn contains_property(&self, prop: &PropertyRef) -> bool {
        self.get_device(prop.device_ref())
            .map(|device| device.prop_values.contains_key(prop.prop_pointer()))
            .unwrap_or(false)
    }

    pub fn device_state(&self, devref: &DeviceRef) -> Option<HomieDeviceStatus> {
        self.get_device(devref).map(|device| device.state)
    }

    /// This will get the root devices state if the device has a root device
    /// otherwise the state of the device belonging to devref will be returned
    pub fn device_state_resolved(&self, devref: &DeviceRef) -> Option<HomieDeviceStatus> {
        // get the actual device first
        let device = self.get_device(devref)?;

        // if the device has any other state than ready return its state
        if !matches!(device.state, HomieDeviceStatus::Ready) {
            return Some(device.state);
        }

        // otherwise check for the root device state

        // if the device has a root device get the ID in the "root" variable
        let Some(root) = device
            .description
            .as_ref()
            .and_then(|desc| desc.root.as_ref())
        else {
            // otherwise return the device state
            return Some(device.state);
        };
        // if the root device exists get the root device state
        let Some(root_device_state) = self
            .0
            .get(devref.homie_domain())
            .and_then(|tr| tr.get(root).map(|device| device.state))
        else {
            // otherwise return the original device state
            return Some(device.state);
        };
        // return the root device state
        Some(root_device_state)
    }

    pub fn topics(&self) -> Keys<HomieDomain, DeviceMap> {
        self.0.keys()
    }

    pub fn get_device_map(&self, domain: &HomieDomain) -> Option<&DeviceMap> {
        self.0.get(domain)
    }

    pub fn clear(&mut self) {
        log::debug!("Clearing all devices!");
        self.0.clear();
    }

    pub fn count(&self) -> usize {
        self.0.values().map(|v| v.keys().count()).sum()
    }

    pub fn iter(&self) -> DeviceStoreIterator {
        DeviceStoreIterator::new(self)
    }

    pub fn is_orphaned(&self, device: &Device) -> bool {
        if let Some(desc) = &device.description {
            if let Some(parent) = &desc.parent {
                let dref = device.ident.clone_with_id(parent.clone());
                if let Some(p) = self.get_device(&dref) {
                    if let Some(pd) = &p.description {
                        if pd.children.contains(device.device_id()) {
                            if pd.parent.is_some() {
                                return self.is_orphaned(p);
                            } else {
                                return false;
                            }
                        } else {
                            return true;
                        }
                    }
                } else {
                    return true;
                }
            }
        }
        false
    }
}

pub struct DeviceStoreIterator<'a> {
    _store: &'a DeviceStore,
    topic_root_iter: std::collections::hash_map::Iter<'a, HomieDomain, DeviceMap>,
    current_topic_root: Option<&'a HomieDomain>,
    device_map_iter: Option<std::collections::hash_map::Iter<'a, HomieID, Device>>,
}

impl<'a> DeviceStoreIterator<'a> {
    pub fn new(_store: &'a DeviceStore) -> Self {
        let mut topic_root_iter = _store.0.iter();

        let first_topic_root = topic_root_iter.next();

        let (current_topic_root, device_map_iter) = match first_topic_root {
            Some((topic_root, device_map)) => (Some(topic_root), Some(device_map.iter())),
            None => (None, None),
        };

        DeviceStoreIterator {
            _store,
            topic_root_iter,
            current_topic_root,
            device_map_iter,
        }
    }
}

impl<'a> Iterator for DeviceStoreIterator<'a> {
    type Item = (&'a HomieDomain, &'a HomieID, &'a Device);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(iter) = self.device_map_iter.as_mut() {
                if let Some((device_id, device)) = iter.next() {
                    return Some((self.current_topic_root.unwrap(), device_id, device));
                }
            }

            match self.topic_root_iter.next() {
                Some((topic_root, device_map)) => {
                    self.current_topic_root = Some(topic_root);
                    self.device_map_iter = Some(device_map.iter())
                }
                None => return None,
            }
        }
    }
}
