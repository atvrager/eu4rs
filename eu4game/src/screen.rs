//! Screen state management for the game UI.
//!
//! Handles transitions between different game screens (main menu, single player setup,
//! multiplayer, and active gameplay).

/// Represents the current screen/state of the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Reserved for Phase 6+ integration
pub enum Screen {
    /// Main menu screen with options to start game, multiplayer, etc.
    MainMenu,
    /// Single player setup screen (country selection, date, etc.).
    SinglePlayer,
    /// Multiplayer lobby/setup screen.
    Multiplayer,
    /// Active gameplay (map view, diplomacy, etc.).
    Playing,
}

/// Manages screen transitions and navigation history.
///
/// Tracks the current screen and maintains a navigation stack for back button support.
#[derive(Debug)]
#[allow(dead_code)] // Reserved for Phase 6+ integration
pub struct ScreenManager {
    /// The currently active screen.
    current_screen: Screen,
    /// Navigation history for back button functionality.
    /// Does not include the current screen.
    history: Vec<Screen>,
}

impl ScreenManager {
    /// Create a new screen manager starting at the main menu.
    pub fn new() -> Self {
        Self {
            current_screen: Screen::MainMenu,
            history: Vec::new(),
        }
    }

    /// Get the current screen.
    #[allow(dead_code)] // Reserved for Phase 6+ integration
    pub fn current(&self) -> Screen {
        self.current_screen
    }

    /// Transition to a new screen.
    ///
    /// The current screen is pushed onto the history stack for back navigation.
    /// Returns the previous screen.
    #[allow(dead_code)] // Reserved for Phase 6+ integration
    pub fn transition_to(&mut self, screen: Screen) -> Screen {
        let previous = self.current_screen;

        // Don't add duplicate consecutive screens to history
        if previous != screen {
            self.history.push(previous);
        }

        self.current_screen = screen;
        previous
    }

    /// Go back to the previous screen in the navigation history.
    ///
    /// Returns `Some(previous_screen)` if successful, or `None` if there's no history.
    #[allow(dead_code)] // Reserved for Phase 6+ integration
    pub fn go_back(&mut self) -> Option<Screen> {
        if let Some(previous) = self.history.pop() {
            self.current_screen = previous;
            Some(previous)
        } else {
            None
        }
    }

    /// Clear the navigation history.
    ///
    /// Useful when starting a game (transitioning to Playing screen) where
    /// back navigation should be disabled.
    #[allow(dead_code)] // Reserved for Phase 6+ integration
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Check if back navigation is available.
    #[allow(dead_code)] // Reserved for Phase 6+ integration
    pub fn can_go_back(&self) -> bool {
        !self.history.is_empty()
    }

    // ========================================================================
    // Navigation Shortcuts (Single Source of Truth)
    // ========================================================================

    /// Handle 'S' key shortcut - navigate to SinglePlayer if on MainMenu.
    ///
    /// Returns `true` if navigation occurred.
    pub fn handle_single_player_shortcut(&mut self) -> bool {
        if self.current_screen == Screen::MainMenu {
            self.transition_to(Screen::SinglePlayer);
            true
        } else {
            false
        }
    }

    /// Handle Escape key - go back if possible.
    ///
    /// Returns `true` if navigation occurred.
    pub fn handle_back(&mut self) -> bool {
        if self.can_go_back() {
            self.go_back();
            true
        } else {
            false
        }
    }
}

