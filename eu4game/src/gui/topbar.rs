//! Main topbar UI panel.
//!
//! Displays player resources, monarch power, and advisors at the top of the screen.
//! Layout is loaded from topbar.gui.

use super::primitives::GuiText;
use super::types::CountryResources;
use eu4_macros::GuiWindow;

/// Main topbar panel using the macro-based binder pattern.
///
/// Binds to specific text widgets from the `topbar` window in `topbar.gui`.
/// Unlike the legacy implementation which collected all widgets into vectors,
/// this struct binds to specific named text fields that display dynamic game data.
#[derive(GuiWindow)]
#[gui(window_name = "topbar")]
#[allow(non_snake_case)] // Widget names match EU4 GUI file (text_ADM, text_DIP, text_MIL)
pub struct TopBar {
    /// Treasury display (ducats)
    pub text_gold: GuiText,

    /// Available manpower
    pub text_manpower: GuiText,

    /// Available sailors
    pub text_sailors: GuiText,

    /// Stability (-3 to +3)
    pub text_stability: GuiText,

    /// Prestige (0 to 100)
    pub text_prestige: GuiText,

    /// Corruption (0.00 to 100.00)
    pub text_corruption: GuiText,

    /// Administrative power
    pub text_ADM: GuiText,

    /// Diplomatic power
    pub text_DIP: GuiText,

    /// Military power
    pub text_MIL: GuiText,

    /// Merchants (current/max)
    pub text_merchants: GuiText,

    /// Colonists (current/max)
    pub text_settlers: GuiText,

    /// Diplomats (current/max)
    pub text_diplomats: GuiText,

    /// Missionaries (current/max)
    pub text_missionaries: GuiText,
}

impl TopBar {
    /// Update all topbar text fields from current game state.
    ///
    /// This method demonstrates the explicit data binding pattern:
    /// each game state value is manually mapped to its corresponding UI widget.
    pub fn update(&mut self, country: &CountryResources) {
        // Resources
        self.text_gold.set_text(&format!("{:.0}", country.treasury));
        self.text_manpower.set_text(&format_k(country.manpower));
        self.text_sailors.set_text(&format_k(country.sailors));

        // Stability and prestige
        self.text_stability
            .set_text(&format!("{:+}", country.stability));
        self.text_prestige
            .set_text(&format!("{:.0}", country.prestige));
        self.text_corruption
            .set_text(&format!("{:.1}", country.corruption));

        // Monarch power
        self.text_ADM.set_text(&format!("{}", country.adm_power));
        self.text_DIP.set_text(&format!("{}", country.dip_power));
        self.text_MIL.set_text(&format!("{}", country.mil_power));

        // Advisors
        self.text_merchants
            .set_text(&format!("{}/{}", country.merchants, country.max_merchants));
        self.text_settlers
            .set_text(&format!("{}/{}", country.colonists, country.max_colonists));
        self.text_diplomats
            .set_text(&format!("{}/{}", country.diplomats, country.max_diplomats));
        self.text_missionaries.set_text(&format!(
            "{}/{}",
            country.missionaries, country.max_missionaries
        ));
    }
}

/// Format large numbers with K/M suffixes.
///
/// - Under 100K: show full number with commas (e.g., "25,000")
/// - 100K to 1M: show with K suffix (e.g., "150K")
/// - 1M+: show with M suffix (e.g., "1.5M")
fn format_k(value: i32) -> String {
    if value >= 1_000_000 {
        let millions = value as f32 / 1_000_000.0;
        if millions >= 10.0 {
            format!("{:.0}M", millions)
        } else {
            format!("{:.1}M", millions)
        }
    } else if value >= 100_000 {
        format!("{}K", value / 1000)
    } else if value >= 1_000 {
        // Format with commas for values 1K-100K
        let s = format!("{}", value);
        let chars: Vec<char> = s.chars().collect();
        let mut result = String::new();
        for (i, c) in chars.iter().enumerate() {
            if i > 0 && (chars.len() - i).is_multiple_of(3) {
                result.push(',');
            }
            result.push(*c);
        }
        result
    } else {
        format!("{}", value)
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
    fn test_format_k() {
        assert_eq!(format_k(500), "500");
        assert_eq!(format_k(1_500), "1,500");
        assert_eq!(format_k(25_000), "25,000");
        assert_eq!(format_k(150_000), "150K");
        assert_eq!(format_k(1_500_000), "1.5M");
        assert_eq!(format_k(15_000_000), "15M");
    }

    /// Test that TopBar can bind to real topbar.gui data.
    #[test]
    fn test_topbar_binding() {
        let game_path = env::var("EU4_GAME_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let mut path = PathBuf::from(env!("HOME"));
                path.push(".steam/steam/steamapps/common/Europa Universalis IV");
                path
            });

        let gui_path = game_path.join("interface/topbar.gui");
        if !gui_path.exists() {
            println!("Skipping test: topbar.gui not found at {:?}", gui_path);
            return;
        }

        let interner = StringInterner::new();
        let db = parse_gui_file(&gui_path, &interner).expect("Should parse topbar.gui");

        // Find topbar window
        let topbar_symbol = interner.intern("topbar");
        let topbar_window = db
            .get(&topbar_symbol)
            .expect("Should find topbar window in topbar.gui");

        // Bind using macro
        let topbar = TopBar::bind(topbar_window, &interner);

        // Verify key widgets bound
        assert_ne!(topbar.text_gold.name(), "<placeholder>");
        assert_ne!(topbar.text_ADM.name(), "<placeholder>");
        assert_ne!(topbar.text_manpower.name(), "<placeholder>");
    }

    /// Test update method with sample data.
    #[test]
    fn test_topbar_update() {
        use crate::gui::types::{GuiElement, Orientation};

        let interner = StringInterner::new();

        // Create minimal test window
        let window = GuiElement::Window {
            name: "topbar".to_string(),
            position: (0, 0),
            size: (1920, 50),
            orientation: Orientation::UpperLeft,
            children: vec![],
        };

        let mut topbar = TopBar::bind(&window, &interner);

        let country = CountryResources {
            treasury: 523.5,
            income: 12.5,
            manpower: 15_234,
            max_manpower: 50_000,
            sailors: 2_500,
            max_sailors: 10_000,
            stability: 2,
            prestige: 45.0,
            corruption: 0.5,
            adm_power: 123,
            dip_power: 456,
            mil_power: 789,
            merchants: 2,
            max_merchants: 5,
            colonists: 0,
            max_colonists: 2,
            diplomats: 1,
            max_diplomats: 4,
            missionaries: 0,
            max_missionaries: 1,
        };

        topbar.update(&country);

        // Verify formatting (placeholders return empty strings)
        assert_eq!(topbar.text_gold.text(), "");
        assert_eq!(topbar.text_stability.text(), "");
    }
}
