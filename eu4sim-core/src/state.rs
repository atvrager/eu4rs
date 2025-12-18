use crate::fixed::Fixed;
use crate::modifiers::{GameModifiers, TradegoodId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

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
pub type WarId = u32;
pub type FleetId = u32;

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
    /// Queued movement path (None if not moving). VecDeque allows O(1) pop_front().
    pub movement_path: Option<VecDeque<ProvinceId>>,
    /// Fleet this army is embarked on (None if on land)
    pub embarked_on: Option<FleetId>,
}

/// Naval transport fleet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fleet {
    pub id: FleetId,
    pub name: String,
    pub owner: Tag,
    pub location: ProvinceId, // Sea province
    /// Transport capacity: 1 ship = 1 regiment
    pub transport_capacity: u32,
    /// Armies currently embarked on this fleet
    pub embarked_armies: Vec<ArmyId>,
    /// Queued movement path (None if not moving). VecDeque allows O(1) pop_front().
    pub movement_path: Option<VecDeque<ProvinceId>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorldState {
    pub date: Date,
    pub rng_seed: u64,
    /// Current RNG state (must be deterministic for replay)
    pub rng_state: u64,
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
    pub fleets: HashMap<FleetId, Fleet>,
    pub next_fleet_id: u32,
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
    /// Whether this province is a sea province (for naval movement)
    pub is_sea: bool,
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

/// Type of diplomatic relationship between two countries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelationType {
    Alliance,
    Rival,
}

/// Active war between countries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct War {
    pub id: WarId,
    pub name: String,
    /// Countries on the attacker's side
    pub attackers: Vec<Tag>,
    /// Countries on the defender's side
    pub defenders: Vec<Tag>,
    /// War start date
    pub start_date: Date,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiplomacyState {
    /// Bilateral relationships: (Tag1, Tag2) -> RelationType
    /// Stored in sorted order (smaller tag first) to avoid duplication
    pub relations: HashMap<(Tag, Tag), RelationType>,
    /// Active wars by ID
    pub wars: HashMap<WarId, War>,
    pub next_war_id: u32,
    /// Military access: (Grantor, Receiver) -> bool
    /// If true, Receiver can move armies through Grantor's territory
    pub military_access: HashMap<(Tag, Tag), bool>,
}

impl DiplomacyState {
    /// Check if two countries are at war with each other.
    pub fn are_at_war(&self, tag1: &str, tag2: &str) -> bool {
        self.wars.values().any(|war| {
            (war.attackers.contains(&tag1.to_string()) && war.defenders.contains(&tag2.to_string()))
                || (war.attackers.contains(&tag2.to_string())
                    && war.defenders.contains(&tag1.to_string()))
        })
    }

    /// Get all wars involving a specific country.
    pub fn get_wars_for_country(&self, tag: &str) -> Vec<&War> {
        self.wars
            .values()
            .filter(|war| {
                war.attackers.contains(&tag.to_string()) || war.defenders.contains(&tag.to_string())
            })
            .collect()
    }

