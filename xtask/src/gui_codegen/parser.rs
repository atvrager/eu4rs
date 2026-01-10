//! GUI file parser for code generation.
//!
//! Parses EU4's `.gui` files and extracts panel information for rendering code generation.

use super::types::{PanelInfo, WidgetInfo};
use anyhow::Result;
use eu4game::gui::interner::StringInterner;
use eu4game::gui::parser::parse_gui_file;
use eu4game::gui::types::GuiElement;
use std::collections::HashMap;
use std::path::Path;

/// Parse all relevant GUI files and return a map of window name → GUI element tree.
///
/// This parses the three main GUI files used for rendering:
/// - `interface/frontend.gui` - Country selection panels (left, top, right)
/// - `interface/topbar.gui` - Gameplay top bar
/// - `interface/speed_controls.gui` - Speed controls
pub fn parse_all_gui_files(game_path: &Path) -> Result<HashMap<String, GuiElement>> {
    let mut trees = HashMap::new();
    let interner = StringInterner::new();

    // Parse frontend.gui (contains multiple panels)
    let frontend_path = game_path.join("interface/frontend.gui");
    if frontend_path.exists() {
        println!("Parsing frontend.gui...");
        let elements = parse_gui_file(&frontend_path, &interner)
            .map_err(|e| anyhow::anyhow!("Failed to parse frontend.gui: {}", e))?;

        // Index all windows (including nested ones) by name
        for (_symbol, element) in elements {
            find_windows_recursive(&element, &mut trees);
        }
    } else {
        println!(
            "Warning: frontend.gui not found at {}",
            frontend_path.display()
        );
    }

    // Parse topbar.gui
    let topbar_path = game_path.join("interface/topbar.gui");
    if topbar_path.exists() {
        println!("Parsing topbar.gui...");
        let elements = parse_gui_file(&topbar_path, &interner)
            .map_err(|e| anyhow::anyhow!("Failed to parse topbar.gui: {}", e))?;

        for (_symbol, element) in elements {
            find_windows_recursive(&element, &mut trees);
        }
    }

    // Parse speed_controls.gui
    let speed_path = game_path.join("interface/speed_controls.gui");
    if speed_path.exists() {
        println!("Parsing speed_controls.gui...");
        let elements = parse_gui_file(&speed_path, &interner)
            .map_err(|e| anyhow::anyhow!("Failed to parse speed_controls.gui: {}", e))?;

        for (_symbol, element) in elements {
            find_windows_recursive(&element, &mut trees);
        }
    }

    println!("Parsed {} total windows", trees.len());
    Ok(trees)
}

/// Recursively find all Window elements and add them to the trees map.
fn find_windows_recursive(element: &GuiElement, trees: &mut HashMap<String, GuiElement>) {
    if let GuiElement::Window { ref name, .. } = element {
        println!("  Found window: {}", name);
        trees.insert(name.clone(), element.clone());
    }

    // Recurse into children
    for child in element.children() {
        find_windows_recursive(child, trees);
    }
}

/// Extract panel information from a GUI element tree.
///
/// # Arguments
/// * `tree` - The root GUI element (typically a Window)
/// * `panel_name` - Name for this panel (e.g., "left", "topbar")
///
/// # Returns
/// Extracted panel information with all widgets.
pub fn extract_panel_info(tree: &GuiElement, panel_name: &str) -> Result<PanelInfo> {
    // Extract window position and orientation
    let (window_pos, window_orientation) = match tree {
        GuiElement::Window {
            position,
            orientation,
            ..
        } => (*position, *orientation),
        _ => {
            anyhow::bail!("Expected Window element at root, got {:?}", tree);
        }
    };

    // Walk tree and extract all widgets
    let mut widgets = Vec::new();
    extract_widgets_recursive(tree, &mut widgets);

    println!(
        "Extracted {} widgets from panel '{}'",
        widgets.len(),
        panel_name
    );

    Ok(PanelInfo {
        name: panel_name.to_string(),
        window_pos,
        window_orientation,
        widgets,
    })
}

/// Recursively extract widgets from a GUI element tree.
fn extract_widgets_recursive(element: &GuiElement, widgets: &mut Vec<WidgetInfo>) {
    // Try to convert this element to a widget
    if let Some(widget) = WidgetInfo::from_element(element) {
        widgets.push(widget);
    }

    // Recurse into children (if any)
    for child in element.children() {
        extract_widgets_recursive(child, widgets);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eu4game::gui::types::Orientation;

    #[test]
    fn test_extract_widgets_from_window() {
        // Create a test GUI tree
        let tree = GuiElement::Window {
            name: "test_window".to_string(),
            position: (10, 20),
            size: (100, 100),
            orientation: Orientation::UpperLeft,
            children: vec![
                GuiElement::Button {
                    name: "test_button".to_string(),
                    position: (5, 5),
                    sprite_type: "GFX_button".to_string(),
                    orientation: Orientation::UpperLeft,
                    shortcut: None,
                    button_text: Some("Click me".to_string()),
                    button_font: Some("vic_18".to_string()),
                },
                GuiElement::TextBox {
                    name: "test_label".to_string(),
                    position: (5, 30),
                    font: "vic_18".to_string(),
                    max_width: 80,
                    max_height: 20,
                    format: eu4game::gui::types::TextFormat::Left,
                    orientation: Orientation::UpperLeft,
                    text: "Hello".to_string(),
                    border_size: (0, 0),
                },
            ],
        };

        let panel = extract_panel_info(&tree, "test").expect("Should extract panel");

        assert_eq!(panel.name, "test");
        assert_eq!(panel.window_pos, (10, 20));
        assert_eq!(panel.widgets.len(), 2);
        assert_eq!(panel.widgets[0].name, "test_button");
        assert_eq!(panel.widgets[1].name, "test_label");
    }
}
