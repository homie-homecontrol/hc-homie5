use std::path::PathBuf;
use std::str::FromStr;

use homie5::{HomieDomain, HomieID};
use rand::{distr::Alphanumeric, rng, RngExt};

use crate::{MqttClientConfig, UnwrapOrExit};

// ── Prefixed env-var helpers ────────────────────────────────────────────

pub fn env_name(prefix: &str, name: &str) -> String {
    format!("{}_{}", prefix, name)
}

pub fn string_setting(prefix: &str, name: &str, default: impl Into<String>) -> String {
    std::env::var(env_name(prefix, name))
        .ok()
        .unwrap_or(default.into())
}

pub fn number_setting<T>(prefix: &str, name: &str, default: T) -> T
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    std::env::var(env_name(prefix, name))
        .ok()
        .map(|value| value.parse::<T>().unwrap_or_exit("Not a valid number!"))
        .unwrap_or(default)
}

pub fn generic_setting<T>(prefix: &str, name: &str, default: T) -> T
where
    T: TryFrom<String>,
    T::Error: std::fmt::Display,
{
    std::env::var(env_name(prefix, name))
        .ok()
        .map(|value| value.try_into().unwrap_or_exit("Invalid setting supplied!"))
        .unwrap_or(default)
}

pub fn bool_setting(prefix: &str, name: &str, default: bool) -> bool {
    std::env::var(env_name(prefix, name))
        .ok()
        .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(default)
}

pub fn optional_path_setting(prefix: &str, name: &str) -> Option<PathBuf> {
    std::env::var(env_name(prefix, name))
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

// ── Direct env-var helpers (for crates with non-standard env var names) ─

pub fn number_setting_min<T>(env_var: &str, default: T, min: T) -> T
where
    T: FromStr + PartialOrd,
{
    match std::env::var(env_var) {
        Ok(raw) => match raw.parse::<T>() {
            Ok(value) if value >= min => value,
            _ => default,
        },
        Err(_) => default,
    }
}

pub fn number_setting_in_range<T>(env_var: &str, default: T, min: T, max: T) -> T
where
    T: FromStr + PartialOrd,
{
    match std::env::var(env_var) {
        Ok(raw) => match raw.parse::<T>() {
            Ok(value) if value >= min && value <= max => value,
            _ => default,
        },
        Err(_) => default,
    }
}

// ── HomieSettings ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HomieSettings {
    pub hostname: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub client_id: String,
    pub homie_domain: HomieDomain,
    pub controller_id: Option<HomieID>,
    pub controller_name: Option<String>,
    pub use_tls: bool,
    pub ca_path: Option<PathBuf>,
    pub client_cert_path: Option<PathBuf>,
    pub client_key_path: Option<PathBuf>,
}

impl HomieSettings {
    /// Read HomieSettings from environment variables with the given prefix.
    ///
    /// Reads: `{prefix}_HOMIE_HOST`, `{prefix}_HOMIE_PORT`, etc.
    ///
    /// `client_id_prefix` is used for auto-generated client IDs (e.g., "hcactl-").
    /// `default_domain` sets the default HomieDomain when the env var is absent.
    pub fn from_env(prefix: &str, client_id_prefix: &str, default_domain: HomieDomain) -> Self {
        let hostname = string_setting(prefix, "HOMIE_HOST", "localhost");
        let port = number_setting(prefix, "HOMIE_PORT", 1883u16);
        let username = string_setting(prefix, "HOMIE_USERNAME", String::default());
        let password = string_setting(prefix, "HOMIE_PASSWORD", String::default());
        let client_id = string_setting(
            prefix,
            "HOMIE_CLIENT_ID",
            format!(
                "{}{}",
                client_id_prefix,
                rng()
                    .sample_iter(&Alphanumeric)
                    .take(8)
                    .map(char::from)
                    .collect::<String>()
            ),
        );
        let homie_domain = generic_setting(prefix, "HOMIE_DOMAIN", default_domain);

        let controller_id = std::env::var(env_name(prefix, "HOMIE_CTRL_ID"))
            .ok()
            .map(|v| v.try_into().unwrap_or_exit("Invalid controller ID"));
        let controller_name = std::env::var(env_name(prefix, "HOMIE_CTRL_NAME")).ok();

        let use_tls = bool_setting(prefix, "HOMIE_USE_TLS", false);
        let ca_path = optional_path_setting(prefix, "HOMIE_CA_PATH");
        let client_cert_path = optional_path_setting(prefix, "HOMIE_CLIENT_CERT");
        let client_key_path = optional_path_setting(prefix, "HOMIE_CLIENT_KEY");

        Self {
            hostname,
            port,
            username,
            password,
            client_id,
            homie_domain,
            controller_id,
            controller_name,
            use_tls,
            ca_path,
            client_cert_path,
            client_key_path,
        }
    }

    pub fn to_mqtt_client_config(&self) -> MqttClientConfig {
        MqttClientConfig::new(&self.hostname)
            .port(self.port)
            .username(&self.username)
            .password(&self.password)
            .client_id(&self.client_id)
            .use_tls(self.use_tls)
            .ca_path(self.ca_path.as_ref())
            .client_cert_path(self.client_cert_path.as_ref())
            .client_key_path(self.client_key_path.as_ref())
    }
}
