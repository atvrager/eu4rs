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

/// Initialize the Tracy tracing subscriber.
///
/// Must be called before any tracing spans are created. Only initializes
/// when the `tracy` feature is enabled; otherwise this is a no-op.
///
/// # Panics
///
/// Panics if a global subscriber has already been set.
#[cfg(feature = "tracy")]
pub fn init_tracy() {
    use tracing_subscriber::prelude::*;

    tracing_subscriber::registry()
        .with(tracing_tracy::TracyLayer::default())
        .init();
}

/// No-op when tracy feature is disabled.
#[cfg(not(feature = "tracy"))]
pub fn init_tracy() {}

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
