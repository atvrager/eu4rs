#![allow(dead_code)] // Reserved for future nested UI panel layouts
//! GuiContainer - wrapper around Window elements for hierarchical layouts.
//!
//! Not yet used in production panels. Reserved for future use when implementing
//! complex nested layouts or scrollable containers (Phase 4+).

use crate::gui::binder::{Bindable, GuiNode};
use crate::gui::core::{EventResult, GuiRenderer, GuiWidget, UiContext, UiEvent};
use crate::gui::types::{GuiElement, Orientation, Rect};

/// Handle to a container widget (window/panel).
///
/// Containers hold other widgets in a hierarchy and handle
/// recursive rendering and input propagation to children.
#[derive(Debug, Clone)]
pub struct GuiContainer {
    /// The underlying element data, if bound.
    element: Option<ContainerData>,
    /// Whether this container is visible (used for panel visibility control).
    visible: bool,
}

#[derive(Debug, Clone)]
struct ContainerData {
    name: String,
    position: (i32, i32),
    size: (u32, u32),
    orientation: Orientation,
    /// Child elements (stored as raw GuiElements for now).
    /// In a more advanced system, these would be recursively bound.
    children: Vec<GuiElement>,
}

impl GuiContainer {
    /// Get the container's position.
    pub fn position(&self) -> (i32, i32) {
        self.element.as_ref().map(|d| d.position).unwrap_or((0, 0))
    }

    /// Get the container's size.
    pub fn size(&self) -> (u32, u32) {
        self.element.as_ref().map(|d| d.size).unwrap_or((0, 0))
    }

    /// Get the number of children.
    pub fn child_count(&self) -> usize {
        self.element.as_ref().map(|d| d.children.len()).unwrap_or(0)
    }

    /// Get the element name (for debugging).
    pub fn name(&self) -> &str {
        self.element
            .as_ref()
            .map(|d| d.name.as_str())
            .unwrap_or("<placeholder>")
    }

    /// Access children (for advanced use cases).
    pub fn children(&self) -> &[GuiElement] {
        self.element
            .as_ref()
            .map(|d| d.children.as_slice())
            .unwrap_or(&[])
    }

    /// Check if this container is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Show this container.
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hide this container.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Set visibility state.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

impl Bindable for GuiContainer {
    fn from_node(node: &GuiNode) -> Option<Self> {
        match node {
            GuiElement::Window {
                name,
                position,
                size,
                orientation,
                children,
            } => Some(Self {
                element: Some(ContainerData {
                    name: name.clone(),
                    position: *position,
                    size: *size,
                    orientation: *orientation,
                    children: children.clone(),
                }),
                visible: true, // Containers are visible by default
            }),
            _ => None,
        }
    }

    fn placeholder() -> Self {
        Self {
            element: None,
            visible: true, // Placeholders are visible by default (no-op)
        }
    }
}

impl GuiWidget for GuiContainer {
    #[allow(clippy::needless_return)] // Early return is clearer for visibility check
    fn render(&self, _ctx: &UiContext, _renderer: &mut dyn GuiRenderer) {
        // Skip rendering if hidden
        if !self.visible {
            return;
        }

        // In a full implementation, this would:
        // 1. Render the container's background (if any)
        // 2. Recursively render all children
        // For now, this is a stub
    }

    fn handle_input(&mut self, _event: &UiEvent, _ctx: &UiContext) -> EventResult {
        // Skip input handling if hidden
        if !self.visible {
            return EventResult::Ignored;
        }

        // In a full implementation, this would recursively dispatch
        // the event to children in reverse render order (topmost first).
        // For now, containers don't handle input directly.
        EventResult::Ignored
    }

    fn bounds(&self) -> Rect {
        if let Some(ref data) = self.element {
            Rect {
                x: data.position.0 as f32,
                y: data.position.1 as f32,
                width: data.size.0 as f32,
                height: data.size.1 as f32,
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
    fn test_placeholder_container() {
        let container = GuiContainer::placeholder();
        assert_eq!(container.child_count(), 0);
        assert_eq!(container.position(), (0, 0));
        assert_eq!(container.size(), (0, 0));
    }

    #[test]
    fn test_bound_container() {
        let node = GuiElement::Window {
            name: "main_panel".to_string(),
            position: (100, 200),
            size: (400, 300),
            orientation: Orientation::UpperLeft,
            children: vec![GuiElement::TextBox {
                name: "label".to_string(),
                position: (10, 10),
                font: "default".to_string(),
                max_width: 100,
                max_height: 20,
                format: crate::gui::types::TextFormat::Left,
                orientation: Orientation::UpperLeft,
                text: "Hello".to_string(),
                border_size: (0, 0),
            }],
        };

        let container = GuiContainer::from_node(&node).expect("Should bind to Window");
        assert_eq!(container.name(), "main_panel");
        assert_eq!(container.position(), (100, 200));
        assert_eq!(container.size(), (400, 300));
        assert_eq!(container.child_count(), 1);
    }

    #[test]
    fn test_container_visibility_default() {
        let container = GuiContainer::placeholder();
        assert!(container.is_visible());

        let node = GuiElement::Window {
            name: "test".to_string(),
            position: (0, 0),
            size: (100, 100),
            orientation: Orientation::UpperLeft,
            children: vec![],
        };
        let container = GuiContainer::from_node(&node).expect("Should bind");
        assert!(container.is_visible());
    }

    #[test]
    fn test_container_show_hide() {
        let mut container = GuiContainer::placeholder();
        assert!(container.is_visible());

        container.hide();
        assert!(!container.is_visible());

        container.show();
        assert!(container.is_visible());
    }

    #[test]
    fn test_container_set_visible() {
        let mut container = GuiContainer::placeholder();

        container.set_visible(false);
        assert!(!container.is_visible());

        container.set_visible(true);
        assert!(container.is_visible());
    }

    // Note: render() behavior with visibility is tested implicitly via integration
    // The render() method checks is_visible() internally

    #[test]
    fn test_hidden_container_ignores_input() {
        use crate::gui::core::{ButtonState as InputButtonState, MouseButton, UiEvent};

        let mut container = GuiContainer::placeholder();
        let ctx = UiContext {
            mouse_pos: (0.0, 0.0),
            time: 0.0,
            delta_time: 0.016,
            localizer: &crate::gui::core::NoOpLocalizer,
            focused_widget: None,
        };

        // Visible container processes input (even if it just ignores it)
        let result = container.handle_input(
            &UiEvent::MouseButton {
                button: MouseButton::Left,
                state: InputButtonState::Released,
                x: 10.0,
                y: 10.0,
            },
            &ctx,
        );
        assert_eq!(result, EventResult::Ignored);

        // Hidden container should also ignore
        container.hide();
        let result = container.handle_input(
            &UiEvent::MouseButton {
                button: MouseButton::Left,
                state: InputButtonState::Released,
                x: 10.0,
                y: 10.0,
            },
            &ctx,
        );
        assert_eq!(result, EventResult::Ignored);
    }
}
