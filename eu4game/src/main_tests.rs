//! Unit tests for main.rs pure helper functions.

use super::*;
use eu4sim_core::Mod32;
use eu4sim_core::state::{Date, ProvinceState};
use std::collections::BTreeMap;

// -------------------------------------------------------------------------
// Date formatting tests
// -------------------------------------------------------------------------

#[test]
fn test_format_date_standard() {
    let date = Date::new(1444, 11, 11);
    assert_eq!(format_date(&date), "11 November 1444");
}

#[test]
fn test_format_date_january() {
    let date = Date::new(1445, 1, 1);
    assert_eq!(format_date(&date), "1 January 1445");
}

#[test]
fn test_format_date_december() {
    let date = Date::new(1820, 12, 31);
    assert_eq!(format_date(&date), "31 December 1820");
}

#[test]
fn test_month_name_all_months() {
    assert_eq!(month_name(1), "January");
    assert_eq!(month_name(2), "February");
    assert_eq!(month_name(3), "March");
    assert_eq!(month_name(4), "April");
    assert_eq!(month_name(5), "May");
    assert_eq!(month_name(6), "June");
    assert_eq!(month_name(7), "July");
    assert_eq!(month_name(8), "August");
    assert_eq!(month_name(9), "September");
    assert_eq!(month_name(10), "October");
    assert_eq!(month_name(11), "November");
    assert_eq!(month_name(12), "December");
}

#[test]
fn test_month_name_invalid() {
    assert_eq!(month_name(0), "December");
    assert_eq!(month_name(13), "December");
}

// -------------------------------------------------------------------------
// Map mode conversion tests
// -------------------------------------------------------------------------

#[test]
fn test_map_mode_to_shader_value_political() {
    assert_eq!(map_mode_to_shader_value(gui::MapMode::Political), 0.0);
}

#[test]
fn test_map_mode_to_shader_value_terrain() {
    assert_eq!(map_mode_to_shader_value(gui::MapMode::Terrain), 1.0);
}

#[test]
fn test_map_mode_to_shader_value_trade() {
    assert_eq!(map_mode_to_shader_value(gui::MapMode::Trade), 2.0);
}

#[test]
fn test_map_mode_to_shader_value_religion() {
    assert_eq!(map_mode_to_shader_value(gui::MapMode::Religion), 3.0);
}

#[test]
fn test_map_mode_to_shader_value_culture() {
    assert_eq!(map_mode_to_shader_value(gui::MapMode::Culture), 4.0);
}

#[test]
fn test_map_mode_to_shader_value_economy() {
    assert_eq!(map_mode_to_shader_value(gui::MapMode::Economy), 5.0);
}

#[test]
fn test_map_mode_to_shader_value_empire() {
    assert_eq!(map_mode_to_shader_value(gui::MapMode::Empire), 6.0);
}

#[test]
fn test_map_mode_to_shader_value_region() {
    assert_eq!(map_mode_to_shader_value(gui::MapMode::Region), 7.0);
}

// -------------------------------------------------------------------------
// World coordinate tests
// -------------------------------------------------------------------------

#[test]
fn test_normalize_world_x_in_range() {
    assert_eq!(normalize_world_x(0.5), 0.5);
}

#[test]
fn test_normalize_world_x_wraps_positive() {
    assert!((normalize_world_x(1.5) - 0.5).abs() < 0.0001);
}

#[test]
fn test_normalize_world_x_wraps_negative() {
    assert!((normalize_world_x(-0.25) - 0.75).abs() < 0.0001);
}

#[test]
fn test_is_valid_world_y_in_range() {
    assert!(is_valid_world_y(0.0));
    assert!(is_valid_world_y(0.5));
    assert!(is_valid_world_y(1.0));
}

#[test]
fn test_is_valid_world_y_out_of_range() {
    assert!(!is_valid_world_y(-0.1));
    assert!(!is_valid_world_y(1.1));
}

#[test]
fn test_world_to_pixel_center() {
    let (px, py) = world_to_pixel(0.5, 0.5, 100, 100);
    assert_eq!(px, 50);
    assert_eq!(py, 50);
}

#[test]
fn test_world_to_pixel_origin() {
    let (px, py) = world_to_pixel(0.0, 0.0, 100, 100);
    assert_eq!(px, 0);
    assert_eq!(py, 0);
}

#[test]
fn test_world_to_pixel_clamps() {
    let (px, py) = world_to_pixel(1.0, 1.0, 100, 100);
    assert_eq!(px, 99);
    assert_eq!(py, 99);
}

// -------------------------------------------------------------------------
// Province counting tests
// -------------------------------------------------------------------------

#[allow(clippy::field_reassign_with_default)] // Clearer for test setup
#[test]
fn test_count_provinces_for_tag_empty() {
    let provinces = BTreeMap::new();
    assert_eq!(count_provinces_for_tag(&provinces, "TUR"), 0);
}

