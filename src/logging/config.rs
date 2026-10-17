use std::fmt;

use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

/// Target of the events that [`init`](super::init) logs on behalf of the service (`starting`,
/// invalid settings, panics). It always follows the service's level.
const LOGGING_TARGET: &str = "hc_homie5::logging";

/// Logging configuration of a service, usually built with [`log_config!`](crate::log_config).
#[derive(Debug, Clone)]
pub struct LogConfig {
    pub(crate) env_prefix: String,
    pub(crate) app_name: String,
    pub(crate) version: String,
    pub(crate) app_targets: Vec<String>,
    pub(crate) directives: Vec<String>,
    pub(crate) exit_on_panic: bool,
}

impl LogConfig {
    /// `env_prefix` selects `<PREFIX>_LOG_LEVEL` and `<PREFIX>_LOG_FORMAT`; `app_target` is the
    /// crate name whose events follow `<PREFIX>_LOG_LEVEL`.
    pub fn new(env_prefix: &str, app_name: &str, app_target: &str, version: &str) -> Self {
        Self {
            env_prefix: env_prefix.to_owned(),
            app_name: app_name.to_owned(),
            version: version.to_owned(),
            app_targets: vec![app_target.replace('-', "_")],
            directives: Vec::new(),
            exit_on_panic: true,
        }
    }

    /// Adds another crate of the service (e.g. a workspace member) that follows
    /// `<PREFIX>_LOG_LEVEL`.
    pub fn app_target(mut self, target: &str) -> Self {
        self.app_targets.push(target.replace('-', "_"));
        self
    }

    /// Adds a filter directive for a dependency, e.g. `"mdns_sd=off"`. Ignored when `RUST_LOG`
    /// is set, because `RUST_LOG` replaces the whole filter.
    pub fn directive(mut self, directive: &str) -> Self {
        self.directives.push(directive.to_owned());
        self
    }

    /// Whether the panic hook exits the process with code 1 after logging the panic
    /// (default `true`).
    pub fn exit_on_panic(mut self, exit: bool) -> Self {
        self.exit_on_panic = exit;
        self
    }

    pub fn level_variable(&self) -> String {
        format!("{}_LOG_LEVEL", self.env_prefix)
    }

    pub fn format_variable(&self) -> String {
        format!("{}_LOG_FORMAT", self.env_prefix)
    }

    /// Resolves filter and format from the given environment lookup.
    pub(crate) fn resolve(&self, env: impl Fn(&str) -> Option<String>) -> ResolvedLogConfig {
        let mut issues = Vec::new();
        let non_empty = |name: &str| {
            env(name)
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        };

        let level_variable = self.level_variable();
        let level = match non_empty(&level_variable) {
            None => LevelFilter::INFO,
            Some(raw) => match raw.parse::<LevelFilter>() {
                Ok(level) => level,
                Err(_) => {
                    issues.push(SettingIssue {
                        variable: level_variable,
                        value: raw,
                        reason: "expected one of off, error, warn, info, debug, trace".to_owned(),
                    });
                    LevelFilter::INFO
                }
            },
        };

        let filter = match non_empty("RUST_LOG") {
            Some(raw) => match EnvFilter::builder().parse(&raw) {
                Ok(_) => raw,
                Err(err) => {
                    issues.push(SettingIssue {
                        variable: "RUST_LOG".to_owned(),
                        value: raw,
                        reason: err.to_string(),
                    });
                    self.default_filter(level)
                }
            },
            None => self.default_filter(level),
        };

        let format_variable = self.format_variable();
        let format = match non_empty(&format_variable) {
            None => LogFormat::Text,
            Some(raw) => match raw.to_ascii_lowercase().as_str() {
                "text" => LogFormat::Text,
                "json" => LogFormat::Json,
                _ => {
                    issues.push(SettingIssue {
                        variable: format_variable,
                        value: raw,
                        reason: "expected text or json".to_owned(),
                    });
                    LogFormat::Text
                }
            },
        };

        ResolvedLogConfig {
            filter,
            format,
            issues,
        }
    }

