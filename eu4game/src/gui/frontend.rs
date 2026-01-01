//! Frontend UI container - manages menu panels.
//!
//! The FrontendUI is a container for menu panels. Screen state management
//! is handled by App's ScreenManager (single source of truth).

use crate::gui::core::UiAction;
use crate::gui::main_menu::MainMenuPanel;
use crate::gui::ui_root::UiRoot;

/// Container for all frontend UI panels.
///
/// This is a simple container - screen state is managed by App's ScreenManager.
#[derive(Debug)]
#[allow(dead_code)] // Reserved for Phase 6.1+ main loop integration
pub struct FrontendUI {
    /// Main menu panel with navigation buttons.
    main_menu: MainMenuPanel,
    /// Input dispatcher and focus manager.
    ui_root: UiRoot,
}

impl FrontendUI {
    /// Create a new frontend UI with the given main menu panel.
    #[allow(dead_code)] // Reserved for Phase 6.1+ main loop integration
    pub fn new(main_menu: MainMenuPanel) -> Self {
        let mut panel = main_menu;
        panel.init_actions();

        Self {
            main_menu: panel,
            ui_root: UiRoot::new(),
        }
    }

    /// Poll for UI actions from the main menu.
    ///
    /// Returns an action if a button was clicked, None otherwise.
    /// The caller (App) is responsible for handling the action and
    /// performing any screen transitions.
    #[allow(dead_code)] // Reserved for Phase 6.1+ main loop integration
    pub fn poll_main_menu(&mut self) -> Option<UiAction> {
        self.main_menu.poll_actions()
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
    fn test_poll_main_menu_returns_none_without_clicks() {
        let panel = create_test_panel();
        let mut frontend = FrontendUI::new(panel);

        // No clicks, should return None
        assert!(frontend.poll_main_menu().is_none());
    }
}
