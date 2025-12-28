use crate::bounded::{new_prestige, new_stability, new_tradition, BoundedFixed, BoundedInt};
use crate::fixed::Fixed;
use crate::modifiers::{GameModifiers, TradegoodId};
use crate::trade::{
    CountryTradeState, ProvinceTradeState, TradeNodeId, TradeNodeState, TradeTopology,
};
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

    /// Calculates total days from an epoch (1444.01.01) using simplified 30-day months.
    /// Used for determining tick counts and relative time differences.
    pub fn days_from_epoch(&self) -> i64 {
        let years_since = self.year as i64 - 1444;
        let months_since = self.month as i64 - 1;
        let days_since = self.day as i64 - 1;
        years_since * 360 + months_since * 30 + days_since
    }

    /// Adds years to the current date.
    pub fn add_years(&self, years: i32) -> Self {
        Self {
            year: self.year + years,
            month: self.month,
            day: self.day,
        }
    }

    /// Calculate months elapsed since another date.
    /// Uses 30-day months for simplicity.
    pub fn months_since(&self, other: &Date) -> i32 {
        let self_days = self.days_from_epoch();
        let other_days = other.days_from_epoch();
        ((self_days - other_days) / 30) as i32
    }
}

impl Default for Date {
    fn default() -> Self {
        Self::new(1444, 11, 11)
    }
}

impl std::fmt::Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{:02}.{:02}", self.year, self.month, self.day)
    }
}

pub type Tag = String;
pub type ProvinceId = u32;
pub type ArmyId = u32;
pub type WarId = u32;
pub type FleetId = u32;
pub type GeneralId = u32;
pub type AdmiralId = u32;
pub type BattleId = u32;
pub type NavalBattleId = u32;
pub type SiegeId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RegimentType {
    Infantry,
    Cavalry,
    Artillery,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Regiment {
    pub type_: RegimentType,
    /// Number of men (0-1000)
    pub strength: Fixed,
    /// Current morale (0.0 to country's max morale, base ~2.0)
    /// Depletes during combat; at 0, army routs.
    pub morale: Fixed,
}

/// Naval unit types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum ShipType {
    HeavyShip, // Best in open sea, expensive
    LightShip, // Trade protection, weak combat
    Galley,    // Best in inland seas, cheap
    Transport, // Troop transport, no combat value
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ship {
    pub type_: ShipType,
    /// Hull integrity (0-100% of hull size)
    pub hull: Fixed,
    /// Current durability (0.0 to base durability)
    /// Depletes during combat; at 0, ship sinks.
    pub durability: Fixed,
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
    /// Previous location (for river crossing detection in combat)
    pub previous_location: Option<ProvinceId>,
    pub regiments: Vec<Regiment>,
    /// Active movement state (None if stationary).
    pub movement: Option<MovementState>,
    /// Fleet this army is embarked on (None if on land)
    pub embarked_on: Option<FleetId>,
    /// Assigned general (if any)
    pub general: Option<GeneralId>,
    /// Active battle this army is participating in (if any)
    pub in_battle: Option<BattleId>,
    /// Cached regiment counts by type (updated via recompute_counts)
    #[serde(default)]
    pub infantry_count: u32,
    #[serde(default)]
    pub cavalry_count: u32,
    #[serde(default)]
    pub artillery_count: u32,
}

impl Army {
    /// Create a new army with correct cached counts computed from regiments.
    pub fn new(
        id: ArmyId,
        name: String,
        owner: Tag,
        location: ProvinceId,
        regiments: Vec<Regiment>,
    ) -> Self {
        let (inf, cav, art) = regiments
            .iter()
            .fold((0, 0, 0), |(i, c, a), r| match r.type_ {
                RegimentType::Infantry => (i + 1, c, a),
                RegimentType::Cavalry => (i, c + 1, a),
                RegimentType::Artillery => (i, c, a + 1),
            });
        Self {
            id,
            name,
            owner,
            location,
            previous_location: None,
            regiments,
            movement: None,
            embarked_on: None,
            general: None,
            in_battle: None,
            infantry_count: inf,
            cavalry_count: cav,
            artillery_count: art,
        }
    }

    /// Recompute cached regiment counts from the regiments vec.
    /// Call this after any modification to regiments.
    pub fn recompute_counts(&mut self) {
        let mut inf = 0u32;
        let mut cav = 0u32;
        let mut art = 0u32;
        for reg in &self.regiments {
            match reg.type_ {
                RegimentType::Infantry => inf += 1,
                RegimentType::Cavalry => cav += 1,
                RegimentType::Artillery => art += 1,
            }
        }
        self.infantry_count = inf;
        self.cavalry_count = cav;
        self.artillery_count = art;
    }

    /// Returns (infantry, cavalry, artillery) counts.
    #[inline]
    pub fn composition(&self) -> (u32, u32, u32) {
        (
            self.infantry_count,
            self.cavalry_count,
            self.artillery_count,
        )
    }

    /// Total regiment count.
    #[inline]
    pub fn regiment_count(&self) -> u32 {
        self.infantry_count + self.cavalry_count + self.artillery_count
    }
}

/// Naval transport fleet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fleet {
    pub id: FleetId,
    pub name: String,
    pub owner: Tag,
    pub location: ProvinceId, // Sea province
    /// Ships in this fleet
    pub ships: Vec<Ship>,
    /// Armies currently embarked on this fleet
    pub embarked_armies: Vec<ArmyId>,
    /// Active movement state (None if stationary).
    pub movement: Option<MovementState>,
    /// Assigned admiral (if any)
    pub admiral: Option<AdmiralId>,
    /// Active naval battle this fleet is participating in (if any)
    pub in_battle: Option<NavalBattleId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Colony {
    pub province: ProvinceId,
    pub owner: Tag,
    /// Current number of settlers (0 to 1000)
    pub settlers: u32,
}

// =========================================================================
// Combat System Types
// =========================================================================

/// A military leader with combat pips.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct General {
    pub id: GeneralId,
    pub name: String,
    pub owner: Tag,
    /// Fire phase pip bonus (0-6)
    pub fire: u8,
    /// Shock phase pip bonus (0-6)
    pub shock: u8,
    /// Maneuver pip (affects terrain penalty negation, pursuit)
    pub maneuver: u8,
    /// Siege pip (not used in field battles)
    pub siege: u8,
}

