//! GuiEditBox - wrapper around EditBox elements for text input.
//!
//! Used for text input fields like player name, date input, IP address entry, etc.

use crate::gui::binder::{Bindable, GuiNode};
use crate::gui::core::{
    ButtonState as InputButtonState, EventResult, GuiRenderer, GuiWidget, KeyCode, MouseButton,
    UiContext, UiEvent,
};
use crate::gui::types::{GuiElement, Orientation, Rect};

/// Handle to an interactive text input widget.
///
/// Tracks text content, cursor position, and handles keyboard input.
#[derive(Debug, Clone)]
pub struct GuiEditBox {
    /// The underlying element data, if bound.
    element: Option<EditBoxData>,
    /// Current text content.
    #[allow(dead_code)] // Used in tests and future production code
    text: String,
    /// Cursor position (index into text string).
    #[allow(dead_code)] // Used in tests and future production code
    cursor: usize,
    /// Whether this editbox has keyboard focus.
    #[allow(dead_code)] // Used in tests and future production code
    focused: bool,
}

#[derive(Debug, Clone)]
struct EditBoxData {
    #[allow(dead_code)] // Used for debugging
    name: String,
    position: (i32, i32),
    size: (u32, u32),
    #[allow(dead_code)] // Reserved for font rendering
    font: String,
    orientation: Orientation,
    max_characters: u32,
}

impl GuiEditBox {
    /// Set the text content.
    #[allow(dead_code)] // Used in tests and future production code
    pub fn set_text(&mut self, text: &str) {
        if let Some(ref data) = self.element {
            let max_len = data.max_characters as usize;
            if text.len() <= max_len {
                self.text = text.to_string();
                self.cursor = text.len();
            } else {
                self.text = text.chars().take(max_len).collect();
                self.cursor = self.text.len();
            }
        }
    }

    /// Get the current text content.
    #[allow(dead_code)] // Used in tests and future production code
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Insert a character at the cursor position.
    #[allow(dead_code)] // Used in tests and future production code
    pub fn insert_char(&mut self, c: char) {
        if let Some(ref data) = self.element
            && self.text.len() < data.max_characters as usize
        {
            self.text.insert(self.cursor, c);
            self.cursor += 1;
        }
    }