    fn default_filter(&self, level: LevelFilter) -> String {
        let mut filter = format!("warn,{LOGGING_TARGET}={level}");
        for target in &self.app_targets {
            filter.push_str(&format!(",{target}={level}"));
        }
        for directive in &self.directives {
            filter.push(',');
            filter.push_str(directive);
        }
        filter
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Text,
    Json,
}

impl fmt::Display for LogFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LogFormat::Text => "text",
            LogFormat::Json => "json",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedLogConfig {
    pub(crate) filter: String,
    pub(crate) format: LogFormat,
    pub(crate) issues: Vec<SettingIssue>,
}

/// A logging variable with an unusable value; the default was used instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SettingIssue {
    pub(crate) variable: String,
    pub(crate) value: String,
    pub(crate) reason: String,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn config() -> LogConfig {
        LogConfig::new("HC_TEST", "hc-test", "hc-test", "1.2.3")
    }

    fn resolve(config: &LogConfig, vars: &[(&str, &str)]) -> ResolvedLogConfig {
        let vars: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        config.resolve(|name| vars.get(name).cloned())
    }

    #[test]
    fn defaults_to_info_for_app_and_warn_for_dependencies() {
        let resolved = resolve(&config(), &[]);
        assert_eq!(resolved.filter, "warn,hc_homie5::logging=info,hc_test=info");
        assert_eq!(resolved.format, LogFormat::Text);
        assert!(resolved.issues.is_empty());
    }

    #[test]
    fn level_variable_sets_app_level() {
        let resolved = resolve(&config(), &[("HC_TEST_LOG_LEVEL", " DEBUG ")]);
        assert_eq!(
            resolved.filter,
            "warn,hc_homie5::logging=debug,hc_test=debug"
        );
    }

    #[test]
    fn app_targets_and_directives_are_added() {
        let config = config().app_target("backend").directive("mdns_sd=off");
        let resolved = resolve(&config, &[("HC_TEST_LOG_LEVEL", "trace")]);
        assert_eq!(
            resolved.filter,
            "warn,hc_homie5::logging=trace,hc_test=trace,backend=trace,mdns_sd=off"
        );
    }

    #[test]
    fn rust_log_overrides_level_and_directives() {
        let config = config().directive("mdns_sd=off");
        let resolved = resolve(
            &config,
            &[
                ("HC_TEST_LOG_LEVEL", "debug"),
                ("RUST_LOG", "info,hc_test=trace"),
            ],
        );
        assert_eq!(resolved.filter, "info,hc_test=trace");
        assert!(resolved.issues.is_empty());
    }

    #[test]
    fn invalid_level_falls_back_to_info_with_issue() {
        let resolved = resolve(&config(), &[("HC_TEST_LOG_LEVEL", "trace,debug,info")]);
        assert_eq!(resolved.filter, "warn,hc_homie5::logging=info,hc_test=info");
        assert_eq!(resolved.issues.len(), 1);
        assert_eq!(resolved.issues[0].variable, "HC_TEST_LOG_LEVEL");
    }

    #[test]
    fn invalid_rust_log_falls_back_to_default_with_issue() {
        let resolved = resolve(
            &config(),
            &[("HC_TEST_LOG_LEVEL", "debug"), ("RUST_LOG", "hc_test=loud")],
        );
        assert_eq!(
            resolved.filter,
            "warn,hc_homie5::logging=debug,hc_test=debug"
        );
        assert_eq!(resolved.issues[0].variable, "RUST_LOG");
    }

    #[test]
    fn empty_values_count_as_unset() {
        let resolved = resolve(&config(), &[("HC_TEST_LOG_LEVEL", ""), ("RUST_LOG", "  ")]);
        assert_eq!(resolved.filter, "warn,hc_homie5::logging=info,hc_test=info");
        assert!(resolved.issues.is_empty());
    }

    #[test]
    fn format_is_text_or_json() {
        assert_eq!(
            resolve(&config(), &[("HC_TEST_LOG_FORMAT", "JSON")]).format,
            LogFormat::Json
        );
        let resolved = resolve(&config(), &[("HC_TEST_LOG_FORMAT", "yaml")]);
        assert_eq!(resolved.format, LogFormat::Text);
        assert_eq!(resolved.issues[0].variable, "HC_TEST_LOG_FORMAT");
    }

    #[test]
    fn crate_names_with_hyphens_are_normalized() {
        let config = LogConfig::new("HC_TEST", "hc-test", "hc-test-app", "1.0.0");
        assert_eq!(
            resolve(&config, &[]).filter,
            "warn,hc_homie5::logging=info,hc_test_app=info"
        );
    }
}