/// A naval leader with combat pips.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Admiral {
    pub id: AdmiralId,
    pub name: String,
    pub owner: Tag,
    /// Fire phase pip bonus (0-6)
    pub fire: u8,
    /// Shock phase pip bonus (0-6)
    pub shock: u8,
    /// Maneuver pip (affects naval engagement/pursuit)
    pub maneuver: u8,
    /// Siege pip (blockade effectiveness)
    pub siege: u8,
}

/// Combat phase: 3 days each, alternating Fire → Shock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CombatPhase {
    #[default]
    Fire,
    Shock,
}

/// Deployment of regiments in a battle.
/// Tracks which regiments are in front/back row.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BattleLine {
    /// Front row slots: (army_id, regiment_index within that army)
    /// Up to combat width. Slots can be None if unit died.
    pub front: Vec<Option<(ArmyId, usize)>>,
    /// Back row: artillery + overflow
    pub back: Vec<(ArmyId, usize)>,
    /// Reserve armies waiting to reinforce
    pub reserves: Vec<ArmyId>,
}

/// Result of a completed battle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BattleResult {
    AttackerVictory {
        pursuit_casualties: u32,
        stackwiped: bool,
    },
    DefenderVictory {
        pursuit_casualties: u32,
        stackwiped: bool,
    },
    /// Both sides broke simultaneously (very rare)
    Draw,
}

/// An active battle between opposing armies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Battle {
    pub id: BattleId,
    pub province: ProvinceId,
    /// Province where the attacker came from (for river crossing penalty)
    pub attacker_origin: Option<ProvinceId>,
    pub start_date: Date,
    /// Current day within the current phase (0, 1, 2)
    pub phase_day: u8,
    /// Current phase (Fire or Shock)
    pub phase: CombatPhase,
    /// Dice roll for attacker this phase (set on phase start)
    pub attacker_dice: u8,
    /// Dice roll for defender this phase (set on phase start)
    pub defender_dice: u8,
    /// Attacker side armies
    pub attackers: Vec<ArmyId>,
    /// Defender side armies
    pub defenders: Vec<ArmyId>,
    /// Attacker battle line deployment
    pub attacker_line: BattleLine,
    /// Defender battle line deployment
    pub defender_line: BattleLine,
    /// Accumulated attacker casualties (for war score)
    pub attacker_casualties: u32,
    /// Accumulated defender casualties (for war score)
    pub defender_casualties: u32,
    /// Battle result (Some when resolved)
    pub result: Option<BattleResult>,
}

/// An active naval battle between opposing fleets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavalBattle {
    pub id: NavalBattleId,
    pub sea_zone: ProvinceId,
    pub start_date: Date,
    /// Current day within the current phase (0, 1, 2)
    pub phase_day: u8,
    /// Current phase (Fire or Shock)
    pub phase: CombatPhase,
    /// Dice roll for attacker this phase (set on phase start)
    pub attacker_dice: u8,
    /// Dice roll for defender this phase (set on phase start)
    pub defender_dice: u8,
    /// Attacker side fleets
    pub attackers: Vec<FleetId>,
    /// Defender side fleets
    pub defenders: Vec<FleetId>,
    /// Accumulated attacker ship losses (for war score)
    pub attacker_losses: u32,
    /// Accumulated defender ship losses (for war score)
    pub defender_losses: u32,
    /// Battle result (Some when resolved)
    pub result: Option<BattleResult>,
}

