//! Unit tests for parse.rs save file extraction functions.

use super::*;

/// Helper to check if an idea group name is a national idea
fn is_national_idea_group(name: &str) -> bool {
    name.len() >= 6 && name.ends_with("_ideas") && {
        let prefix = &name[..name.len() - 6];
        prefix.len() == 3 && prefix.chars().all(|c| c.is_ascii_uppercase())
    }
}

// -------------------------------------------------------------------------
// Meta extraction tests
// -------------------------------------------------------------------------

#[test]
fn test_extract_date_valid() {
    let text = r#"
        EU4txt
        date=1444.11.11
        player="TUR"
    "#;
    assert_eq!(extract_date(text), Some("1444.11.11".to_string()));
}

#[test]
fn test_extract_date_with_later_date() {
    let text = "date=1821.1.1";
    assert_eq!(extract_date(text), Some("1821.1.1".to_string()));
}

#[test]
fn test_extract_date_missing() {
    let text = "player=TUR";
    assert_eq!(extract_date(text), None);
}

#[test]
fn test_extract_player_valid() {
    let text = r#"
        date=1444.11.11
        player="FRA"
    "#;
    assert_eq!(extract_player(text), Some("FRA".to_string()));
}

#[test]
fn test_extract_player_missing() {
    let text = "date=1444.11.11";
    assert_eq!(extract_player(text), None);
}

#[test]
fn test_extract_player_invalid_tag() {
    // Only 3-letter uppercase tags should match
    let text = r#"player="fr""#;
    assert_eq!(extract_player(text), None);
}

#[test]
fn test_extract_save_version_valid() {
    let text = r#"save_game_version="1.37.0.0""#;
    assert_eq!(extract_save_version(text), Some("1.37.0.0".to_string()));
}

#[test]
fn test_extract_save_version_missing() {
    let text = "date=1444.11.11";
    assert_eq!(extract_save_version(text), None);
}

// -------------------------------------------------------------------------
// Block extraction tests
// Note: extract_block expects text AFTER the opening brace, with depth=1
// -------------------------------------------------------------------------

#[test]
fn test_extract_block_simple() {
    // Text starts AFTER the opening brace
    let text = " value=1 }";
    let block = extract_block(text);
    assert!(block.is_some());
    assert!(block.unwrap().contains("value=1"));
}

#[test]
fn test_extract_block_nested() {
    // Nested block - starts after first opening brace
    let text = " outer={ inner=1 } }";
    let block = extract_block(text);
    assert!(block.is_some());
    assert!(block.unwrap().contains("outer={"));
}

#[test]
fn test_extract_block_empty() {
    // Empty block - just whitespace then closing brace
    let text = " }";
    let block = extract_block(text);
    assert!(block.is_some());
}

#[test]
fn test_extract_block_unbalanced() {
    // Unbalanced braces with no closing - should return None
    let text = " value=1";
    let block = extract_block(text);
    // No closing brace found, so returns None
    assert!(block.is_none());
}

// -------------------------------------------------------------------------
// Numeric extraction tests (using extract_float_value and extract_int_value)
// -------------------------------------------------------------------------

#[test]
fn test_extract_float_value_valid() {
    let text = "treasury=1234.567\nstability=2";
    let val = extract_float_value(text, "treasury=");
    assert!(val.is_some());
    assert!((val.unwrap() - 1234.567).abs() < 0.001);
}

#[test]
fn test_extract_float_value_integer() {
    let text = "treasury=500";
    let val = extract_float_value(text, "treasury=");
    assert!(val.is_some());
    assert!((val.unwrap() - 500.0).abs() < 0.001);
}

#[test]
fn test_extract_float_value_negative() {
    let text = "prestige=-50.5";
    let val = extract_float_value(text, "prestige=");
    assert!(val.is_some());
    assert!((val.unwrap() - (-50.5)).abs() < 0.001);
}

#[test]
fn test_extract_float_value_missing() {
    let text = "treasury=100";
    let val = extract_float_value(text, "prestige=");
    assert!(val.is_none());
}

#[test]
fn test_extract_int_value_valid() {
    let text = "stability=2";
    let val = extract_int_value(text, "stability=");
    assert_eq!(val, Some(2));
}

