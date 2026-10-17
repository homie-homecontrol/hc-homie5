use std::time::{Duration, Instant};

/// Tracks whether a recurring operation (connection, poll, ping) is failing, so a failure is
/// logged once when it starts and the recovery once when it ends.
///
/// ```ignore
/// match connect().await {
///     Err(err) if outage.fail() => warn!(broker = %host, error = %err, "MQTT connection lost"),
///     Err(err) => debug!(attempt = outage.failures(), error = %err, "MQTT reconnect failed"),
///     Ok(()) => {
///         if let Some(down) = outage.recover() {
///             info!(broker = %host, outage_secs = down.as_secs(), "MQTT connection restored");
///         }
///     }
/// }
/// ```
#[derive(Debug, Default, Clone)]
pub struct Outage {
    since: Option<Instant>,
    failures: u32,
}

impl Outage {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a failure. Returns `true` only for the first failure of an outage.
    pub fn fail(&mut self) -> bool {
        self.failures = self.failures.saturating_add(1);
        if self.since.is_some() {
            return false;
        }
        self.since = Some(Instant::now());
        true
    }

    /// Records a success. Returns the outage duration if this success ends an outage.
    pub fn recover(&mut self) -> Option<Duration> {
        self.failures = 0;
        self.since.take().map(|since| since.elapsed())
    }

    pub fn is_down(&self) -> bool {
        self.since.is_some()
    }

    /// Number of failures since the outage started.
    pub fn failures(&self) -> u32 {
        self.failures
    }

    /// Time since the outage started.
    pub fn down_for(&self) -> Option<Duration> {
        self.since.map(|since| since.elapsed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_failure_starts_outage() {
        let mut outage = Outage::new();
        assert!(!outage.is_down());
        assert!(outage.fail());
        assert!(!outage.fail());
        assert!(!outage.fail());
        assert!(outage.is_down());
        assert_eq!(outage.failures(), 3);
    }

    #[test]
    fn recovery_reports_duration_once() {
        let mut outage = Outage::new();
        assert_eq!(outage.recover(), None);
        outage.fail();
        assert!(outage.recover().is_some());
        assert_eq!(outage.recover(), None);
        assert!(!outage.is_down());
        assert_eq!(outage.failures(), 0);
        assert!(outage.fail());
    }
}