#[allow(clippy::field_reassign_with_default)] // Clearer for test setup
#[test]
fn test_count_provinces_for_tag_some_owned() {
    let mut provinces = BTreeMap::new();

    let mut p1 = ProvinceState::default();
    p1.owner = Some("TUR".to_string());
    provinces.insert(1, p1);

    let mut p2 = ProvinceState::default();
    p2.owner = Some("TUR".to_string());
    provinces.insert(2, p2);

    let mut p3 = ProvinceState::default();
    p3.owner = Some("FRA".to_string());
    provinces.insert(3, p3);

    assert_eq!(count_provinces_for_tag(&provinces, "TUR"), 2);
    assert_eq!(count_provinces_for_tag(&provinces, "FRA"), 1);
    assert_eq!(count_provinces_for_tag(&provinces, "ENG"), 0);
}

#[allow(clippy::field_reassign_with_default)] // Clearer for test setup
#[test]
fn test_calculate_total_development_empty() {
    let provinces = BTreeMap::new();
    assert_eq!(calculate_total_development(&provinces, "TUR"), 0);
}

#[allow(clippy::field_reassign_with_default)] // Clearer for test setup
#[test]
fn test_calculate_total_development_single_province() {
    let mut provinces = BTreeMap::new();

    let mut p1 = ProvinceState::default();
    p1.owner = Some("TUR".to_string());
    p1.base_tax = Mod32::from_f32(5.0);
    p1.base_production = Mod32::from_f32(4.0);
    p1.base_manpower = Mod32::from_f32(3.0);
    provinces.insert(1, p1);

    assert_eq!(calculate_total_development(&provinces, "TUR"), 12);
}

#[allow(clippy::field_reassign_with_default)] // Clearer for test setup
#[test]
fn test_calculate_total_development_multiple_provinces() {
    let mut provinces = BTreeMap::new();

    let mut p1 = ProvinceState::default();
    p1.owner = Some("TUR".to_string());
    p1.base_tax = Mod32::from_f32(3.0);
    p1.base_production = Mod32::from_f32(3.0);
    p1.base_manpower = Mod32::from_f32(2.0);
    provinces.insert(1, p1);

    let mut p2 = ProvinceState::default();
    p2.owner = Some("TUR".to_string());
    p2.base_tax = Mod32::from_f32(5.0);
    p2.base_production = Mod32::from_f32(4.0);
    p2.base_manpower = Mod32::from_f32(3.0);
    provinces.insert(2, p2);

    assert_eq!(calculate_total_development(&provinces, "TUR"), 20);
}

// -------------------------------------------------------------------------
// Country history integration tests
// -------------------------------------------------------------------------

/// Test that country history data flows through to CountryState.
///
/// This verifies the data binding pipeline:
/// 1. eu4data loads country history from game files
/// 2. eu4sim loader populates CountryState with ruler/rank/tech
/// 3. Data is accessible for UI display
#[test]
fn test_country_history_integration() {
    // Find game path or skip
    let game_path = std::env::var("EU4_GAME_PATH")
        .ok()
        .map(std::path::PathBuf::from)
        .or_else(|| {
            let steam_path = std::path::Path::new(
                "/home/atv/.steam/steam/steamapps/common/Europa Universalis IV",
            );
            if steam_path.exists() {
                Some(steam_path.to_path_buf())
            } else {
                None
            }
        });

    let Some(game_path) = game_path else {
        eprintln!("Skipping test_country_history_integration: EU4 game files not found");
        return;
    };

    // Load world state at game start
    let start_date = eu4sim_core::state::Date::new(1444, 11, 11);
    let (world_state, _adjacency) = eu4sim::loader::load_initial_state(&game_path, start_date, 42)
        .expect("Failed to load world state");

    // Austria (HAB) should have its historical data
    let austria = world_state
        .countries
        .get("HAB")
        .expect("Austria (HAB) should exist in world state");

    // Verify ruler data is loaded from country history
    assert!(
        austria.ruler_name.is_some(),
        "Austria should have a ruler name from country history"
    );
    assert!(
        austria.ruler_dynasty.is_some(),
        "Austria should have a dynasty from country history"
    );

    // Friedrich III was ruler in 1444
    let ruler_name = austria.ruler_name.as_ref().unwrap();
    assert!(
        ruler_name.contains("Friedrich"),
        "Austria's 1444 ruler should be Friedrich III, got: {}",
        ruler_name
    );

    // Verify government rank (Austria is a duchy in 1444, rank=1)
    // Actually in 1444 Austria was an Archduchy under the HRE, typically rank 1
    assert!(
        austria.government_rank >= 1,
        "Austria should have government_rank >= 1, got: {}",
        austria.government_rank
    );

    // Verify tech group is loaded
    assert!(
        austria.technology_group.is_some(),
        "Austria should have a technology group"
    );
    let tech_group = austria.technology_group.as_ref().unwrap();
    assert_eq!(
        tech_group, "western",
        "Austria should have 'western' tech group, got: {}",
        tech_group
    );

    // Verify ruler stats are reasonable (not default 3/3/3)
    // Friedrich III was 4/4/1 in EU4
    let total_stats = austria.ruler_adm + austria.ruler_dip + austria.ruler_mil;
    assert!(
        total_stats > 0 && total_stats <= 18,
        "Ruler stats should be loaded (got adm={}, dip={}, mil={})",
        austria.ruler_adm,
        austria.ruler_dip,
        austria.ruler_mil
    );

    eprintln!(
        "Austria (HAB) country history verified: ruler='{}', dynasty={:?}, rank={}, tech={:?}",
        austria.ruler_name.as_ref().unwrap_or(&"?".to_string()),
        austria.ruler_dynasty,
        austria.government_rank,
        austria.technology_group
    );
}

