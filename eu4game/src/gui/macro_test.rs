//! Test module for the GuiWindow derive macro.
//!
//! This demonstrates and tests the macro-based binding system.

use crate::gui::interner::StringInterner;
use crate::gui::primitives::{GuiButton, GuiIcon, GuiText};
use crate::gui::types::{GuiElement, Orientation, TextFormat};
use eu4_macros::GuiWindow;

/// Example panel using the GuiWindow macro.
///
/// This demonstrates the binder pattern where widget names
/// match those in the GUI file, but can be overridden with #[gui(name = "...")]
#[derive(GuiWindow)]
#[gui(window_name = "test_panel")]
pub struct TestPanel {
    /// Text label - uses field name "title" as widget name
    pub title: GuiText,

    /// Icon with explicit name override
    #[gui(name = "shield_icon")]
    pub flag: GuiIcon,

    /// Button with explicit name
    #[gui(name = "confirm_btn")]
    pub confirm: GuiButton,

    /// Optional element
    pub optional_label: Option<GuiText>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a test GUI tree
    fn create_test_tree() -> GuiElement {
        GuiElement::Window {
            name: "test_panel".to_string(),
            position: (0, 0),
            size: (400, 300),
            orientation: Orientation::UpperLeft,
            children: vec![
                GuiElement::TextBox {
                    name: "title".to_string(),
                    position: (10, 10),
                    font: "default".to_string(),
                    max_width: 200,
                    max_height: 30,
                    format: TextFormat::Left,
                    orientation: Orientation::UpperLeft,
                    text: "Test Panel".to_string(),
                    border_size: (0, 0),
                },
                GuiElement::Icon {
                    name: "shield_icon".to_string(),
                    position: (10, 50),
                    sprite_type: "GFX_shield".to_string(),
                    orientation: Orientation::UpperLeft,
                    frame: 1,
                    scale: 1.0,
                },
                GuiElement::Button {
                    name: "confirm_btn".to_string(),
                    position: (10, 250),
                    sprite_type: "GFX_button".to_string(),
                    orientation: Orientation::UpperLeft,
                    shortcut: None,
                },
            ],
        }
    }

    #[test]
    fn test_macro_generates_bind_method() {
        let tree = create_test_tree();
        let interner = StringInterner::new();

        // The macro should generate a bind() method
        let panel = TestPanel::bind(&tree, &interner);

        // Verify widgets were bound correctly
        assert_eq!(panel.title.name(), "title");
        assert_eq!(panel.flag.name(), "shield_icon");
        assert_eq!(panel.confirm.name(), "confirm_btn");
    }

    #[test]
    fn test_missing_widgets_use_placeholders() {
        // Create tree with missing widgets
        let tree = GuiElement::Window {
            name: "test_panel".to_string(),
            position: (0, 0),
            size: (400, 300),
            orientation: Orientation::UpperLeft,
            children: vec![], // No children = all widgets missing
        };
        let interner = StringInterner::new();

        let panel = TestPanel::bind(&tree, &interner);

        // Placeholders should work without panicking
        assert_eq!(panel.title.name(), "<placeholder>");
        assert_eq!(panel.flag.name(), "<placeholder>");
        assert_eq!(panel.confirm.name(), "<placeholder>");
    }

    #[test]
    fn test_optional_widgets() {
        let tree = create_test_tree();
        let interner = StringInterner::new();

        let panel = TestPanel::bind(&tree, &interner);

        // Optional widget should be None when not found
        assert!(panel.optional_label.is_none());
    }

    #[test]
    fn test_widget_mutation() {
        let tree = create_test_tree();
        let interner = StringInterner::new();

        let mut panel = TestPanel::bind(&tree, &interner);

        // Should be able to update widget state
        panel.title.set_text("Updated Title");
        assert_eq!(panel.title.text(), "Updated Title");
    }
}
