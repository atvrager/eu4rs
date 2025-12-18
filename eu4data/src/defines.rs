//! Game mechanic constants (defines).
//!
//! These correspond to values in EU4's `common/defines/00_defines.lua`.
//! Values are hardcoded to EU4 1.35+ mechanics for the simulation.

/// Manpower constants
pub mod manpower {
    /// Men per point of base manpower development (EU4: 1000)
    pub const MEN_PER_DEV: i64 = 1000;

    /// Base manpower pool for all countries (EU4: ~10000)
    pub const BASE_MANPOWER: i64 = 10000;

    /// Months to recover from 0 to max manpower (EU4: 120 = 10 years)
    pub const RECOVERY_MONTHS: i64 = 120;
}

/// Combat constants
pub mod combat {
    /// Base combat power for infantry regiments
    pub const INFANTRY_POWER: f32 = 1.0;

    /// Base combat power for cavalry regiments (EU4: ~1.5x infantry)
    pub const CAVALRY_POWER: f32 = 1.5;

    /// Base combat power for artillery regiments
    pub const ARTILLERY_POWER: f32 = 1.2;

    /// Daily casualty rate during combat (1% per day)
    pub const DAILY_CASUALTY_RATE: f32 = 0.01;

    /// Standard regiment size in men
    pub const REGIMENT_SIZE: i64 = 1000;
}

/// Economy constants
pub mod economy {
    /// Goods produced per point of base production (EU4: 0.2)
    pub const BASE_PRODUCTION_MULTIPLIER: f32 = 0.2;

    /// Base army maintenance cost per regiment per month (ducats)
    pub const BASE_ARMY_COST: f32 = 0.2;

    /// Base fort maintenance cost per month (ducats)
    pub const BASE_FORT_COST: f32 = 1.0;

    /// Months in a year for tax calculations
    pub const MONTHS_PER_YEAR: i64 = 12;
}
