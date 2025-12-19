//! AI decision-making subsystem

use crate::input::Command;
use crate::state::{CountryState, Date, Tag};
use rand::seq::SliceRandom;
use rand::Rng;
use rand::SeedableRng;

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
#[derive(Debug, Clone)]
pub struct VisibleWorldState {
    pub date: Date,
    pub observer: Tag,
    pub own_country: CountryState,
    pub at_war: bool,
    pub known_countries: Vec<Tag>,
}

/// Available commands for a country
pub type AvailableCommands = Vec<Command>;

/// AI decision-making trait
pub trait AiPlayer: Send + Sync {
    /// Choose commands for this tick
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
    fn decide(
        &mut self,
        _visible_state: &VisibleWorldState,
        available_commands: &AvailableCommands,
    ) -> Vec<Command> {
        // For minimal AI: pick one random command (or none)
        if available_commands.is_empty() {
            return vec![];
        }

        // 50% chance to issue a command this tick
        if self.rng.gen::<bool>() {
            if let Some(cmd) = available_commands.choose(&mut self.rng) {
                return vec![cmd.clone()];
            }
        }

        vec![]
    }
}

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
