//! Intermediate representation for GUI code generation.
//!
//! These types represent the parsed GUI elements in a simplified form that's
//! easier to work with during code generation than the full `GuiElement` tree.

use eu4game::gui::types::{GuiElement, Orientation};

/// Information about a single widget extracted from a GUI tree.
#[derive(Debug, Clone)]
pub struct WidgetInfo {
    /// Widget name (used for binding to panel fields).
    pub name: String,
    /// Type of widget (button, text, icon, etc.).
    pub widget_type: WidgetType,
    /// Sprite name for rendering (if applicable).
    pub sprite_name: Option<String>,
    /// Position in GUI file coordinates.
    pub position: (i32, i32),
    /// Orientation for position calculation.
    pub orientation: Orientation,
    /// Font name for text widgets.
    pub font: Option<String>,
    /// Initial text content for text widgets.
    pub text: Option<String>,
}

/// Type of GUI widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetType {
    /// Clickable button.
    Button,
    /// Text display label.
    TextBox,
    /// Static icon/sprite.
    Icon,
    /// Container window.
    #[allow(dead_code)]
    Window,
    /// Scrollable list.
    Listbox,
    /// Text input field.
    EditBox,
}

/// Information about a complete panel extracted from a GUI tree.
#[derive(Debug, Clone)]
pub struct PanelInfo {
    /// Panel name (e.g., "left", "topbar").
    pub name: String,
    /// Window position from GUI file.
    pub window_pos: (i32, i32),
    /// Window orientation.
    pub window_orientation: Orientation,
    /// All widgets in this panel.
    pub widgets: Vec<WidgetInfo>,
}

impl WidgetInfo {
    /// Convert a GuiElement to a WidgetInfo.
    ///
    /// Returns None if the element type is not supported or doesn't contain
    /// enough information for code generation.
    pub fn from_element(element: &GuiElement) -> Option<Self> {
        match element {
            GuiElement::Button {
                name,
                position,
                sprite_type,
                orientation,
                button_text,
                ..
            } => Some(Self {
                name: name.clone(),
                widget_type: WidgetType::Button,
                sprite_name: Some(sprite_type.clone()),
                position: *position,
                orientation: *orientation,
                font: None,
                text: button_text.clone(),
            }),
            GuiElement::TextBox {
                name,
                position,
                font,
                orientation,
                text,
                ..
            } => Some(Self {
                name: name.clone(),
                widget_type: WidgetType::TextBox,
                sprite_name: None,
                position: *position,
                orientation: *orientation,
                font: Some(font.clone()),
                text: Some(text.clone()),
            }),
            GuiElement::Icon {
                name,
                position,
                sprite_type,
                orientation,
                ..
            } => Some(Self {
                name: name.clone(),
                widget_type: WidgetType::Icon,
                sprite_name: Some(sprite_type.clone()),
                position: *position,
                orientation: *orientation,
                font: None,
                text: None,
            }),
            GuiElement::Listbox {
                name,
                position,
                orientation,
                ..
            } => Some(Self {
                name: name.clone(),
                widget_type: WidgetType::Listbox,
                sprite_name: None,
                position: *position,
                orientation: *orientation,
                font: None,
                text: None,
            }),
            GuiElement::EditBox {
                name,
                position,
                orientation,
                ..
            } => Some(Self {
                name: name.clone(),
                widget_type: WidgetType::EditBox,
                sprite_name: None,
                position: *position,
                orientation: *orientation,
                font: None,
                text: None,
            }),
            // Windows and other container types don't directly render
            _ => None,
        }
    }
}
