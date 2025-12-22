//! Bridge between OCR extraction and AI state types.
//!
//! Converts `ExtractedState` (OCR-derived) to `VisibleWorldState` (AI-expected).

use crate::extraction::ExtractedState;
use eu4sim_core::ai::VisibleWorldState;
use eu4sim_core::bounded::{BoundedInt, new_prestige, new_tradition};
use eu4sim_core::fixed::Fixed;
use eu4sim_core::state::{CountryState, Date};

/// Month name lookup for date parsing.
const MONTHS: &[(&str, u8)] = &[
    ("january", 1),
    ("february", 2),
    ("march", 3),
    ("april", 4),
    ("may", 5),
    ("june", 6),
    ("july", 7),
    ("august", 8),
    ("september", 9),
    ("october", 10),
    ("november", 11),
    ("december", 12),
];

/// Parse date string "11 November 1444" → Date struct.
fn parse_date_string(s: &str) -> Option<Date> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() != 3 {
        return None;
    }

    let day: u8 = parts[0].parse().ok()?;
    let month_name = parts[1].to_lowercase();
    let month = MONTHS
        .iter()
        .find(|(name, _)| *name == month_name)
        .map(|(_, num)| *num)?;
    let year: i32 = parts[2].parse().ok()?;

    Some(Date::new(year, month, day))
}

/// Known country name to tag mapping.
/// Expand as needed for common nations.
fn country_name_to_tag(name: &str) -> String {
    let name_lower = name.to_lowercase();

    // Common nations (add more as needed)
    let mappings = [
        ("austria", "HAB"),
        ("france", "FRA"),
        ("castile", "CAS"),
        ("aragon", "ARA"),
        ("england", "ENG"),
        ("ottomans", "TUR"),
        ("ottoman", "TUR"),
        ("ming", "MNG"),
        ("portugal", "POR"),
        ("poland", "POL"),
        ("venice", "VEN"),
        ("genoa", "GEN"),
        ("milan", "MLO"),
        ("florence", "TUS"),
        ("papal state", "PAP"),
        ("the papal state", "PAP"),
        ("muscovy", "MOS"),
        ("russia", "RUS"),
        ("denmark", "DAN"),
        ("sweden", "SWE"),
        ("norway", "NOR"),
        ("scotland", "SCO"),
        ("brandenburg", "BRA"),
        ("prussia", "PRU"),
        ("bohemia", "BOH"),
        ("hungary", "HUN"),
        ("burgundy", "BUR"),
        ("netherlands", "NED"),
        ("spain", "SPA"),
        ("great britain", "GBR"),
        ("oirat", "OIR"),
        ("mongolia", "KHA"),
        ("timurids", "TIM"),
        ("mamluks", "MAM"),
        ("ethiopia", "ETH"),
        ("vijayanagar", "VIJ"),
        ("delhi", "DLH"),
        ("bengal", "BNG"),
        ("japan", "JAP"),
        ("korea", "KOR"),
    ];

    for (name_pattern, tag) in mappings {
        if name_lower.contains(name_pattern) {
            return tag.to_string();
        }
    }

    // Default: use first 3 uppercase letters as tag
    name.chars()
        .filter(|c| c.is_alphabetic())
        .take(3)
        .collect::<String>()
        .to_uppercase()
}

impl ExtractedState {
    /// Convert OCR extraction to AI-compatible state.
    ///
    /// Fields not extractable from OCR get sensible defaults.
    pub fn to_visible_state(&self) -> VisibleWorldState {
        // Parse date
        let date = self
            .date
            .as_ref()
            .and_then(|s| parse_date_string(s))
            .unwrap_or_default();

        // Observer tag from country name
        let observer = self
            .country
            .as_ref()
            .map(|s| country_name_to_tag(s))
            .unwrap_or_else(|| "UNK".to_string());

        // Build CountryState from extracted values
        let own_country = CountryState {
            treasury: Fixed::from_f32(self.treasury.unwrap_or(0.0)),
            manpower: Fixed::from_f32(self.manpower.unwrap_or(0) as f32),
            stability: BoundedInt::new(self.stability.unwrap_or(0) as i32, -3, 3),
            prestige: {
                let mut p = new_prestige();
                p.set(Fixed::from_f32(self.prestige.unwrap_or(0.0)));
                p
            },
            army_tradition: new_tradition(), // Can't extract from OCR yet
            adm_mana: Fixed::from_f32(self.adm_mana.unwrap_or(0) as f32),
            dip_mana: Fixed::from_f32(self.dip_mana.unwrap_or(0) as f32),
            mil_mana: Fixed::from_f32(self.mil_mana.unwrap_or(0) as f32),
            // 1444 starting tech - can't extract from OCR yet
            adm_tech: 3,
            dip_tech: 3,
            mil_tech: 3,
            embraced_institutions: Default::default(),
            religion: None,
        };

        VisibleWorldState {
            date,
            observer,
            own_country,
            at_war: false,           // TODO: detect from UI (war icon, etc.)
            known_countries: vec![], // TODO: outliner OCR
            enemy_provinces: Default::default(),
            known_country_strength: Default::default(),
            our_war_score: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_date() {
        let date = parse_date_string("11 November 1444").unwrap();
        assert_eq!(date.year, 1444);
        assert_eq!(date.month, 11);
        assert_eq!(date.day, 11);

        let date = parse_date_string("1 January 1500").unwrap();
        assert_eq!(date.year, 1500);
        assert_eq!(date.month, 1);
        assert_eq!(date.day, 1);
    }

    #[test]
    fn test_country_tags() {
        assert_eq!(country_name_to_tag("Austria"), "HAB");
        assert_eq!(country_name_to_tag("Ottomans"), "TUR");
        assert_eq!(country_name_to_tag("The Papal State"), "PAP");
        assert_eq!(country_name_to_tag("Venice"), "VEN");
    }

    #[test]
    fn test_extracted_to_visible() {
        let extracted = ExtractedState {
            date: Some("11 November 1444".to_string()),
            treasury: Some(100.0),
            manpower: Some(29000),
            sailors: Some(5000),
            adm_mana: Some(50),
            dip_mana: Some(50),
            mil_mana: Some(50),
            stability: Some(1),
            corruption: Some(0.0),
            prestige: Some(10.0),
            govt_strength: Some(100.0),
            power_projection: Some(0.0),
            country: Some("Austria".to_string()),
            age: Some("Age of Discovery".to_string()),
        };

        let visible = extracted.to_visible_state();
        assert_eq!(visible.date.year, 1444);
        assert_eq!(visible.observer, "HAB");
        assert_eq!(visible.own_country.stability.get(), 1);
        assert!((visible.own_country.treasury.to_f32() - 100.0).abs() < 0.01);
    }
}
