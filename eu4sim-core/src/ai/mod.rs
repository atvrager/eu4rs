//! AI decision-making subsystem
//!
//! This module defines the [`AiPlayer`] trait and implementations for AI-controlled countries.
//!
//! # ML Training Readiness
//!
//! The AI interface is designed to support future machine learning approaches:
//!
//! - **Structured inputs**: [`VisibleWorldState`] is serializable and can be converted to
//!   training prompts for language models or feature vectors for RL agents.
//!
//! - **Enumerable actions**: The `available_commands` slice provides a bounded action space
//!   per tick. ML models can output an action *index* rather than generating free-form text,
//!   avoiding parsing errors and invalid commands.
//!
//! - **Structured outputs**: The [`Command`] enum provides type-safe, serializable actions
//!   that can be logged for imitation learning datasets.
//!
//! # Data Generation Pattern
//!
//! Any `AiPlayer` can generate training data by logging `(state, actions, choice, outcome)`:
//!
//! ```ignore
//! for tick in game {
//!     let state = visible_state(country);
//!     let available = available_commands(country);
//!     let chosen = ai.decide(&state, &available);
//!     log_sample(state, available, chosen, game_outcome);
//! }
//! ```
//!
//! # Determinism
//!
//! AI implementations must be deterministic given the same RNG seed. For ML models,
//! use argmax decoding (not temperature sampling) to ensure reproducibility for
//! replays and multiplayer lockstep.
//!
//! See `docs/design/simulation/learned-ai.md` for the full ML architecture.

use crate::fixed::Fixed;
use crate::input::Command;
use crate::state::{ArmyId, CountryState, Date, FleetId, GeneralId, ProvinceId, Tag, WarId};
use rand::Rng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// =============================================================================
// Summary Structs for AI Visibility
// =============================================================================

/// Summary of a general for AI decision-making
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralSummary {
    pub id: GeneralId,
    pub fire: u8,
    pub shock: u8,
    pub maneuver: u8,
    pub siege: u8,
    /// Which army this general is assigned to (None if unassigned)
    pub assigned_to: Option<ArmyId>,
}

/// Summary of a fleet for AI decision-making
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetSummary {
    pub id: FleetId,
    pub location: ProvinceId,
    pub ship_count: u32,
    pub transport_capacity: u32,
    pub in_battle: bool,
}

/// Summary of an ongoing siege for AI decision-making
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiegeSummary {
    pub province: ProvinceId,
    pub fort_level: u8,
    pub progress_modifier: i32,
    /// Estimated days until siege completes (rough heuristic)
    pub days_remaining_estimate: u32,
}

/// Visibility mode for AI and UI filtering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityMode {
    /// Fog of war, realistic constraints
    Realistic,
    /// See everything (testing, observer, cheating AI)
    Omniscient,
}

/// Minimal visible state for AI decision-making
///
/// In Omniscient mode, this is a direct copy of relevant fields.
/// In Realistic mode, this would be filtered (future work).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VisibleWorldState {
    pub date: Date,
    pub observer: Tag,
    pub own_country: CountryState,
    pub at_war: bool,
    pub known_countries: Vec<Tag>,
    /// Provinces owned by enemies in active wars
    pub enemy_provinces: HashSet<ProvinceId>,
    /// Military strength (total regiments) of countries.
    /// Note: Currently populated with ALL countries (Omniscient mode).
    /// Will be filtered to actually-visible countries when Realistic mode is implemented.
    pub known_country_strength: HashMap<Tag, u32>,
    /// War score for each war the observer is participating in
    /// Positive = observer is winning, negative = observer is losing
    pub our_war_score: HashMap<WarId, Fixed>,

    // =========================================================================
    // Warfare Extensions (Tier 1-3)
    // =========================================================================
    /// Generals owned by the observer country
    #[serde(default)]
    pub own_generals: Vec<GeneralSummary>,

    /// Armies that currently lack a general (candidates for assignment)
    #[serde(default)]
    pub armies_without_general: Vec<ArmyId>,

    /// Fleets owned by the observer country
    #[serde(default)]
    pub own_fleets: Vec<FleetSummary>,

    /// Straits currently blocked by enemy fleets (from, to)
    #[serde(default)]
    pub blocked_straits: HashSet<(ProvinceId, ProvinceId)>,

    /// Supply limit for each known province (1 regiment per development)
    #[serde(default)]
    pub province_supply: HashMap<ProvinceId, u32>,

    /// Current regiment count per province (for attrition awareness)
    #[serde(default)]
    pub army_locations: HashMap<ProvinceId, u32>,

    /// Aggressive expansion accumulated by the observer toward each country
    #[serde(default)]
    pub own_ae: HashMap<Tag, Fixed>,

    /// Coalition against the observer (if one exists)
    #[serde(default)]
    pub coalition_against_us: Option<Vec<Tag>>,

    /// Enemy provinces with forts (priority siege targets)
    #[serde(default)]
    pub fort_provinces: HashSet<ProvinceId>,

    /// Active sieges conducted by the observer
    #[serde(default)]
    pub active_sieges: Vec<SiegeSummary>,

    /// Pending calls-to-arms: (war_id, ally_requesting)
    #[serde(default)]
    pub pending_call_to_arms: Vec<(WarId, Tag)>,

    /// Total military strength (regiment count) of all current war enemies
    /// Used for war declaration heuristics - don't start new wars if overextended
    #[serde(default)]
    pub current_war_enemy_strength: u32,

    /// Regiment count for each of our armies (for Move scoring)
    #[serde(default)]
    pub our_army_sizes: HashMap<ArmyId, u32>,

    /// Provinces containing our armies, with their regiment counts
    /// Used for consolidation scoring - move toward other friendly stacks
    #[serde(default)]
    pub our_army_provinces: HashMap<ProvinceId, u32>,

    /// Friendly provinces adjacent to enemy territory (staging areas)
    /// Used for army consolidation before attack
    #[serde(default)]
    pub staging_provinces: HashSet<ProvinceId>,
}

