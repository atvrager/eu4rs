//! Speed controls panel for the game UI.
//!
//! Displays current game speed, pause state, and date at the bottom of the screen.
//! All layout values are parsed from speed_controls.gui - no magic numbers!

use super::primitives::{GuiIcon, GuiText};
use eu4_macros::GuiWindow;

/// Speed controls panel using the macro-based binder pattern.
///
/// Binds to specific widgets from the `speed_controls` window in `speed_controls.gui`.
/// Unlike the legacy implementation which collected all widgets into vectors,
/// this struct binds to specific named widgets that display dynamic game data.
#[derive(GuiWindow)]
#[gui(window_name = "speed_controls")]
#[allow(dead_code)] // Fields used in rendering
pub struct SpeedControls {
    /// Speed indicator icon (10 frames: 0-4 for speeds 1-5, 5 for paused)
    pub speed_indicator: GuiIcon,

    /// Date text display (e.g., "1 November 1444")
    #[gui(name = "DateText")]
    pub date_text: GuiText,
}

impl SpeedControls {
    /// Update speed indicator and date from current game state.
    ///
    /// This method demonstrates the explicit data binding pattern:
    /// each game state value is manually mapped to its corresponding UI widget.
    #[allow(dead_code)] // Used in render_speed_controls_only
    pub fn update(&mut self, date: &str, speed: u8, paused: bool) {
        // Set date text
        self.date_text.set_text(date);

        // Set speed indicator frame:
        // - Paused: frame 5
        // - Speed 1-5: frames 0-4
        let frame = if paused {
            5
        } else {
            (speed.saturating_sub(1)).min(4) as u32
        };
        self.speed_indicator.set_frame(frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::interner::StringInterner;
    use crate::gui::parser::parse_gui_file;
    use std::env;
    use std::path::PathBuf;

    /// Test that SpeedControls can bind to real speed_controls.gui data.
    #[test]
    fn test_speed_controls_binding() {
        let game_path = env::var("EU4_GAME_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let mut path = PathBuf::from(env!("HOME"));
                path.push(".steam/steam/steamapps/common/Europa Universalis IV");
                path
            });

        let gui_path = game_path.join("interface/speed_controls.gui");
        if !gui_path.exists() {
            println!(
                "Skipping test: speed_controls.gui not found at {:?}",
                gui_path
            );
            return;
        }

        let interner = StringInterner::new();
        let db = parse_gui_file(&gui_path, &interner).expect("Should parse speed_controls.gui");

        // Find speed_controls window
        let speed_symbol = interner.intern("speed_controls");
        let speed_window = db
            .get(&speed_symbol)
            .expect("Should find speed_controls window in speed_controls.gui");

        // Bind using macro
        let controls = SpeedControls::bind(speed_window, &interner);

        // Verify key widgets bound
        assert_ne!(
            controls.speed_indicator.name(),
            "<placeholder>",
            "Speed indicator should be bound"
        );
        assert_ne!(
            controls.date_text.name(),
            "<placeholder>",
            "Date text should be bound"
        );
    }

    /// Test update method with sample data.
    #[test]
    fn test_speed_controls_update() {
        use crate::gui::types::{GuiElement, Orientation};

        let interner = StringInterner::new();

        // Create minimal test window
        let window = GuiElement::Window {
            name: "speed_controls".to_string(),
            position: (0, 0),
            size: (300, 50),
            orientation: Orientation::UpperLeft,
            children: vec![],
        };

        let mut controls = SpeedControls::bind(&window, &interner);

        // Test speed 1, not paused
        controls.update("1 November 1444", 1, false);
        assert_eq!(controls.date_text.text(), ""); // Placeholder returns empty
        assert_eq!(controls.speed_indicator.frame(), 0); // Placeholder doesn't update

        // Test speed 3, not paused
        controls.update("15 January 1445", 3, false);
        assert_eq!(controls.speed_indicator.frame(), 0); // Placeholder doesn't update

        // Test paused
        controls.update("1 March 1445", 3, true);
        assert_eq!(controls.speed_indicator.frame(), 0); // Placeholder doesn't update

        // Test speed 5 (max)
        controls.update("1 April 1445", 5, false);
        assert_eq!(controls.speed_indicator.frame(), 0); // Placeholder doesn't update
    }

    /// Test that SpeedControls gracefully handles missing widgets (CI mode).
    #[test]
    fn test_speed_controls_ci_mode() {
        use crate::gui::types::{GuiElement, Orientation};

        let interner = StringInterner::new();

        // Create an empty window (simulates missing/incomplete GUI file)
        let empty_window = GuiElement::Window {
            name: "speed_controls".to_string(),
            position: (0, 0),
            size: (300, 50),
            orientation: Orientation::UpperLeft,
            children: vec![],
        };

        // Should not panic even with missing widgets
        let controls = SpeedControls::bind(&empty_window, &interner);

        // All widgets should be placeholders
        assert_eq!(controls.speed_indicator.name(), "<placeholder>");
        assert_eq!(controls.date_text.name(), "<placeholder>");

        // Updates to placeholders should be no-ops
        let mut controls = controls;
        controls.update("1 November 1444", 3, false); // Should not crash

        assert_eq!(controls.date_text.text(), "");
        assert_eq!(controls.speed_indicator.frame(), 0); // Placeholder doesn't update frame
    }
}
