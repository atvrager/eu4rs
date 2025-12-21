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
use crate::state::{CountryState, Date, ProvinceId, Tag, WarId};
use rand::Rng;
use rand::SeedableRng;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

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
#[derive(Debug, Clone, Serialize)]
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
}

/// Available commands for a country
pub type AvailableCommands = Vec<Command>;

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

        // Weighted selection:
        // We assign a higher weight to strategic actions (war, peace, colonization)
        // to prevent the AI from just moving armies aimlessly when it has many move options.
        let weights: Vec<u32> = available_commands
            .iter()
            .map(|cmd| match cmd {
                Command::DeclareWar { .. } => 100,
                Command::AcceptPeace { .. } => 500, // AI should almost always accept peace if offered
                Command::OfferPeace { .. } => 30,
                Command::DevelopProvince { .. } => 20,
                Command::BuyTech { .. } => 40,
                Command::EmbraceInstitution { .. } => 40,
                Command::StartColony { .. } => 50,
                Command::Move { .. } => 1,
                Command::MoveFleet { .. } => 1,
                Command::RejectPeace { .. } => 1, // Pride is rarely worth the bloodshed
                _ => 10,
            })
            .collect();

        use rand::distributions::{Distribution, WeightedIndex};
        if let Ok(dist) = WeightedIndex::new(&weights) {
            // 50% chance to act this tick
            if self.rng.gen_bool(0.5) {
                let idx = dist.sample(&mut self.rng);
                return vec![available_commands[idx].clone()];
            }
        }

        vec![]
    }
}

mod greedy;
pub use greedy::GreedyAI;

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::state::CountryState; // Assuming CountryState is available/constructible

    // Helper to create a dummy state
    fn dummy_state() -> VisibleWorldState {
        VisibleWorldState {
            date: Date::new(1444, 11, 11),
            observer: "SWE".to_string(), // Tag is String or similar
            own_country: CountryState::default(), // Assuming default or minimal construction
            at_war: false,
            known_countries: vec![],
            enemy_provinces: HashSet::new(),
            known_country_strength: HashMap::new(),
            our_war_score: HashMap::new(),
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
