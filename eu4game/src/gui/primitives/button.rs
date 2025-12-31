#![allow(dead_code)] // Reserved for future interactive UI panels
//! GuiButton - wrapper around Button elements with interactive state.
//!
//! Currently used in macro tests to verify the binding system works with buttons.
//! Will be used in production when implementing interactive UI panels (Phase 4).

use crate::gui::binder::{Bindable, GuiNode};
use crate::gui::core::{
    ButtonState as InputButtonState, EventResult, GuiRenderer, GuiWidget, MouseButton, UiAction,
    UiContext, UiEvent,
};
use crate::gui::types::{GuiElement, Orientation, Rect};

/// Interactive button state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Normal,
    Hovered,
    Pressed,
    Disabled,
}

/// Handle to an interactive button widget.
///
/// Tracks button state (normal/hover/pressed/disabled) and fires
/// click callbacks when activated.
#[derive(Debug, Clone)]
pub struct GuiButton {
    /// The underlying element data, if bound.
    element: Option<ButtonData>,
    /// Current button state.
    state: ButtonState,
    /// Callback to invoke on click (stored as a function ID or similar).
    /// For now, we track click state and let the parent panel poll it.
    was_clicked: bool,
    /// Action to perform when clicked.
    action: UiAction,
}

#[derive(Debug, Clone)]
struct ButtonData {
    name: String,
    position: (i32, i32),
    sprite_type: String,
    orientation: Orientation,
    shortcut: Option<String>,
}

impl GuiButton {
    /// Set whether the button is enabled.
    pub fn set_enabled(&mut self, enabled: bool) {
        if enabled {
            if self.state == ButtonState::Disabled {
                self.state = ButtonState::Normal;
            }
        } else {
            self.state = ButtonState::Disabled;
        }
    }

    /// Check if the button is enabled.
    pub fn is_enabled(&self) -> bool {
        self.state != ButtonState::Disabled
    }

    /// Get the current button state.
    pub fn state(&self) -> ButtonState {
        self.state
    }

    /// Check if the button was clicked since the last reset.
    ///
    /// This is a polling-based approach - the parent panel should
    /// call this to detect clicks and then reset the flag.
    pub fn was_clicked(&self) -> bool {
        self.was_clicked
    }

    /// Reset the click flag (called after processing the click).
    pub fn reset_click(&mut self) {
        self.was_clicked = false;
    }

    /// Set the action to perform when clicked.
    pub fn set_action(&mut self, action: UiAction) {
        self.action = action;
    }

    /// Get the action that will be performed when clicked.
    pub fn action(&self) -> UiAction {
        self.action
    }

    /// Check if the button was clicked and return the action.
    ///
    /// Resets the click flag if a click was detected.
    pub fn poll_click(&mut self) -> Option<UiAction> {
        if self.was_clicked {
            self.was_clicked = false;
            Some(self.action)
        } else {
            None
        }
    }

    /// Get the element name (for debugging).
    pub fn name(&self) -> &str {
        self.element
            .as_ref()
            .map(|d| d.name.as_str())
            .unwrap_or("<placeholder>")
    }
}

impl Bindable for GuiButton {
    fn from_node(node: &GuiNode) -> Option<Self> {
        match node {
            GuiElement::Button {
                name,
                position,
                sprite_type,
                orientation,
                shortcut,
            } => Some(Self {
                element: Some(ButtonData {
                    name: name.clone(),
                    position: *position,
                    sprite_type: sprite_type.clone(),
                    orientation: *orientation,
                    shortcut: shortcut.clone(),
                }),
                state: ButtonState::Normal,
                was_clicked: false,
                action: UiAction::None, // Default to no action, set by panel after binding
            }),
            _ => None,
        }
    }

    fn placeholder() -> Self {
        Self {
            element: None,
            state: ButtonState::Disabled,
            was_clicked: false,
            action: UiAction::None,
        }
    }
}

