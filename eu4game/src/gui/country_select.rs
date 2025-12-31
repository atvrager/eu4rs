//! Country selection panel for the game start screen.
//!
//! Renders EU4-style country info when a nation is selected on the map.
//! All layout values are parsed from frontend.gui - no magic numbers!

use super::primitives::{GuiIcon, GuiText};
use super::types::{Orientation, TextFormat};
use eu4_macros::GuiWindow;

/// Country selection panel using the new binder pattern.
///
/// This demonstrates Phase 3 of the UI engine: using the `#[derive(GuiWindow)]`
/// macro to automatically bind to specific widgets from the `singleplayer` window
/// in `frontend.gui`.
///
/// Unlike `CountrySelectLayout` which collects all widgets into vectors,
/// this struct binds to specific named widgets that we need to interact with.
#[derive(GuiWindow)]
#[gui(window_name = "singleplayer")]
pub struct CountrySelectPanel {
    /// Main country name label
    pub selected_nation_label: GuiText,

    /// Government type/status text (e.g., "Archduchy")
    pub selected_nation_status_label: GuiText,

    /// Fog of war status text
    pub selected_fog: GuiText,

    /// Ruler name and dynasty
    pub selected_ruler: GuiText,

    /// Administrative stat (0-6)
    pub ruler_adm_value: GuiText,

    /// Diplomatic stat (0-6)
    pub ruler_dip_value: GuiText,

    /// Military stat (0-6)
    pub ruler_mil_value: GuiText,

    /// Admin tech level
    pub admtech_value: GuiText,

    /// Diplomatic tech level
    pub diptech_value: GuiText,

    /// Military tech level
    pub miltech_value: GuiText,

    /// National ideas group name
    pub national_ideagroup_name: GuiText,

    /// Ideas unlocked count
    pub ideas_value: GuiText,

    /// Province count
    pub provinces_value: GuiText,

    /// Total development
    pub economy_value: GuiText,

    /// Fort level
    pub fort_value: GuiText,

    /// Diplomacy banner text
    pub diplomacy_banner_label: GuiText,

    /// Government rank icon (Duchy/Kingdom/Empire)
    pub government_rank: GuiIcon,

    /// Religion icon
    pub religion_icon: GuiIcon,

    /// Tech group icon
    pub techgroup_icon: GuiIcon,
}

impl CountrySelectPanel {
    /// Update all text fields from game state.
    ///
    /// This demonstrates the explicit data binding pattern: we manually
    /// specify which game state values go where, keeping the logic clear and testable.
    pub fn update(&mut self, state: &SelectedCountryState) {
        self.selected_nation_label.set_text(&state.name);
        self.selected_nation_status_label
            .set_text(&state.government_type);
        self.selected_fog.set_text(&state.fog_status);
        self.selected_ruler.set_text(&state.ruler_name);

        // Ruler stats
        self.ruler_adm_value.set_text(&state.ruler_adm.to_string());
        self.ruler_dip_value.set_text(&state.ruler_dip.to_string());
        self.ruler_mil_value.set_text(&state.ruler_mil.to_string());

        // Tech levels
        self.admtech_value.set_text(&state.adm_tech.to_string());
        self.diptech_value.set_text(&state.dip_tech.to_string());
        self.miltech_value.set_text(&state.mil_tech.to_string());

        // Ideas and development
        self.national_ideagroup_name.set_text(&state.ideas_name);
        self.ideas_value.set_text(&state.ideas_unlocked.to_string());
        self.provinces_value
            .set_text(&state.province_count.to_string());
        self.economy_value
            .set_text(&state.total_development.to_string());
        self.fort_value.set_text(&state.fort_level.to_string());
        self.diplomacy_banner_label
            .set_text(&state.diplomacy_header);

        // Update icon frames (these would be used during rendering)
        self.government_rank
            .set_frame(state.government_rank.saturating_sub(1) as u32);
        self.religion_icon.set_frame(state.religion_frame);
        self.techgroup_icon.set_frame(state.tech_group_frame);
    }
}

/// Icon element parsed from the singleplayer window.
#[derive(Debug, Clone)]
pub struct CountrySelectIcon {
    pub name: String,
    pub sprite: String,
    pub position: (i32, i32),
    pub orientation: Orientation,
    pub frame: u32,
    /// Scale factor (1.0 = normal size).
    pub scale: f32,
}

/// Text element parsed from the singleplayer window.
#[derive(Debug, Clone)]
pub struct CountrySelectText {
    pub name: String,
    pub position: (i32, i32),
    pub font: String,
    pub max_width: u32,
    pub max_height: u32,
    pub format: TextFormat,
    pub orientation: Orientation,
    pub border_size: (i32, i32),
}

