//! Country selection left panel for singleplayer screen.
//!
//! Contains bookmarks list, save games list, date widget, and back button.
//!
//! # Observer Mode UI (Repurposed from Multiplayer)
//!
//! The `observe_mode_button` and `observe_mode_title` are originally from EU4's
//! multiplayer lobby UI (`frontend.gui` multiplayer view). We're repurposing them
//! for single player to enable headless testing and AI-only simulations.
//!
//! **Deviation from vanilla EU4**: In the actual game, observer mode is only
//! available in multiplayer. We expose it in single player for development/testing.

use super::core::UiAction;
use super::primitives::{GuiButton, GuiEditBox, GuiListbox, GuiText};
use eu4_macros::GuiWindow;

/// Left panel for country selection screen.
///
/// Binds to widgets within the `left` window in `frontend.gui`.
/// Contains bookmarks list, save games list, date widget, and lobby control buttons.
#[derive(Debug, GuiWindow)]
#[gui(window_name = "left")]
#[allow(dead_code)] // Fields used in future phases
pub struct CountrySelectLeftPanel {
    /// Bookmarks listbox (historical start dates).
    #[gui(name = "bookmarks_list")]
    pub bookmarks_list: GuiListbox,

    /// Save games listbox.
    #[gui(name = "save_games_list")]
    pub save_games_list: GuiListbox,

    /// Year editor textbox.
    #[gui(name = "year")]
    pub year_editor: GuiEditBox,

    /// Day/month display label (e.g., "11th of November").
    #[gui(name = "daymonth")]
    pub daymonth: GuiText,

    /// Year up button (ones digit).
    #[gui(name = "year_up1")]
    pub year_up_1: GuiButton,

    /// Year down button (ones digit).
    #[gui(name = "year_down1")]
    pub year_down_1: GuiButton,

    /// Year up button (tens digit).
    #[gui(name = "year_up2")]
    pub year_up_2: GuiButton,

    /// Year down button (tens digit).
    #[gui(name = "year_down2")]
    pub year_down_2: GuiButton,

    /// Year up button (hundreds digit).
    #[gui(name = "year_up3")]
    pub year_up_3: GuiButton,

    /// Year down button (hundreds digit).
    #[gui(name = "year_down3")]
    pub year_down_3: GuiButton,

    /// Month up button.
    #[gui(name = "month_up")]
    pub month_up: GuiButton,

    /// Month down button.
    #[gui(name = "month_down")]
    pub month_down: GuiButton,

    /// Day up button.
    #[gui(name = "day_up")]
    pub day_up: GuiButton,

    /// Day down button.
    #[gui(name = "day_down")]
    pub day_down: GuiButton,

    /// Back button (returns to main menu).
    #[gui(name = "back_button")]
    pub back_button: GuiButton,

    /// Observe mode toggle button (checkbox-style button).
    /// When enabled, starts the game in observer mode without a player country.
    #[gui(name = "observe_mode_button")]
    pub observe_mode_button: GuiButton,

    /// Text label for observer mode ("OBSERVE_MODE").
    /// Positioned next to the observe_mode_button checkbox.
    #[gui(name = "observe_mode_title")]
    pub observe_mode_title: GuiText,
}

impl CountrySelectLeftPanel {
    /// Initialize button actions after binding.
    ///
    /// Sets the appropriate `UiAction` for each button based on its function.
    #[allow(dead_code)] // Used in future phases
    pub fn init_actions(&mut self) {
        use crate::gui::core::DatePart;

        self.back_button.set_action(UiAction::Back);
        self.observe_mode_button
            .set_action(UiAction::ToggleObserveMode);

        // Year buttons: up1/down1 = +/-1, up2/down2 = +/-10, up3/down3 = +/-100
        self.year_up_1
            .set_action(UiAction::DateAdjust(DatePart::Year, 1));
        self.year_down_1
            .set_action(UiAction::DateAdjust(DatePart::Year, -1));
        self.year_up_2
            .set_action(UiAction::DateAdjust(DatePart::Year, 10));
        self.year_down_2
            .set_action(UiAction::DateAdjust(DatePart::Year, -10));
        self.year_up_3
            .set_action(UiAction::DateAdjust(DatePart::Year, 100));
        self.year_down_3
            .set_action(UiAction::DateAdjust(DatePart::Year, -100));

        self.month_up
            .set_action(UiAction::DateAdjust(DatePart::Month, 1));
        self.month_down
            .set_action(UiAction::DateAdjust(DatePart::Month, -1));
        self.day_up
            .set_action(UiAction::DateAdjust(DatePart::Day, 1));
        self.day_down
            .set_action(UiAction::DateAdjust(DatePart::Day, -1));
    }

    /// Update the date display labels.
    ///
    /// Sets the daymonth text to show the formatted date (e.g., "11 November")
    /// and the year editor to show the year (e.g., "1444").
    pub fn update_date(&mut self, date: &eu4data::Eu4Date) {
        let day = date.day();
        let month_name = match date.month() {
            1 => "January",
            2 => "February",
            3 => "March",
            4 => "April",
            5 => "May",
            6 => "June",
            7 => "July",
            8 => "August",
            9 => "September",
            10 => "October",
            11 => "November",
            12 => "December",
            _ => "December",
        };
        let text = format!("{} {}", day, month_name);
        self.daymonth.set_text(&text);

        // Set year text
        self.year_editor.set_text(&date.year().to_string());
    }

