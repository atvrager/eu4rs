//! Tracy profiling support.
//!
//! This module provides Tracy integration via `tracing-tracy`. When the `tracy`
//! feature is enabled, spans are reported to Tracy for visualization.
//!
//! ## Usage
//!
//! 1. Enable the `tracy` feature: `cargo build -p eu4sim --features tracy`
//! 2. Call [`init_tracy()`] early in main
//! 3. Connect Tracy GUI or capture tool to collect traces
//!
//! Frame markers for daily/monthly ticks are emitted via [`frame_mark_daily`] etc.

/// Trace level for Tracy profiling.
#[derive(Debug, Clone, Copy, Default)]
pub enum TraceLevel {
    /// Only capture INFO level spans (default, lowest overhead)
    #[default]
    Info,
    /// Capture DEBUG level spans (more detail)
    Debug,
    /// Capture TRACE level spans (maximum detail, higher overhead)
    Trace,
}

impl std::str::FromStr for TraceLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "info" => Ok(TraceLevel::Info),
            "debug" => Ok(TraceLevel::Debug),
            "trace" => Ok(TraceLevel::Trace),
            _ => Err(format!(
                "Invalid trace level: {}. Use info, debug, or trace.",
                s
            )),
        }
    }
}

/// Initialize the Tracy tracing subscriber.
///
/// Must be called before any tracing spans are created. Only initializes
/// when the `tracy` feature is enabled; otherwise this is a no-op.
///
/// # Arguments
///
/// * `level` - The minimum trace level to capture. Use `TraceLevel::Trace` for
///   maximum detail (including per-chunk spans in SIMD batches).
///
/// # Panics
///
/// Panics if a global subscriber has already been set.
#[cfg(feature = "tracy")]
pub fn init_tracy(level: TraceLevel) {
    use tracing_subscriber::filter::LevelFilter;
    use tracing_subscriber::prelude::*;

    let filter = match level {
        TraceLevel::Info => LevelFilter::INFO,
        TraceLevel::Debug => LevelFilter::DEBUG,
        TraceLevel::Trace => LevelFilter::TRACE,
    };

    tracing_subscriber::registry()
        .with(tracing_tracy::TracyLayer::default())
        .with(filter)
        .init();
}

/// No-op when tracy feature is disabled.
#[cfg(not(feature = "tracy"))]
pub fn init_tracy(_level: TraceLevel) {}

/// Emit a Tracy frame marker for daily ticks.
///
/// Frame markers create visual boundaries in Tracy's timeline view,
/// useful for identifying tick boundaries in the simulation.
#[cfg(feature = "tracy")]
#[inline]
pub fn frame_mark_daily() {
    tracy_client::secondary_frame_mark!("daily");
}

/// No-op when tracy feature is disabled.
#[cfg(not(feature = "tracy"))]
#[inline]
pub fn frame_mark_daily() {}

/// Emit a Tracy frame marker for monthly ticks.
#[cfg(feature = "tracy")]
#[inline]
pub fn frame_mark_monthly() {
    tracy_client::secondary_frame_mark!("monthly");
}

/// No-op when tracy feature is disabled.
#[cfg(not(feature = "tracy"))]
#[inline]
pub fn frame_mark_monthly() {}
