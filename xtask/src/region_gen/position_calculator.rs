//! Position calculation for GUI elements.
//!
//! Converts GUI element data to absolute screen coordinates for 1920x1080 resolution.

use eu4game::gui::layout::{get_window_anchor, position_from_anchor};
use eu4game::gui::types::GuiElement;

/// Calculate absolute screen position for a GUI element.
///
/// Takes an element and its parent window, calculates the element's absolute
/// screen position using EU4's layout system.
///
/// # Returns
/// (x, y, width, height) in pixels for 1920x1080 resolution.
pub fn calculate_screen_position(
    element: &GuiElement,
    window: &GuiElement,
    screen_size: (u32, u32),
) -> (u32, u32, u32, u32) {
    // Get window anchor point
    let window_pos = window.position();
    let window_orientation = window.orientation();
    let anchor = get_window_anchor(window_pos, window_orientation, screen_size);

    // Calculate element position from anchor
    let element_pos = element.position();
    let element_orientation = element.orientation();
    let element_size = estimate_element_size(element);

    let (x, y) = position_from_anchor(anchor, element_pos, element_orientation, element_size);

    // Clamp to screen bounds and convert to u32
    let x_clamped = x.max(0.0).min((screen_size.0 - element_size.0) as f32) as u32;
    let y_clamped = y.max(0.0).min((screen_size.1 - element_size.1) as f32) as u32;

    (x_clamped, y_clamped, element_size.0, element_size.1)
}

/// Estimate the size of a GUI element.
///
/// Extracts size from element fields where available, or uses reasonable defaults.
fn estimate_element_size(element: &GuiElement) -> (u32, u32) {
    match element {
        GuiElement::Window { size, .. } => *size,
        GuiElement::TextBox {
            max_width,
            max_height,
            ..
        } => (*max_width, *max_height),
        GuiElement::EditBox { size, .. } => *size,
        GuiElement::Listbox { size, .. } => *size,
        GuiElement::Scrollbar { size, .. } => *size,
        // For icons and buttons without explicit size, use reasonable defaults
        GuiElement::Icon { .. } => (32, 32),
        GuiElement::Button { .. } => (32, 32),
        GuiElement::Checkbox { .. } => (24, 24),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eu4game::gui::types::{GuiElement, Orientation, TextFormat};

    #[test]
    fn test_calculate_position_upper_left() {
        // Window at origin with UPPER_LEFT orientation
        let window = GuiElement::Window {
            name: "topbar".to_string(),
            position: (0, 0),
            size: (1920, 100),
            orientation: Orientation::UpperLeft,
            children: vec![],
        };

        // Text element offset from window anchor
        let element = GuiElement::TextBox {
            name: "text_gold".to_string(),
            position: (169, 13),
            font: "vic_18".to_string(),
            max_width: 48,
            max_height: 21,
            format: TextFormat::Right,
            orientation: Orientation::UpperLeft,
            text: "".to_string(),
            border_size: (0, 0),
        };

        let (x, y, w, h) = calculate_screen_position(&element, &window, (1920, 1080));

        assert_eq!(x, 169);
        assert_eq!(y, 13);
        assert_eq!(w, 48);
        assert_eq!(h, 21);
    }

    #[test]
    fn test_calculate_position_upper_right() {
        // Window anchored to upper-right with negative offset
        let window = GuiElement::Window {
            name: "speed_controls".to_string(),
            position: (0, 0),
            size: (300, 100),
            orientation: Orientation::UpperRight,
            children: vec![],
        };

        // Date element with negative X offset (227 pixels from right edge)
        let element = GuiElement::TextBox {
            name: "DateText".to_string(),
            position: (-227, 13),
            font: "vic_18".to_string(),
            max_width: 140,
            max_height: 32,
            format: TextFormat::Center,
            orientation: Orientation::UpperRight,
            text: "".to_string(),
            border_size: (0, 0),
        };

        let (x, y, w, h) = calculate_screen_position(&element, &window, (1920, 1080));

        // Anchor is at (1920, 0), element offset is -227
        // Expected x = 1920 + (-227) = 1693
        assert_eq!(x, 1693);
        assert_eq!(y, 13);
        assert_eq!(w, 140);
        assert_eq!(h, 32);
    }

    #[test]
    fn test_estimate_element_size() {
        // TextBox should use max_width/max_height
        let textbox = GuiElement::TextBox {
            name: "test".to_string(),
            position: (0, 0),
            font: "vic_18".to_string(),
            max_width: 100,
            max_height: 50,
            format: TextFormat::Left,
            orientation: Orientation::UpperLeft,
            text: "".to_string(),
            border_size: (0, 0),
        };
        assert_eq!(estimate_element_size(&textbox), (100, 50));

        // Icon should use default size
        let icon = GuiElement::Icon {
            name: "icon_test".to_string(),
            position: (0, 0),
            sprite_type: "GFX_test".to_string(),
            frame: 0,
            orientation: Orientation::UpperLeft,
            scale: 1.0,
        };
        assert_eq!(estimate_element_size(&icon), (32, 32));
    }

    #[test]
    fn test_clamp_to_bounds() {
        let window = GuiElement::Window {
            name: "test".to_string(),
            position: (0, 0),
            size: (1920, 1080),
            orientation: Orientation::UpperLeft,
            children: vec![],
        };

        // Element positioned beyond screen bounds
        let element = GuiElement::TextBox {
            name: "text_overflow".to_string(),
            position: (2000, 50), // Beyond right edge
            font: "vic_18".to_string(),
            max_width: 100,
            max_height: 20,
            format: TextFormat::Left,
            orientation: Orientation::UpperLeft,
            text: "".to_string(),
            border_size: (0, 0),
        };

        let (x, y, _, _) = calculate_screen_position(&element, &window, (1920, 1080));

        // Should be clamped to fit within screen
        assert!(x <= 1920 - 100); // x + width <= screen_width
        assert_eq!(y, 50);
    }
}
