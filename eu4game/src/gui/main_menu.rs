//! Main menu panel for the game frontend.
//!
//! Renders the main menu with buttons for single player, multiplayer,
//! tutorial, credits, settings, and exit.

use super::core::UiAction;
use super::primitives::GuiButton;
use eu4_macros::GuiWindow;

/// Main menu panel using the binder pattern.
///
/// This panel binds to the `mainmenu` window in `frontend.gui`
/// and provides interactive buttons for navigating the game frontend.
#[derive(GuiWindow)]
#[gui(window_name = "mainmenu")]
#[allow(dead_code)] // Reserved for Phase 4+ frontend implementation
pub struct MainMenuPanel {
    /// Single player button - starts country selection
    #[gui(name = "single_player")]
    pub single_player: GuiButton,

    /// Multiplayer button - starts multiplayer setup
    #[gui(name = "multi_player")]
    pub multi_player: GuiButton,

    /// Tutorial button - shows tutorial
    #[gui(optional)]
    pub tutorial: Option<GuiButton>,

    /// Credits button - shows credits
    #[gui(optional)]
    pub credits: Option<GuiButton>,

    /// Settings button - shows settings
    #[gui(optional)]
    pub settings: Option<GuiButton>,

    /// Exit button - exits the game
    pub exit: GuiButton,
}

impl MainMenuPanel {
    /// Initialize button actions after binding.
    ///
    /// The binder creates buttons with `UiAction::None` by default.
    /// This method sets the appropriate action for each button.
    #[allow(dead_code)] // Reserved for Phase 4+ frontend implementation
    pub fn init_actions(&mut self) {
        self.single_player.set_action(UiAction::ShowSinglePlayer);
        self.multi_player.set_action(UiAction::ShowMultiplayer);
        self.exit.set_action(UiAction::Exit);

        if let Some(ref mut tutorial) = self.tutorial {
            tutorial.set_action(UiAction::ShowTutorial);
        }
        if let Some(ref mut credits) = self.credits {
            credits.set_action(UiAction::ShowCredits);
        }
        if let Some(ref mut settings) = self.settings {
            settings.set_action(UiAction::ShowSettings);
        }
    }

    /// Poll all buttons for clicks and return any action.
    ///
    /// Returns the first action found, or None if no buttons were clicked.
    #[allow(dead_code)] // Reserved for Phase 4+ frontend implementation
    pub fn poll_actions(&mut self) -> Option<UiAction> {
        if let Some(action) = self.single_player.poll_click() {
            return Some(action);
        }
        if let Some(action) = self.multi_player.poll_click() {
            return Some(action);
        }
        if let Some(action) = self.exit.poll_click() {
            return Some(action);
        }

        if let Some(ref mut tutorial) = self.tutorial
            && let Some(action) = tutorial.poll_click()
        {
            return Some(action);
        }
        if let Some(ref mut credits) = self.credits
            && let Some(action) = credits.poll_click()
        {
            return Some(action);
        }
        if let Some(ref mut settings) = self.settings
            && let Some(action) = settings.poll_click()
        {
            return Some(action);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::binder::Bindable;
    use crate::gui::primitives::GuiButton;

    #[test]
    fn test_main_menu_panel_ci_mode() {
        // In CI mode (no assets), panel should still construct with placeholders
        let panel = MainMenuPanel {
            single_player: GuiButton::placeholder(),
            multi_player: GuiButton::placeholder(),
            tutorial: Some(GuiButton::placeholder()),
            credits: Some(GuiButton::placeholder()),
            settings: Some(GuiButton::placeholder()),
            exit: GuiButton::placeholder(),
        };

        assert_eq!(panel.single_player.name(), "<placeholder>");
    }

    #[test]
    fn test_init_actions() {
        let mut panel = MainMenuPanel {
            single_player: GuiButton::placeholder(),
            multi_player: GuiButton::placeholder(),
            tutorial: Some(GuiButton::placeholder()),
            credits: None,
            settings: None,
            exit: GuiButton::placeholder(),
        };

        panel.init_actions();

        assert_eq!(panel.single_player.action(), UiAction::ShowSinglePlayer);
        assert_eq!(panel.multi_player.action(), UiAction::ShowMultiplayer);
        assert_eq!(panel.exit.action(), UiAction::Exit);
        assert_eq!(
            panel.tutorial.as_ref().unwrap().action(),
            UiAction::ShowTutorial
        );
    }
}
