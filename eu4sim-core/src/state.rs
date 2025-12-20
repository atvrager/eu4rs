use crate::bounded::{new_prestige, new_stability, new_tradition, BoundedFixed, BoundedInt};
use crate::fixed::Fixed;
use crate::modifiers::{GameModifiers, TradegoodId};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};

pub use im::HashMap;

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
    ///
    /// **Calendar Simplification**: We currently use a simplified calendar with
    /// uniform 30-day months (360-day year).
    ///
    /// This differs from EU4's Gregorian-ish calendar but simplifies simulation math.
    /// Dates will drift relative to historical events over time.
    /// This is an intentional prototype decision.
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
pub struct MovementState {
    /// Provinces left to visit. Front is next destination.
    pub path: VecDeque<ProvinceId>,
    /// Accumulated movement progress towards the next province (0.0 to 1.0 or similar).
    /// Typically maps to days traveled.
    pub progress: Fixed,
    /// Total cost (in days/points) required to enter the next province using current speed.
    pub required_progress: Fixed,
}

impl Hash for MovementState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state);
        self.progress.0.hash(state);
        self.required_progress.0.hash(state);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Army {
    pub id: ArmyId,
    pub name: String,
    pub owner: Tag,
    pub location: ProvinceId,
    pub regiments: Vec<Regiment>,
    /// Active movement state (None if stationary).
    pub movement: Option<MovementState>,
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
    /// Active movement state (None if stationary).
    pub movement: Option<MovementState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Colony {
    pub province: ProvinceId,
    pub owner: Tag,
    /// Current number of settlers (0 to 1000)
    pub settlers: u32,
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
    pub colonies: HashMap<ProvinceId, Colony>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Terrain {
    Plains,
    Farmlands,
    Hills,
    Mountains,
    Forest,
    Marsh,
    Jungle,
    Desert,
    Sea,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProvinceState {
    pub owner: Option<Tag>,
    /// Current controller (differs from owner when occupied in war)
    pub controller: Option<Tag>,
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
    /// Terrain type (e.g., "plains", "mountains", "forest")
    pub terrain: Option<Terrain>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountryState {
    /// Treasury balance (Fixed for determinism)
    pub treasury: Fixed,
    /// Available manpower pool
    /// Available manpower pool
    pub manpower: Fixed,
    /// Country stability (-3 to +3)
    pub stability: BoundedInt,
    /// Country prestige (-100 to +100), decays toward 0
    pub prestige: BoundedFixed,
    /// Army tradition (0 to 100), decays toward 0
    pub army_tradition: BoundedFixed,
    /// Administrative monarch power
    pub adm_mana: Fixed,
    /// Diplomatic monarch power
    pub dip_mana: Fixed,
    /// Military monarch power
    pub mil_mana: Fixed,
}

impl Default for CountryState {
    fn default() -> Self {
        Self {
            treasury: Fixed::ZERO,
            manpower: Fixed::ZERO,
            stability: new_stability(),
            prestige: new_prestige(),
            army_tradition: new_tradition(),
            adm_mana: Fixed::ZERO,
            dip_mana: Fixed::ZERO,
            mil_mana: Fixed::ZERO,
        }
    }
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
    /// War score for attacker side (0-100)
    pub attacker_score: u8,
    /// War score from battles only (capped at 40)
    pub attacker_battle_score: u8,
    /// War score for defender side (0-100)
    pub defender_score: u8,
    /// War score from battles only (capped at 40)
    pub defender_battle_score: u8,
    /// Pending peace offer (if any)
    pub pending_peace: Option<PendingPeace>,
}

/// A pending peace offer in a war.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PendingPeace {
    /// True if attacker is offering, false if defender
    pub from_attacker: bool,
    /// The terms being offered
    pub terms: PeaceTerms,
    /// Date the offer was made
    pub offered_on: Date,
}

/// Terms of a peace deal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PeaceTerms {
    /// No territorial changes
    WhitePeace,
    /// Transfer specific provinces to the victor
    TakeProvinces { provinces: Vec<ProvinceId> },
    /// Complete annexation of the defeated country
    FullAnnexation,
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

/// Terrain movement cost multipliers (base cost = 10 days)
fn terrain_cost_multiplier(terrain: Option<Terrain>) -> u32 {
    match terrain {
        Some(Terrain::Mountains) => 20, // 2.0x
        Some(Terrain::Hills) | Some(Terrain::Marsh) | Some(Terrain::Jungle) => 15, // 1.5x
        Some(Terrain::Forest) | Some(Terrain::Desert) => 12, // 1.2x
        Some(Terrain::Sea) => 5,        // 0.5x (naval)
        _ => 10,                        // plains, farmlands, default
    }
}

impl eu4data::adjacency::CostCalculator for WorldState {
    fn calculate_cost(&self, _from: ProvinceId, to: ProvinceId) -> u32 {
        // Look up destination terrain and return cost
        self.provinces
            .get(&to)
            .map(|p| terrain_cost_multiplier(p.terrain))
            .unwrap_or(10)
    }

    fn calculate_heuristic(&self, _from: ProvinceId, _to: ProvinceId) -> u32 {
        // Dijkstra (0 heuristic) until we load province centroids for Euclidean distance.
        0
    }
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
            c.prestige.hash(&mut hasher);
            c.army_tradition.hash(&mut hasher);
            c.adm_mana.0.hash(&mut hasher);
            c.dip_mana.0.hash(&mut hasher);
            c.mil_mana.0.hash(&mut hasher);
        }

        // Provinces (sorted by ID)
        let mut province_ids: Vec<_> = self.provinces.keys().collect();
        province_ids.sort();
        for &id in province_ids {
            let p = &self.provinces[&id];
            id.hash(&mut hasher);
            p.owner.hash(&mut hasher);
            p.controller.hash(&mut hasher);
            p.religion.hash(&mut hasher);
            p.culture.hash(&mut hasher);
            p.trade_goods_id.hash(&mut hasher);
            p.base_production.0.hash(&mut hasher);
            p.base_tax.0.hash(&mut hasher);
            p.base_manpower.0.hash(&mut hasher);
            p.has_fort.hash(&mut hasher);
            p.is_sea.hash(&mut hasher);
            p.terrain.hash(&mut hasher);
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
            a.movement.hash(&mut hasher);
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
            f.movement.hash(&mut hasher);
        }

        // Colonies (sorted by province ID)
        let mut colony_ids: Vec<_> = self.colonies.keys().collect();
        colony_ids.sort();
        for &id in colony_ids {
            let c = &self.colonies[&id];
            id.hash(&mut hasher);
            c.owner.hash(&mut hasher);
            c.settlers.hash(&mut hasher);
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
            w.attacker_score.hash(&mut hasher);
            w.attacker_battle_score.hash(&mut hasher);
            w.defender_score.hash(&mut hasher);
            w.defender_battle_score.hash(&mut hasher);
            // Note: pending_peace intentionally excluded from checksum
            // (offers are transient and don't affect simulation state)
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
