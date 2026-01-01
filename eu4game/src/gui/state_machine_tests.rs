//! State machine tests for GUI screen transitions.
//!
//! Tests that verify the correct GUI is rendered for each screen state,
//! and that state transitions work correctly.

use crate::screen::Screen;
use crate::testing::{GuiTestHarness, assert_snapshot};

/// Standard screen size for state machine tests (1080p).
const TEST_SCREEN_SIZE: (u32, u32) = (1920, 1080);

/// Test that MainMenu screen renders nothing (empty background).
#[test]
fn test_main_menu_renders_empty() {
    let Some(mut harness) = GuiTestHarness::new() else {
        // Skip if no GPU or game path
        return;
    };

    assert_eq!(harness.current_screen(), Screen::MainMenu);

    let image = harness.render_to_image(TEST_SCREEN_SIZE);
    assert_snapshot(&image, "state_machine_main_menu");
}

/// Test that SinglePlayer screen renders country selection panels only.
#[test]
fn test_single_player_renders_country_selection() {
    let Some(mut harness) = GuiTestHarness::new() else {
        return;
    };

    harness.transition_to(Screen::SinglePlayer);
    assert_eq!(harness.current_screen(), Screen::SinglePlayer);

    let image = harness.render_to_image(TEST_SCREEN_SIZE);
    assert_snapshot(&image, "state_machine_single_player");
}

/// Test that Playing screen renders gameplay UI (topbar + speed controls).
#[test]
fn test_playing_renders_gameplay_ui() {
    let Some(mut harness) = GuiTestHarness::new() else {
        return;
    };

    harness.transition_to(Screen::Playing);
    assert_eq!(harness.current_screen(), Screen::Playing);

    let image = harness.render_to_image(TEST_SCREEN_SIZE);
    assert_snapshot(&image, "state_machine_playing");
}

/// Test that Multiplayer screen renders nothing (like MainMenu).
#[test]
fn test_multiplayer_renders_empty() {
    let Some(mut harness) = GuiTestHarness::new() else {
        return;
    };

    harness.transition_to(Screen::Multiplayer);
    assert_eq!(harness.current_screen(), Screen::Multiplayer);

    let image = harness.render_to_image(TEST_SCREEN_SIZE);
    assert_snapshot(&image, "state_machine_multiplayer");
}

/// Test the full navigation flow: MainMenu -> SinglePlayer -> Playing.
#[test]
fn test_navigation_flow_forward() {
    let Some(mut harness) = GuiTestHarness::new() else {
        return;
    };

    // Start at MainMenu
    assert_eq!(harness.current_screen(), Screen::MainMenu);

    // Navigate to SinglePlayer
    harness.transition_to(Screen::SinglePlayer);
    assert_eq!(harness.current_screen(), Screen::SinglePlayer);
    assert!(harness.can_go_back());

    // Navigate to Playing
    harness.transition_to(Screen::Playing);
    assert_eq!(harness.current_screen(), Screen::Playing);
    assert!(harness.can_go_back());
}

/// Test back navigation: Playing -> SinglePlayer -> MainMenu.
#[test]
fn test_navigation_flow_backward() {
    let Some(mut harness) = GuiTestHarness::new() else {
        return;
    };

    // Set up: MainMenu -> SinglePlayer -> Playing
    harness.transition_to(Screen::SinglePlayer);
    harness.transition_to(Screen::Playing);
    assert_eq!(harness.current_screen(), Screen::Playing);

    // Go back to SinglePlayer
    let prev = harness.go_back();
    assert_eq!(prev, Some(Screen::SinglePlayer));
    assert_eq!(harness.current_screen(), Screen::SinglePlayer);

    // Go back to MainMenu
    let prev = harness.go_back();
    assert_eq!(prev, Some(Screen::MainMenu));
    assert_eq!(harness.current_screen(), Screen::MainMenu);

    // No more history
    assert!(!harness.can_go_back());
}

/// Test that each screen state produces different visual output.
///
/// This test verifies that the three main screen states produce
/// visually distinct output by comparing the images pixel-by-pixel.
#[test]
fn test_screen_states_produce_different_output() {
    let Some(mut harness) = GuiTestHarness::new() else {
        return;
    };

    // Render MainMenu
    let main_menu_image = harness.render_to_image(TEST_SCREEN_SIZE);

    // Render SinglePlayer
    harness.transition_to(Screen::SinglePlayer);
    let single_player_image = harness.render_to_image(TEST_SCREEN_SIZE);

    // Render Playing
    harness.transition_to(Screen::Playing);
    let playing_image = harness.render_to_image(TEST_SCREEN_SIZE);

    // Compare images pixel-by-pixel to ensure they're different
    fn count_different_pixels(a: &image::RgbaImage, b: &image::RgbaImage) -> u32 {
        a.pixels()
            .zip(b.pixels())
            .filter(|(pa, pb)| pa != pb)
            .count() as u32
    }

    let main_vs_single = count_different_pixels(&main_menu_image, &single_player_image);
    let main_vs_playing = count_different_pixels(&main_menu_image, &playing_image);
    let single_vs_playing = count_different_pixels(&single_player_image, &playing_image);

    // All three screens should produce different images
    // (MainMenu is empty, SinglePlayer has country selection, Playing has topbar/speed)
    assert!(
        main_vs_single > 100,
        "MainMenu and SinglePlayer should look different, only {} pixels differ",
        main_vs_single
    );

    assert!(
        main_vs_playing > 100,
        "MainMenu and Playing should look different, only {} pixels differ",
        main_vs_playing
    );

    assert!(
        single_vs_playing > 100,
        "SinglePlayer and Playing should look different, only {} pixels differ",
        single_vs_playing
    );
}
