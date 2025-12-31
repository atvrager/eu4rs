//! GuiCheckbox - wrapper around Checkbox elements with toggle state.
//!
//! Used for interactive checkboxes like Ironman mode, nation designer options, etc.

use crate::gui::binder::{Bindable, GuiNode};
use crate::gui::core::{
    ButtonState as InputButtonState, EventResult, GuiRenderer, GuiWidget, MouseButton, UiContext,
    UiEvent,
};
use crate::gui::types::{GuiElement, Orientation, Rect};

/// Handle to an interactive checkbox widget.
///
/// Tracks checkbox state (checked/unchecked) and responds to clicks.
#[derive(Debug, Clone)]
pub struct GuiCheckbox {
    /// The underlying element data, if bound.
    element: Option<CheckboxData>,
    /// Whether the checkbox is checked.
    #[allow(dead_code)] // Used in tests and future production code
    checked: bool,
    /// Whether the checkbox is enabled.
    #[allow(dead_code)] // Used in tests and future production code
    enabled: bool,
}

#[derive(Debug, Clone)]
struct CheckboxData {
    #[allow(dead_code)] // Used for debugging
    name: String,
    position: (i32, i32),
    #[allow(dead_code)] // Reserved for sprite rendering
    sprite_type: String,
    orientation: Orientation,
}

impl GuiCheckbox {
    /// Set whether the checkbox is checked.
    #[allow(dead_code)] // Used in tests and future production code
    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }

    /// Get whether the checkbox is checked.
    #[allow(dead_code)] // Used in tests and future production code
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Toggle the checked state.
    #[allow(dead_code)] // Used in tests and future production code
    pub fn toggle(&mut self) {
        self.checked = !self.checked;
    }

    /// Set whether the checkbox is enabled.
    #[allow(dead_code)] // Used in tests and future production code
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if the checkbox is enabled.
    #[allow(dead_code)] // Used in tests and future production code
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the element name (for debugging).
    #[allow(dead_code)] // Used in tests
    pub fn name(&self) -> &str {
        self.element
            .as_ref()
            .map(|d| d.name.as_str())
            .unwrap_or("<placeholder>")
    }

    /// Get the position (for rendering).
    #[allow(dead_code)] // Used for rendering
    pub fn position(&self) -> (i32, i32) {
        self.element.as_ref().map(|d| d.position).unwrap_or((0, 0))
    }

    /// Get the orientation (for rendering).
    #[allow(dead_code)] // Used for rendering
    pub fn orientation(&self) -> Orientation {
        self.element
            .as_ref()
            .map(|d| d.orientation)
            .unwrap_or(Orientation::UpperLeft)
    }
}

impl Bindable for GuiCheckbox {
    fn from_node(node: &GuiNode) -> Option<Self> {
        match node {
            GuiElement::Checkbox {
                name,
                position,
                sprite_type,
                orientation,
            } => Some(Self {
                element: Some(CheckboxData {
                    name: name.clone(),
                    position: *position,
                    sprite_type: sprite_type.clone(),
                    orientation: *orientation,
                }),
                checked: false, // Start unchecked by default
                enabled: true,
            }),
            _ => None,
        }
    }

    fn placeholder() -> Self {
        Self {
            element: None,
            checked: false,
            enabled: false,
        }
    }
}

impl GuiWidget for GuiCheckbox {
    fn render(&self, _ctx: &UiContext, _renderer: &mut dyn GuiRenderer) {
        // Rendering logic will be implemented when integrating with the actual renderer
        // Should draw different sprite frames based on self.checked and self.enabled
    }

    fn handle_input(&mut self, event: &UiEvent, _ctx: &UiContext) -> EventResult {
        if !self.enabled || self.element.is_none() {
            return EventResult::Ignored;
        }

        match event {
            UiEvent::MouseButton {
                button: MouseButton::Left,
                state: InputButtonState::Released,
                x,
                y,
            } => {
                if self.hit_test(*x, *y) {
                    // Toggle on click
                    self.toggle();
                    EventResult::Consumed
                } else {
                    EventResult::Ignored
                }
            }
            _ => EventResult::Ignored,
        }
    }

    fn bounds(&self) -> Rect {
        if let Some(ref data) = self.element {
            // TODO: Get actual checkbox sprite dimensions
            // For now, assume a reasonable default (checkboxes are usually square)
            Rect {
                x: data.position.0 as f32,
                y: data.position.1 as f32,
                width: 24.0,
                height: 24.0,
            }
        } else {
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

    #[test]
    fn test_placeholder_is_disabled() {
        let checkbox = GuiCheckbox::placeholder();
        assert!(!checkbox.is_enabled());
        assert!(!checkbox.is_checked());
    }

    #[test]
    fn test_checkbox_toggle() {
        let node = GuiElement::Checkbox {
            name: "test_checkbox".to_string(),
            position: (0, 0),
            sprite_type: "GFX_checkbox".to_string(),
            orientation: Orientation::UpperLeft,
        };

        let mut checkbox = GuiCheckbox::from_node(&node).expect("Should bind to Checkbox");
        assert!(!checkbox.is_checked());

        checkbox.toggle();
        assert!(checkbox.is_checked());

        checkbox.toggle();
        assert!(!checkbox.is_checked());
    }

    #[test]
    fn test_checkbox_set_checked() {
        let node = GuiElement::Checkbox {
            name: "test_checkbox".to_string(),
            position: (0, 0),
            sprite_type: "GFX_checkbox".to_string(),
            orientation: Orientation::UpperLeft,
        };

        let mut checkbox = GuiCheckbox::from_node(&node).expect("Should bind to Checkbox");

        checkbox.set_checked(true);
        assert!(checkbox.is_checked());

        checkbox.set_checked(false);
        assert!(!checkbox.is_checked());
    }

    #[test]
    fn test_checkbox_click_handling() {
        let node = GuiElement::Checkbox {
            name: "test_checkbox".to_string(),
            position: (0, 0),
            sprite_type: "GFX_checkbox".to_string(),
            orientation: Orientation::UpperLeft,
        };

        let mut checkbox = GuiCheckbox::from_node(&node).expect("Should bind to Checkbox");
        let ctx = UiContext {
            mouse_pos: (10.0, 10.0),
            time: 0.0,
            delta_time: 0.016,
            localizer: &crate::gui::core::NoOpLocalizer,
            focused_widget: None,
        };

        assert!(!checkbox.is_checked());

        // Simulate click
        let result = checkbox.handle_input(
            &UiEvent::MouseButton {
                button: MouseButton::Left,
                state: InputButtonState::Released,
                x: 10.0,
                y: 10.0,
            },
            &ctx,
        );

        assert_eq!(result, EventResult::Consumed);
        assert!(checkbox.is_checked());

        // Click again to uncheck
        checkbox.handle_input(
            &UiEvent::MouseButton {
                button: MouseButton::Left,
                state: InputButtonState::Released,
                x: 10.0,
                y: 10.0,
            },
            &ctx,
        );

        assert!(!checkbox.is_checked());
    }
}