    /// Delete the character before the cursor (backspace).
    #[allow(dead_code)] // Used in tests and future production code
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.text.remove(self.cursor - 1);
            self.cursor -= 1;
        }
    }

    /// Delete the character at the cursor (delete key).
    #[allow(dead_code)] // Used in tests and future production code
    pub fn delete(&mut self) {
        if self.cursor < self.text.len() {
            self.text.remove(self.cursor);
        }
    }

    /// Move the cursor left.
    #[allow(dead_code)] // Used in tests and future production code
    pub fn move_cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move the cursor right.
    #[allow(dead_code)] // Used in tests and future production code
    pub fn move_cursor_right(&mut self) {
        if self.cursor < self.text.len() {
            self.cursor += 1;
        }
    }

    /// Set keyboard focus state.
    #[allow(dead_code)] // Used in tests and future production code
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Check if this editbox has keyboard focus.
    #[allow(dead_code)] // Used in tests and future production code
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Get the cursor position.
    #[allow(dead_code)] // Used in tests and future production code
    pub fn cursor(&self) -> usize {
        self.cursor
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

    /// Get the size (for rendering).
    #[allow(dead_code)] // Used for rendering
    pub fn size(&self) -> (u32, u32) {
        self.element.as_ref().map(|d| d.size).unwrap_or((0, 0))
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

impl Bindable for GuiEditBox {
    fn from_node(node: &GuiNode) -> Option<Self> {
        match node {
            GuiElement::EditBox {
                name,
                position,
                size,
                font,
                orientation,
                max_characters,
            } => Some(Self {
                element: Some(EditBoxData {
                    name: name.clone(),
                    position: *position,
                    size: *size,
                    font: font.clone(),
                    orientation: *orientation,
                    max_characters: *max_characters,
                }),
                text: String::new(),
                cursor: 0,
                focused: false,
            }),
            _ => None,
        }
    }

    fn placeholder() -> Self {
        Self {
            element: None,
            text: String::new(),
            cursor: 0,
            focused: false,
        }
    }
}

impl GuiWidget for GuiEditBox {
    fn render(&self, _ctx: &UiContext, _renderer: &mut dyn GuiRenderer) {
        // Rendering logic will be implemented when integrating with the actual renderer
        // Should draw background, text content, and cursor if focused
    }

    fn handle_input(&mut self, event: &UiEvent, _ctx: &UiContext) -> EventResult {
        if self.element.is_none() {
            return EventResult::Ignored;
        }

        match event {
            UiEvent::MouseButton {
                button: MouseButton::Left,
                state: InputButtonState::Released,
                x,
                y,
            } => {
                // Click to focus
                if self.hit_test(*x, *y) {
                    self.focused = true;
                    EventResult::Consumed
                } else if self.focused {
                    // Click outside to unfocus
                    self.focused = false;
                    EventResult::Ignored
                } else {
                    EventResult::Ignored
                }
            }
            UiEvent::TextInput { character } if self.focused => {
                // Insert character at cursor
                self.insert_char(*character);
                EventResult::Consumed
            }
            UiEvent::KeyPress { key, .. } if self.focused => match key {
                KeyCode::Backspace => {
                    self.backspace();
                    EventResult::Consumed
                }
                KeyCode::Delete => {
                    self.delete();
                    EventResult::Consumed
                }
                KeyCode::Left => {
                    self.move_cursor_left();
                    EventResult::Consumed
                }
                KeyCode::Right => {
                    self.move_cursor_right();
                    EventResult::Consumed
                }
                _ => EventResult::Ignored,
            },
            UiEvent::FocusLost => {
                self.focused = false;
                EventResult::Consumed
            }
            UiEvent::FocusGained => {
                self.focused = true;
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
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
    fn test_placeholder_is_empty() {
        let editbox = GuiEditBox::placeholder();
        assert_eq!(editbox.text(), "");
        assert!(!editbox.is_focused());
        assert_eq!(editbox.cursor(), 0);
    }

    #[test]
    fn test_set_text() {
        let node = GuiElement::EditBox {
            name: "test_editbox".to_string(),
            position: (0, 0),
            size: (200, 30),
            font: "default".to_string(),
            orientation: Orientation::UpperLeft,
            max_characters: 50,
        };

        let mut editbox = GuiEditBox::from_node(&node).expect("Should bind to EditBox");
        assert_eq!(editbox.text(), "");

        editbox.set_text("Hello");
        assert_eq!(editbox.text(), "Hello");
        assert_eq!(editbox.cursor(), 5);
    }

    #[test]
    fn test_insert_char() {
        let node = GuiElement::EditBox {
            name: "test_editbox".to_string(),
            position: (0, 0),
            size: (200, 30),
            font: "default".to_string(),
            orientation: Orientation::UpperLeft,
            max_characters: 50,
        };

        let mut editbox = GuiEditBox::from_node(&node).expect("Should bind to EditBox");
        editbox.insert_char('A');
        editbox.insert_char('B');
        editbox.insert_char('C');
        assert_eq!(editbox.text(), "ABC");
        assert_eq!(editbox.cursor(), 3);
    }

    #[test]
    fn test_backspace() {
        let node = GuiElement::EditBox {
            name: "test_editbox".to_string(),
            position: (0, 0),
            size: (200, 30),
            font: "default".to_string(),
            orientation: Orientation::UpperLeft,
            max_characters: 50,
        };

        let mut editbox = GuiEditBox::from_node(&node).expect("Should bind to EditBox");
        editbox.set_text("ABC");
        editbox.backspace();
        assert_eq!(editbox.text(), "AB");
        assert_eq!(editbox.cursor(), 2);
    }

    #[test]
    fn test_delete() {
        let node = GuiElement::EditBox {
            name: "test_editbox".to_string(),
            position: (0, 0),
            size: (200, 30),
            font: "default".to_string(),
            orientation: Orientation::UpperLeft,
            max_characters: 50,
        };

        let mut editbox = GuiEditBox::from_node(&node).expect("Should bind to EditBox");
        editbox.set_text("ABC");
        editbox.move_cursor_left(); // Move to position 2
        editbox.move_cursor_left(); // Move to position 1
        editbox.delete(); // Delete 'B'
        assert_eq!(editbox.text(), "AC");
        assert_eq!(editbox.cursor(), 1);
    }

    #[test]
    fn test_cursor_movement() {
        let node = GuiElement::EditBox {
            name: "test_editbox".to_string(),
            position: (0, 0),
            size: (200, 30),
            font: "default".to_string(),
            orientation: Orientation::UpperLeft,
            max_characters: 50,
        };

        let mut editbox = GuiEditBox::from_node(&node).expect("Should bind to EditBox");
        editbox.set_text("ABCDE");

        editbox.move_cursor_left();
        assert_eq!(editbox.cursor(), 4);
        editbox.move_cursor_left();
        assert_eq!(editbox.cursor(), 3);
        editbox.move_cursor_right();
        assert_eq!(editbox.cursor(), 4);
    }

    #[test]
    fn test_max_characters() {
        let node = GuiElement::EditBox {
            name: "test_editbox".to_string(),
            position: (0, 0),
            size: (200, 30),
            font: "default".to_string(),
            orientation: Orientation::UpperLeft,
            max_characters: 5,
        };

        let mut editbox = GuiEditBox::from_node(&node).expect("Should bind to EditBox");
        editbox.set_text("ABCDEFGH"); // Try to set 8 chars when max is 5
        assert_eq!(editbox.text(), "ABCDE");
        assert_eq!(editbox.cursor(), 5);

        // Try to insert beyond limit
        editbox.insert_char('X');
        assert_eq!(editbox.text(), "ABCDE"); // Should not insert
    }

    #[test]
    fn test_focus_handling() {
        let node = GuiElement::EditBox {
            name: "test_editbox".to_string(),
            position: (0, 0),
            size: (200, 30),
            font: "default".to_string(),
            orientation: Orientation::UpperLeft,
            max_characters: 50,
        };

        let mut editbox = GuiEditBox::from_node(&node).expect("Should bind to EditBox");
        let ctx = UiContext {
            mouse_pos: (10.0, 10.0),
            time: 0.0,
            delta_time: 0.016,
            localizer: &crate::gui::core::NoOpLocalizer,
            focused_widget: None,
        };

        assert!(!editbox.is_focused());

        // Click to focus
        editbox.handle_input(
            &UiEvent::MouseButton {
                button: MouseButton::Left,
                state: InputButtonState::Released,
                x: 10.0,
                y: 10.0,
            },
            &ctx,
        );
        assert!(editbox.is_focused());

        // Typing should work when focused
        let result = editbox.handle_input(&UiEvent::TextInput { character: 'A' }, &ctx);
        assert_eq!(result, EventResult::Consumed);
        assert_eq!(editbox.text(), "A");
    }
}