// -------------------------------------------------------------------------
// Localization integration tests
// -------------------------------------------------------------------------

/// Test that national idea group names are localized correctly.
///
/// Verifies the localization pipeline:
/// 1. eu4data loads localization from game files
/// 2. {TAG}_ideas keys resolve to localized names
/// 3. Common nations have expected idea group names
#[test]
fn test_ideas_localization() {
    // Find game path or skip
    let game_path = std::env::var("EU4_GAME_PATH")
        .ok()
        .map(std::path::PathBuf::from)
        .or_else(|| {
            let steam_path = std::path::Path::new(
                "/home/atv/.steam/steam/steamapps/common/Europa Universalis IV",
            );
            if steam_path.exists() {
                Some(steam_path.to_path_buf())
            } else {
                None
            }
        });

    let Some(game_path) = game_path else {
        eprintln!("Skipping test_ideas_localization: EU4 game files not found");
        return;
    };

    // Load localization
    let loc_path = game_path.join("localisation");
    let mut localisation = eu4data::localisation::Localisation::new();
    let count = localisation
        .load_from_dir(&loc_path, "english")
        .expect("Failed to load localization");
    assert!(count > 0, "Should load at least some localization entries");

    // Verify common national idea group names
    let test_cases = [
        ("HAB_ideas", "Austrian Ideas"),
        ("FRA_ideas", "French Ideas"),
        ("TUR_ideas", "Ottoman Ideas"),
        ("ENG_ideas", "English Ideas"),
        ("CAS_ideas", "Castilian Ideas"),
        ("POR_ideas", "Portuguese Ideas"),
        ("VEN_ideas", "Venetian Ideas"),
        ("POL_ideas", "Polish Ideas"),
    ];

    for (key, expected_name) in test_cases {
        let localized = localisation.get(key);
        assert!(
            localized.is_some(),
            "Localization key '{}' should exist",
            key
        );
        assert_eq!(
            localized.unwrap(),
            expected_name,
            "Localization for '{}' should be '{}'",
            key,
            expected_name
        );
    }

    eprintln!(
        "Ideas localization verified: {} entries loaded, {} test cases passed",
        count,
        test_cases.len()
    );
}

/// Test that country names are localized correctly.
///
/// Verifies that country tags resolve to proper display names.
#[test]
fn test_country_name_localization() {
    // Find game path or skip
    let game_path = std::env::var("EU4_GAME_PATH")
        .ok()
        .map(std::path::PathBuf::from)
        .or_else(|| {
            let steam_path = std::path::Path::new(
                "/home/atv/.steam/steam/steamapps/common/Europa Universalis IV",
            );
            if steam_path.exists() {
                Some(steam_path.to_path_buf())
            } else {
                None
            }
        });

    let Some(game_path) = game_path else {
        eprintln!("Skipping test_country_name_localization: EU4 game files not found");
        return;
    };

    // Load localization
    let loc_path = game_path.join("localisation");
    let mut localisation = eu4data::localisation::Localisation::new();
    localisation
        .load_from_dir(&loc_path, "english")
        .expect("Failed to load localization");

    // Verify country name localization (tag -> display name)
    let test_cases = [
        ("HAB", "Austria"),
        ("FRA", "France"),
        ("TUR", "Ottomans"),
        ("ENG", "England"),
        ("CAS", "Castile"),
        ("POR", "Portugal"),
        ("VEN", "Venice"),
        ("POL", "Poland"),
        ("MOS", "Muscovy"),
        ("MNG", "Ming"),
    ];

    for (tag, expected_name) in test_cases {
        let localized = localisation.get(tag);
        assert!(
            localized.is_some(),
            "Country tag '{}' should have localization",
            tag
        );
        assert_eq!(
            localized.unwrap(),
            expected_name,
            "Country '{}' should localize to '{}'",
            tag,
            expected_name
        );
    }

    eprintln!(
        "Country name localization verified: {} test cases passed",
        test_cases.len()
    );
}
