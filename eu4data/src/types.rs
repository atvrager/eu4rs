//! Public type definitions for EU4 game data.
//!
//! This module provides a clean public API for accessing EU4 data types.
//! Types are either:
//! - Auto-generated from schema (re-exported from `generated::types`)
//! - Manually implemented for complex cases
//!
//! To override a generated type, replace the shim file with a manual implementation.

// Re-exports for convenience (optional, but good for flat API eu4data::types::Technology)
pub use crate::generated::types::*;

/// EU4 date in year.month.day format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct Eu4Date {
    year: i32,
    month: u8,
    day: u8,
}

impl Eu4Date {
    /// Create a new EU4 date.
    pub fn from_ymd(year: i32, month: u8, day: u8) -> Self {
        Self { year, month, day }
    }

    /// Get the year.
    pub fn year(&self) -> i32 {
        self.year
    }

    /// Get the month (1-12).
    pub fn month(&self) -> u8 {
        self.month
    }

    /// Get the day (1-31).
    pub fn day(&self) -> u8 {
        self.day
    }
}
