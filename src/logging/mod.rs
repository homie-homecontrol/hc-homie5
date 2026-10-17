//! Shared logging conventions and setup for Homecontrol services.
//!
//! The display helpers ([`truncate`], [`chain`]) are always available. The subscriber setup
//! ([`init`], [`LogConfig`]), the panic hook and [`Outage`] need the `logging` feature.
//!
//! A service initializes logging as the first statement of `main`:
//!
//! ```ignore
//! use hc_homie5::logging;
//!
//! logging::init(hc_homie5::log_config!("HC_HM2HOMIE"))?;
//! ```
//!
//! It then reads `<PREFIX>_LOG_LEVEL` (level of the service's own crates, default `info`;
//! every other crate stays at `warn`), `RUST_LOG` (full filter, overrides the level) and
//! `<PREFIX>_LOG_FORMAT` (`text` or `json`).

mod display;
pub use display::*;

#[cfg(feature = "logging")]
mod config;
#[cfg(feature = "logging")]
mod outage;
#[cfg(feature = "logging")]
mod setup;

#[cfg(feature = "logging")]
pub use config::*;
#[cfg(feature = "logging")]
pub use outage::*;
#[cfg(feature = "logging")]
pub use setup::*;

/// Maximum size of a payload logged at ERROR, WARN, INFO or DEBUG.
pub const PAYLOAD_MAX: usize = 512;
/// Maximum size of a raw protocol message or description logged at TRACE.
pub const TRACE_PAYLOAD_MAX: usize = 4096;

/// Builds a [`LogConfig`] for the calling crate.
///
/// Captures the package name, crate name and version of the crate that invokes the macro, so the
/// service's own crate follows `<PREFIX>_LOG_LEVEL`.
#[cfg(feature = "logging")]
#[macro_export]
macro_rules! log_config {
    ($env_prefix:expr) => {
        $crate::logging::LogConfig::new(
            $env_prefix,
            env!("CARGO_PKG_NAME"),
            env!("CARGO_CRATE_NAME"),
            env!("CARGO_PKG_VERSION"),
        )
    };
}