impl Default for ScreenManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_manager_starts_at_main_menu() {
        let manager = ScreenManager::new();
        assert_eq!(manager.current(), Screen::MainMenu);
        assert!(!manager.can_go_back());
    }

    #[test]
    fn test_transition_to_new_screen() {
        let mut manager = ScreenManager::new();

        let prev = manager.transition_to(Screen::SinglePlayer);
        assert_eq!(prev, Screen::MainMenu);
        assert_eq!(manager.current(), Screen::SinglePlayer);
        assert!(manager.can_go_back());
    }

    #[test]
    fn test_transition_builds_history() {
        let mut manager = ScreenManager::new();

        manager.transition_to(Screen::SinglePlayer);
        manager.transition_to(Screen::Playing);

        assert_eq!(manager.current(), Screen::Playing);
        assert!(manager.can_go_back());
    }

    #[test]
    fn test_go_back_pops_history() {
        let mut manager = ScreenManager::new();

        manager.transition_to(Screen::SinglePlayer);
        manager.transition_to(Screen::Playing);

        let prev = manager.go_back();
        assert_eq!(prev, Some(Screen::SinglePlayer));
        assert_eq!(manager.current(), Screen::SinglePlayer);
        assert!(manager.can_go_back());

        let prev = manager.go_back();
        assert_eq!(prev, Some(Screen::MainMenu));
        assert_eq!(manager.current(), Screen::MainMenu);
        assert!(!manager.can_go_back());
    }

    #[test]
    fn test_go_back_with_no_history() {
        let mut manager = ScreenManager::new();

        let result = manager.go_back();
        assert_eq!(result, None);
        assert_eq!(manager.current(), Screen::MainMenu);
    }

    #[test]
    fn test_clear_history() {
        let mut manager = ScreenManager::new();

        manager.transition_to(Screen::SinglePlayer);
        manager.transition_to(Screen::Playing);
        assert!(manager.can_go_back());

        manager.clear_history();
        assert!(!manager.can_go_back());
        assert_eq!(manager.current(), Screen::Playing);
    }

    #[test]
    fn test_duplicate_transitions_dont_add_to_history() {
        let mut manager = ScreenManager::new();

        manager.transition_to(Screen::SinglePlayer);
        manager.transition_to(Screen::SinglePlayer);
        manager.transition_to(Screen::SinglePlayer);

        // Should only have one entry in history (MainMenu)
        let prev = manager.go_back();
        assert_eq!(prev, Some(Screen::MainMenu));
        assert!(!manager.can_go_back());
    }

    #[test]
    fn test_transition_chain() {
        let mut manager = ScreenManager::new();

        // MainMenu -> SinglePlayer -> MainMenu -> Multiplayer -> Playing
        manager.transition_to(Screen::SinglePlayer);
        manager.transition_to(Screen::MainMenu);
        manager.transition_to(Screen::Multiplayer);
        manager.transition_to(Screen::Playing);

        // Navigate back through the chain
        assert_eq!(manager.go_back(), Some(Screen::Multiplayer));
        assert_eq!(manager.go_back(), Some(Screen::MainMenu));
        assert_eq!(manager.go_back(), Some(Screen::SinglePlayer));
        assert_eq!(manager.go_back(), Some(Screen::MainMenu));
        assert_eq!(manager.go_back(), None);
    }

    #[test]
    fn test_single_player_shortcut_from_main_menu() {
        let mut manager = ScreenManager::new();
        assert_eq!(manager.current(), Screen::MainMenu);

        let handled = manager.handle_single_player_shortcut();
        assert!(handled);
        assert_eq!(manager.current(), Screen::SinglePlayer);
    }

    #[test]
    fn test_single_player_shortcut_from_other_screen() {
        let mut manager = ScreenManager::new();
        manager.transition_to(Screen::SinglePlayer);

        let handled = manager.handle_single_player_shortcut();
        assert!(!handled);
        assert_eq!(manager.current(), Screen::SinglePlayer);
    }

    #[test]
    fn test_handle_back_with_history() {
        let mut manager = ScreenManager::new();
        manager.transition_to(Screen::SinglePlayer);

        let handled = manager.handle_back();
        assert!(handled);
        assert_eq!(manager.current(), Screen::MainMenu);
    }

    #[test]
    fn test_handle_back_without_history() {
        let mut manager = ScreenManager::new();

        let handled = manager.handle_back();
        assert!(!handled);
        assert_eq!(manager.current(), Screen::MainMenu);
    }
}
