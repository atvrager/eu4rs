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

    /// Vanilla EU4's default start year range (for fallback if no bookmarks loaded).
    /// Extended Timeline and other mods may have different ranges derived from their bookmarks.
    pub const VANILLA_MIN_YEAR: i32 = 1444;
    pub const VANILLA_MAX_YEAR: i32 = 1821;

    /// Adjust the year by delta, clamping to a provided range.
    ///
    /// Use `get_year_range_from_bookmarks()` to derive the range from loaded bookmarks,
    /// which supports mods like Extended Timeline.
    pub fn adjust_year(&mut self, delta: i32, min_year: i32, max_year: i32) {
        self.year = (self.year + delta).clamp(min_year, max_year);
    }

    /// Set the year directly, clamping to a provided range.
    ///
    /// Returns true if the year was valid (within range), false if it was clamped.
    pub fn set_year(&mut self, year: i32, min_year: i32, max_year: i32) -> bool {
        let clamped = year.clamp(min_year, max_year);
        self.year = clamped;
        clamped == year
    }

    /// Try to parse and set the year from a string, with validation against a range.
    ///
    /// Returns Ok(true) if parsed and within range,
    /// Ok(false) if parsed but clamped to range,
    /// Err if the string is not a valid number.
    pub fn try_set_year_from_str(
        &mut self,
        s: &str,
        min_year: i32,
        max_year: i32,
    ) -> Result<bool, std::num::ParseIntError> {
        let year = s.parse::<i32>()?;
        Ok(self.set_year(year, min_year, max_year))
    }

    /// Adjust the month by delta, wrapping and adjusting year as needed.
    ///
    /// NOTE: Year wrapping uses a wide range (1-9999). For UI validation,
    /// use `adjust_year()` with bookmark-derived min/max after this call.
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
        // Prevent absurd years, but don't enforce bookmark-specific range here
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