    /// Check if a country has military access to another country's territory.
    pub fn has_military_access(&self, receiver: &str, grantor: &str) -> bool {
        self.military_access
            .get(&(grantor.to_string(), receiver.to_string()))
            .copied()
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalState {
    // HRE, Curia, etc.
}

impl WorldState {
    /// Compute a deterministic checksum of the world state.
    ///
    /// This checksum is used for:
    /// - Desync detection in multiplayer
    /// - Replay validation
    /// - Debugging state divergence
    ///
    /// The checksum is deterministic: identical states produce identical checksums.
    pub fn checksum(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        // Date
        self.date.hash(&mut hasher);

        // RNG state (not seed, as seed is constant)
        self.rng_state.hash(&mut hasher);

        // Countries (sorted by tag for determinism)
        let mut tags: Vec<_> = self.countries.keys().collect();
        tags.sort();
        for tag in tags {
            let c = &self.countries[tag];
            tag.hash(&mut hasher);
            c.treasury.0.hash(&mut hasher);
            c.manpower.0.hash(&mut hasher);
            c.stability.hash(&mut hasher);
            c.prestige.0.hash(&mut hasher);
        }

        // Provinces (sorted by ID)
        let mut province_ids: Vec<_> = self.provinces.keys().collect();
        province_ids.sort();
        for &id in province_ids {
            let p = &self.provinces[&id];
            id.hash(&mut hasher);
            p.owner.hash(&mut hasher);
            p.religion.hash(&mut hasher);
            p.culture.hash(&mut hasher);
            p.trade_goods_id.hash(&mut hasher);
            p.base_production.0.hash(&mut hasher);
            p.base_tax.0.hash(&mut hasher);
            p.base_manpower.0.hash(&mut hasher);
            p.has_fort.hash(&mut hasher);
            p.is_sea.hash(&mut hasher);
        }

        // Armies (sorted by ID)
        let mut army_ids: Vec<_> = self.armies.keys().collect();
        army_ids.sort();
        for &id in army_ids {
            let a = &self.armies[&id];
            id.hash(&mut hasher);
            a.name.hash(&mut hasher);
            a.owner.hash(&mut hasher);
            a.location.hash(&mut hasher);
            a.movement_path.hash(&mut hasher);
            a.embarked_on.hash(&mut hasher);
            for reg in &a.regiments {
                reg.type_.hash(&mut hasher);
                reg.strength.0.hash(&mut hasher);
            }
        }

        // Fleets (sorted by ID)
        let mut fleet_ids: Vec<_> = self.fleets.keys().collect();
        fleet_ids.sort();
        for &id in fleet_ids {
            let f = &self.fleets[&id];
            id.hash(&mut hasher);
            f.name.hash(&mut hasher);
            f.owner.hash(&mut hasher);
            f.location.hash(&mut hasher);
            f.transport_capacity.hash(&mut hasher);
            f.embarked_armies.hash(&mut hasher);
            f.movement_path.hash(&mut hasher);
        }

        // Diplomacy
        // Relations (sorted by key)
        let mut relation_keys: Vec<_> = self.diplomacy.relations.keys().collect();
        relation_keys.sort();
        for key in relation_keys {
            key.hash(&mut hasher);
            self.diplomacy.relations[key].hash(&mut hasher);
        }

        // Wars (sorted by ID)
        let mut war_ids: Vec<_> = self.diplomacy.wars.keys().collect();
        war_ids.sort();
        for &id in war_ids {
            let w = &self.diplomacy.wars[&id];
            id.hash(&mut hasher);
            w.name.hash(&mut hasher);
            w.attackers.hash(&mut hasher);
            w.defenders.hash(&mut hasher);
            w.start_date.hash(&mut hasher);
        }

        // Military access (sorted by key)
        let mut access_keys: Vec<_> = self.diplomacy.military_access.keys().collect();
        access_keys.sort();
        for key in access_keys {
            key.hash(&mut hasher);
            self.diplomacy.military_access[key].hash(&mut hasher);
        }

        hasher.finish()
    }
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

    #[test]
    fn test_checksum_determinism() {
        use crate::testing::WorldStateBuilder;

        // Same state should produce same checksum
        let state = WorldStateBuilder::new()
            .date(1444, 11, 11)
            .with_country("SWE")
            .with_province(1, Some("SWE"))
            .build();

        let checksum1 = state.checksum();
        let checksum2 = state.checksum();

        assert_eq!(
            checksum1, checksum2,
            "Identical states must produce identical checksums"
        );
    }

    #[test]
    fn test_checksum_sensitivity() {
        use crate::testing::WorldStateBuilder;

        // Different states should produce different checksums
        let state1 = WorldStateBuilder::new()
            .date(1444, 11, 11)
            .with_country("SWE")
            .build();

        let state2 = WorldStateBuilder::new()
            .date(1444, 11, 12) // Different date
            .with_country("SWE")
            .build();

        let checksum1 = state1.checksum();
        let checksum2 = state2.checksum();

        assert_ne!(
            checksum1, checksum2,
            "Different states must produce different checksums"
        );
    }
}