    /// Poll all buttons for clicks and return any action.
    ///
    /// Returns the first action found, or None if no buttons were clicked.
    pub fn poll_actions(&mut self) -> Option<UiAction> {
        // Back button
        if let Some(action) = self.back_button.poll_click() {
            return Some(action);
        }

        // Observer mode button
        if let Some(action) = self.observe_mode_button.poll_click() {
            return Some(action);
        }

        // Year buttons
        for btn in [
            &mut self.year_up_1,
            &mut self.year_down_1,
            &mut self.year_up_2,
            &mut self.year_down_2,
            &mut self.year_up_3,
            &mut self.year_down_3,
        ] {
            if let Some(action) = btn.poll_click() {
                return Some(action);
            }
        }

        // Month/day buttons
        for btn in [
            &mut self.month_up,
            &mut self.month_down,
            &mut self.day_up,
            &mut self.day_down,
        ] {
            if let Some(action) = btn.poll_click() {
                return Some(action);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::binder::Bindable;

    #[test]
    fn test_left_panel_ci_mode() {
        // In CI mode (no assets), panel should still construct with placeholders
        let panel = CountrySelectLeftPanel {
            bookmarks_list: GuiListbox::placeholder(),
            save_games_list: GuiListbox::placeholder(),
            year_editor: GuiEditBox::placeholder(),
            daymonth: GuiText::placeholder(),
            year_up_1: GuiButton::placeholder(),
            year_down_1: GuiButton::placeholder(),
            year_up_2: GuiButton::placeholder(),
            year_down_2: GuiButton::placeholder(),
            year_up_3: GuiButton::placeholder(),
            year_down_3: GuiButton::placeholder(),
            month_up: GuiButton::placeholder(),
            month_down: GuiButton::placeholder(),
            day_up: GuiButton::placeholder(),
            day_down: GuiButton::placeholder(),
            back_button: GuiButton::placeholder(),
            observe_mode_button: GuiButton::placeholder(),
            observe_mode_title: GuiText::placeholder(),
        };

        assert_eq!(panel.back_button.name(), "<placeholder>");
        assert_eq!(panel.bookmarks_list.name(), "<placeholder>");
    }

    #[test]
    fn test_init_actions() {
        let mut panel = CountrySelectLeftPanel {
            bookmarks_list: GuiListbox::placeholder(),
            save_games_list: GuiListbox::placeholder(),
            year_editor: GuiEditBox::placeholder(),
            daymonth: GuiText::placeholder(),
            year_up_1: GuiButton::placeholder(),
            year_down_1: GuiButton::placeholder(),
            year_up_2: GuiButton::placeholder(),
            year_down_2: GuiButton::placeholder(),
            year_up_3: GuiButton::placeholder(),
            year_down_3: GuiButton::placeholder(),
            month_up: GuiButton::placeholder(),
            month_down: GuiButton::placeholder(),
            day_up: GuiButton::placeholder(),
            day_down: GuiButton::placeholder(),
            back_button: GuiButton::placeholder(),
            observe_mode_button: GuiButton::placeholder(),
            observe_mode_title: GuiText::placeholder(),
        };

        panel.init_actions();

        use crate::gui::core::DatePart;
        assert_eq!(panel.back_button.action(), UiAction::Back);
        assert_eq!(
            panel.year_up_1.action(),
            UiAction::DateAdjust(DatePart::Year, 1)
        );
        assert_eq!(
            panel.year_down_1.action(),
            UiAction::DateAdjust(DatePart::Year, -1)
        );
        assert_eq!(
            panel.month_up.action(),
            UiAction::DateAdjust(DatePart::Month, 1)
        );
    }

    #[test]
    fn test_poll_actions_no_clicks() {
        let mut panel = CountrySelectLeftPanel {
            bookmarks_list: GuiListbox::placeholder(),
            save_games_list: GuiListbox::placeholder(),
            year_editor: GuiEditBox::placeholder(),
            daymonth: GuiText::placeholder(),
            year_up_1: GuiButton::placeholder(),
            year_down_1: GuiButton::placeholder(),
            year_up_2: GuiButton::placeholder(),
            year_down_2: GuiButton::placeholder(),
            year_up_3: GuiButton::placeholder(),
            year_down_3: GuiButton::placeholder(),
            month_up: GuiButton::placeholder(),
            month_down: GuiButton::placeholder(),
            day_up: GuiButton::placeholder(),
            day_down: GuiButton::placeholder(),
            back_button: GuiButton::placeholder(),
            observe_mode_button: GuiButton::placeholder(),
            observe_mode_title: GuiText::placeholder(),
        };

        // No clicks, should return None
        assert!(panel.poll_actions().is_none());
    }
}
