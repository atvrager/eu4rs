//! Country selection panel for the game start screen.
//!
//! Renders EU4-style country info when a nation is selected on the map.
//! All layout values are parsed from frontend.gui - no magic numbers!

use super::types::{Orientation, TextFormat};

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
}
