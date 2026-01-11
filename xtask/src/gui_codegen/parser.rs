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

    // Iterate children of the root window to avoid double-applying the root window's position
    // (since panel.window_pos is handled separately by the renderer)
    for child in tree.children() {
        extract_widgets_recursive(child, &mut widgets, (0, 0));
    }

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

/// Recursively extract widgets from a GUI element tree with position offset.
fn extract_widgets_recursive(
    element: &GuiElement,
    widgets: &mut Vec<WidgetInfo>,
    parent_offset: (i32, i32),
) {
    // Try to convert this element to a widget
    if let Some(mut widget) = WidgetInfo::from_element(element) {
        // Apply parent offset to usage position
        widget.position.0 += parent_offset.0;
        widget.position.1 += parent_offset.1;
        widgets.push(widget);
    }

    // Calculate new offset for children
    // Only containers (Windows) shift the coordinate system
    let child_offset = if let GuiElement::Window { position, .. } = element {
        (parent_offset.0 + position.0, parent_offset.1 + position.1)
    } else {
        parent_offset
    };

    // Recurse into children (if any)
    for child in element.children() {
        extract_widgets_recursive(child, widgets, child_offset);
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
                // Direct child
                GuiElement::Button {
                    name: "test_button".to_string(),
                    position: (5, 5),
                    sprite_type: "GFX_button".to_string(),
                    orientation: Orientation::UpperLeft,
                    shortcut: None,
                    button_text: Some("Click me".to_string()),
                    button_font: Some("vic_18".to_string()),
                },
                // Nested container
                GuiElement::Window {
                    name: "nested_container".to_string(),
                    position: (10, 10),
                    size: (50, 50),
                    orientation: Orientation::UpperLeft,
                    children: vec![GuiElement::TextBox {
                        name: "nested_label".to_string(),
                        position: (5, 5), // Should become (10+5, 10+5) = (15, 15) relative to root content
                        font: "vic_18".to_string(),
                        max_width: 80,
                        max_height: 20,
                        format: eu4game::gui::types::TextFormat::Left,
                        orientation: Orientation::UpperLeft,
                        text: "Nested".to_string(),
                        border_size: (0, 0),
                    }],
                },
            ],
        };

        let panel = extract_panel_info(&tree, "test").expect("Should extract panel");

        assert_eq!(panel.name, "test");
        assert_eq!(panel.window_pos, (10, 20)); // Root pos extracted
        assert_eq!(panel.widgets.len(), 2);

        // Button: (5, 5) + (0, 0) = (5, 5)
        assert_eq!(panel.widgets[0].name, "test_button");
        assert_eq!(panel.widgets[0].position, (5, 5));

        // Label: (5, 5) + (10, 10) = (15, 15)
        assert_eq!(panel.widgets[1].name, "nested_label");
        assert_eq!(panel.widgets[1].position, (15, 15));
    }
}
