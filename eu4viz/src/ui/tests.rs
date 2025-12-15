use super::*;
use crate::logger::ConsoleLog;
use crate::text::TextRenderer;

fn get_font_data() -> Option<Vec<u8>> {
    // Try workspace root (if running from root)
    if let Ok(data) = std::fs::read("assets/Roboto-Regular.ttf") {
        return Some(data);
    }
    // Try parent (if running from crate dir)
    if let Ok(data) = std::fs::read("../assets/Roboto-Regular.ttf") {
        return Some(data);
    }
    None
}

#[test]
fn test_ui_state_initialization() {
    let ui = UIState::new();
    assert!(!ui.sidebar_open);
    assert!(ui.dirty);
    assert_eq!(ui.map_mode, MapMode::Province);
}

#[test]
fn test_ui_state_updates() {
    let mut ui = UIState::new();
    ui.dirty = false;

    ui.set_sidebar_open(true);
    assert!(ui.sidebar_open);
    assert!(ui.dirty);

    ui.dirty = false;
    ui.set_hovered_tooltip(Some("Test".to_string()));
    assert_eq!(ui.hovered_tooltip.as_deref(), Some("Test"));
    assert!(ui.dirty);

    ui.dirty = false;
    ui.set_selected_province(Some((1, "Prov".into())));
    assert!(ui.dirty);

    ui.dirty = false;
    ui.toggle_console();
    assert!(ui.console_open);
    assert!(ui.dirty);
}

#[test]
fn test_click_handling() {
    let mut ui = UIState::new();
    let width = 1000.0;

    // Sidebar closed
    assert!(!ui.on_click(900.0, width)); // Should not consume click

    // Sidebar open
    ui.set_sidebar_open(true);
    assert!(ui.on_click(900.0, width)); // Click in sidebar (last 300px)
    assert!(!ui.on_click(600.0, width)); // Click out of sidebar
}

#[test]
fn test_render_snapshot() {
    let font_data = match get_font_data() {
        Some(d) => d,
        None => panic!("Failed to load assets/Roboto-Regular.ttf for test"),
    };

    let text_renderer = TextRenderer::new(font_data);
    let console_log = ConsoleLog::new(10);
    let mut ui = UIState::new();

    // Setup complex state for snapshot
    ui.set_sidebar_open(true);
    ui.set_selected_province(Some((42, "Stockholm\nOwner: SWE".into())));
    ui.set_hovered_tooltip(Some("Hovering...".into()));
    ui.set_cursor_pos(Some((50.0, 950.0))); // Bottom left
    ui.map_mode = MapMode::Political;

    let img = ui.render(&text_renderer, 800, 600, &console_log);

    // Use crate::testing for verify
    crate::testing::assert_snapshot(&img, "ui_render_complex");
}

#[test]
fn test_render_full_ui() {
    let font_data = match get_font_data() {
        Some(d) => d,
        None => panic!("Failed to load assets/Roboto-Regular.ttf for test"),
    };
    let text_renderer = TextRenderer::new(font_data);
    let console_log = ConsoleLog::new(10);

    // Populate logger buffer manually
    console_log.push(log::Level::Info, "System started".into());
    console_log.push(log::Level::Warn, "Low memory".into());
    console_log.push(log::Level::Error, "Crash imminent".into());

    let mut ui = UIState::new();
    ui.set_sidebar_open(true);
    ui.set_selected_province(Some((1, "Test Prov".into())));
    ui.console_open = true; // Enable console overlay
    ui.map_mode = MapMode::TradeGoods;

    // Render
    let img = ui.render(&text_renderer, 800, 600, &console_log);

    // Snapshot verify
    crate::testing::assert_snapshot(&img, "ui_render_full");
}