/// Button element parsed from the singleplayer window.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields used for layout data, rendering WIP
pub struct CountrySelectButton {
    pub name: String,
    pub sprite: String,
    pub position: (i32, i32),
    pub orientation: Orientation,
}

/// Parsed layout for the country selection panel.
/// All positions come from frontend.gui's singleplayer windowType.
#[derive(Debug, Clone, Default)]
pub struct CountrySelectLayout {
    /// Window position relative to screen.
    pub window_pos: (i32, i32),
    /// Window orientation (typically UPPER_RIGHT).
    pub window_orientation: Orientation,
    /// Window size as declared in GUI file.
    pub window_size: (u32, u32),

    /// All icon elements (shield, rank, religion, tech icons, etc.)
    pub icons: Vec<CountrySelectIcon>,
    /// All text elements (country name, ruler stats, tech values, etc.)
    pub texts: Vec<CountrySelectText>,
    /// All button elements (player_shield is a button type)
    pub buttons: Vec<CountrySelectButton>,

    /// Whether successfully loaded from game files.
    pub loaded: bool,
}

/// Dynamic state for the selected country.
/// Values are populated from game data when a country is clicked.
#[derive(Debug, Clone, Default)]
pub struct SelectedCountryState {
    /// Country tag (e.g., "HAB" for Austria).
    pub tag: String,
    /// Localized country name.
    pub name: String,
    /// Government type/status (e.g., "Archduchy", "Feudal Monarchy").
    pub government_type: String,
    /// Fog of war status (empty if visible, "Terra Incognita" if not).
    pub fog_status: String,
    /// Government rank (1=Duchy, 2=Kingdom, 3=Empire).
    pub government_rank: u8,
    /// Religion icon frame index.
    pub religion_frame: u32,
    /// Tech group icon frame index.
    pub tech_group_frame: u32,
    /// Ruler's name and dynasty.
    pub ruler_name: String,
    /// Ruler's administrative skill (0-6).
    pub ruler_adm: u8,
    /// Ruler's diplomatic skill (0-6).
    pub ruler_dip: u8,
    /// Ruler's military skill (0-6).
    pub ruler_mil: u8,
    /// Administrative technology level.
    pub adm_tech: u8,
    /// Diplomatic technology level.
    pub dip_tech: u8,
    /// Military technology level.
    pub mil_tech: u8,
    /// National ideas group name.
    pub ideas_name: String,
    /// Number of ideas unlocked (0-7).
    pub ideas_unlocked: u8,
    /// Number of owned provinces.
    pub province_count: u32,
    /// Total development across all provinces.
    pub total_development: i32,
    /// Highest fort level.
    pub fort_level: u8,
    /// Diplomacy section header (usually "Diplomacy").
    pub diplomacy_header: String,
}

#[allow(dead_code)] // Utility methods for future use
impl CountrySelectLayout {
    /// Find an icon by name.
    pub fn get_icon(&self, name: &str) -> Option<&CountrySelectIcon> {
        self.icons.iter().find(|i| i.name == name)
    }

    /// Find a text element by name.
    pub fn get_text(&self, name: &str) -> Option<&CountrySelectText> {
        self.texts.iter().find(|t| t.name == name)
    }

    /// Find a button by name.
    pub fn get_button(&self, name: &str) -> Option<&CountrySelectButton> {
        self.buttons.iter().find(|b| b.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::interner::StringInterner;
    use crate::gui::parser::parse_gui_file;
    use std::env;
    use std::path::PathBuf;

    #[test]
    fn test_default_layout_not_loaded() {
        let layout = CountrySelectLayout::default();
        assert!(!layout.loaded);
        assert!(layout.icons.is_empty());
        assert!(layout.texts.is_empty());
    }

    #[test]
    fn test_selected_country_state_default() {
        let state = SelectedCountryState::default();
        assert!(state.tag.is_empty());
        assert_eq!(state.government_rank, 0);
        assert_eq!(state.ruler_adm, 0);
    }

    /// Test that CountrySelectPanel can bind to real frontend.gui data.
    ///
    /// This verifies Phase 3.3: the macro-generated bind() method works with
    /// actual EU4 GUI files, not just synthetic test data.
    #[test]
    fn test_country_select_panel_binding() {
        // Try to find EU4 installation
        let game_path = env::var("EU4_GAME_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let mut path = PathBuf::from(env!("HOME"));
                path.push(".steam/steam/steamapps/common/Europa Universalis IV");
                path
            });

        let gui_path = game_path.join("interface/frontend.gui");
        if !gui_path.exists() {
            println!("Skipping test: frontend.gui not found at {:?}", gui_path);
            println!("Set EU4_GAME_PATH environment variable to test with real game files");
            return;
        }

        // Parse the GUI file
        let interner = StringInterner::new();
        let db =
            parse_gui_file(&gui_path, &interner).expect("Should parse frontend.gui successfully");

        // Find the singleplayer window (it's nested, so we need to search)
        fn find_window_by_name<'a>(
            db: &'a crate::gui::types::WindowDatabase,
            target_name: &str,
        ) -> Option<&'a crate::gui::types::GuiElement> {
            use crate::gui::types::GuiElement;

            for element in db.values() {
                if element.name() == target_name {
                    return Some(element);
                }
                // Search recursively in children
                if let GuiElement::Window { children, .. } = element {
                    for child in children {
                        if let Some(found) = search_in_element(child, target_name) {
                            return Some(found);
                        }
                    }
                }
            }
            None
        }