/// Available commands for a country
pub type AvailableCommands = Vec<Command>;

// =============================================================================
// Command Categories (for multi-action AI decisions)
// =============================================================================

/// Categories of commands for AI decision-making.
///
/// Used to enforce "one diplomatic action per day" while allowing unlimited
/// military moves, economic actions, etc. in the same tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommandCategory {
    /// Diplomatic actions: one per day max (DeclareWar, OfferPeace, etc.)
    Diplomatic,
    /// Military orders: unlimited (Move, MoveFleet, Embark, etc.)
    Military,
    /// Economic actions: unlimited (DevelopProvince, BuyTech, etc.)
    Economic,
    /// Trade actions: unlimited (SendMerchant, RecallMerchant)
    Trade,
    /// Colonization: unlimited (StartColony, AbandonColony)
    Colonization,
    /// Other actions: unlimited (Pass, religion, etc.)
    Other,
}

/// Categorize a command for multi-action AI selection.
pub fn categorize_command(cmd: &Command) -> CommandCategory {
    match cmd {
        // Diplomatic: one per day (except Accept/Reject/Grant/Deny which have no cooldown)
        Command::DeclareWar { .. }
        | Command::OfferPeace { .. }
        | Command::AcceptPeace { .. }
        | Command::RejectPeace { .. }
        | Command::JoinWar { .. }
        | Command::CallAllyToWar { .. }
        | Command::SetRival { .. }
        | Command::RemoveRival { .. }
        | Command::OfferAlliance { .. }
        | Command::AcceptAlliance { .. }
        | Command::RejectAlliance { .. }
        | Command::BreakAlliance { .. }
        | Command::OfferRoyalMarriage { .. }
        | Command::AcceptRoyalMarriage { .. }
        | Command::RejectRoyalMarriage { .. }
        | Command::BreakRoyalMarriage { .. }
        | Command::RequestMilitaryAccess { .. }
        | Command::GrantMilitaryAccess { .. }
        | Command::DenyMilitaryAccess { .. }
        | Command::CancelMilitaryAccess { .. } => CommandCategory::Diplomatic,

        // Military: unlimited
        Command::Move { .. }
        | Command::MoveFleet { .. }
        | Command::Embark { .. }
        | Command::Disembark { .. }
        | Command::MergeArmies { .. }
        | Command::SplitArmy { .. }
        | Command::RecruitGeneral
        | Command::AssignGeneral { .. }
        | Command::UnassignGeneral { .. }
        | Command::RecruitRegiment { .. } => CommandCategory::Military,

        // Economic: unlimited
        Command::DevelopProvince { .. }
        | Command::BuyTech { .. }
        | Command::EmbraceInstitution { .. }
        | Command::BuildInProvince { .. }
        | Command::Core { .. }
        | Command::PickIdeaGroup { .. }
        | Command::UnlockIdea { .. } => CommandCategory::Economic,

        // Trade: unlimited
        Command::SendMerchant { .. }
        | Command::RecallMerchant { .. }
        | Command::UpgradeCenterOfTrade { .. } => CommandCategory::Trade,

        // Colonization: unlimited
        Command::StartColony { .. } | Command::AbandonColony { .. } => {
            CommandCategory::Colonization
        }

        // Other: Pass, religion, etc.
        _ => CommandCategory::Other,
    }
}

