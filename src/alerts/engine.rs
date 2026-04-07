use std::{borrow::Cow, collections::HashMap};

use hc_homie5_smarthome::alerts::SmarthomeAlert;
use homie5::HomieID;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertState {
    Unknown,
    Active,
    Inactive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MissingInPartialPolicy {
    #[default]
    KeepLast,
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconcileMode {
    FullSnapshot,
    PartialSnapshot,
}

#[derive(Debug, Clone)]
pub struct AlertSpec {
    pub id: HomieID,
    pub missing_in_partial: MissingInPartialPolicy,
    pub default_active_payload: Cow<'static, str>,
}

impl AlertSpec {
    pub fn new(id: HomieID) -> Self {
        Self {
            id,
            missing_in_partial: MissingInPartialPolicy::KeepLast,
            default_active_payload: Cow::Borrowed("true"),
        }
    }

    pub fn smarthome(alert: SmarthomeAlert) -> Self {
        let id = HomieID::try_from(alert.as_str())
            .expect("SmarthomeAlert constants must be valid Homie IDs");
        Self::new(id)
    }

    pub fn with_missing_policy(mut self, policy: MissingInPartialPolicy) -> Self {
        self.missing_in_partial = policy;
        self
    }

    pub fn with_default_payload(mut self, payload: impl Into<Cow<'static, str>>) -> Self {
        self.default_active_payload = payload.into();
        self
    }
}

#[derive(Debug, Clone)]
pub struct AlertObservation<'a> {
    pub id: &'a HomieID,
    pub active: bool,
    pub payload_if_active: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertOp {
    Set { id: HomieID, payload: String },
    Clear { id: HomieID },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AlertApplyStats {
    pub set_count: usize,
    pub clear_count: usize,
    pub unchanged_count: usize,
}

impl AlertApplyStats {
    fn from_op(op: &AlertOp) -> Self {
        match op {
            AlertOp::Set { .. } => Self {
                set_count: 1,
                clear_count: 0,
                unchanged_count: 0,
            },
            AlertOp::Clear { .. } => Self {
                set_count: 0,
                clear_count: 1,
                unchanged_count: 0,
            },
        }
    }
}

#[derive(Debug, Clone)]
struct AlertEntry {
    spec: AlertSpec,
    state: AlertState,
}

#[derive(Debug, Clone, Default)]
pub struct AlertEngine {
    index: HashMap<HomieID, usize>,
    entries: Vec<AlertEntry>,
}

impl AlertEngine {
    pub fn new(specs: impl IntoIterator<Item = AlertSpec>) -> Self {
        let mut index: HashMap<HomieID, usize> = HashMap::new();
        let mut entries: Vec<AlertEntry> = Vec::new();

        for spec in specs {
            if let Some(existing_idx) = index.get(&spec.id).copied() {
                entries[existing_idx].spec = spec;
                continue;
            }

            let idx = entries.len();
            index.insert(spec.id.clone(), idx);
            entries.push(AlertEntry {
                spec,
                state: AlertState::Unknown,
            });
        }

        Self { index, entries }
    }

    pub fn update_one(
        &mut self,
        id: &HomieID,
        active: bool,
        payload_if_active: Option<&str>,
    ) -> Option<AlertOp> {
        let idx = self.index.get(id).copied()?;
        self.transition_with_idx(idx, active, payload_if_active)
    }

    pub fn apply_cycle<'a>(
        &mut self,
        mode: ReconcileMode,
        observed: impl IntoIterator<Item = AlertObservation<'a>>,
        out: &mut Vec<AlertOp>,
    ) -> AlertApplyStats {
        out.clear();

        let mut stats = AlertApplyStats::default();
        let mut observed_states = vec![None; self.entries.len()];
        let mut observed_payloads: Vec<Option<String>> = vec![None; self.entries.len()];

        for obs in observed {
            let Some(idx) = self.index.get(obs.id).copied() else {
                continue;
            };
            observed_states[idx] = Some(obs.active);
            observed_payloads[idx] = obs.payload_if_active.map(ToOwned::to_owned);
        }

        for idx in 0..self.entries.len() {
            let desired = match observed_states[idx] {
                Some(active) => Some((active, observed_payloads[idx].as_deref())),
                None => match mode {
                    ReconcileMode::FullSnapshot => Some((false, None)),
                    ReconcileMode::PartialSnapshot => {
                        match self.entries[idx].spec.missing_in_partial {
                            MissingInPartialPolicy::KeepLast => None,
                            MissingInPartialPolicy::Clear => Some((false, None)),
                        }
                    }
                },
            };

            let Some((active, payload_if_active)) = desired else {
                stats.unchanged_count += 1;
                continue;
            };

            if let Some(op) = self.transition_with_idx(idx, active, payload_if_active) {
                stats = add_stats(stats, AlertApplyStats::from_op(&op));
                out.push(op);
            } else {
                stats.unchanged_count += 1;
            }
        }

        stats
    }

    pub fn reset_runtime_state(&mut self) {
        for entry in &mut self.entries {
            entry.state = AlertState::Unknown;
        }
    }

    pub fn state(&self, id: &HomieID) -> Option<AlertState> {
        self.index.get(id).map(|idx| self.entries[*idx].state)
    }

    fn transition_with_idx(
        &mut self,
        idx: usize,
        active: bool,
        payload_if_active: Option<&str>,
    ) -> Option<AlertOp> {
        let entry = &mut self.entries[idx];

        let desired_state = if active {
            AlertState::Active
        } else {
            AlertState::Inactive
        };

        if entry.state == desired_state {
            return None;
        }

        entry.state = desired_state;
        if active {
            let payload = payload_if_active
                .unwrap_or(entry.spec.default_active_payload.as_ref())
                .to_owned();
            Some(AlertOp::Set {
                id: entry.spec.id.clone(),
                payload,
            })
        } else {
            Some(AlertOp::Clear {
                id: entry.spec.id.clone(),
            })
        }
    }
}

fn add_stats(lhs: AlertApplyStats, rhs: AlertApplyStats) -> AlertApplyStats {
    AlertApplyStats {
        set_count: lhs.set_count + rhs.set_count,
        clear_count: lhs.clear_count + rhs.clear_count,
        unchanged_count: lhs.unchanged_count + rhs.unchanged_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(raw: &'static str) -> HomieID {
        HomieID::new_const(raw)
    }

    #[test]
    fn update_one_uses_default_payload_and_transitions() {
        let alert_id = id("hc-unreachable");
        let mut engine = AlertEngine::new([AlertSpec::new(alert_id.clone())]);

        let op = engine.update_one(&alert_id, true, None);
        assert_eq!(
            op,
            Some(AlertOp::Set {
                id: alert_id.clone(),
                payload: "true".to_owned(),
            })
        );
        assert_eq!(engine.state(&alert_id), Some(AlertState::Active));

        let noop = engine.update_one(&alert_id, true, None);
        assert_eq!(noop, None);

        let clear = engine.update_one(&alert_id, false, None);
        assert_eq!(clear, Some(AlertOp::Clear { id: alert_id }));
    }

    #[test]
    fn full_snapshot_clears_unobserved_from_unknown() {
        let a = id("hc-unreachable");
        let b = id("hc-comm-error");
        let mut engine = AlertEngine::new([AlertSpec::new(a.clone()), AlertSpec::new(b.clone())]);
        let mut out = Vec::new();

        let stats = engine.apply_cycle(
            ReconcileMode::FullSnapshot,
            [AlertObservation {
                id: &a,
                active: true,
                payload_if_active: Some("offline"),
            }],
            &mut out,
        );

        assert_eq!(
            out,
            vec![
                AlertOp::Set {
                    id: a,
                    payload: "offline".to_owned(),
                },
                AlertOp::Clear { id: b }
            ]
        );
        assert_eq!(
            stats,
            AlertApplyStats {
                set_count: 1,
                clear_count: 1,
                unchanged_count: 0
            }
        );
    }

    #[test]
    fn partial_snapshot_respects_missing_policy() {
        let keep = id("hc-unreachable");
        let clear = id("hc-comm-error");
        let mut engine = AlertEngine::new([
            AlertSpec::new(keep.clone()).with_missing_policy(MissingInPartialPolicy::KeepLast),
            AlertSpec::new(clear.clone()).with_missing_policy(MissingInPartialPolicy::Clear),
        ]);
        let mut out = Vec::new();

        let stats = engine.apply_cycle(ReconcileMode::PartialSnapshot, [], &mut out);

        assert_eq!(out, vec![AlertOp::Clear { id: clear }]);
        assert_eq!(
            stats,
            AlertApplyStats {
                set_count: 0,
                clear_count: 1,
                unchanged_count: 1
            }
        );
        assert_eq!(engine.state(&keep), Some(AlertState::Unknown));
    }
}
