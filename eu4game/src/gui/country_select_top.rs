//! Country selection top panel for map mode controls and title display.
//!
//! Renders the top section of the country selection screen with:
//! - Map mode buttons (terrain, political, religion, etc.)
//! - Start date label ("The World in 1444")
//! - Nation selection prompt

#![allow(dead_code)] // WIP - not yet integrated into rendering pipeline

use super::core::{MapMode, UiAction};
use super::primitives::{GuiButton, GuiText};
use eu4_macros::GuiWindow;

/// Country selection top panel using the binder pattern.
///
/// Binds to the `top` window in `frontend.gui`, which contains:
/// - 10 map mode buttons arranged horizontally
/// - Start date label (e.g., "The World in 1444")
/// - Nation selection prompt label
#[derive(GuiWindow)]
#[gui(window_name = "top")]
pub struct CountrySelectTopPanel {
    // Map mode buttons (left to right)
    /// Terrain map mode button
    pub mapmode_terrain: GuiButton,

    /// Political map mode button (default)
    pub mapmode_political: GuiButton,

    /// Trade nodes map mode button
    pub mapmode_trade: GuiButton,

    /// Religion map mode button
    pub mapmode_religion: GuiButton,

    /// HRE (empire) map mode button
    pub mapmode_empire: GuiButton,

    /// Diplomatic relations map mode button
    pub mapmode_diplomacy: GuiButton,

    /// Economic development map mode button
    pub mapmode_economy: GuiButton,

    /// Geographic regions map mode button
    pub mapmode_region: GuiButton,

    /// Culture groups map mode button
    pub mapmode_culture: GuiButton,

    /// Multiplayer players map mode button
    pub mapmode_players: GuiButton,

    // Text labels
    /// Start date label (e.g., "The World in 1444")
    pub year_label: GuiText,

    /// Nation selection prompt (e.g., "Select Nation")
    pub select_label: GuiText,
}

impl CountrySelectTopPanel {
    /// Update the panel state and poll for button actions.
    ///
    /// Returns an action if any button was clicked.
    pub fn update(&mut self, _current_mode: MapMode, start_year: i32) -> Option<UiAction> {
        // Update start date label
        self.year_label
            .set_text(&format!("The World in {}", start_year));

        // Poll all map mode buttons and return action if clicked
        // Each button represents a specific map mode
        if self.mapmode_terrain.poll_click().is_some() {
            return Some(UiAction::SetMapMode(MapMode::Terrain));
        }
        if self.mapmode_political.poll_click().is_some() {
            return Some(UiAction::SetMapMode(MapMode::Political));
        }
        if self.mapmode_trade.poll_click().is_some() {
            return Some(UiAction::SetMapMode(MapMode::Trade));
        }
        if self.mapmode_religion.poll_click().is_some() {
            return Some(UiAction::SetMapMode(MapMode::Religion));
        }
        if self.mapmode_empire.poll_click().is_some() {
            return Some(UiAction::SetMapMode(MapMode::Empire));
        }
        if self.mapmode_diplomacy.poll_click().is_some() {
            return Some(UiAction::SetMapMode(MapMode::Diplomacy));
        }
        if self.mapmode_economy.poll_click().is_some() {
            return Some(UiAction::SetMapMode(MapMode::Economy));
        }
        if self.mapmode_region.poll_click().is_some() {
            return Some(UiAction::SetMapMode(MapMode::Region));
        }
        if self.mapmode_culture.poll_click().is_some() {
            return Some(UiAction::SetMapMode(MapMode::Culture));
        }
        if self.mapmode_players.poll_click().is_some() {
            return Some(UiAction::SetMapMode(MapMode::Players));
        }

        None
    }

    /// Set the active map mode button state.
    ///
    /// Visually highlights the currently selected map mode button.
    /// Currently this is a placeholder - button state management will be
    /// implemented when we add button pressed/normal state rendering.
    #[allow(dead_code)]
    pub fn set_active_mode(&mut self, mode: MapMode) {
        // TODO: Implement button state management (pressed vs normal)
        // For now, all buttons use default state
        let _ = mode; // Suppress unused warning
    }
}
