use homie5::{Homie5DeviceProtocol, HomieID};

use crate::client::HomieMQTTClient;

use super::{AlertApplyStats, AlertEngine, AlertObservation, AlertOp, AlertSpec, ReconcileMode};

#[derive(Debug, Clone)]
pub struct DeviceAlertPublisher {
    engine: AlertEngine,
    homie_proto: Homie5DeviceProtocol,
    mqtt: HomieMQTTClient,
    scratch_ops: Vec<AlertOp>,
}

impl DeviceAlertPublisher {
    pub fn new(
        homie_proto: &Homie5DeviceProtocol,
        mqtt: &HomieMQTTClient,
        specs: impl IntoIterator<Item = AlertSpec>,
    ) -> Self {
        Self {
            engine: AlertEngine::new(specs),
            homie_proto: homie_proto.clone(),
            mqtt: mqtt.clone(),
            scratch_ops: Vec::new(),
        }
    }

    pub async fn update_one(
        &mut self,
        id: &HomieID,
        active: bool,
        payload_if_active: Option<&str>,
    ) -> Result<bool, rumqttc::ClientError> {
        let Some(op) = self.engine.update_one(id, active, payload_if_active) else {
            return Ok(false);
        };

        self.publish_op(&op).await?;
        Ok(true)
    }

    pub async fn apply_cycle<'a>(
        &mut self,
        mode: ReconcileMode,
        observed: impl IntoIterator<Item = AlertObservation<'a>>,
    ) -> Result<AlertApplyStats, rumqttc::ClientError> {
        let stats = self.engine.apply_cycle(mode, observed, &mut self.scratch_ops);

        for op in &self.scratch_ops {
            self.publish_op(op).await?;
        }

        Ok(stats)
    }

    pub async fn reconcile_on_ready<'a>(
        &mut self,
        observed: impl IntoIterator<Item = AlertObservation<'a>>,
    ) -> Result<AlertApplyStats, rumqttc::ClientError> {
        self.apply_cycle(ReconcileMode::FullSnapshot, observed).await
    }

    pub fn engine_mut(&mut self) -> &mut AlertEngine {
        &mut self.engine
    }

    pub fn engine(&self) -> &AlertEngine {
        &self.engine
    }

    async fn publish_op(&self, op: &AlertOp) -> Result<(), rumqttc::ClientError> {
        let publish = match op {
            AlertOp::Set { id, payload } => self.homie_proto.publish_alert(id, payload),
            AlertOp::Clear { id } => self.homie_proto.publish_clear_alert(id),
        };

        self.mqtt.homie_publish(publish).await
    }
}
