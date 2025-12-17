use crate::fixed::Fixed;
use crate::modifiers::{GameModifiers, TradegoodId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A specific date in history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Date {
    pub year: i32,
    pub month: u8, // 1-12
    pub day: u8,   // 1-31 (approx, EU4 uses fixed days usually)
}

impl Date {
    pub fn new(year: i32, month: u8, day: u8) -> Self {
        Self { year, month, day }
    }

    /// Adds days to the current date.
    /// Simplified calendar: 12 months of 30 days is common in PDX games internally,
    /// but we'll implement a basic Gregorian-ish tick for now or simpler.
    /// EU4 actually uses standard calendar but simplified logic sometimes.
    /// We'll assume a simple increment for the prototype.
    pub fn add_days(&self, days: u32) -> Self {
        // Very naive implementation for prototype
        let mut d = self.day as u32 + days;
        let mut m = self.month as u32;
        let mut y = self.year;

        while d > 30 {
            d -= 30;
            m += 1;
            if m > 12 {
                m -= 12;
                y += 1;
            }
        }

        Self {
            year: y,
            month: m as u8,
            day: d as u8,
        }
    }
}

impl Default for Date {
    fn default() -> Self {
        Self::new(1444, 11, 11)
    }
}

impl std::fmt::Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.year, self.month, self.day)
    }
}

pub type Tag = String;
pub type ProvinceId = u32;
pub type ArmyId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RegimentType {
    Infantry,
    Cavalry,
    Artillery,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Regiment {
    pub type_: RegimentType,
    /// Number of men (e.g. 1000.0)
    pub strength: Fixed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Army {
    pub id: ArmyId,
    pub name: String,
    pub owner: Tag,
    pub location: ProvinceId,
    pub regiments: Vec<Regiment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorldState {
    pub date: Date,
    pub rng_seed: u64,
    pub provinces: HashMap<ProvinceId, ProvinceState>,
    pub countries: HashMap<Tag, CountryState>,
    /// Base prices for trade goods (loaded from data model).
    pub base_goods_prices: HashMap<TradegoodId, Fixed>,
    /// Dynamic modifiers (mutated by events).
    pub modifiers: GameModifiers,
    pub diplomacy: DiplomacyState,
    pub global: GlobalState,
    pub armies: HashMap<ArmyId, Army>,
    pub next_army_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProvinceState {
    pub owner: Option<Tag>,
    pub religion: Option<String>,
    pub culture: Option<String>,
    /// Trade good produced by this province
    pub trade_goods_id: Option<TradegoodId>,
    /// Base production development (Fixed for determinism)
    pub base_production: Fixed,
    /// Base tax development
    pub base_tax: Fixed,
    /// Base manpower development
    pub base_manpower: Fixed,
    /// Has a level 2 fort (Castle)
    pub has_fort: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CountryState {
    /// Treasury balance (Fixed for determinism)
    pub treasury: Fixed,
    /// Available manpower pool
    pub manpower: Fixed,
    pub stability: i8,
    pub prestige: Fixed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiplomacyState {
    // Relationships, wars, alliances
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalState {
    // HRE, Curia, etc.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_simple_add() {
        let d = Date::new(1444, 1, 1);
        let d2 = d.add_days(1);
        assert_eq!(d2, Date::new(1444, 1, 2));
    }

    #[test]
    fn test_date_month_rollover() {
        let d = Date::new(1444, 1, 30);
        let d2 = d.add_days(1);
        // Naive 30-day months logic:
        assert_eq!(d2, Date::new(1444, 2, 1));
    }

    #[test]
    fn test_date_year_rollover() {
        let d = Date::new(1444, 12, 30);
        let d2 = d.add_days(1);
        assert_eq!(d2, Date::new(1445, 1, 1));
    }

    #[test]
    fn test_date_multi_month_add() {
        let d = Date::new(1444, 1, 1);
        let d2 = d.add_days(65); // 2 months + 5 days
                                 // 1.1 + 65 -> 3.5 (assuming 30d months: 1.1->2.1 (+30)->3.1 (+30)->3.6 (+5)
                                 // Wait, math: 1 + 65 = 66
                                 // 66 - 30 = 36 (m=2)
                                 // 36 - 30 = 6 (m=3)
                                 // Result: 1444.3.6
        assert_eq!(d2, Date::new(1444, 3, 6));
    }
}
