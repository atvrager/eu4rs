//! GUI file parsing and element lookup.
//!
//! This module interfaces with eu4game's GUI parser to extract element
//! positions from actual `.gui` files instead of using hardcoded coordinates.

use anyhow::{anyhow, Context, Result};
use eu4game::gui::interner::StringInterner;
use eu4game::gui::parser::parse_gui_file;
use eu4game::gui::types::{GuiElement, WindowDatabase};
use std::path::Path;

/// Container for all parsed GUI databases.
pub struct GuiDatabases {
    pub topbar: WindowDatabase,
    pub speed_controls: WindowDatabase,
    pub provinceview: WindowDatabase,
    pub interner: StringInterner,
}

/// Parse all required GUI files from the game installation.
///
/// This parses:
/// - interface/topbar.gui (treasury, manpower, sailors, mana, stats, envoys)
/// - interface/speed_controls.gui (date display)
/// - interface/provinceview.gui (province name, state, development)
///
/// Returns `GuiDatabases` even if some files fail to parse (graceful degradation).
pub fn parse_gui_files(game_path: &str) -> Result<GuiDatabases> {
    let game_dir = Path::new(game_path);
    let interface_dir = game_dir.join("interface");

    // StringInterner for symbol storage
    let interner = StringInterner::new();

    // Parse each GUI file with detailed error context
    let topbar_path = interface_dir.join("topbar.gui");
    let topbar = parse_gui_file(&topbar_path, &interner)
        .map_err(|e| anyhow!(e))
        .with_context(|| format!("Failed to parse topbar.gui at {:?}", topbar_path))?;

    let speed_controls_path = interface_dir.join("speed_controls.gui");
    let speed_controls = parse_gui_file(&speed_controls_path, &interner)
        .map_err(|e| anyhow!(e))
        .with_context(|| {
            format!(
                "Failed to parse speed_controls.gui at {:?}",
                speed_controls_path
            )
        })?;

    let provinceview_path = interface_dir.join("provinceview.gui");
    let provinceview = parse_gui_file(&provinceview_path, &interner)
        .map_err(|e| anyhow!(e))
        .with_context(|| {
            format!(
                "Failed to parse provinceview.gui at {:?}",
                provinceview_path
            )
        })?;

    Ok(GuiDatabases {
        topbar,
        speed_controls,
        provinceview,
        interner,
    })
}

/// Find a GUI element by fuzzy matching against multiple patterns.
///
/// Searches the specified window's children recursively, trying each pattern
/// in order until a match is found.
///
/// # Matching Strategy
/// - Case-insensitive comparison
/// - Exact match (element name == pattern)
/// - Contains match (element name contains pattern or vice versa)
/// - First successful match wins
///
/// # Returns
/// The first matching element, or `None` if no pattern matches.
pub fn find_element<'a>(
    db: &'a WindowDatabase,
    window_name: &str,
    patterns: &[&str],
    interner: &StringInterner,
) -> Option<&'a GuiElement> {
    // Get the root window from database
    let window_symbol = interner.intern(window_name);
    let window = db.get(&window_symbol)?;

    // Try each pattern in priority order
    for pattern in patterns {
        if let Some(element) = search_element_recursive(window, pattern) {
            return Some(element);
        }
    }

    None
}

/// Recursively search for an element matching the pattern.
fn search_element_recursive<'a>(element: &'a GuiElement, pattern: &str) -> Option<&'a GuiElement> {
    // Check if this element matches
    if matches_pattern(element.name(), pattern) {
        return Some(element);
    }

    // Recursively search children for Window elements
    if let GuiElement::Window { children, .. } = element {
        for child in children {
            if let Some(found) = search_element_recursive(child, pattern) {
                return Some(found);
            }
        }
    }

    None
}

/// Check if an element name matches a pattern (fuzzy matching).
fn matches_pattern(element_name: &str, pattern: &str) -> bool {
    let element_lower = element_name.to_lowercase();
    let pattern_lower = pattern.to_lowercase();

    // 1. Exact match (case-insensitive)
    if element_lower == pattern_lower {
        return true;
    }

    // 2. Element name contains pattern
    if element_lower.contains(&pattern_lower) {
        return true;
    }

    // 3. Pattern contains element name (less common but valid)
    if pattern_lower.contains(&element_lower) {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_pattern_exact() {
        assert!(matches_pattern("text_gold", "text_gold"));
        assert!(matches_pattern("TEXT_GOLD", "text_gold"));
        assert!(matches_pattern("text_gold", "TEXT_GOLD"));
    }

    #[test]
    fn test_matches_pattern_contains() {
        assert!(matches_pattern("text_gold", "gold"));
        assert!(matches_pattern("gold_text", "gold"));
        assert!(matches_pattern("my_text_gold_value", "text_gold"));
    }

    #[test]
    fn test_matches_pattern_no_match() {
        assert!(!matches_pattern("text_gold", "silver"));
        assert!(!matches_pattern("manpower", "treasury"));
    }
}
