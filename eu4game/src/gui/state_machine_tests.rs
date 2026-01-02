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

/// Test that listboxes render correctly on the SinglePlayer screen.
///
/// Verifies that both the bookmarks listbox and save games listbox
/// are rendered with their content visible.
#[test]
fn test_listbox_rendering() {
    let Some(mut harness) = GuiTestHarness::new() else {
        return;
    };

    harness.transition_to(Screen::SinglePlayer);
    let image = harness.render_to_image(TEST_SCREEN_SIZE);

    // This golden will capture the listboxes in their initial state
    // (bookmarks loaded, save games loaded, default scroll position)
    assert_snapshot(&image, "listbox_rendering");
}

/// Test that the date widget renders correctly with the default date.
#[test]
fn test_date_widget_default() {
    let Some(mut harness) = GuiTestHarness::new() else {
        return;
    };

    harness.transition_to(Screen::SinglePlayer);
    let image = harness.render_to_image(TEST_SCREEN_SIZE);

    // Default date is 1444.11.11 (11 November 1444)
    // This should render the year "1444" in the year editor
    // and "11 November" in the day/month label
    assert_snapshot(&image, "date_widget_default");
}

/// Test date widget with minimum year edge case.
///
/// Verifies that years at the lower bound of the valid range
/// (derived from bookmarks) render correctly.
#[test]
fn test_date_widget_min_year() {
    let Some(mut harness) = GuiTestHarness::new() else {
        return;
    };

    harness.transition_to(Screen::SinglePlayer);
    harness.set_start_date(eu4data::Eu4Date::from_ymd(1, 1, 1));

    let image = harness.render_to_image(TEST_SCREEN_SIZE);
    assert_snapshot(&image, "date_widget_min_year");
}

/// Test date widget with maximum year edge case.
///
/// Verifies that years at the upper bound (9999) render correctly.
#[test]
fn test_date_widget_max_year() {
    let Some(mut harness) = GuiTestHarness::new() else {
        return;
    };

    harness.transition_to(Screen::SinglePlayer);
    harness.set_start_date(eu4data::Eu4Date::from_ymd(9999, 12, 31));

    let image = harness.render_to_image(TEST_SCREEN_SIZE);
    assert_snapshot(&image, "date_widget_max_year");
}

/// Test date widget with a date outside vanilla EU4 range.
///
/// This verifies mod support (e.g., Extended Timeline) where years
/// can be outside the vanilla 1444-1821 range.
#[test]
fn test_date_widget_extended_timeline() {
    let Some(mut harness) = GuiTestHarness::new() else {
        return;
    };

    harness.transition_to(Screen::SinglePlayer);
    // Extended Timeline allows dates like year 2 (very early) or 9999 (far future)
    // Test with year 58 BC would be -58, but let's use year 2 as it's valid
    harness.set_start_date(eu4data::Eu4Date::from_ymd(2, 1, 1));

    let image = harness.render_to_image(TEST_SCREEN_SIZE);
    assert_snapshot(&image, "date_widget_extended_timeline_early");
}

/// Test date widget with different months to verify month wrapping.
#[test]
fn test_date_widget_various_months() {
    let Some(mut harness) = GuiTestHarness::new() else {
        return;
    };

    harness.transition_to(Screen::SinglePlayer);

    // Test January (month 1)
    harness.set_start_date(eu4data::Eu4Date::from_ymd(1500, 1, 15));
    let jan_image = harness.render_to_image(TEST_SCREEN_SIZE);
    assert_snapshot(&jan_image, "date_widget_january");

    // Test December (month 12)
    harness.set_start_date(eu4data::Eu4Date::from_ymd(1500, 12, 25));
    let dec_image = harness.render_to_image(TEST_SCREEN_SIZE);
    assert_snapshot(&dec_image, "date_widget_december");
}

/// Test that leap year dates render correctly (February 29).
#[test]
fn test_date_widget_leap_year() {
    let Some(mut harness) = GuiTestHarness::new() else {
        return;
    };

    harness.transition_to(Screen::SinglePlayer);
    harness.set_start_date(eu4data::Eu4Date::from_ymd(1600, 2, 29));

    let image = harness.render_to_image(TEST_SCREEN_SIZE);
    assert_snapshot(&image, "date_widget_leap_year");
}
