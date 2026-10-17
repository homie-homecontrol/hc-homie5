use std::backtrace::{Backtrace, BacktraceStatus};
use std::io::IsTerminal;
use std::panic::PanicHookInfo;

use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use super::{LogConfig, LogFormat};

#[derive(Debug, thiserror::Error)]
pub enum LogInitError {
    #[error("invalid log filter: {0}")]
    Filter(#[from] tracing_subscriber::filter::ParseError),
    #[error("cannot install the log subscriber: {0}")]
    Subscriber(#[from] tracing_subscriber::util::TryInitError),
}

/// Installs the global subscriber and the panic hook, then logs `starting`.
///
/// Reads `<PREFIX>_LOG_LEVEL`, `RUST_LOG` and `<PREFIX>_LOG_FORMAT`. Text output uses ANSI colors
/// only when stdout is a terminal and `NO_COLOR` is unset. Every line carries target and
/// file:line. Records from crates that use `log` are forwarded.
pub fn init(config: LogConfig) -> Result<(), LogInitError> {
    let resolved = config.resolve(|name| std::env::var(name).ok());
    let filter = EnvFilter::builder().parse(&resolved.filter)?;
    let registry = tracing_subscriber::registry().with(filter);
    match resolved.format {
        LogFormat::Text => {
            let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
            let ansi = std::io::stdout().is_terminal() && !no_color;
            registry
                .with(
                    fmt::layer()
                        .with_ansi(ansi)
                        .with_file(true)
                        .with_line_number(true),
                )
                .try_init()?;
        }
        LogFormat::Json => {
            registry
                .with(
                    fmt::layer()
                        .json()
                        .flatten_event(true)
                        .with_file(true)
                        .with_line_number(true),
                )
                .try_init()?;
        }
    }

    let exit_on_panic = config.exit_on_panic;
    std::panic::set_hook(Box::new(move |info| {
        log_panic(info);
        if exit_on_panic {
            std::process::exit(1);
        }
    }));

    tracing::info!(
        app = %config.app_name,
        version = %config.version,
        log_filter = %resolved.filter,
        log_format = %resolved.format,
        "starting"
    );
    for issue in &resolved.issues {
        tracing::warn!(
            variable = %issue.variable,
            value = %issue.value,
            reason = %issue.reason,
            "invalid log setting; using the default"
        );
    }
    Ok(())
}

/// Logs a panic as one ERROR event with message, location and thread. Includes the backtrace
/// when `RUST_BACKTRACE` enables it.
pub fn log_panic(info: &PanicHookInfo<'_>) {
    let message = info
        .payload()
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
        .unwrap_or("non-string panic payload");
    let location = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_default();
    let thread = std::thread::current();
    let thread = thread.name().unwrap_or("unnamed");
    let backtrace = Backtrace::capture();
    if backtrace.status() == BacktraceStatus::Captured {
        tracing::error!(error = %message, location = %location, thread, backtrace = ?backtrace, "panic");
    } else {
        tracing::error!(error = %message, location = %location, thread, "panic");
    }
}
