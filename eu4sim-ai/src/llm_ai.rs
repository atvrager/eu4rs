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
/// Uses a trained language model (SmolLM2, Gemma-3, or Gemma-2 with LoRA adapter)
/// to make decisions based on game state.
pub struct LlmAi {
    model: Eu4AiModel,
    prompt_builder: PromptBuilder,
}

impl LlmAi {
    /// Create a new LLM AI with the given configuration.
    ///
    /// # Arguments
    /// * `base_model` - HuggingFace model ID (e.g., "HuggingFaceTB/SmolLM2-360M", "google/gemma-3-270m")
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

    /// Create with SmolLM2 base model and LoRA adapter.
    pub fn with_adapter(adapter_path: PathBuf) -> Result<Self> {
        Self::new("HuggingFaceTB/SmolLM2-360M", Some(adapter_path))
    }

    /// Create with Gemma-3-270M base model and LoRA adapter.
    pub fn with_gemma3_adapter(adapter_path: PathBuf) -> Result<Self> {
        Self::new("google/gemma-3-270m", Some(adapter_path))
    }

    /// Create with SmolLM2 base model (no adapter, uses pretrained weights only).
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

    /// Parse multi-action response format.
    ///
    /// Expected format:
    /// ```text
    /// DIPLOMATIC:1
    /// MILITARY:2,3
    /// ECONOMIC:0
    /// TRADE:0
    /// COLONIZATION:0
    /// OTHER:0
    /// ```
    ///
    /// Returns Vec of commands from the available list.
    /// Invalid indices are skipped with a warning.
    fn parse_multi_action_response(response: &str, available_commands: &[Command]) -> Vec<Command> {
        use eu4sim_core::ai::{CommandCategory, categorize_command};

        let mut result = Vec::new();

        for line in response.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Parse "CATEGORY:INDEX[,INDEX]*"
            let Some((cat_str, indices_str)) = line.split_once(':') else {
                log::warn!("Malformed line (no colon): '{}'", line);
                continue;
            };

            let cat_str = cat_str.trim();
            let indices_str = indices_str.trim();

            // Parse category
            let category = match cat_str {
                "DIPLOMATIC" => CommandCategory::Diplomatic,
                "MILITARY" => CommandCategory::Military,
                "ECONOMIC" => CommandCategory::Economic,
                "TRADE" => CommandCategory::Trade,
                "COLONIZATION" => CommandCategory::Colonization,
                "OTHER" => CommandCategory::Other,
                _ => {
                    log::warn!("Unknown category: '{}'", cat_str);
                    continue;
                }
            };

            // Parse indices (comma-separated)
            for idx_str in indices_str.split(',') {
                let idx_str = idx_str.trim();
                if idx_str.is_empty() {
                    continue;
                }

                let Ok(idx) = idx_str.parse::<usize>() else {
                    log::warn!("Invalid index '{}' for category {}", idx_str, cat_str);
                    continue;
                };

                // 0 = Pass, skip
                if idx == 0 {
                    continue;
                }

                // Validate index
                if idx >= available_commands.len() {
                    log::warn!(
                        "Index {} out of range (max {}) for category {}",
                        idx,
                        available_commands.len() - 1,
                        cat_str
                    );
                    continue;
                }

                // Validate category matches
                let cmd = &available_commands[idx];
                if categorize_command(cmd) != category {
                    log::warn!(
                        "Index {} is {:?} but listed under {}",
                        idx,
                        categorize_command(cmd),
                        cat_str
                    );
                    continue;
                }

                result.push(cmd.clone());
            }
        }

        result
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
        if available_commands.is_empty() {
            return vec![];
        }

        // Build multi-action prompt
        let state_text = Self::format_state(visible_state);
        let prompt = self.prompt_builder.build_multi_action(
            &visible_state.observer,
            &state_text,
            available_commands,
        );

        // Run inference (up to 50 tokens)
        match self.model.choose_multi_action(prompt, 50) {
            Ok(response) => {
                let commands = Self::parse_multi_action_response(&response, available_commands);

                // Log decision
                if commands.is_empty() {
                    log::warn!(
                        "[LLM] {} @ {}.{}.{} → Pass (no valid actions parsed)",
                        visible_state.observer,
                        visible_state.date.year,
                        visible_state.date.month,
                        visible_state.date.day
                    );
                } else {
                    log::warn!(
                        "[LLM] {} @ {}.{}.{} → {} actions: {}",
                        visible_state.observer,
                        visible_state.date.year,
                        visible_state.date.month,
                        visible_state.date.day,
                        commands.len(),
                        commands
                            .iter()
                            .map(Self::format_command_brief)
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }

                commands
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
