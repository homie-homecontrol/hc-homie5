#[cfg(test)]
mod tests {
    use std::time::Duration;

    use hc_homie5::client::MqttClientConfig;

    #[test]
    fn test_max_disconnect_default_none() {
        let config = MqttClientConfig::new("localhost");
        assert!(config.max_disconnect.is_none());
    }

    #[test]
    fn test_max_disconnect_builder() {
        let config = MqttClientConfig::new("localhost")
            .max_disconnect(Some(Duration::from_secs(30)));
        assert_eq!(config.max_disconnect, Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_max_disconnect_none_builder() {
        let config = MqttClientConfig::new("localhost")
            .max_disconnect(Some(Duration::from_secs(60)))
            .max_disconnect(None);
        assert!(config.max_disconnect.is_none());
    }

    #[test]
    fn test_into_bridge_setup() {
        let config = MqttClientConfig::new("localhost")
            .port(1883)
            .max_disconnect(Some(Duration::from_secs(120)));

        let controller_id = "test-bridge".try_into().unwrap();
        let domain = "homie".try_into().unwrap();

        let setup = config.into_bridge_setup(controller_id, domain).unwrap();

        // Check that max_disconnect was preserved
        assert_eq!(setup.max_disconnect, Some(Duration::from_secs(120)));
    }
}
