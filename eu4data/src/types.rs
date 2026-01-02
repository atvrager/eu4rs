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

    /// Adjust the year by delta, clamping to valid range (1-9999).
    pub fn adjust_year(&mut self, delta: i32) {
        self.year = (self.year + delta).clamp(1, 9999);
    }

    /// Adjust the month by delta, wrapping and adjusting year as needed.
    pub fn adjust_month(&mut self, delta: i32) {
        let mut new_month = self.month as i32 + delta;
        while new_month < 1 {
            new_month += 12;
            self.year -= 1;
        }
        while new_month > 12 {
            new_month -= 12;
            self.year += 1;
        }
        self.month = new_month as u8;
        // Clamp day to valid range for new month
        self.day = self.day.min(Self::days_in_month(self.year, self.month));
        self.year = self.year.clamp(1, 9999);
    }

    /// Adjust the day by delta, wrapping and adjusting month as needed.
    pub fn adjust_day(&mut self, delta: i32) {
        let mut new_day = self.day as i32 + delta;
        while new_day < 1 {
            self.adjust_month(-1);
            new_day += Self::days_in_month(self.year, self.month) as i32;
        }
        let days_in_month = Self::days_in_month(self.year, self.month) as i32;
        while new_day > days_in_month {
            new_day -= days_in_month;
            self.adjust_month(1);
        }
        self.day = new_day as u8;
    }

    /// Get the number of days in a given month.
    fn days_in_month(year: i32, month: u8) -> u8 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                    29
                } else {
                    28
                }
            }
            _ => 30,
        }
    }

    /// Get month name.
    pub fn month_name(&self) -> &'static str {
        match self.month {
            1 => "January",
            2 => "February",
            3 => "March",
            4 => "April",
            5 => "May",
            6 => "June",
            7 => "July",
            8 => "August",
            9 => "September",
            10 => "October",
            11 => "November",
            12 => "December",
            _ => "Unknown",
        }
    }

    /// Format as "Day Month" (e.g., "11 November").
    pub fn day_month_str(&self) -> String {
        format!("{} {}", self.day, self.month_name())
    }
}