/// An active siege of a fortified province.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Siege {
    pub id: SiegeId,
    pub province: ProvinceId,
    pub attacker: Tag,
    /// Original defender (controller when siege started)
    pub defender: Tag,
    pub besieging_armies: Vec<ArmyId>,
    /// Fort level being sieged (1-8)
    pub fort_level: u8,
    /// Current garrison troops
    pub garrison: u32,
    /// Progress modifier (0-12, increases each failed phase)
    pub progress_modifier: i32,
    /// Days since last dice roll
    pub days_in_phase: u32,
    pub start_date: Date,
    /// Adjacent sea controlled by enemy (affects garrison starvation)
    pub is_blockaded: bool,
    /// Wall breach (roll 14) - enables assault
    pub breached: bool,
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

    // =========================================================================
    // Combat System
    // =========================================================================
    /// All generals in the game
    pub generals: HashMap<GeneralId, General>,
    pub next_general_id: GeneralId,
    /// All admirals in the game
    pub admirals: HashMap<AdmiralId, Admiral>,
    pub next_admiral_id: AdmiralId,
    /// Active land battles
    pub battles: HashMap<BattleId, Battle>,
    pub next_battle_id: BattleId,
    /// Active naval battles
    pub naval_battles: HashMap<NavalBattleId, NavalBattle>,
    pub next_naval_battle_id: NavalBattleId,
    /// Active sieges
    pub sieges: HashMap<ProvinceId, Siege>,
    pub next_siege_id: SiegeId,

    // =========================================================================
    // Trade System
    // =========================================================================
    /// Runtime state for each trade node (updated monthly).
    pub trade_nodes: HashMap<TradeNodeId, TradeNodeState>,

    /// Province to trade node mapping (which node a province belongs to).
    pub province_trade_node: HashMap<ProvinceId, TradeNodeId>,

    /// Cached topological order for trade propagation (computed once at init).
    #[serde(skip)]
    pub trade_topology: TradeTopology,

    // =========================================================================
    // Building System
    // =========================================================================
    /// Building name to ID mapping (for save hydration and command parsing).
    #[serde(skip)]
    pub building_name_to_id: HashMap<String, crate::modifiers::BuildingId>,

    /// Building definitions (loaded from game files, immutable).
    #[serde(skip)]
    pub building_defs: HashMap<crate::modifiers::BuildingId, crate::buildings::BuildingDef>,

    /// Reverse lookup: which building is this one replaced by?
    /// E.g., Temple -> Cathedral means building_upgraded_by[temple] = cathedral.
    #[serde(skip)]
    pub building_upgraded_by: HashMap<crate::modifiers::BuildingId, crate::modifiers::BuildingId>,

    /// Subject type definitions (loaded from common/subject_types/, immutable).
    #[serde(skip)]
    pub subject_types: crate::subjects::SubjectTypeRegistry,

    /// Idea group definitions (loaded from common/ideas/, immutable).
    #[serde(skip)]
    pub idea_groups: crate::ideas::IdeaGroupRegistry,

    /// Policy definitions (loaded from common/policies/, immutable).
    #[serde(skip)]
    pub policies: crate::systems::PolicyRegistry,

    /// Event modifier definitions (loaded from common/event_modifiers/, immutable).
    #[serde(skip)]
    pub event_modifiers: eu4data::event_modifiers::EventModifiersRegistry,

    /// Government type definitions (hardcoded for Phase 0, immutable).
    #[serde(skip)]
    pub government_types: crate::government::GovernmentRegistry,

    /// Estate definitions (hardcoded for Phase 1, loaded from files in Phase 2).
    #[serde(skip)]
    pub estates: crate::estates::EstateRegistry,
}

impl WorldState {
    /// Returns all valid commands for a country at the current state.
    /// This is the single source of truth for valid AI and player actions.
    pub fn available_commands(
        &self,
        tag: &str,
        adjacency: Option<&eu4data::adjacency::AdjacencyGraph>,
    ) -> Vec<crate::input::Command> {
        crate::step::available_commands(self, tag, adjacency)
    }

    /// Generate a random Fixed in [0, 1) using deterministic RNG.
    ///
    /// Uses xorshift64 for deterministic random number generation.
    /// Critical for replay compatibility - same seed produces same sequence.
    /// Returns Fixed for netcode-safe arithmetic (no floats in sim logic).
    pub fn random_fixed(&mut self) -> Fixed {
        // xorshift64 - deterministic PRNG
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_state = x;
        // Convert to Fixed in [0, 1) range
        // Fixed uses SCALE=10000, so we need raw value in [0, 10000)
        // Use upper 32 bits for better distribution, then modulo SCALE
        Fixed::from_raw(((x >> 32) % (Fixed::SCALE as u64)) as i64)
    }

    /// Generate a random u64 using the deterministic RNG.
    /// Uses xorshift64 to maintain replay compatibility.
    pub fn random_u64(&mut self) -> u64 {
        let mut x = self.rng_state;
        if x == 0 {
            x = 1;
        }
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_state = x;
        x
    }