        fn search_in_element<'a>(
            element: &'a crate::gui::types::GuiElement,
            target_name: &str,
        ) -> Option<&'a crate::gui::types::GuiElement> {
            use crate::gui::types::GuiElement;

            if element.name() == target_name {
                return Some(element);
            }
            if let GuiElement::Window { children, .. } = element {
                for child in children {
                    if let Some(found) = search_in_element(child, target_name) {
                        return Some(found);
                    }
                }
            }
            None
        }

        let singleplayer_window = find_window_by_name(&db, "singleplayer")
            .expect("Should find singleplayer window in frontend.gui");

        // Bind the panel using the macro-generated method
        let panel = CountrySelectPanel::bind(singleplayer_window, &interner);

        // Verify that key widgets were bound (not placeholders)
        assert_ne!(
            panel.selected_nation_label.name(),
            "<placeholder>",
            "Nation label should be bound to real widget"
        );
        assert_ne!(
            panel.government_rank.name(),
            "<placeholder>",
            "Government rank icon should be bound"
        );
        assert_ne!(
            panel.religion_icon.name(),
            "<placeholder>",
            "Religion icon should be bound"
        );

        // Test the update method
        let mut panel = panel;
        let state = SelectedCountryState {
            tag: "HAB".to_string(),
            name: "Austria".to_string(),
            government_type: "Archduchy".to_string(),
            government_rank: 3,
            ruler_name: "Friedrich III von Habsburg".to_string(),
            ruler_adm: 3,
            ruler_dip: 4,
            ruler_mil: 2,
            adm_tech: 3,
            dip_tech: 3,
            mil_tech: 3,
            ideas_name: "Austrian Ideas".to_string(),
            ideas_unlocked: 2,
            province_count: 12,
            total_development: 156,
            fog_status: String::new(),
            religion_frame: 0,
            tech_group_frame: 0,
            fort_level: 1,
            diplomacy_header: "Diplomacy".to_string(),
        };

        panel.update(&state);

        // Verify the update worked
        assert_eq!(panel.selected_nation_label.text(), "Austria");
        assert_eq!(panel.selected_nation_status_label.text(), "Archduchy");
        assert_eq!(panel.selected_ruler.text(), "Friedrich III von Habsburg");
        assert_eq!(panel.ruler_adm_value.text(), "3");
        assert_eq!(panel.ruler_dip_value.text(), "4");
        assert_eq!(panel.ruler_mil_value.text(), "2");
    }

    /// Test that CountrySelectPanel gracefully handles missing widgets.
    ///
    /// This ensures CI compatibility when game files aren't available.
    #[test]
    fn test_country_select_panel_ci_mode() {
        use crate::gui::types::{GuiElement, Orientation};

        let interner = StringInterner::new();

        // Create an empty window (simulates missing/incomplete GUI file)
        let empty_window = GuiElement::Window {
            name: "singleplayer".to_string(),
            position: (0, 0),
            size: (400, 600),
            orientation: Orientation::UpperLeft,
            children: vec![],
        };

        // Should not panic even with missing widgets
        let panel = CountrySelectPanel::bind(&empty_window, &interner);

        // All widgets should be placeholders
        assert_eq!(panel.selected_nation_label.name(), "<placeholder>");
        assert_eq!(panel.government_rank.name(), "<placeholder>");

        // Updates to placeholders should be no-ops
        let mut panel = panel;
        let state = SelectedCountryState::default();
        panel.update(&state); // Should not crash

        assert_eq!(panel.selected_nation_label.text(), "");
    }
}
