// ── base modules (WASM-safe) ─────────────────────────
pub mod connection;
pub mod model;
pub mod query;
pub mod store;
pub mod util;
pub mod value;

// ── alerts: engine is base, publisher is framework ────
pub mod alerts;

// ── framework modules ────────────────────────────────
#[cfg(feature = "framework")]
pub mod client;
#[cfg(feature = "framework")]
pub mod controller;
#[cfg(feature = "framework")]
pub mod device;
#[cfg(feature = "framework")]
pub mod settings;

// ── tokio modules ────────────────────────────────────
#[cfg(feature = "tokio")]
mod event_multiplexer;

// ── macro re-exports ─────────────────────────────────
#[cfg(feature = "macros")]
pub use hc_homie5_macros::homie_device;
#[cfg(feature = "macros")]
pub use hc_homie5_macros::homie_device_enum;

// paste re-export (used by event_multiplexer macro)
pub use paste;
