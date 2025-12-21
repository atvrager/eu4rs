//! LLM-based AI player implementation.
//!
//! Wraps the Eu4AiModel to implement the AiPlayer trait,
//! allowing trained language models to control countries in the simulation.

use crate::model::{Eu4AiModel, ModelConfig};
use crate::prompt::PromptBuilder;
use anyhow::Result;
use eu4sim_core::Command;
use eu4sim_core::ai::{AiPlayer, AvailableCommands, VisibleWorldState};
use std::path::PathBuf;

/// LLM-powered AI player.
///
/// Uses a trained language model (SmolLM2 or Gemma with LoRA adapter)
/// to make decisions based on game state.
pub struct LlmAi {
    model: Eu4AiModel,
    prompt_builder: PromptBuilder,
}

impl LlmAi {
    /// Create a new LLM AI with the given configuration.
    ///
    /// # Arguments
    /// * `base_model` - HuggingFace model ID (e.g., "HuggingFaceTB/SmolLM2-360M")
    /// * `adapter_path` - Path to LoRA adapter directory (optional)
    pub fn new(base_model: &str, adapter_path: Option<PathBuf>) -> Result<Self> {
        let config = ModelConfig {
            base_model: base_model.to_string(),
            adapter_path: adapter_path.unwrap_or_default(),
            ..Default::default()
        };

        let model = Eu4AiModel::load(config)?;

        Ok(Self {
            model,
            prompt_builder: PromptBuilder::new(),
        })
    }

    /// Create with default SmolLM2 base model and LoRA adapter.
    pub fn with_adapter(adapter_path: PathBuf) -> Result<Self> {
        Self::new("HuggingFaceTB/SmolLM2-360M", Some(adapter_path))
    }

    /// Create with default SmolLM2 base model (no adapter, uses pretrained weights only).
    pub fn with_base_model() -> Result<Self> {
        Self::new("HuggingFaceTB/SmolLM2-360M", None)
    }

    /// Format the visible state as a text description for the prompt.
    fn format_state(state: &VisibleWorldState) -> String {
        let mut s = String::new();

        // Date
        s.push_str(&format!(
            "Date: {}.{}.{}\n",
            state.date.year, state.date.month, state.date.day
        ));

        // Treasury and resources
        s.push_str(&format!(
            "Treasury: {:.0} ducats\n",
            state.own_country.treasury.to_f32()
        ));
        s.push_str(&format!(
            "Manpower: {:.0}\n",
            state.own_country.manpower.to_f32()
        ));

        // Mana points
        s.push_str(&format!(
            "Admin: {:.0} / Diplo: {:.0} / Mil: {:.0}\n",
            state.own_country.adm_mana.to_f32(),
            state.own_country.dip_mana.to_f32(),
            state.own_country.mil_mana.to_f32()
        ));

        // War status
        if state.at_war {
            s.push_str("Status: AT WAR\n");
            for (war_id, score) in &state.our_war_score {
                s.push_str(&format!("  War {}: {:.0}% score\n", war_id, score.to_f32()));
            }
        } else {
            s.push_str("Status: At peace\n");
        }

        // Known neighbors
        if !state.known_countries.is_empty() {
            s.push_str(&format!(
                "Known nations: {}\n",
                state
                    .known_countries
                    .iter()
                    .take(5)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        s
    }

    /// Format a command for brief logging output.
    fn format_command_brief(cmd: &Command) -> String {
        match cmd {
            Command::Move {
                army_id,
                destination,
            } => {
                format!("Move Army {} → {}", army_id, destination)
            }
            Command::MoveFleet {
                fleet_id,
                destination,
            } => {
                format!("Move Fleet {} → {}", fleet_id, destination)
            }
            Command::DeclareWar { target, cb } => {
                if let Some(cb) = cb {
                    format!("War on {} ({})", target, cb)
                } else {
                    format!("War on {}", target)
                }
            }
            Command::OfferPeace { war_id, .. } => format!("Peace in war {}", war_id),
            Command::OfferAlliance { target } => format!("Alliance → {}", target),
            Command::BuyTech { tech_type } => format!("Tech {:?}", tech_type),
            Command::BuildInProvince { province, building } => {
                format!("Build {} in {}", building, province)
            }
            Command::DevelopProvince { province, dev_type } => {
                format!("Dev {:?} in {}", dev_type, province)
            }
            Command::Pass => "Pass".to_string(),
            other => format!("{:?}", other),
        }
    }
}

impl AiPlayer for LlmAi {
    fn name(&self) -> &'static str {
        "LlmAi"
    }

    fn decide(
        &mut self,
        visible_state: &VisibleWorldState,
        available_commands: &AvailableCommands,
    ) -> Vec<Command> {
        // Always include Pass as action 0, then up to 9 other actions
        let max_other_actions = 9;
        let mut commands: Vec<Command> = vec![Command::Pass];
        commands.extend(
            available_commands
                .iter()
                .filter(|c| **c != Command::Pass)
                .take(max_other_actions)
                .cloned(),
        );

        // Build the prompt
        let state_text = Self::format_state(visible_state);
        let prompt = self
            .prompt_builder
            .build(&visible_state.observer, &state_text, &commands);

        // Run inference
        match self.model.choose_action(prompt) {
            Ok(action_idx) => {
                if action_idx < commands.len() {
                    let cmd = &commands[action_idx];
                    // Always log the decision with action index and description
                    log::warn!(
                        "[LLM] {} @ {}.{}.{} → [{}] {}",
                        visible_state.observer,
                        visible_state.date.year,
                        visible_state.date.month,
                        visible_state.date.day,
                        action_idx,
                        Self::format_command_brief(cmd)
                    );
                    vec![cmd.clone()]
                } else {
                    log::warn!(
                        "LlmAi returned invalid action index {} (only {} actions available)",
                        action_idx,
                        commands.len()
                    );
                    vec![]
                }
            }
            Err(e) => {
                log::error!("LlmAi inference failed: {}", e);
                vec![] // Pass on error
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eu4sim_core::state::Date;

    #[test]
    fn test_format_state() {
        let state = VisibleWorldState {
            date: Date::new(1444, 11, 11),
            observer: "FRA".to_string(),
            at_war: false,
            ..Default::default()
        };

        let text = LlmAi::format_state(&state);
        assert!(text.contains("1444.11.11"));
        assert!(text.contains("At peace"));
    }
}