    /// Check if movement from `from` to `to` is blocked by enemy Zone of Control (ZoC).
    ///
    /// # ZoC Rules (EU4-authentic)
    /// - Forts project ZoC to all adjacent provinces
    /// - Cannot move through ZoC (e.g., from province A to B, if both are adjacent to enemy fort C)
    /// - CAN move directly TO the fort to siege it
    /// - Mothballed forts do NOT project ZoC (they fall instantly)
    /// - Only applies during wartime
    ///
    /// # Returns
    /// `true` if movement is blocked, `false` if allowed.
    pub fn is_blocked_by_zoc(
        &self,
        from: ProvinceId,
        to: ProvinceId,
        mover: &str,
        adjacency: Option<&eu4data::adjacency::AdjacencyGraph>,
    ) -> bool {
        let Some(graph) = adjacency else {
            return false; // No adjacency data - cannot check ZoC
        };

        // Get provinces adjacent to `from` that might have enemy forts
        for neighbor_id in graph.neighbors(from) {
            // Direct move to the fort is always allowed (to siege it)
            if neighbor_id == to {
                continue;
            }

            let Some(province) = self.provinces.get(&neighbor_id) else {
                continue;
            };

            // Must have a non-mothballed fort to project ZoC
            if province.fort_level == 0 || province.is_mothballed {
                continue;
            }

            // Must be enemy-controlled (use controller first, fallback to owner)
            let controller = province.controller.as_ref().or(province.owner.as_ref());
            let Some(ctrl) = controller else {
                continue; // Unowned/uncontrolled province - no ZoC
            };

            // Only blocks if at war with the controller
            if !self.diplomacy.are_at_war(mover, ctrl) {
                continue;
            }

            // This fort projects ZoC - check if `to` is also adjacent to it
            if graph.are_adjacent(neighbor_id, to) {
                // Blocked: trying to move from A to B, both adjacent to enemy fort C
                return true;
            }
        }

        false // No ZoC blocking
    }

    /// Check if movement across a strait is blocked by enemy fleets.
    ///
    /// # Strait Blocking Rules (EU4-authentic)
    /// - Straits connect two land provinces through a sea zone
    /// - Movement is blocked if an enemy fleet is in the strait's sea zone
    /// - Fleets must be at war with the mover to block
    pub fn is_strait_blocked(
        &self,
        from: ProvinceId,
        to: ProvinceId,
        mover: &str,
        adjacency: Option<&eu4data::adjacency::AdjacencyGraph>,
    ) -> bool {
        let Some(graph) = adjacency else {
            return false; // No adjacency data - cannot check straits
        };

        // Check if this movement crosses a strait
        let Some(sea_zone) = graph.get_strait_sea_zone(from, to) else {
            return false; // Not a strait crossing
        };

        // Check if any enemy fleet is in the sea zone
        for fleet in self.fleets.values() {
            if fleet.location == sea_zone && self.diplomacy.are_at_war(mover, &fleet.owner) {
                return true; // Strait blocked by enemy fleet
            }
        }

        false // Strait is clear
    }
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

pub type InstitutionId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TechType {
    Adm,
    Dip,
    Mil,
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
    /// Fort level (0 = no fort, 1-8 = fort levels). Capital provinces get a free level-1 fort.
    pub fort_level: u8,
    /// Whether this province is the capital of its owner
    pub is_capital: bool,
    /// Whether the fort is mothballed (no ZoC, no garrison, reduced maintenance)
    pub is_mothballed: bool,
    /// Whether this province is a sea province (for naval movement)
    pub is_sea: bool,
    /// Whether this province is a wasteland (impassable, uncolonizable)
    #[serde(default)]
    pub is_wasteland: bool,
    /// Terrain type (e.g., "plains", "mountains", "forest")
    pub terrain: Option<Terrain>,
    /// Progress of institutions in this province (0.0 to 100.0)
    pub institution_presence: HashMap<InstitutionId, f32>,
    /// Trade-related state (center of trade level, protecting ships).
    #[serde(default)]
    pub trade: ProvinceTradeState,
    /// Countries that have cores on this province.
    /// A core represents permanent ownership claim and removes autonomy/overextension.
    #[serde(default)]
    pub cores: std::collections::HashSet<Tag>,
    /// In-progress coring (owner country working to establish a core).
    #[serde(default)]
    pub coring_progress: Option<CoringProgress>,
    /// Completed buildings in this province (bitmask for efficiency).
    #[serde(default)]
    pub buildings: crate::buildings::BuildingSet,
    /// Active building construction (only one at a time per province).
    #[serde(default)]
    pub building_construction: Option<crate::buildings::BuildingConstruction>,
    /// Whether this province has a port (required for naval buildings).
    #[serde(default)]
    pub has_port: bool,
}

/// Progress towards establishing a core on a province.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoringProgress {
    /// Country establishing the core
    pub coring_country: Tag,
    /// Date coring started
    pub start_date: Date,
    /// Months of progress completed (0 to required)
    pub progress: u8,
    /// Total months required (base 36)
    pub required: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountryState {
    /// Treasury balance (Fixed for determinism)
    pub treasury: Fixed,
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
    /// Administrative technology level
    pub adm_tech: u8,
    /// Diplomatic technology level
    pub dip_tech: u8,
    /// Military technology level
    pub mil_tech: u8,
    /// Set of institutions embraced by this country
    pub embraced_institutions: std::collections::HashSet<InstitutionId>,
    /// State religion (e.g., "catholic", "protestant")
    pub religion: Option<String>,
    /// Government type (Monarchy, Republic, Theocracy, Tribal, etc.)
    #[serde(default)]
    pub government_type: crate::government::GovernmentTypeId,
    /// Government reforms unlocked by this country
    #[serde(default)]
    pub government_reforms: std::collections::HashSet<crate::government::ReformId>,
    /// Trade-related state (merchants, home node, embargoes).
    #[serde(default)]
    pub trade: CountryTradeState,
    /// Income breakdown for last month (for display purposes).
    #[serde(default)]
    pub income: IncomeBreakdown,
    /// Fixed monthly expenses from save file (army/fleet maintenance).
    /// Used for passive simulation when armies/fleets are cleared.
    #[serde(default)]
    pub fixed_expenses: Fixed,
    /// Last date a diplomatic action was taken (for one-per-day limit).
    /// Diplomatic actions: war declarations, peace offers, alliances, etc.
    #[serde(default)]
    pub last_diplomatic_action: Option<Date>,
    /// Cooldowns for peace offers per war (date when offer is allowed again).
    /// Set after a peace offer is rejected; cleared when war ends.
    #[serde(default)]
    pub peace_offer_cooldowns: std::collections::HashMap<WarId, Date>,
    /// Pending call-to-arms offers from allies (war_id -> which side to join).
    /// Defensive allies auto-join; offensive allies get a choice.
    #[serde(default)]
    pub pending_call_to_arms: std::collections::HashMap<WarId, crate::input::WarSide>,
    /// Overextension percentage (1 dev = 1% OE).
    /// Calculated from total development in owned provinces without cores.
    /// High OE causes unrest and other penalties.
    #[serde(default)]
    pub overextension: Fixed,
    /// Aggressive expansion toward each country.
    /// Accumulates when conquering provinces, decays over time (~2 AE per year).
    /// High AE (>50) can trigger coalition formation.
    #[serde(default)]
    pub aggressive_expansion: HashMap<Tag, Fixed>,
    /// Idea state: which groups and ideas this country has.
    #[serde(default)]
    pub ideas: crate::ideas::CountryIdeaState,
    /// Enabled policies (combinations of idea groups granting bonuses).
    #[serde(default)]
    pub enabled_policies: Vec<crate::systems::PolicyId>,
    /// Number of policy slots available (increases with completed idea groups).
    #[serde(default)]
    pub policy_slots: u8,
    /// Estate state (loyalty, influence, privileges).
    #[serde(default)]
    pub estates: crate::estates::CountryEstateState,
    /// Countries marked as rivals (max 3).
    /// Rivals provide power projection bonus and AE reduction against them.
    /// Unilateral relationship: you can rival someone who doesn't rival you back.
    #[serde(default)]
    pub rivals: std::collections::HashSet<Tag>,
    /// Advisors employed by this country.
    /// Each advisor provides monthly monarch points but costs ducats per month.
    #[serde(default)]
    pub advisors: Vec<Advisor>,
}

/// An advisor employed by a country.
///
/// Advisors provide monthly monarch points but cost ducats each month.
/// Cost scales with skill level (typically quadratically).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Advisor {
    /// Advisor name (for debugging/UI)
    pub name: String,
    /// Skill level (1-5). Higher skill = more expensive but better bonuses.
    pub skill: u8,
    /// Type of advisor (affects which monarch point category they boost)
    pub advisor_type: AdvisorType,
    /// Monthly salary cost in ducats
    pub monthly_cost: Fixed,
}