impl GuiWidget for GuiButton {
    fn render(&self, _ctx: &UiContext, _renderer: &mut dyn GuiRenderer) {
        // Rendering logic will be implemented when integrating with the actual renderer
        // Should draw different sprite frames based on self.state
    }

    fn handle_input(&mut self, event: &UiEvent, ctx: &UiContext) -> EventResult {
        if self.state == ButtonState::Disabled || self.element.is_none() {
            return EventResult::Ignored;
        }

        match event {
            UiEvent::MouseMove { x, y } => {
                if self.hit_test(*x, *y) {
                    if self.state == ButtonState::Normal {
                        self.state = ButtonState::Hovered;
                    }
                } else if self.state == ButtonState::Hovered {
                    self.state = ButtonState::Normal;
                }
                EventResult::Ignored
            }
            UiEvent::MouseButton {
                button: MouseButton::Left,
                state: InputButtonState::Pressed,
                x,
                y,
            } => {
                if self.hit_test(*x, *y) {
                    self.state = ButtonState::Pressed;
                    EventResult::Consumed
                } else {
                    EventResult::Ignored
                }
            }
            UiEvent::MouseButton {
                button: MouseButton::Left,
                state: InputButtonState::Released,
                x,
                y,
            } => {
                if self.state == ButtonState::Pressed {
                    if self.hit_test(*x, *y) {
                        // Click completed!
                        self.was_clicked = true;
                        self.state = ButtonState::Hovered;
                    } else {
                        // Drag released outside button
                        self.state = if self.hit_test(ctx.mouse_pos.0, ctx.mouse_pos.1) {
                            ButtonState::Hovered
                        } else {
                            ButtonState::Normal
                        };
                    }
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
            // TODO: Get actual button sprite dimensions
            // For now, assume a reasonable default
            Rect {
                x: data.position.0 as f32,
                y: data.position.1 as f32,
                width: 64.0,
                height: 32.0,
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
        let button = GuiButton::placeholder();
        assert_eq!(button.state(), ButtonState::Disabled);
        assert!(!button.is_enabled());
    }

    #[test]
    fn test_button_enable_disable() {
        let node = GuiElement::Button {
            name: "test_button".to_string(),
            position: (0, 0),
            sprite_type: "GFX_button".to_string(),
            orientation: Orientation::UpperLeft,
            shortcut: None,
        };

        let mut button = GuiButton::from_node(&node).expect("Should bind to Button");
        assert!(button.is_enabled());

        button.set_enabled(false);
        assert!(!button.is_enabled());
        assert_eq!(button.state(), ButtonState::Disabled);

        button.set_enabled(true);
        assert!(button.is_enabled());
        assert_eq!(button.state(), ButtonState::Normal);
    }

    #[test]
    fn test_button_click_tracking() {
        let node = GuiElement::Button {
            name: "test_button".to_string(),
            position: (0, 0),
            sprite_type: "GFX_button".to_string(),
            orientation: Orientation::UpperLeft,
            shortcut: None,
        };

        let mut button = GuiButton::from_node(&node).expect("Should bind to Button");
        assert!(!button.was_clicked());

        // Simulate click sequence
        let ctx = UiContext {
            mouse_pos: (10.0, 10.0),
            time: 0.0,
            delta_time: 0.016,
            localizer: &crate::gui::core::NoOpLocalizer,
            focused_widget: None,
        };

        // Press
        button.handle_input(
            &UiEvent::MouseButton {
                button: MouseButton::Left,
                state: InputButtonState::Pressed,
                x: 10.0,
                y: 10.0,
            },
            &ctx,
        );
        assert_eq!(button.state(), ButtonState::Pressed);

        // Release
        button.handle_input(
            &UiEvent::MouseButton {
                button: MouseButton::Left,
                state: InputButtonState::Released,
                x: 10.0,
                y: 10.0,
            },
            &ctx,
        );
        assert!(button.was_clicked());

        button.reset_click();
        assert!(!button.was_clicked());
    }
}
