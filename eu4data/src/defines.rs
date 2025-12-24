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
    // === Existing (kept for backwards compatibility) ===

    /// Base combat power for infantry regiments
    pub const INFANTRY_POWER: f32 = 1.0;

    /// Base combat power for cavalry regiments (EU4: ~1.5x infantry)
    pub const CAVALRY_POWER: f32 = 1.5;

    /// Base combat power for artillery regiments
    pub const ARTILLERY_POWER: f32 = 1.2;

    /// Daily casualty rate during combat (1% per day) - legacy, will be replaced
    pub const DAILY_CASUALTY_RATE: f32 = 0.01;

    /// Standard regiment size in men
    pub const REGIMENT_SIZE: i64 = 1000;

    /// Mil tech required to recruit artillery (EU4: 7)
    /// TODO: Load from game data (common/units/*.txt)
    pub const ARTILLERY_TECH_REQUIRED: u8 = 7;

    // === Phase-Based Combat System ===

    /// Days per combat phase (EU4: 3 days per phase)
    pub const DAYS_PER_PHASE: u8 = 3;

    /// Base combat width at mil tech 0 (EU4: 15)
    pub const BASE_COMBAT_WIDTH: u8 = 15;

    /// Max combat width at highest tech (EU4: ~40)
    pub const MAX_COMBAT_WIDTH: u8 = 40;

    /// Base morale for all units (EU4: 2.0)
    pub const BASE_MORALE: f32 = 2.0;

    // === Unit Fire/Shock Pips ===
    // These represent base damage per phase type

    /// Infantry fire damage (EU4: 0.35 at tech 0)
    pub const INFANTRY_FIRE: f32 = 0.35;
    /// Infantry shock damage (EU4: 0.5 at tech 0)
    pub const INFANTRY_SHOCK: f32 = 0.5;

    /// Cavalry fire damage (cavalry is bad at fire)
    pub const CAVALRY_FIRE: f32 = 0.0;
    /// Cavalry shock damage (EU4: 1.0 - cavalry shines in shock)
    pub const CAVALRY_SHOCK: f32 = 1.0;

    /// Artillery fire damage (EU4: 1.0 at tech 7+)
    pub const ARTILLERY_FIRE: f32 = 1.0;
    /// Artillery shock damage (artillery is bad at shock)
    pub const ARTILLERY_SHOCK: f32 = 0.0;

    // === Damage Calculation ===

    /// Morale damage as fraction of casualties dealt
    pub const MORALE_DAMAGE_MULTIPLIER: f32 = 0.01;

    /// Dice range (0-9 in EU4)
    pub const DICE_MIN: u8 = 0;
    pub const DICE_MAX: u8 = 9;

    // === Terrain Penalties (to attacker dice) ===

    pub const MOUNTAIN_PENALTY: i8 = -2;
    pub const HILLS_PENALTY: i8 = -1;
    pub const FOREST_PENALTY: i8 = -1;
    pub const MARSH_PENALTY: i8 = -1;
    pub const JUNGLE_PENALTY: i8 = -1;
    pub const CROSSING_RIVER_PENALTY: i8 = -1;
    pub const CROSSING_STRAIT_PENALTY: i8 = -2;

    // === Battle Resolution ===

    /// Pursuit casualties multiplier (when enemy routs)
    pub const PURSUIT_MULTIPLIER: f32 = 2.0;

    /// Stackwipe ratio: if winner has >= 10x strength, loser is annihilated
    pub const STACKWIPE_RATIO: f32 = 10.0;

    // === Cavalry Ratio ===
    // Checked per-side during battle: cav / (cav + inf)

    /// Base cavalry ratio limit (50% = cav can be up to 50% of cav+inf in front line)
    pub const BASE_CAVALRY_RATIO: f32 = 0.5;

    /// Tactics penalty for exceeding cavalry ratio (EU4: -25% tactics = ~33% more damage taken)
    pub const CAVALRY_RATIO_PENALTY: f32 = 0.25;

    // === Backrow Mechanics ===

    /// Backrow morale damage fraction (EU4: 40%)
    pub const BACKROW_MORALE_DAMAGE_FRACTION: f32 = 0.4;
}