/// Category of advisor, determining their bonuses and costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AdvisorType {
    Administrative,
    Diplomatic,
    Military,
}

/// Breakdown of monthly income by source.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IncomeBreakdown {
    /// Income from taxation
    pub taxation: Fixed,
    /// Income from trade
    pub trade: Fixed,
    /// Income from production (direct, if any)
    pub production: Fixed,
    /// Total expenses (maintenance, etc.)
    pub expenses: Fixed,
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
            fixed_expenses: Fixed::ZERO,
            dip_mana: Fixed::ZERO,
            mil_mana: Fixed::ZERO,
            adm_tech: 0,
            dip_tech: 0,
            mil_tech: 0,
            embraced_institutions: std::collections::HashSet::new(),
            religion: None,
            government_type: crate::government::GovernmentTypeId::MONARCHY,
            government_reforms: std::collections::HashSet::new(),
            trade: CountryTradeState::default(),
            income: IncomeBreakdown::default(),
            last_diplomatic_action: None,
            peace_offer_cooldowns: std::collections::HashMap::new(),
            pending_call_to_arms: std::collections::HashMap::new(),
            overextension: Fixed::ZERO,
            aggressive_expansion: HashMap::new(),
            ideas: crate::ideas::CountryIdeaState::default(),
            enabled_policies: Vec::new(),
            policy_slots: 0,
            estates: crate::estates::CountryEstateState::default(),
            rivals: std::collections::HashSet::new(),
            advisors: Vec::new(),
        }
    }
}

/// Type of diplomatic relationship between two countries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelationType {
    Alliance,
    RoyalMarriage,
    Rival,
}

