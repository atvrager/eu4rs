//! Lobby control panel for game start options.
//!
//! Contains the play button and related controls in the lower-right section
//! of the country selection screen.

#![allow(dead_code)] // WIP - not yet integrated into rendering pipeline

use super::core::UiAction;
use super::primitives::GuiButton;
use eu4_macros::GuiWindow;

/// Lobby controls panel using the binder pattern.
///
/// Binds to the `right` window in `frontend.gui`, which contains:
/// - Play button (starts the game)
/// - Random country button (selects a random nation)
/// - Nation designer button (opens custom nation creator)
/// - Random new world button (toggles RNW mode)
/// - Enable custom nation button (checkbox for custom nation toggle)
///
/// Note: observe_mode_button is in the `left` panel, not here.
#[derive(Debug, GuiWindow)]
#[gui(window_name = "right")]
pub struct LobbyControlsPanel {
    /// Main play button - starts the game with selected country
    pub play_button: GuiButton,

    /// Random country button - selects a random playable nation
    pub random_country_button: GuiButton,

    /// Nation designer button - opens the custom nation designer
    pub nation_designer_button: GuiButton,

    /// Random New World button - toggles random new world mode
    pub random_new_world_button: GuiButton,

    /// Enable custom nation checkbox (uses button with checkbox sprite)
    pub enable_custom_nation_button: GuiButton,
}

impl LobbyControlsPanel {
    /// Update the panel state and poll for button actions.
    ///
    /// Returns an action if the play button was clicked.
    pub fn update(&mut self, _country_selected: bool) -> Option<UiAction> {
        // Poll play button
        if self.play_button.poll_click().is_some() {
            // TODO: Validate country is selected before allowing start
            // For now, always allow start (validation will be added in Phase 9)
            return Some(UiAction::StartGame);
        }

        None
    }

    /// Enable or disable the play button based on selection state.
    ///
    /// The play button should only be enabled when a country is selected.
    pub fn set_play_enabled(&mut self, enabled: bool) {
        self.play_button.set_enabled(enabled);
    }
}
