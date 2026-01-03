//! Unit tests for main.rs pure helper functions.

use super::*;
use eu4sim_core::Fixed;
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
    p1.base_tax = Fixed::from_f32(5.0);
    p1.base_production = Fixed::from_f32(4.0);
    p1.base_manpower = Fixed::from_f32(3.0);
    provinces.insert(1, p1);

    assert_eq!(calculate_total_development(&provinces, "TUR"), 12);
}

#[allow(clippy::field_reassign_with_default)] // Clearer for test setup
#[test]
fn test_calculate_total_development_multiple_provinces() {
    let mut provinces = BTreeMap::new();

    let mut p1 = ProvinceState::default();
    p1.owner = Some("TUR".to_string());
    p1.base_tax = Fixed::from_f32(3.0);
    p1.base_production = Fixed::from_f32(3.0);
    p1.base_manpower = Fixed::from_f32(2.0);
    provinces.insert(1, p1);

    let mut p2 = ProvinceState::default();
    p2.owner = Some("TUR".to_string());
    p2.base_tax = Fixed::from_f32(5.0);
    p2.base_production = Fixed::from_f32(4.0);
    p2.base_manpower = Fixed::from_f32(3.0);
    provinces.insert(2, p2);

    assert_eq!(calculate_total_development(&provinces, "TUR"), 20);
}