/// Siege constants
pub mod siege {
    /// Occupation time for unfortified provinces (instant)
    pub const UNFORTIFIED_OCCUPATION_DAYS: u32 = 1;

    /// Days per siege phase (one dice roll)
    pub const SIEGE_PHASE_DAYS: u32 = 30;

    /// Dice roll system (EU4-authentic)
    pub const SIEGE_WIN_THRESHOLD: i32 = 20; // Roll + progress - fort_level >= 20 wins
    pub const MAX_SIEGE_PROGRESS: i32 = 12; // Progress modifier caps here
    pub const DISEASE_OUTBREAK_ROLL: u32 = 1; // Roll of 1 = disease (bad)
    pub const WALL_BREACH_ROLL: u32 = 14; // Roll of 14 = breach

    /// Bonuses
    pub const ARTILLERY_BONUS_MAX: i32 = 5; // +5 max from cannons
    pub const ARTILLERY_PER_BONUS: u32 = 1; // 1 artillery = +1 (up to max)
    pub const GENERAL_SIEGE_PIP_BONUS: i32 = 1; // +1 per siege pip
    pub const BLOCKADE_BONUS: i32 = 1; // +1 if coastal and blockaded

    /// Garrison
    pub const GARRISON_BASE_SIZE: u32 = 1000; // Per fort level
    pub const GARRISON_SURRENDER_THRESHOLD: u32 = 100; // Garrison below 100 = surrender
    pub const STARVATION_MONTHLY_LOSS_PERCENT: f32 = 10.0; // 10% garrison loss if blockaded

    /// Regiment requirements
    pub const REGIMENTS_PER_FORT_LEVEL: u32 = 3; // Need fort_level * 3 regiments
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

/// Attrition constants
pub mod attrition {
    /// Supply limit: 1 regiment per 1 total development
    pub const BASE_SUPPLY_LIMIT_PER_DEV: f32 = 1.0;

    /// Base monthly attrition when over supply limit
    pub const ATTRITION_BASE_PERCENT: f32 = 1.0;

    /// Multiplier for over-limit ratio (5% per 100% over limit)
    pub const ATTRITION_OVER_LIMIT_MULTIPLIER: f32 = 5.0;

    /// Additional attrition in winter months (Dec, Jan, Feb)
    pub const WINTER_ATTRITION_BONUS: f32 = 2.0;

    /// Additional attrition in hostile (enemy) territory
    pub const HOSTILE_ATTRITION: f32 = 1.0;
}

/// Naval combat constants
pub mod naval {
    /// Days per naval combat phase (EU4: 3 days per phase, same as land)
    pub const DAYS_PER_PHASE: u8 = 3;

    /// Base durability for all ships (EU4: 1.0)
    pub const BASE_DURABILITY: f32 = 1.0;

    // === Ship Combat Values ===
    // Heavy ships (best in open sea)
    pub const HEAVY_SHIP_FIRE: f32 = 1.0;
    pub const HEAVY_SHIP_SHOCK: f32 = 0.0;
    pub const HEAVY_SHIP_HULL_SIZE: u32 = 50; // Base hull size

    // Light ships (trade protection, weak combat)
    pub const LIGHT_SHIP_FIRE: f32 = 0.2;
    pub const LIGHT_SHIP_SHOCK: f32 = 0.1;
    pub const LIGHT_SHIP_HULL_SIZE: u32 = 10;

    // Galleys (best in inland seas)
    pub const GALLEY_FIRE: f32 = 0.1;
    pub const GALLEY_SHOCK: f32 = 0.8;
    pub const GALLEY_HULL_SIZE: u32 = 20;

    // Transports (no combat value)
    pub const TRANSPORT_FIRE: f32 = 0.0;
    pub const TRANSPORT_SHOCK: f32 = 0.0;
    pub const TRANSPORT_HULL_SIZE: u32 = 5;

    /// Durability damage as fraction of hull damage dealt
    pub const DURABILITY_DAMAGE_MULTIPLIER: f32 = 0.01;

    /// Dice range (0-9 in EU4, same as land combat)
    pub const DICE_MIN: u8 = 0;
    pub const DICE_MAX: u8 = 9;
}