/// AI decision-making trait.
///
/// Implementations choose which commands to issue each tick based on visible game state
/// and the set of currently available (legal) commands.
///
/// # ML Training Interface
///
/// This trait is designed to be training-data-friendly:
///
/// - `visible_state` is serializable → can become a training prompt
/// - `available_commands` is a finite list → model outputs an index, not free text
/// - Return value is structured → no parsing required
///
/// For learned AI, the typical pattern is:
/// 1. Serialize `visible_state` to a prompt string
/// 2. Format `available_commands` as a numbered list
/// 3. Run model inference to get an action index
/// 4. Return `vec![available_commands[index].clone()]`
///
/// See [`crate::ai`] module docs and `docs/design/simulation/learned-ai.md`.
pub trait AiPlayer: Send + Sync {
    /// Returns the name of this AI type (for debugging and pool management)
    fn name(&self) -> &'static str;

    /// Choose commands for this tick.
    ///
    /// - `visible_state`: What the AI can "see" (respects fog of war in Realistic mode)
    /// - `available_commands`: Legal commands the AI can issue this tick
    ///
    /// Returns a list of commands to execute. May return empty to "pass" this tick.
    fn decide(
        &mut self,
        visible_state: &VisibleWorldState,
        available_commands: &AvailableCommands,
    ) -> Vec<Command>;
}

/// Random AI that picks valid commands at random
pub struct RandomAi {
    rng: rand::rngs::StdRng,
}

impl RandomAi {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: rand::rngs::StdRng::seed_from_u64(seed),
        }
    }
}

impl AiPlayer for RandomAi {
    fn name(&self) -> &'static str {
        "RandomAi"
    }

    fn decide(
        &mut self,
        _visible_state: &VisibleWorldState,
        available_commands: &AvailableCommands,
    ) -> Vec<Command> {
        if available_commands.is_empty() {
            return vec![];
        }

        let mut result = Vec::new();
        let mut used_diplomatic = false;

        // Multi-command selection: sample from each category independently
        for cmd in available_commands {
            let category = categorize_command(cmd);

            match category {
                // Diplomatic: 30% chance, max 1 per tick
                CommandCategory::Diplomatic => {
                    if !used_diplomatic {
                        // Higher chance for AcceptPeace (almost always accept)
                        let chance = match cmd {
                            Command::AcceptPeace { .. } => 0.95,
                            Command::DeclareWar { .. } => 0.15,
                            Command::OfferPeace { .. } => 0.10,
                            Command::RejectPeace { .. } => 0.02, // Pride is rarely worth it
                            _ => 0.10,
                        };
                        if self.rng.gen_bool(chance) {
                            result.push(cmd.clone());
                            used_diplomatic = true;
                        }
                    }
                }
                // Military: 30% chance each (reduced from 50% to avoid spam)
                CommandCategory::Military => {
                    if self.rng.gen_bool(0.3) {
                        result.push(cmd.clone());
                    }
                }
                // Economic: 25% chance each
                CommandCategory::Economic => {
                    if self.rng.gen_bool(0.25) {
                        result.push(cmd.clone());
                    }
                }
                // Trade: 40% chance each
                CommandCategory::Trade => {
                    if self.rng.gen_bool(0.4) {
                        result.push(cmd.clone());
                    }
                }
                // Colonization: 50% chance each (colonies are valuable)
                CommandCategory::Colonization => {
                    if self.rng.gen_bool(0.5) {
                        result.push(cmd.clone());
                    }
                }
                // Other: 10% chance
                CommandCategory::Other => {
                    if self.rng.gen_bool(0.1) {
                        result.push(cmd.clone());
                    }
                }
            }
        }

        result
    }
}

mod greedy;
pub use greedy::GreedyAI;

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::state::CountryState; // Assuming CountryState is available/constructible

    // Helper to create a dummy state
    pub fn dummy_state() -> VisibleWorldState {
        VisibleWorldState {
            date: Date::new(1444, 11, 11),
            observer: "SWE".to_string(),
            own_country: CountryState::default(),
            at_war: false,
            known_countries: vec![],
            enemy_provinces: HashSet::new(),
            known_country_strength: HashMap::new(),
            our_war_score: HashMap::new(),
            own_generals: vec![],
            armies_without_general: vec![],
            own_fleets: vec![],
            blocked_straits: HashSet::new(),
            province_supply: HashMap::new(),
            army_locations: HashMap::new(),
            own_ae: HashMap::new(),
            coalition_against_us: None,
            fort_provinces: HashSet::new(),
            active_sieges: vec![],
            pending_call_to_arms: vec![],
            current_war_enemy_strength: 0,
            our_army_sizes: HashMap::new(),
            our_army_provinces: HashMap::new(),
            staging_provinces: HashSet::new(),
        }
    }

    #[test]
    fn random_ai_smoke_test() {
        let mut ai = RandomAi::new(12345);
        let state = dummy_state();
        let commands: AvailableCommands = vec![]; // No commands available

        // Should return empty if no commands
        let decisions = ai.decide(&state, &commands);
        assert!(decisions.is_empty());
    }
}
