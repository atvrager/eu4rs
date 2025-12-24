use crate::ai::{
    categorize_command, AiPlayer, AvailableCommands, CommandCategory, VisibleWorldState,
};
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
                // Evaluate strength vs COMBINED enemies (current wars + new target)
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

                // Total enemy strength = current war enemies + new target
                let total_enemy_strength = state.current_war_enemy_strength + target_strength;

                // Check coalition risk - count countries with high AE toward us
                let ae_risk = state
                    .own_ae
                    .values()
                    .filter(|ae| **ae > crate::fixed::Fixed::from_int(40))
                    .count();

                // Don't declare if coalition is forming (3+ angry countries)
                if ae_risk >= 3 {
                    -2000 // Coalition risk too high
                } else if own_strength * 2 >= total_enemy_strength * 3 {
                    // Need 1.5x advantage over ALL enemies combined
                    2000 // Strong enough to handle all enemies
                } else {
                    -1000 // Can't afford another war right now
                }
            }

            // Tier 2.5: Leadership & Generals
            Command::RecruitGeneral => {
                // Only recruit during war if we have armies without generals
                if state.at_war
                    && state.own_country.mil_mana >= crate::fixed::Fixed::from_int(50)
                    && !state.armies_without_general.is_empty()
                {
                    3000 // High priority during war
                } else {
                    -100 // Save mana otherwise
                }
            }

            Command::AssignGeneral { army, .. } => {
                // High priority to assign generals to armies that need them
                if state.armies_without_general.contains(army) {
                    2500 // Immediate benefit
                } else {
                    -100 // Army already has one
                }
            }

            // Tier 2.5: Army Consolidation - merge small stacks for efficiency
            Command::MergeArmies { army_ids } => {
                // Consolidating armies is almost always good - reduces micro, improves combat
                if army_ids.len() >= 2 {
                    // Higher score for more armies merged (2 armies = 1500, 3 = 2250, etc.)
                    1500 * (army_ids.len() as i32 - 1)
                } else {
                    -100 // Invalid merge
                }
            }

            // Tier 3: War Ops / Tactical Movement
            Command::Move {
                army_id,
                destination,
            } => {
                let army_size = state.our_army_sizes.get(army_id).copied().unwrap_or(0);

                let mut base_score = if state.enemy_provinces.contains(destination) {
                    // Small armies should NOT move into enemy territory - consolidate first!
                    if army_size < 5 {
                        return -1000;
                    }
                    // Bonus for forts (priority siege targets)
                    if state.fort_provinces.contains(destination) {
                        2000 // Fort = high priority
                    } else {
                        1500 // Regular enemy province
                    }
                } else if state.at_war {
                    200 // Positioning
                } else {
                    50 // Peacetime movement
                };

                // Consolidation bonus: move toward provinces with friendly stacks
                // This creates "gravitational pull" so armies cluster together
                if let Some(&friendly_regs) = state.our_army_provinces.get(destination) {
                    if friendly_regs > 0 && friendly_regs < 20 {
                        // Bonus scales with how many troops are there
                        base_score += 500 + (friendly_regs as i32 * 50);
                    }
                }

                // Attrition penalty: avoid stacking over supply limit
                let supply = state
                    .province_supply
                    .get(destination)
                    .copied()
                    .unwrap_or(10);
                let current = state.army_locations.get(destination).copied().unwrap_or(0);
                if current >= supply {
                    base_score -= 1000; // Heavy penalty for attrition risk
                }

                base_score
            }

            Command::MoveFleet { .. } => {
                // Check if this position would block a strategically important strait
                // For now, basic scoring - could be enhanced with strait awareness
                if state.at_war {
                    100 // Fleet positioning during war
                } else {
                    40 // Peacetime
                }
            }

            // Call-to-arms: honor alliances
            Command::JoinWar { war_id, .. } => {
                // Almost always honor alliances (affects trust)
                if state.pending_call_to_arms.iter().any(|(w, _)| w == war_id) {
                    1500 // High priority to maintain alliances
                } else {
                    -100 // Not a valid CTA
                }
            }

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

            // Tier 5: Trade Actions - Steady income benefits. ✧
            Command::SendMerchant { action, .. } => {
                use crate::trade::MerchantAction;
                match action {
                    MerchantAction::Collect => 150,      // Collecting is good
                    MerchantAction::Steer { .. } => 180, // Steering is slightly better (value magnification)
                }
            }
            Command::RecallMerchant { .. } => -200, // Rarely want to recall

            // Negative or Low Priority
            Command::OfferPeace { war_id, terms } => {
                // Offer peace if we have any war score from occupation
                if let Some(&score) = state.our_war_score.get(war_id) {
                    let threshold = crate::fixed::Fixed::from_int(1);
                    if score >= threshold {
                        // Prefer TakeProvinces over WhitePeace when winning
                        match terms {
                            crate::state::PeaceTerms::TakeProvinces { provinces }
                                if !provinces.is_empty() =>
                            {
                                1000 // Take the provinces!
                            }
                            crate::state::PeaceTerms::WhitePeace => 400, // Less preferred
                            _ => 500,
                        }
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

        let mut result = Vec::new();

        // Group commands by category
        let mut diplomatic: Vec<(&Command, i32)> = Vec::new();
        let mut military: Vec<(&Command, i32)> = Vec::new();
        let mut economic: Vec<(&Command, i32)> = Vec::new();
        let mut trade: Vec<(&Command, i32)> = Vec::new();
        let mut colonization: Vec<(&Command, i32)> = Vec::new();

        for cmd in available_commands {
            let score = self.score_command(cmd, visible_state);
            if score <= 0 {
                continue; // Skip negative-value commands
            }

            match categorize_command(cmd) {
                CommandCategory::Diplomatic => diplomatic.push((cmd, score)),
                CommandCategory::Military => military.push((cmd, score)),
                CommandCategory::Economic => economic.push((cmd, score)),
                CommandCategory::Trade => trade.push((cmd, score)),
                CommandCategory::Colonization => colonization.push((cmd, score)),
                CommandCategory::Other => {} // Skip Pass, etc.
            }
        }

        // 1. Pick ONE best diplomatic action (one per day limit)
        if let Some((cmd, _)) = diplomatic.iter().max_by_key(|(_, score)| *score) {
            result.push((*cmd).clone());
        }

        // 2. Add ALL positive-score military moves (armies should move together)
        for (cmd, _) in &military {
            result.push((*cmd).clone());
        }

        // 3. Add ONE best economic action (mana management)
        if let Some((cmd, _)) = economic.iter().max_by_key(|(_, score)| *score) {
            result.push((*cmd).clone());
        }

        // 4. Add ALL positive-score trade actions
        for (cmd, _) in &trade {
            result.push((*cmd).clone());
        }

        // 5. Add ALL positive-score colonization
        for (cmd, _) in &colonization {
            result.push((*cmd).clone());
        }

        result
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
            own_generals: vec![],
            armies_without_general: vec![],
            own_fleets: vec![],
            blocked_straits: HashSet::new(),
            province_supply: std::collections::HashMap::new(),
            army_locations: std::collections::HashMap::new(),
            own_ae: std::collections::HashMap::new(),
            coalition_against_us: None,
            fort_provinces: HashSet::new(),
            active_sieges: vec![],
            pending_call_to_arms: vec![],
            current_war_enemy_strength: 0,
            our_army_sizes: std::collections::HashMap::new(),
            our_army_provinces: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn test_greedy_multi_category() {
        let mut ai = GreedyAI::new();
        let state = dummy_state();

        let tech = Command::BuyTech {
            tech_type: TechType::Mil,
        };
        let colony = Command::StartColony { province: 1 };

        let available = vec![tech.clone(), colony.clone()];
        let decisions = ai.decide(&state, &available);

        // Multi-command: Tech (Economic) + Colony (Colonization) both returned
        // since they're in different categories
        assert_eq!(decisions.len(), 2);
        assert!(decisions.contains(&tech));
        assert!(decisions.contains(&colony));
    }

    #[test]
    fn test_greedy_same_category_picks_best() {
        let mut ai = GreedyAI::new();
        let state = dummy_state();

        // Two economic actions: should pick the higher-scored one (tech > dev)
        let mil_tech = Command::BuyTech {
            tech_type: TechType::Mil,
        };
        let adm_tech = Command::BuyTech {
            tech_type: TechType::Adm,
        };

        let available = vec![mil_tech.clone(), adm_tech.clone()];
        let decisions = ai.decide(&state, &available);

        // Both are Economic category - picks ONE best (Mil tech = 4500 > Adm tech = 4200)
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0], mil_tech);
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

    // =========================================================================
    // Warfare Tests
    // =========================================================================

    #[test]
    fn test_greedy_recruits_general_during_war() {
        let ai = GreedyAI::new();
        let mut state = dummy_state();
        state.at_war = true;
        state.own_country.mil_mana = crate::fixed::Fixed::from_int(100);
        state.armies_without_general = vec![1, 2];

        let cmd = Command::RecruitGeneral;
        let score = ai.score_command(&cmd, &state);

        // Should prioritize recruiting general during war
        assert_eq!(score, 3000);
    }

    #[test]
    fn test_greedy_skips_general_at_peace() {
        let ai = GreedyAI::new();
        let mut state = dummy_state();
        state.at_war = false;
        state.own_country.mil_mana = crate::fixed::Fixed::from_int(100);

        let cmd = Command::RecruitGeneral;
        let score = ai.score_command(&cmd, &state);

        // Should NOT recruit general during peace
        assert_eq!(score, -100);
    }

    #[test]
    fn test_greedy_assigns_general_to_army() {
        let ai = GreedyAI::new();
        let mut state = dummy_state();
        state.armies_without_general = vec![5];

        let cmd = Command::AssignGeneral {
            general: 1,
            army: 5,
        };
        let score = ai.score_command(&cmd, &state);

        // Should assign general to army that needs one
        assert_eq!(score, 2500);
    }

    #[test]
    fn test_greedy_avoids_attrition_stacking() {
        let ai = GreedyAI::new();
        let mut state = dummy_state();
        state.province_supply.insert(10, 5); // Supply limit = 5
        state.army_locations.insert(10, 5); // Already at limit

        let cmd = Command::Move {
            army_id: 1,
            destination: 10,
        };
        let score = ai.score_command(&cmd, &state);

        // Should heavily penalize moving to over-supplied province
        assert!(score < 0);
    }

    #[test]
    fn test_greedy_prioritizes_fort_sieges() {
        let ai = GreedyAI::new();
        let mut state = dummy_state();
        state.enemy_provinces.insert(10);
        state.enemy_provinces.insert(20);
        state.fort_provinces.insert(20); // Province 20 has a fort

        // Add army sizes so Move scoring doesn't reject them as too small
        state.our_army_sizes.insert(1, 10);
        state.our_army_sizes.insert(2, 10);

        let move_regular = Command::Move {
            army_id: 1,
            destination: 10,
        };
        let move_fort = Command::Move {
            army_id: 2,
            destination: 20,
        };

        let score_regular = ai.score_command(&move_regular, &state);
        let score_fort = ai.score_command(&move_fort, &state);

        // Fort siege should score higher
        assert!(score_fort > score_regular);
        assert_eq!(score_fort, 2000);
        assert_eq!(score_regular, 1500);
    }

    #[test]
    fn test_greedy_honors_call_to_arms() {
        let ai = GreedyAI::new();
        let mut state = dummy_state();
        state.pending_call_to_arms.push((42, "FRA".to_string()));

        let cmd = Command::JoinWar {
            war_id: 42,
            side: crate::input::WarSide::Defender,
        };
        let score = ai.score_command(&cmd, &state);

        // Should honor alliance
        assert_eq!(score, 1500);
    }

    #[test]
    fn test_greedy_coalition_awareness() {
        let ai = GreedyAI::new();
        let mut state = dummy_state();

        // High AE with 3 countries
        state
            .own_ae
            .insert("FRA".to_string(), crate::fixed::Fixed::from_int(50));
        state
            .own_ae
            .insert("CAS".to_string(), crate::fixed::Fixed::from_int(45));
        state
            .own_ae
            .insert("ENG".to_string(), crate::fixed::Fixed::from_int(42));

        state.known_country_strength.insert("SWE".to_string(), 100);
        state.known_country_strength.insert("DEN".to_string(), 30);

        let cmd = Command::DeclareWar {
            target: "DEN".to_string(),
            cb: None,
        };
        let score = ai.score_command(&cmd, &state);

        // Should avoid war due to coalition risk
        assert_eq!(score, -2000);
    }

    #[test]
    fn test_greedy_declares_war_without_coalition_risk() {
        let ai = GreedyAI::new();
        let mut state = dummy_state();

        // Low AE
        state
            .own_ae
            .insert("FRA".to_string(), crate::fixed::Fixed::from_int(20));

        state.known_country_strength.insert("SWE".to_string(), 100);
        state.known_country_strength.insert("DEN".to_string(), 30);

        let cmd = Command::DeclareWar {
            target: "DEN".to_string(),
            cb: None,
        };
        let score = ai.score_command(&cmd, &state);

        // Should declare war (strong + no coalition risk)
        assert_eq!(score, 2000);
    }
}
