#![allow(dead_code)]
//! GuiIcon - wrapper around Icon elements for runtime binding.

use crate::gui::binder::{Bindable, GuiNode};
use crate::gui::core::{EventResult, GuiRenderer, GuiWidget, UiContext, UiEvent};
use crate::gui::types::{GuiElement, Orientation, Rect};

/// Handle to an icon/sprite widget.
///
/// Wraps an Icon element and provides a mutable interface for
/// updating the sprite frame, visibility, and other properties.
#[derive(Debug, Clone)]
pub struct GuiIcon {
    /// The underlying element data, if bound.
    element: Option<IconData>,
}

#[derive(Debug, Clone)]
struct IconData {
    name: String,
    position: (i32, i32),
    sprite_type: String,
    /// Current animation frame.
    frame: u32,
    orientation: Orientation,
    scale: f32,
    /// Whether the icon is visible.
    visible: bool,
}

impl GuiIcon {
    /// Set the animation frame.
    ///
    /// For sprite strips with multiple frames (e.g., speed indicators),
    /// this selects which frame to display.
    pub fn set_frame(&mut self, frame: u32) {
        if let Some(ref mut data) = self.element {
            data.frame = frame;
        }
    }

    /// Get the current frame.
    pub fn frame(&self) -> u32 {
        self.element.as_ref().map(|d| d.frame).unwrap_or(0)
    }

    /// Set visibility.
    pub fn set_visible(&mut self, visible: bool) {
        if let Some(ref mut data) = self.element {
            data.visible = visible;
        }
    }

    /// Check if visible.
    pub fn is_visible(&self) -> bool {
        self.element.as_ref().map(|d| d.visible).unwrap_or(false)
    }

    /// Get the sprite type name.
    pub fn sprite_type(&self) -> &str {
        self.element
            .as_ref()
            .map(|d| d.sprite_type.as_str())
            .unwrap_or("")
    }

    /// Get the element name (for debugging).
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
    pub fn orientation(&self) -> Orientation {
        self.element
            .as_ref()
            .map(|d| d.orientation)
            .unwrap_or(Orientation::UpperLeft)
    }
}

impl Bindable for GuiIcon {
    fn from_node(node: &GuiNode) -> Option<Self> {
        match node {
            GuiElement::Icon {
                name,
                position,
                sprite_type,
                frame,
                orientation,
                scale,
            } => Some(Self {
                element: Some(IconData {
                    name: name.clone(),
                    position: *position,
                    sprite_type: sprite_type.clone(),
                    frame: *frame,
                    orientation: *orientation,
                    scale: *scale,
                    visible: true, // Icons start visible by default
                }),
            }),
            // Speed indicators in EU4 are Button elements (sprite strips)
            GuiElement::Button {
                name,
                position,
                sprite_type,
                orientation,
                ..
            } => Some(Self {
                element: Some(IconData {
                    name: name.clone(),
                    position: *position,
                    sprite_type: sprite_type.clone(),
                    frame: 0, // Start at frame 0
                    orientation: *orientation,
                    scale: 1.0, // Buttons don't have scale, use default
                    visible: true,
                }),
            }),
            _ => None,
        }
    }

    fn placeholder() -> Self {
        Self { element: None }
    }
}

impl GuiWidget for GuiIcon {
    fn render(&self, _ctx: &UiContext, _renderer: &mut dyn GuiRenderer) {
        // Rendering logic will be implemented when integrating with the actual renderer
    }

    fn handle_input(&mut self, _event: &UiEvent, _ctx: &UiContext) -> EventResult {
        // Icons don't handle input (unless they're used as buttons, which is GuiButton's job)
        EventResult::Ignored
    }

    fn bounds(&self) -> Rect {
        if let Some(ref data) = self.element {
            // TODO: Get actual sprite dimensions from sprite cache
            // For now, assume a reasonable default
            let size = 32.0 * data.scale;
            Rect {
                x: data.position.0 as f32,
                y: data.position.1 as f32,
                width: size,
                height: size,
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
    fn test_placeholder_is_noop() {
        let mut icon = GuiIcon::placeholder();
        icon.set_frame(5);
        icon.set_visible(true);
        assert_eq!(icon.frame(), 0);
        assert!(!icon.is_visible());
    }

    #[test]
    fn test_bound_icon_updates() {
        let node = GuiElement::Icon {
            name: "speed_icon".to_string(),
            position: (100, 200),
            sprite_type: "GFX_speed_indicator".to_string(),
            frame: 0,
            orientation: Orientation::UpperLeft,
            scale: 1.0,
        };

        let mut icon = GuiIcon::from_node(&node).expect("Should bind to Icon");
        assert_eq!(icon.frame(), 0);
        assert!(icon.is_visible());

        icon.set_frame(3);
        assert_eq!(icon.frame(), 3);

        icon.set_visible(false);
        assert!(!icon.is_visible());
    }
}
