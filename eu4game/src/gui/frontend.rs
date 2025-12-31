//! Frontend UI container - manages menu panels and screen transitions.
//!
//! The FrontendUI coordinates all menu panels, screen state, and input routing
//! for the game's frontend (main menu, country selection, etc.).

use crate::gui::core::UiAction;
use crate::gui::main_menu::MainMenuPanel;
use crate::gui::ui_root::UiRoot;
use crate::screen::{Screen, ScreenManager};

/// Container for all frontend UI panels and state.
///
/// Manages screen transitions, panel visibility, and input dispatch
/// for the main menu and game setup screens.
#[derive(Debug)]
#[allow(dead_code)] // Reserved for Phase 6.1+ main loop integration
pub struct FrontendUI {
    /// Main menu panel with navigation buttons.
    main_menu: MainMenuPanel,
    /// Screen state manager with navigation history.
    screen_manager: ScreenManager,
    /// Input dispatcher and focus manager.
    ui_root: UiRoot,
}

impl FrontendUI {
    /// Create a new frontend UI with the given main menu panel.
    ///
    /// Starts at the main menu screen.
    #[allow(dead_code)] // Reserved for Phase 6.1+ main loop integration
    pub fn new(main_menu: MainMenuPanel) -> Self {
        let mut panel = main_menu;
        panel.init_actions();

        Self {
            main_menu: panel,
            screen_manager: ScreenManager::new(),
            ui_root: UiRoot::new(),
        }
    }

    /// Get the current screen.
    #[allow(dead_code)] // Reserved for Phase 6.1+ main loop integration
    pub fn current_screen(&self) -> Screen {
        self.screen_manager.current()
    }

    /// Update the frontend UI - poll for button clicks and handle screen transitions.
    ///
    /// Returns `true` if the game should exit, `false` otherwise.
    #[allow(dead_code)] // Reserved for Phase 6.1+ main loop integration
    pub fn update(&mut self) -> bool {
        // Only poll actions when on the main menu screen
        if self.current_screen() != Screen::MainMenu {
            return false;
        }

        // Poll button clicks
        if let Some(action) = self.main_menu.poll_actions() {
            self.handle_action(action)
        } else {
            false
        }
    }

    /// Handle a UI action and perform the appropriate screen transition.
    ///
    /// Returns `true` if the game should exit, `false` otherwise.
    fn handle_action(&mut self, action: UiAction) -> bool {
        match action {
            UiAction::ShowSinglePlayer => {
                self.screen_manager.transition_to(Screen::SinglePlayer);
                false
            }
            UiAction::ShowMultiplayer => {
                self.screen_manager.transition_to(Screen::Multiplayer);
                false
            }
            UiAction::Exit => {
                // Exit the game
                true
            }
            UiAction::Back => {
                // Go back to previous screen
                self.go_back();
                false
            }
            UiAction::StartGame => {
                // Start the game from country selection
                self.screen_manager.transition_to(Screen::Playing);
                self.screen_manager.clear_history(); // Can't go back from gameplay
                false
            }
            UiAction::ShowTutorial | UiAction::ShowCredits | UiAction::ShowSettings => {
                // Not implemented yet - stay on current screen
                false
            }
            UiAction::None => false,
        }
    }

    /// Go back to the previous screen in the navigation history.
    ///
    /// Returns `true` if navigation was successful, `false` if no history.
    #[allow(dead_code)] // Reserved for Phase 6.1+ main loop integration
    pub fn go_back(&mut self) -> bool {
        self.screen_manager.go_back().is_some()
    }

    /// Check if back navigation is available.
    #[allow(dead_code)] // Reserved for Phase 6.1+ main loop integration
    pub fn can_go_back(&self) -> bool {
        self.screen_manager.can_go_back()
    }

    /// Access the main menu panel (for rendering).
    #[allow(dead_code)] // Reserved for Phase 6.1+ main loop integration
    pub fn main_menu(&self) -> &MainMenuPanel {
        &self.main_menu
    }

    /// Access the main menu panel mutably (for input handling).
    #[allow(dead_code)] // Will be used when integrating input dispatch
    pub fn main_menu_mut(&mut self) -> &mut MainMenuPanel {
        &mut self.main_menu
    }

    /// Access the UI root (for input dispatch).
    #[allow(dead_code)] // Will be used when integrating input dispatch
    pub fn ui_root(&self) -> &UiRoot {
        &self.ui_root
    }

    /// Access the UI root mutably (for input dispatch).
    #[allow(dead_code)] // Will be used when integrating input dispatch
    pub fn ui_root_mut(&mut self) -> &mut UiRoot {
        &mut self.ui_root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::binder::Bindable;
    use crate::gui::primitives::GuiButton;

    fn create_test_panel() -> MainMenuPanel {
        MainMenuPanel {
            single_player: GuiButton::placeholder(),
            multi_player: GuiButton::placeholder(),
            tutorial: None,
            credits: None,
            settings: None,
            exit: GuiButton::placeholder(),
        }
    }

    #[test]
    fn test_frontend_ui_starts_at_main_menu() {
        let panel = create_test_panel();
        let frontend = FrontendUI::new(panel);

        assert_eq!(frontend.current_screen(), Screen::MainMenu);
        assert!(!frontend.can_go_back());
    }

    #[test]
    fn test_frontend_ui_initializes_actions() {
        let panel = create_test_panel();
        let frontend = FrontendUI::new(panel);

        // Actions should be initialized
        assert_eq!(
            frontend.main_menu().single_player.action(),
            UiAction::ShowSinglePlayer
        );
        assert_eq!(frontend.main_menu().exit.action(), UiAction::Exit);
    }

    #[test]
    fn test_handle_action_single_player() {
        let panel = create_test_panel();
        let mut frontend = FrontendUI::new(panel);

        let should_exit = frontend.handle_action(UiAction::ShowSinglePlayer);
        assert!(!should_exit);
        assert_eq!(frontend.current_screen(), Screen::SinglePlayer);
        assert!(frontend.can_go_back());
    }

    #[test]
    fn test_handle_action_exit() {
        let panel = create_test_panel();
        let mut frontend = FrontendUI::new(panel);

        let should_exit = frontend.handle_action(UiAction::Exit);
        assert!(should_exit);
        assert_eq!(frontend.current_screen(), Screen::MainMenu); // Still on main menu
    }

    #[test]
    fn test_go_back_navigation() {
        let panel = create_test_panel();
        let mut frontend = FrontendUI::new(panel);

        // Navigate to single player
        frontend.handle_action(UiAction::ShowSinglePlayer);
        assert_eq!(frontend.current_screen(), Screen::SinglePlayer);

        // Navigate to multiplayer
        frontend.handle_action(UiAction::ShowMultiplayer);
        assert_eq!(frontend.current_screen(), Screen::Multiplayer);

        // Go back to single player
        assert!(frontend.go_back());
        assert_eq!(frontend.current_screen(), Screen::SinglePlayer);

        // Go back to main menu
        assert!(frontend.go_back());
        assert_eq!(frontend.current_screen(), Screen::MainMenu);

        // Can't go back further
        assert!(!frontend.go_back());
    }

    #[test]
    fn test_update_only_polls_on_main_menu() {
        let panel = create_test_panel();
        let mut frontend = FrontendUI::new(panel);

        // On main menu, update returns false (no exit action without actual click)
        assert!(!frontend.update());

        // Transition to single player
        frontend.handle_action(UiAction::ShowSinglePlayer);

        // On single player screen, update returns false (doesn't poll buttons)
        assert!(!frontend.update());
    }
}
