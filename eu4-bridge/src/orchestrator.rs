//! Main AI decision loop orchestrator.
//!
//! Coordinates: pause → capture → OCR → AI → execute → unpause.

use crate::actions::ActionExecutor;
use crate::capture;
use crate::extraction::Extractor;
use crate::input::InputController;
use anyhow::Result;
use eu4sim_ai::LlmAi;
use eu4sim_core::Command;
use eu4sim_core::ai::AiPlayer;
use image::DynamicImage;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

/// Orchestrator for the AI decision loop.
pub struct Orchestrator {
    extractor: Extractor,
    input: InputController,
    ai: LlmAi,
    /// Delay between AI decision ticks.
    pub tick_delay: Duration,
    /// Whether to skip pause/unpause (for testing with screenshots).
    pub skip_pause: bool,
    /// Whether to execute AI decisions (click buttons). Default: true.
    pub execute_actions: bool,
}

impl Orchestrator {
    /// Create a new orchestrator with the given LoRA adapter path.
    pub fn new(adapter_path: &str) -> Result<Self> {
        log::info!("Initializing orchestrator...");

        let extractor = Extractor::new(None)?;
        log::info!("  OCR engine loaded");

        let input = InputController::new()?;
        log::info!("  Input controller ready");

        let ai = LlmAi::with_adapter(PathBuf::from(adapter_path))?;
        log::info!("  AI model loaded from: {}", adapter_path);

        Ok(Self {
            extractor,
            input,
            ai,
            tick_delay: Duration::from_secs(5),
            skip_pause: false,
            execute_actions: true,
        })
    }

    /// Create orchestrator without AI (for testing OCR loop only).
    pub fn without_ai() -> Result<Self> {
        log::info!("Initializing orchestrator (no AI)...");

        let extractor = Extractor::new(None)?;
        let input = InputController::new()?;

        // Use base model without adapter for testing
        let ai = LlmAi::with_base_model()?;

        Ok(Self {
            extractor,
            input,
            ai,
            tick_delay: Duration::from_secs(5),
            skip_pause: false,
            execute_actions: true,
        })
    }

    /// Run one decision cycle.
    ///
    /// 1. Pause game
    /// 2. Capture screen
    /// 3. Extract state via OCR
    /// 4. Call AI for decision
    /// 5. Execute decisions (if `execute_actions` is true)
    /// 6. Unpause game
    pub fn tick_once(&mut self, window_title: &str) -> Result<()> {
        // 1. Pause game
        if !self.skip_pause {
            log::debug!("Pausing game...");
            self.input.toggle_pause()?;
            thread::sleep(Duration::from_millis(500)); // Wait for pause animation
        }

        // 2. Capture screen
        log::debug!("Capturing screen...");
        let window = capture::find_window(window_title)?;
        let rgba_image = capture::capture_window(&window)?;
        let image = DynamicImage::ImageRgba8(rgba_image);

        // 3. Extract state via OCR
        log::debug!("Running OCR extraction...");
        let extracted = self.extractor.extract_all_verbose(&image, false);

        // Log extracted state summary
        log::info!(
            "Extracted: {} @ {} | Treasury: {} | Mana: {}/{}/{} | Stability: {}",
            extracted.country.as_deref().unwrap_or("?"),
            extracted.date.as_deref().unwrap_or("?"),
            extracted
                .treasury
                .map(|v| format!("{:.0}", v))
                .unwrap_or_else(|| "?".into()),
            extracted
                .adm_mana
                .map(|v| v.to_string())
                .unwrap_or_else(|| "?".into()),
            extracted
                .dip_mana
                .map(|v| v.to_string())
                .unwrap_or_else(|| "?".into()),
            extracted
                .mil_mana
                .map(|v| v.to_string())
                .unwrap_or_else(|| "?".into()),
            extracted
                .stability
                .map(|v| format!("{:+}", v))
                .unwrap_or_else(|| "?".into()),
        );

        // Convert to AI-compatible state
        let visible_state = extracted.to_visible_state();

        // 4. Get available commands (hardcoded for Phase B)
        let available_commands = self.get_available_commands();

        // 5. Call AI for decision
        log::debug!("Calling AI...");
        let decisions = self.ai.decide(&visible_state, &available_commands);

        // 6. Execute decisions (Phase C)
        if self.execute_actions {
            let mut executor = ActionExecutor::new(&mut self.input);
            let executed = executor.execute_all(&decisions);
            log::info!(
                "AI decisions: {} total, {} executed",
                decisions.len(),
                executed
            );
        } else {
            for cmd in &decisions {
                log::info!("AI decision (no exec): {:?}", cmd);
            }
        }

        // 7. Unpause game
        if !self.skip_pause {
            log::debug!("Unpausing game...");
            self.input.toggle_pause()?;
        }

        Ok(())
    }

    /// Run continuous decision loop.
    pub fn run_loop(&mut self, window_title: &str) -> Result<()> {
        log::info!(
            "Starting AI loop (tick every {}s, Ctrl+C to stop)",
            self.tick_delay.as_secs()
        );

        loop {
            if let Err(e) = self.tick_once(window_title) {
                log::error!("Tick failed: {}", e);
                // Don't crash on single tick failure, continue loop
            }
            thread::sleep(self.tick_delay);
        }
    }

    /// Get available commands for Phase B (hardcoded simple set).
    ///
    /// In later phases, this would be computed from game state.
    fn get_available_commands(&self) -> Vec<Command> {
        // Pass is always available (do nothing)
        // For Phase B, we just need some commands for the AI to choose from
        vec![
            Command::Pass,
            // Add more as we implement execution in Phase C
        ]
    }
}

#[cfg(test)]
mod tests {
    // Integration tests would go here, but require game running
    // For now, we test individual components separately
}