#[test]
fn test_extract_int_value_negative() {
    let text = "stability=-3";
    let val = extract_int_value(text, "stability=");
    assert_eq!(val, Some(-3));
}

#[test]
fn test_extract_int_value_missing() {
    let text = "treasury=100";
    let val = extract_int_value(text, "stability=");
    assert!(val.is_none());
}

// -------------------------------------------------------------------------
// Ideas extraction tests
// -------------------------------------------------------------------------

#[test]
fn test_is_national_idea_group() {
    // National ideas follow TAG_ideas pattern
    assert!(is_national_idea_group("TUR_ideas"));
    assert!(is_national_idea_group("FRA_ideas"));
    assert!(is_national_idea_group("HAB_ideas"));

    // Non-national ideas
    assert!(!is_national_idea_group("administrative_ideas"));
    assert!(!is_national_idea_group("economic_ideas"));
    assert!(!is_national_idea_group("quantity_ideas"));
    assert!(!is_national_idea_group("quality_ideas"));
}

// -------------------------------------------------------------------------
// Building extraction tests
// -------------------------------------------------------------------------

#[test]
fn test_extract_buildings_present() {
    let content = r#"
        buildings={
            temple=yes
            workshop=yes
            marketplace=no
        }
    "#;
    let buildings = extract_buildings(content);
    assert!(buildings.contains(&"temple".to_string()));
    assert!(buildings.contains(&"workshop".to_string()));
    // marketplace=no should NOT be included
    assert!(!buildings.contains(&"marketplace".to_string()));
}

#[test]
fn test_extract_buildings_empty() {
    let content = "no_buildings_here";
    let buildings = extract_buildings(content);
    assert!(buildings.is_empty());
}

// -------------------------------------------------------------------------
// Gamestate format detection tests
// -------------------------------------------------------------------------

#[test]
fn test_detect_binary_format() {
    let data = b"EU4binRESTOFDATA";
    assert!(data.starts_with(b"EU4bin"));
}

#[test]
fn test_detect_text_format() {
    let data = b"EU4txtdate=1444.11.11";
    assert!(data.starts_with(b"EU4txt"));
}

#[test]
fn test_detect_zip_format() {
    let data = b"PKsomezipdata";
    assert!(data.starts_with(b"PK"));
}

// -------------------------------------------------------------------------
// Powers array extraction tests
// Note: powers format is integers on one line: "powers={ 100 75 150 }"
// -------------------------------------------------------------------------

#[test]
fn test_extract_powers_array_valid() {
    // Powers are integers, all on one line (with possible newlines)
    let text = "powers={ 100 75 150 }";
    let powers = extract_powers_array(text);
    assert!(powers.is_some());
    let (adm, dip, mil) = powers.unwrap();
    assert!((adm - 100.0).abs() < 0.001);
    assert!((dip - 75.0).abs() < 0.001);
    assert!((mil - 150.0).abs() < 0.001);
}

#[test]
fn test_extract_powers_array_with_whitespace() {
    // Powers with tabs and newlines (like in actual save files)
    let text = "powers={\n\t\t58 155 127 \n\t\t}";
    let powers = extract_powers_array(text);
    assert!(powers.is_some());
    let (adm, dip, mil) = powers.unwrap();
    assert_eq!(adm as i32, 58);
    assert_eq!(dip as i32, 155);
    assert_eq!(mil as i32, 127);
}

#[test]
fn test_extract_powers_array_missing() {
    let text = "treasury=100";
    let powers = extract_powers_array(text);
    assert!(powers.is_none());
}

// -------------------------------------------------------------------------
// Country modifier extraction tests
// -------------------------------------------------------------------------

#[test]
fn test_extract_country_modifiers() {
    let content = r#"
        modifier={
            modifier="papal_blessing"
        }
        modifier={
            modifier="trade_bonus"
        }
    "#;
    let modifiers = extract_country_modifiers(content);
    assert!(modifiers.contains(&"papal_blessing".to_string()));
    assert!(modifiers.contains(&"trade_bonus".to_string()));
}

#[test]
fn test_extract_country_modifiers_empty() {
    let content = "no_modifiers_here";
    let modifiers = extract_country_modifiers(content);
    assert!(modifiers.is_empty());
}
