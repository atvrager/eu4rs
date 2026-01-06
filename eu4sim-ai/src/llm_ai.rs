//! LLM-based AI player implementation.
//!
//! Wraps inference backends (Candle or Bridge) to implement the AiPlayer trait,
//! allowing trained language models to control countries in the simulation.

use crate::bridge::{BridgeClient, BridgeServer};
use crate::model::{Eu4AiModel, ModelConfig};
use crate::prompt::PromptBuilder;
use anyhow::Result;
use eu4sim_core::Command;
use eu4sim_core::ai::{AiPlayer, AvailableCommands, VisibleWorldState};
use std::path::PathBuf;
use std::sync::mpsc::Sender;

/// Inference backend selection.
#[derive(Debug, Clone)]
pub enum InferenceBackend {
    /// Use Candle for local inference (CPU/CUDA/Metal)
    Candle {
        base_model: String,
        adapter_path: Option<PathBuf>,
    },
    /// Use Python bridge for ROCm inference (server must be running)
    Bridge { host: String, port: u16 },
    /// Auto-spawn Python inference server (ROCm)
    AutoBridge { adapter_path: Option<PathBuf> },
}

impl Default for InferenceBackend {
    fn default() -> Self {
        Self::Candle {
            base_model: "HuggingFaceTB/SmolLM2-360M".to_string(),
            adapter_path: None,
        }
    }
}

/// Internal model wrapper supporting multiple backends.
/// Size difference is acceptable - Candle variant is large due to model weights,
/// and boxing would add indirection overhead on the hot inference path.
#[allow(clippy::large_enum_variant)]
enum ModelBackend {
    Candle(Eu4AiModel),
    Bridge(BridgeClient),
    /// Managed server (auto-spawned, killed on drop)
    AutoBridge(BridgeServer),
}

impl ModelBackend {
    /// Run inference and return generated text.
    fn generate(&mut self, prompt: &str, max_tokens: usize) -> Result<(String, u64)> {
        match self {
            ModelBackend::Candle(model) => {
                let start = std::time::Instant::now();
                let response = model.choose_multi_action(prompt, max_tokens)?;
                let inference_ms = start.elapsed().as_millis() as u64;
                Ok((response, inference_ms))
            }
            ModelBackend::Bridge(client) => client.generate(prompt, max_tokens),
            ModelBackend::AutoBridge(server) => server.generate(prompt, max_tokens),
        }
    }
}

/// Message containing LLM prompt and response for TUI display.
#[derive(Debug, Clone)]
pub struct LlmMessage {
    /// Country tag making the decision
    pub country: String,
    /// Game date as string (e.g., "1445.3.15")
    pub date: String,
    /// Truncated prompt (last N lines showing actions)
    pub prompt_excerpt: String,
    /// Model response
    pub response: String,
    /// Parsed commands (formatted for display)
    pub commands: Vec<String>,
    /// Inference time in milliseconds (0 for errors)
    pub inference_ms: u64,
}

/// LLM-powered AI player.
///
/// Uses a trained language model (SmolLM2, Gemma-3, or Gemma-2 with LoRA adapter)
/// to make decisions based on game state.
///
/// Supports multiple inference backends:
/// - **Candle**: Pure Rust, supports CPU/CUDA/Metal
/// - **Bridge**: Python server, supports ROCm (AMD GPU)
pub struct LlmAi {
    backend: ModelBackend,
    prompt_builder: PromptBuilder,
    /// Optional sender for TUI display of prompt/response
    tui_tx: Option<Sender<LlmMessage>>,
}

