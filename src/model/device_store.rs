use homie5::{
    device_description::HomieDeviceDescription, DeviceRef, HomieDeviceStatus, HomieDomain, HomieID,
};

use crate::{AlertStore, PropertyValueStore};

pub enum DeviceUpdate<'a> {
    Added(&'a DeviceRef),
    StateUpdate {
        device: &'a DeviceRef,
        from: HomieDeviceStatus,
        to: HomieDeviceStatus,
    },
    NoChange,
}

#[allow(clippy::large_enum_variant)] // Suppress the Clippy warning for large enum variants - most
                                     // messages will be Update
pub enum DescriptionUpdate<'a> {
    Update {
        device: &'a DeviceRef,
        from: Option<HomieDeviceDescription>,
        to: &'a HomieDeviceDescription,
    },
    NoChange,
    NotFound,
}

#[allow(clippy::large_enum_variant)] // Suppress the Clippy warning for large enum variants - most
                                     // messages will be Remove
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