/// A subject relationship between an overlord and subject country.
///
/// Subject relationships are asymmetric: overlord controls subject.
/// Keyed by subject tag in [`DiplomacyState::subjects`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubjectRelationship {
    /// The overlord (senior partner) country tag.
    pub overlord: Tag,
    /// The subject (junior partner) country tag.
    pub subject: Tag,
    /// Type of subject relationship (vassal, march, PU, etc.).
    /// References into the SubjectTypeRegistry.
    pub subject_type: crate::subjects::SubjectTypeId,
    /// Date the relationship was established.
    pub start_date: Date,
    /// Current liberty desire (0-100). Subjects rebel at 50+.
    pub liberty_desire: u8,
    /// Integration/annexation progress (0-100).
    pub integration_progress: u8,
    /// Whether overlord is actively integrating this subject.
    pub integrating: bool,
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum PeaceTerms {
    /// No territorial changes
    #[default]
    WhitePeace,
    /// Transfer specific provinces to the victor
    TakeProvinces { provinces: Vec<ProvinceId> },
    /// Complete annexation of the defeated country
    FullAnnexation,
}

/// Coalition against an aggressive nation.
///
/// Forms when multiple countries accumulate high AE (>50) toward a target.
/// Coalition members can declare war together as a defensive alliance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coalition {
    /// Target country the coalition is against
    pub target: Tag,
    /// Countries in the coalition
    pub members: Vec<Tag>,
    /// Date the coalition was formed
    pub formed_date: Date,
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
    /// Active truces: (Tag1, Tag2) -> expiry date
    /// Keys stored in sorted order (smaller tag first) to avoid duplication
    pub truces: HashMap<(Tag, Tag), Date>,
    /// Active coalitions against aggressive countries (keyed by target)
    #[serde(default)]
    pub coalitions: HashMap<Tag, Coalition>,
    /// Subject relationships: subject tag -> relationship details.
    /// Keyed by subject since each country can only have one overlord.
    #[serde(default)]
    pub subjects: HashMap<Tag, SubjectRelationship>,
    /// Pending alliance offers awaiting response: (offerer, target) -> date offered
    /// Unsorted pairs (directional: who → whom)
    #[serde(default)]
    pub pending_alliance_offers: HashMap<(Tag, Tag), Date>,
    /// Pending royal marriage offers awaiting response: (offerer, target) -> date offered
    #[serde(default)]
    pub pending_marriage_offers: HashMap<(Tag, Tag), Date>,
    /// Pending military access requests awaiting response: (requester, grantor) -> date requested
    #[serde(default)]
    pub pending_access_requests: HashMap<(Tag, Tag), Date>,
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

    /// Check if there is an active truce between two countries.
    pub fn has_active_truce(&self, tag1: &str, tag2: &str, current_date: Date) -> bool {
        let key = Self::sorted_pair(tag1, tag2);
        self.truces
            .get(&key)
            .map(|expiry| *expiry > current_date)
            .unwrap_or(false)
    }

    /// Create a new truce between two countries.
    pub fn create_truce(&mut self, tag1: &str, tag2: &str, expiry_date: Date) {
        let key = Self::sorted_pair(tag1, tag2);
        self.truces.insert(key, expiry_date);
    }

    pub fn sorted_pair(a: &str, b: &str) -> (String, String) {
        if a < b {
            (a.to_string(), b.to_string())
        } else {
            (b.to_string(), a.to_string())
        }
    }

    // === Subject relationship methods ===

    /// Get the overlord of a country (if it's a subject).
    pub fn get_overlord(&self, tag: &str) -> Option<&SubjectRelationship> {
        self.subjects.get(tag)
    }

    /// Get all subjects of a country.
    pub fn get_subjects(&self, overlord: &str) -> Vec<&SubjectRelationship> {
        self.subjects
            .values()
            .filter(|rel| rel.overlord == overlord)
            .collect()
    }

    /// Check if `overlord` is the direct overlord of `subject`.
    pub fn is_overlord_of(&self, overlord: &str, subject: &str) -> bool {
        self.subjects
            .get(subject)
            .is_some_and(|rel| rel.overlord == overlord)
    }

    /// Get the top-level overlord for a country (handles PU chains).
    ///
    /// For Austria → Hungary → Bohemia, calling with "Bohemia" returns "Austria".
    /// Returns the tag itself if it has no overlord.
    pub fn get_top_overlord<'a>(&'a self, tag: &'a str) -> &'a str {
        let mut current = tag;
        // Guard against cycles (shouldn't happen, but defensive)
        for _ in 0..10 {
            match self.subjects.get(current) {
                Some(rel) => current = &rel.overlord,
                None => break,
            }
        }
        current
    }

    /// Check if two countries are in the same realm (can't war each other).
    ///
    /// Returns true if:
    /// - One is overlord of the other
    /// - They share the same top-level overlord (fellow subjects)
    ///
    /// **Exception**: Tributaries are NOT considered in the same realm.
    /// They can war each other and their overlord (independence wars).
    pub fn in_same_realm(
        &self,
        tag1: &str,
        tag2: &str,
        registry: &crate::subjects::SubjectTypeRegistry,
    ) -> bool {
        // Check if tag1 is overlord of tag2
        if let Some(rel) = self.subjects.get(tag2) {
            if rel.overlord == tag1 {
                // Exception: tributaries can war their overlord
                return !registry.is_tributary(rel.subject_type);
            }
        }

        // Check if tag2 is overlord of tag1
        if let Some(rel) = self.subjects.get(tag1) {
            if rel.overlord == tag2 {
                // Exception: tributaries can war their overlord
                return !registry.is_tributary(rel.subject_type);
            }
        }

        // Check if they share the same top overlord (fellow subjects)
        let top1 = self.get_top_overlord(tag1);
        let top2 = self.get_top_overlord(tag2);

        if top1 == top2 && top1 != tag1 && top2 != tag2 {
            // Both are subjects of the same realm
            // Check if either is a tributary (tributaries can war fellow subjects)
            let is_trib1 = self
                .subjects
                .get(tag1)
                .is_some_and(|r| registry.is_tributary(r.subject_type));
            let is_trib2 = self
                .subjects
                .get(tag2)
                .is_some_and(|r| registry.is_tributary(r.subject_type));

            // If either is a tributary, they can war each other
            return !is_trib1 && !is_trib2;
        }

        false
    }

    /// Add a subject relationship.
    ///
    /// Returns an error if:
    /// - Subject already has an overlord
    /// - Subject-of-subject would be created (overlord is already someone's subject)
    pub fn add_subject(
        &mut self,
        overlord: &str,
        subject: &str,
        subject_type: crate::subjects::SubjectTypeId,
        start_date: Date,
    ) -> Result<(), &'static str> {
        // Check if subject already has an overlord
        if self.subjects.contains_key(subject) {
            return Err("Subject already has an overlord");
        }

        // Check for subject-of-subject (forbidden in vanilla)
        if self.subjects.contains_key(overlord) {
            return Err("Overlord is already someone's subject (subject-of-subject forbidden)");
        }

        self.subjects.insert(
            subject.to_string(),
            SubjectRelationship {
                overlord: overlord.to_string(),
                subject: subject.to_string(),
                subject_type,
                start_date,
                liberty_desire: 0,
                integration_progress: 0,
                integrating: false,
            },
        );

        Ok(())
    }

    /// Remove a subject relationship (independence, integration, etc.).
    pub fn remove_subject(&mut self, subject: &str) -> Option<SubjectRelationship> {
        self.subjects.remove(subject)
    }
}