impl LlmAi {
    /// Create a new LLM AI with the specified backend.
    pub fn with_backend(backend: InferenceBackend) -> Result<Self> {
        let model_backend = match backend {
            InferenceBackend::Candle {
                base_model,
                adapter_path,
            } => {
                let config = ModelConfig {
                    base_model,
                    adapter_path: adapter_path.unwrap_or_default(),
                    ..Default::default()
                };
                ModelBackend::Candle(Eu4AiModel::load(config)?)
            }
            InferenceBackend::Bridge { host, port } => {
                let mut client = BridgeClient::with_address(host, port);
                // Try to connect eagerly to fail fast
                if !client.try_connect()? {
                    anyhow::bail!(
                        "Could not connect to inference server. \
                        Start it with: cd scripts && python inference_server.py"
                    );
                }
                ModelBackend::Bridge(client)
            }
            InferenceBackend::AutoBridge { adapter_path } => {
                log::info!("Auto-spawning inference server...");
                let server = BridgeServer::spawn(adapter_path)?;
                ModelBackend::AutoBridge(server)
            }
        };

        Ok(Self {
            backend: model_backend,
            prompt_builder: PromptBuilder::new(),
            tui_tx: None,
        })
    }

    /// Create a new LLM AI with the given configuration (Candle backend).
    ///
    /// # Arguments
    /// * `base_model` - HuggingFace model ID (e.g., "HuggingFaceTB/SmolLM2-360M", "google/gemma-3-270m")
    /// * `adapter_path` - Path to LoRA adapter directory (optional)
    pub fn new(base_model: &str, adapter_path: Option<PathBuf>) -> Result<Self> {
        Self::with_backend(InferenceBackend::Candle {
            base_model: base_model.to_string(),
            adapter_path,
        })
    }

    /// Create with Bridge backend (for ROCm inference).
    ///
    /// Connects to a Python inference server running at the specified address.
    /// Default: 127.0.0.1:9876
    pub fn with_bridge(host: impl Into<String>, port: u16) -> Result<Self> {
        Self::with_backend(InferenceBackend::Bridge {
            host: host.into(),
            port,
        })
    }

    /// Create with Bridge backend using default address.
    pub fn with_default_bridge() -> Result<Self> {
        use crate::bridge::{DEFAULT_HOST, DEFAULT_PORT};
        Self::with_bridge(DEFAULT_HOST, DEFAULT_PORT)
    }

    /// Create with auto-spawned inference server (ROCm).
    ///
    /// Spawns the Python inference server as a subprocess. Server is automatically
    /// killed when the `LlmAi` is dropped.
    pub fn with_auto_bridge(adapter_path: Option<PathBuf>) -> Result<Self> {
        Self::with_backend(InferenceBackend::AutoBridge { adapter_path })
    }

    /// Set a sender for TUI display of LLM I/O.
    pub fn with_tui_sender(mut self, tx: Sender<LlmMessage>) -> Self {
        self.tui_tx = Some(tx);
        self
    }

    /// Set the TUI sender (mutable version for post-construction).
    pub fn set_tui_sender(&mut self, tx: Sender<LlmMessage>) {
        self.tui_tx = Some(tx);
    }

    /// Create with SmolLM2 base model and LoRA adapter (Candle backend).
    pub fn with_adapter(adapter_path: PathBuf) -> Result<Self> {
        Self::new("HuggingFaceTB/SmolLM2-360M", Some(adapter_path))
    }

    /// Create with Gemma-3-270M base model and LoRA adapter (Candle backend).
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

