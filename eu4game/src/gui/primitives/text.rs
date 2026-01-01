//! GuiText - wrapper around TextBox elements for runtime binding.
//!
//! Used by TopBar, SpeedControls, and CountrySelectPanel for dynamic text labels.

use crate::gui::binder::{Bindable, GuiNode};
use crate::gui::core::{EventResult, GuiRenderer, GuiWidget, UiContext, UiEvent};
use crate::gui::types::{GuiElement, Rect};

/// Handle to a text display widget.
///
/// Wraps a TextBox element and provides a mutable interface for
/// updating the displayed text at runtime. In placeholder mode (CI),
/// all operations are no-ops.
#[derive(Debug, Clone)]
pub struct GuiText {
    /// The underlying element data, if bound.
    element: Option<TextBoxData>,
}

#[derive(Debug, Clone)]
struct TextBoxData {
    #[allow(dead_code)] // Used in tests for binding verification
    name: String,
    position: (i32, i32),
    #[allow(dead_code)] // Reserved for font queries
    font: String,
    max_width: u32,
    max_height: u32,
    format: crate::gui::types::TextFormat,
    orientation: crate::gui::types::Orientation,
    /// Current text content (mutable).
    text: String,
    border_size: (i32, i32),
}

impl GuiText {
    /// Set the displayed text.
    ///
    /// In placeholder mode, this is a no-op.
    pub fn set_text(&mut self, text: &str) {
        if let Some(ref mut data) = self.element {
            data.text = text.to_string();
        }
    }

    /// Get the current text.
    ///
    /// Returns empty string in placeholder mode.
    pub fn text(&self) -> &str {
        self.element.as_ref().map(|d| d.text.as_str()).unwrap_or("")
    }

    /// Get the element name (for debugging).
    #[allow(dead_code)] // Used in tests for binding verification
    pub fn name(&self) -> &str {
        self.element
            .as_ref()
            .map(|d| d.name.as_str())
            .unwrap_or("<placeholder>")
    }

    /// Get the position (for rendering).
    pub fn position(&self) -> (i32, i32) {
        self.element.as_ref().map(|d| d.position).unwrap_or((0, 0))
    }

    /// Get the orientation (for rendering).
    pub fn orientation(&self) -> crate::gui::types::Orientation {
        self.element
            .as_ref()
            .map(|d| d.orientation)
            .unwrap_or(crate::gui::types::Orientation::UpperLeft)
    }

    /// Get the text format (for rendering).
    pub fn format(&self) -> crate::gui::types::TextFormat {
        self.element
            .as_ref()
            .map(|d| d.format)
            .unwrap_or(crate::gui::types::TextFormat::Left)
    }

    /// Get the max dimensions (for rendering).
    pub fn max_dimensions(&self) -> (u32, u32) {
        self.element
            .as_ref()
            .map(|d| (d.max_width, d.max_height))
            .unwrap_or((0, 0))
    }

    /// Get the border size (for rendering).
    pub fn border_size(&self) -> (i32, i32) {
        self.element
            .as_ref()
            .map(|d| d.border_size)
            .unwrap_or((0, 0))
    }

    /// Get the font name (for rendering).
    pub fn font(&self) -> &str {
        self.element
            .as_ref()
            .map(|d| d.font.as_str())
            .unwrap_or("vic_18")
    }
}

impl Bindable for GuiText {
    fn from_node(node: &GuiNode) -> Option<Self> {
        match node {
            GuiElement::TextBox {
                name,
                position,
                font,
                max_width,
                max_height,
                format,
                orientation,
                text,
                border_size,
            } => Some(Self {
                element: Some(TextBoxData {
                    name: name.clone(),
                    position: *position,
                    font: font.clone(),
                    max_width: *max_width,
                    max_height: *max_height,
                    format: *format,
                    orientation: *orientation,
                    text: text.clone(),
                    border_size: *border_size,
                }),
            }),
            _ => None,
        }
    }

    fn placeholder() -> Self {
        Self { element: None }
    }
}

impl GuiWidget for GuiText {
    fn render(&self, _ctx: &UiContext, _renderer: &mut dyn GuiRenderer) {
        // Rendering logic will be implemented when integrating with the actual renderer
        // For now, this is a stub to satisfy the trait
    }

    fn handle_input(&mut self, _event: &UiEvent, _ctx: &UiContext) -> EventResult {
        // Text widgets don't handle input
        EventResult::Ignored
    }

    fn bounds(&self) -> Rect {
        if let Some(ref data) = self.element {
            Rect {
                x: data.position.0 as f32,
                y: data.position.1 as f32,
                width: data.max_width as f32,
                height: data.max_height as f32,
            }
        } else {
            // Placeholder has no bounds
            Rect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::types::Orientation;

    #[test]
    fn test_placeholder_is_noop() {
        let mut text = GuiText::placeholder();
        text.set_text("Should not crash");
        assert_eq!(text.text(), "");
        assert_eq!(text.name(), "<placeholder>");
    }

    #[test]
    fn test_bound_text_updates() {
        let node = GuiElement::TextBox {
            name: "test_label".to_string(),
            position: (10, 20),
            font: "default".to_string(),
            max_width: 100,
            max_height: 30,
            format: crate::gui::types::TextFormat::Left,
            orientation: Orientation::UpperLeft,
            text: "Initial".to_string(),
            border_size: (0, 0),
        };

        let mut text = GuiText::from_node(&node).expect("Should bind to TextBox");
        assert_eq!(text.text(), "Initial");

        text.set_text("Updated");
        assert_eq!(text.text(), "Updated");
    }
}
