#![allow(dead_code)]
//! GuiContainer - wrapper around Window elements for hierarchical layouts.

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
            }),
            _ => None,
        }
    }

    fn placeholder() -> Self {
        Self { element: None }
    }
}

impl GuiWidget for GuiContainer {
    fn render(&self, _ctx: &UiContext, _renderer: &mut dyn GuiRenderer) {
        // In a full implementation, this would:
        // 1. Render the container's background (if any)
        // 2. Recursively render all children
        // For now, this is a stub
    }

    fn handle_input(&mut self, _event: &UiEvent, _ctx: &UiContext) -> EventResult {
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
}
