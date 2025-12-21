use crate::ai::{AiPlayer, AvailableCommands, VisibleWorldState};
use crate::input::{Command, DevType};
use crate::state::TechType;

/// A deterministic, priority-based AI that maximizes country growth.
///
/// Unlike `RandomAi`, this implementation always picks the action with the
/// highest score based on immediate rewards.
#[derive(Default)]
pub struct GreedyAI;

impl GreedyAI {
    pub fn new() -> Self {
        Self
    }

    /// Scores a command based on immediate heuristic value.
    ///
    /// Higher scores are prioritized. 0 or negative scores are ignored.
    fn score_command(&self, cmd: &Command, state: &VisibleWorldState) -> i32 {
        match cmd {
            // Tier 0: Survival / Peace (Losing)
            Command::AcceptPeace { war_id } => {
                // Accept peace if we're losing badly (war_score < -25)
                if let Some(&score) = state.our_war_score.get(war_id) {
                    let threshold = crate::fixed::Fixed::from_int(-25);
                    if score < threshold {
                        10000 // Losing badly, accept peace immediately
                    } else {
                        -100 // Not losing badly enough to accept
                    }
                } else {
                    // Missing war score data is a bug—don't suddenly go pacifist.
                    // Keep fighting until we can properly evaluate the situation.
                    -100
                }
            }

            // Tier 1: Power Spikes (Tech & Institutions)
            Command::EmbraceInstitution { .. } => 5000,
            Command::BuyTech { tech_type } => match tech_type {
                TechType::Mil => 4500, // Military advantage is critical
                TechType::Adm => 4200,
                TechType::Dip => 4000,
            },

            // Tier 2: Expansion
            Command::StartColony { .. } => 3000,
            Command::DeclareWar { target, .. } => {
                // Only declare war if we have 1.5x strength advantage
                let own_strength = state
                    .known_country_strength
                    .get(&state.observer)
                    .copied()
                    .unwrap_or(0);
                let target_strength = state
                    .known_country_strength
                    .get(target)
                    .copied()
                    .unwrap_or(0);

                // Check if own_strength >= target_strength * 1.5
                // Using integer math: own >= target * 1.5  ⟺  own * 2 >= target * 3
                if own_strength * 2 >= target_strength * 3 {
                    2000 // Strong enough to attack
                } else {
                    -1000 // Too risky, avoid war
                }
            }

            // Tier 3: War Ops / Tactical Movement
            Command::Move { destination, .. } => {
                if state.enemy_provinces.contains(destination) {
                    1500 // Toward siege targets
                } else if state.at_war {
                    200 // Positioning
                } else {
                    50 // Peacetime movement
                }
            }
            Command::MoveFleet { .. } => 40,

            // Tier 4: Economy (Mana Sinks)
            Command::DevelopProvince { dev_type, .. } => {
                // Only develop if mana is high (avoid blocking tech)
                let mana = match dev_type {
                    DevType::Tax => state.own_country.adm_mana,
                    DevType::Production => state.own_country.dip_mana,
                    DevType::Manpower => state.own_country.mil_mana,
                };

                if mana >= crate::fixed::Fixed::from_int(800) {
                    100
                } else {
                    -1000 // Hold mana for tech
                }
            }

            // Negative or Low Priority
            Command::OfferPeace { war_id, .. } => {
                // Offer peace if we're winning decisively (war_score > 50)
                if let Some(&score) = state.our_war_score.get(war_id) {
                    let threshold = crate::fixed::Fixed::from_int(50);
                    if score > threshold {
                        800 // Winning decisively, offer peace to secure gains
                    } else {
                        -100 // Not winning enough to offer peace
                    }
                } else {
                    -100 // No war score data, don't offer
                }
            }
            Command::RejectPeace { .. } => -500, // Risky/Proud
            Command::Pass => 0,
            _ => 10, // Default for other legal commands
        }
    }
}

impl AiPlayer for GreedyAI {
    fn name(&self) -> &'static str {
        "GreedyAI"
    }

    fn decide(
        &mut self,
        visible_state: &VisibleWorldState,
        available_commands: &AvailableCommands,
    ) -> Vec<Command> {
        if available_commands.is_empty() {
            return vec![];
        }

        // Find the best move
        let mut best_cmd = None;
        let mut best_score = -999999;

        for cmd in available_commands {
            let score = self.score_command(cmd, visible_state);
            if score > best_score && score > 0 {
                best_score = score;
                best_cmd = Some(cmd.clone());
            }
        }

        if let Some(cmd) = best_cmd {
            vec![cmd]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{CountryState, Date};
    use std::collections::HashSet;

    fn dummy_state() -> VisibleWorldState {
        VisibleWorldState {
            date: Date::new(1444, 11, 11),
            observer: "SWE".to_string(),
            own_country: CountryState::default(),
            at_war: false,
            known_countries: vec![],
            enemy_provinces: HashSet::new(),
            known_country_strength: std::collections::HashMap::new(),
            our_war_score: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn test_greedy_priority() {
        let mut ai = GreedyAI::new();
        let state = dummy_state();

        let tech = Command::BuyTech {
            tech_type: TechType::Mil,
        };
        let colony = Command::StartColony { province: 1 };

        let available = vec![tech.clone(), colony.clone()];
        let decisions = ai.decide(&state, &available);

        // Tech (4500) > Colony (3000)
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0], tech);
    }

    #[test]
    fn test_greedy_mana_saving() {
        let mut ai = GreedyAI::new();
        let mut state = dummy_state();
        state.own_country.adm_mana = crate::fixed::Fixed::from_int(100);

        let dev = Command::DevelopProvince {
            province: 1,
            dev_type: DevType::Tax,
        };
        let available = vec![dev];

        let decisions = ai.decide(&state, &available);

        // Should NOT develop if mana is low
        assert!(decisions.is_empty());
    }
}