    /// Parse multi-action response format with local per-category indices.
    ///
    /// Expected format (uses local indices within each category):
    /// ```text
    /// DIPLOMATIC:1
    /// MILITARY:1,2
    /// ECONOMIC:0
    /// TRADE:0
    /// COLONIZATION:0
    /// OTHER:0
    /// ```
    ///
    /// Local index 0 = Pass (skip), 1 = first command in category, etc.
    /// Uses index_map to convert local indices to global command array indices.
    ///
    /// Returns Vec of commands from the available list.
    /// Invalid indices are skipped with a warning.
    fn parse_multi_action_response(
        response: &str,
        available_commands: &[Command],
        index_map: &std::collections::BTreeMap<(eu4sim_core::ai::CommandCategory, usize), usize>,
    ) -> Vec<Command> {
        use eu4sim_core::ai::CommandCategory;

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

            // Parse local indices (comma-separated)
            for idx_str in indices_str.split(',') {
                let idx_str = idx_str.trim();
                if idx_str.is_empty() {
                    continue;
                }

                let Ok(local_idx) = idx_str.parse::<usize>() else {
                    log::warn!("Invalid index '{}' for category {}", idx_str, cat_str);
                    continue;
                };

                // 0 = Pass, skip
                if local_idx == 0 {
                    continue;
                }

                // Look up global index from local category index
                let Some(&global_idx) = index_map.get(&(category, local_idx)) else {
                    log::warn!(
                        "Local index {} not found in {} (max local: {})",
                        local_idx,
                        cat_str,
                        index_map
                            .keys()
                            .filter(|(cat, _)| *cat == category)
                            .map(|(_, idx)| *idx)
                            .max()
                            .unwrap_or(0)
                    );
                    continue;
                };

                result.push(available_commands[global_idx].clone());
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
        let date_str = format!(
            "{}.{}.{}",
            visible_state.date.year, visible_state.date.month, visible_state.date.day
        );

        if available_commands.is_empty() {
            // Still notify TUI that we were called but had no commands
            if let Some(ref tx) = self.tui_tx {
                let msg = LlmMessage {
                    country: visible_state.observer.clone(),
                    date: date_str,
                    prompt_excerpt: "(no commands available)".to_string(),
                    response: "PASS".to_string(),
                    commands: vec![],
                    inference_ms: 0,
                };
                let _ = tx.send(msg);
            }
            return vec![];
        }

        // Build index map for local->global index conversion
        let index_map = PromptBuilder::build_index_map(available_commands);

        // Build multi-action prompt (uses local per-category indices)
        let state_text = Self::format_state(visible_state);
        let prompt = self.prompt_builder.build_multi_action(
            &visible_state.observer,
            &state_text,
            available_commands,
        );

        // Run inference (up to 50 tokens) with timing
        match self.backend.generate(prompt, 50) {
            Ok((response, inference_ms)) => {
                let commands =
                    Self::parse_multi_action_response(&response, available_commands, &index_map);

                // Log decision
                let cmd_strings: Vec<String> =
                    commands.iter().map(Self::format_command_brief).collect();

                if commands.is_empty() {
                    log::warn!(
                        "[LLM] {} @ {} → Pass ({}ms)",
                        visible_state.observer,
                        date_str,
                        inference_ms
                    );
                } else {
                    log::warn!(
                        "[LLM] {} @ {} → {} actions ({}ms): {}",
                        visible_state.observer,
                        date_str,
                        commands.len(),
                        inference_ms,
                        cmd_strings.join(", ")
                    );
                }

                // Send to TUI if connected
                if let Some(ref tx) = self.tui_tx {
                    // Extract the <|actions|> section from the prompt
                    let prompt_excerpt = if let Some(start) = prompt.find("<|actions|>") {
                        if let Some(end) = prompt.find("<|/actions|>") {
                            prompt[start + 11..end].trim().to_string()
                        } else {
                            prompt[start + 11..]
                                .lines()
                                .take(20)
                                .collect::<Vec<_>>()
                                .join("\n")
                        }
                    } else {
                        // Fallback: last 15 lines
                        prompt
                            .lines()
                            .rev()
                            .take(15)
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev()
                            .collect::<Vec<_>>()
                            .join("\n")
                    };

                    let msg = LlmMessage {
                        country: visible_state.observer.clone(),
                        date: date_str,
                        prompt_excerpt,
                        response: response.clone(),
                        commands: cmd_strings,
                        inference_ms,
                    };
                    let _ = tx.send(msg); // Ignore send errors (TUI may have closed)
                }

                commands
            }
            Err(e) => {
                // Use {:#} for full error chain
                log::error!("LlmAi inference failed: {:#}", e);

                // Still send error to TUI so user can see what's happening
                if let Some(ref tx) = self.tui_tx {
                    let msg = LlmMessage {
                        country: visible_state.observer.clone(),
                        date: date_str,
                        prompt_excerpt: "(error)".to_string(),
                        response: format!("{:#}", e), // Full error chain
                        commands: vec![],
                        inference_ms: 0,
                    };
                    let _ = tx.send(msg);
                }

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