/// Tracks the global state of the Reformation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReformationState {
    /// Has the Protestant Reformation fired?
    pub protestant_reformation_fired: bool,
    /// Has the Reformed movement fired?
    pub reformed_reformation_fired: bool,
    /// Active Centers of Reformation: province_id -> religion
    pub centers_of_reformation: HashMap<ProvinceId, String>,
    /// When each center was created (for expiry)
    pub center_creation_dates: HashMap<ProvinceId, Date>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalState {
    pub reformation: ReformationState,
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
            c.religion.hash(&mut hasher);
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
            p.fort_level.hash(&mut hasher);
            p.is_capital.hash(&mut hasher);
            p.is_mothballed.hash(&mut hasher);
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
            f.ships.len().hash(&mut hasher);
            for ship in &f.ships {
                ship.type_.hash(&mut hasher);
                ship.hull.0.hash(&mut hasher);
                ship.durability.0.hash(&mut hasher);
            }
            f.embarked_armies.hash(&mut hasher);
            f.movement.hash(&mut hasher);
            f.admiral.hash(&mut hasher);
            f.in_battle.hash(&mut hasher);
        }

        // Reformation state
        self.global
            .reformation
            .protestant_reformation_fired
            .hash(&mut hasher);
        self.global
            .reformation
            .reformed_reformation_fired
            .hash(&mut hasher);
        let mut center_ids: Vec<_> = self
            .global
            .reformation
            .centers_of_reformation
            .keys()
            .collect();
        center_ids.sort();
        for &id in center_ids {
            id.hash(&mut hasher);
            self.global.reformation.centers_of_reformation[&id].hash(&mut hasher);
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

        // Truces (sorted by key)
        let mut truce_keys: Vec<_> = self.diplomacy.truces.keys().collect();
        truce_keys.sort();
        for key in truce_keys {
            key.hash(&mut hasher);
            self.diplomacy.truces[key].hash(&mut hasher);
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

    // === Subject relationship tests ===

    fn make_test_subject_registry() -> crate::subjects::SubjectTypeRegistry {
        use crate::subjects::{SubjectTypeDef, SubjectTypeRegistry};

        let mut registry = SubjectTypeRegistry::new();

        // Vassal
        registry.add(SubjectTypeDef {
            name: "vassal".into(),
            joins_overlords_wars: true,
            ..Default::default()
        });

        // Tributary (doesn't join wars)
        registry.add(SubjectTypeDef {
            name: "tributary_state".into(),
            joins_overlords_wars: false,
            is_voluntary: true,
            ..Default::default()
        });

        // Personal union
        registry.add(SubjectTypeDef {
            name: "personal_union".into(),
            joins_overlords_wars: true,
            has_overlords_ruler: true,
            ..Default::default()
        });

        registry
    }

    #[test]
    fn test_add_subject() {
        let registry = make_test_subject_registry();
        let mut diplomacy = DiplomacyState::default();
        let start_date = Date::new(1444, 11, 11);

        // Add FRA -> PRO (Provence is French vassal)
        let result = diplomacy.add_subject("FRA", "PRO", registry.vassal_id, start_date);
        assert!(result.is_ok());

        // Verify relationship exists
        assert!(diplomacy.is_overlord_of("FRA", "PRO"));
        assert!(diplomacy.get_overlord("PRO").is_some());
        assert_eq!(diplomacy.get_subjects("FRA").len(), 1);
    }

    #[test]
    fn test_subject_already_has_overlord() {
        let registry = make_test_subject_registry();
        let mut diplomacy = DiplomacyState::default();
        let start_date = Date::new(1444, 11, 11);

        // Add FRA -> PRO
        diplomacy
            .add_subject("FRA", "PRO", registry.vassal_id, start_date)
            .unwrap();

        // Try to add ENG -> PRO (should fail - PRO already has overlord)
        let result = diplomacy.add_subject("ENG", "PRO", registry.vassal_id, start_date);
        assert!(result.is_err());
    }

    #[test]
    fn test_subject_of_subject_forbidden() {
        let registry = make_test_subject_registry();
        let mut diplomacy = DiplomacyState::default();
        let start_date = Date::new(1444, 11, 11);

        // Add FRA -> PRO
        diplomacy
            .add_subject("FRA", "PRO", registry.vassal_id, start_date)
            .unwrap();

        // Try to add PRO -> AVG (PRO is already a subject, can't be overlord)
        let result = diplomacy.add_subject("PRO", "AVG", registry.vassal_id, start_date);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_top_overlord_simple() {
        let registry = make_test_subject_registry();
        let mut diplomacy = DiplomacyState::default();
        let start_date = Date::new(1444, 11, 11);

        // FRA -> PRO
        diplomacy
            .add_subject("FRA", "PRO", registry.vassal_id, start_date)
            .unwrap();

        // PRO's top overlord is FRA
        assert_eq!(diplomacy.get_top_overlord("PRO"), "FRA");
        // FRA has no overlord
        assert_eq!(diplomacy.get_top_overlord("FRA"), "FRA");
        // ENG has no overlord
        assert_eq!(diplomacy.get_top_overlord("ENG"), "ENG");
    }

    #[test]
    fn test_in_same_realm_overlord_subject() {
        let registry = make_test_subject_registry();
        let mut diplomacy = DiplomacyState::default();
        let start_date = Date::new(1444, 11, 11);

        // FRA -> PRO (vassal)
        diplomacy
            .add_subject("FRA", "PRO", registry.vassal_id, start_date)
            .unwrap();

        // FRA and PRO are in same realm
        assert!(diplomacy.in_same_realm("FRA", "PRO", &registry));
        assert!(diplomacy.in_same_realm("PRO", "FRA", &registry));

        // ENG is not in same realm with FRA
        assert!(!diplomacy.in_same_realm("FRA", "ENG", &registry));
    }

    #[test]
    fn test_tributary_not_in_same_realm() {
        let registry = make_test_subject_registry();
        let mut diplomacy = DiplomacyState::default();
        let start_date = Date::new(1444, 11, 11);

        // MNG -> KOR (tributary)
        diplomacy
            .add_subject("MNG", "KOR", registry.tributary_id, start_date)
            .unwrap();

        // Tributaries are NOT in same realm - they can war their overlord
        assert!(!diplomacy.in_same_realm("MNG", "KOR", &registry));
    }

    #[test]
    fn test_fellow_subjects_in_same_realm() {
        let registry = make_test_subject_registry();
        let mut diplomacy = DiplomacyState::default();
        let start_date = Date::new(1444, 11, 11);

        // FRA -> PRO (vassal)
        diplomacy
            .add_subject("FRA", "PRO", registry.vassal_id, start_date)
            .unwrap();
        // FRA -> BRI (vassal)
        diplomacy
            .add_subject("FRA", "BRI", registry.vassal_id, start_date)
            .unwrap();

        // PRO and BRI are fellow subjects, in same realm
        assert!(diplomacy.in_same_realm("PRO", "BRI", &registry));
    }

    #[test]
    fn test_remove_subject() {
        let registry = make_test_subject_registry();
        let mut diplomacy = DiplomacyState::default();
        let start_date = Date::new(1444, 11, 11);

        diplomacy
            .add_subject("FRA", "PRO", registry.vassal_id, start_date)
            .unwrap();

        // Remove subject
        let removed = diplomacy.remove_subject("PRO");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().overlord, "FRA");

        // No longer overlord
        assert!(!diplomacy.is_overlord_of("FRA", "PRO"));
        assert!(diplomacy.get_overlord("PRO").is_none());
    }
}
