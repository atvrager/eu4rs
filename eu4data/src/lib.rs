use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod countries;
pub mod coverage;
pub mod cultures;
pub mod discovery;
pub mod generated;
pub mod history;
pub mod localisation;
pub mod map;
pub mod path;
pub mod religions;
pub mod technologies;
pub mod timed_modifiers;

/// Represents a trade good in EU4, determining the value and bonuses of a province's production.
#[derive(Debug, Deserialize, Serialize, coverage::SchemaType)]
pub struct Tradegood {
    // Core fields
    /// The RGB color used to represent this trade good on the map.
    #[serde(default)]
    pub color: Option<Vec<f32>>,

    /// Nested modifier block (contains various game modifiers).
    #[serde(default)]
    pub modifier: Option<HashMap<String, f32>>,

    /// Chance/probability conditions for this trade good appearing.
    #[serde(default)]
    pub chance: Option<HashMap<String, serde_json::Value>>,

    /// Factor for trade good spawn probability.
    #[serde(default)]
    pub factor: Option<f32>,

    // Global modifiers (country-wide effects)
    #[serde(default)]
    pub adm_tech_cost_modifier: Option<f32>,
    #[serde(default)]
    pub advisor_cost: Option<f32>,
    #[serde(default)]
    pub cavalry_cost: Option<f32>,
    #[serde(default)]
    pub development_cost: Option<f32>,
    #[serde(default)]
    pub dip_tech_cost_modifier: Option<f32>,
    #[serde(default)]
    pub diplomatic_reputation: Option<f32>,
    #[serde(default)]
    pub garrison_growth: Option<f32>,
    #[serde(default)]
    pub global_colonial_growth: Option<f32>,
    #[serde(default)]
    pub global_institution_spread: Option<f32>,
    #[serde(default)]
    pub global_regiment_cost: Option<f32>,
    #[serde(default)]
    pub global_regiment_recruit_speed: Option<f32>,
    #[serde(default)]
    pub global_sailors_modifier: Option<f32>,
    #[serde(default)]
    pub global_ship_cost: Option<f32>,
    #[serde(default)]
    pub global_spy_defence: Option<f32>,
    #[serde(default)]
    pub global_tariffs: Option<f32>,
    #[serde(default)]
    pub global_trade_goods_size_modifier: Option<f32>,
    #[serde(default)]
    pub global_unrest: Option<f32>,
    #[serde(default)]
    pub heir_chance: Option<f32>,
    #[serde(default)]
    pub inflation_reduction: Option<f32>,
    #[serde(default)]
    pub land_forcelimit: Option<f32>,
    #[serde(default)]
    pub land_forcelimit_modifier: Option<f32>,
    #[serde(default)]
    pub land_maintenance_modifier: Option<f32>,
    #[serde(default)]
    pub manpower_recovery_speed: Option<f32>,
    #[serde(default)]
    pub merc_maintenance_modifier: Option<f32>,
    #[serde(default)]
    pub naval_forcelimit: Option<f32>,
    #[serde(default)]
    pub naval_forcelimit_modifier: Option<f32>,
    #[serde(default)]
    pub num_accepted_cultures: Option<f32>,
    #[serde(default)]
    pub prestige: Option<f32>,
    #[serde(default)]
    pub regiment_recruit_speed: Option<f32>,
    #[serde(default)]
    pub spy_offence: Option<f32>,
    #[serde(default)]
    pub supply_limit_modifier: Option<f32>,
    #[serde(default)]
    pub tolerance_own: Option<f32>,
    #[serde(default)]
    pub trade_efficiency: Option<f32>,
    #[serde(default)]
    pub trade_value_modifier: Option<f32>,
    #[serde(default)]
    pub war_exhaustion_cost: Option<f32>,

    // Government-specific modifiers
    #[serde(default)]
    pub devotion: Option<f32>,
    #[serde(default)]
    pub horde_unity: Option<f32>,
    #[serde(default)]
    pub legitimacy: Option<f32>,
    #[serde(default)]
    pub meritocracy: Option<f32>,
    #[serde(default)]
    pub republican_tradition: Option<f32>,

    // Local/province modifiers
    #[serde(default)]
    pub local_autonomy: Option<f32>,
    #[serde(default)]
    pub local_build_cost: Option<f32>,
    #[serde(default)]
    pub local_build_time: Option<f32>,
    #[serde(default)]
    pub local_defensiveness: Option<f32>,
    #[serde(default)]
    pub local_development_cost: Option<f32>,
    #[serde(default)]
    pub local_friendly_movement_speed: Option<f32>,
    #[serde(default)]
    pub local_institution_spread: Option<f32>,
    #[serde(default)]
    pub local_manpower_modifier: Option<f32>,
    #[serde(default)]
    pub local_missionary_strength: Option<f32>,
    #[serde(default)]
    pub local_monthly_devastation: Option<f32>,
    #[serde(default)]
    pub local_production_efficiency: Option<f32>,
    #[serde(default)]
    pub local_sailors_modifier: Option<f32>,
    #[serde(default)]
    pub local_state_maintenance_modifier: Option<f32>,
    #[serde(default)]
    pub local_tax_modifier: Option<f32>,
    #[serde(default)]
    pub local_unrest: Option<f32>,
    #[serde(default)]
    pub province_trade_power_modifier: Option<f32>,
    #[serde(default)]
    pub province_trade_power_value: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use eu4txt::{DefaultEU4Txt, EU4Txt, from_node};
    use std::io::Write;

    #[test]
    fn test_tradegood_mock() {
        let data = r#"
            color = { 0.5 0.5 0.5 }
            modifier = {
                trade_efficiency = 0.1
            }
            chance = {
                factor = 1
            }
        "#;

        // Write to temp file
        let mut file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
        write!(file, "{}", data).expect("Failed to write to temp file");
        let path = file
            .path()
            .to_str()
            .expect("Failed to get path")
            .to_string();

        // Use parser
        // Note: DefaultEU4Txt::open_txt re-opens the file by path.
        // We must keep 'file' alive so tempfile doesn't delete it yet.
        let tokens = DefaultEU4Txt::open_txt(&path).expect("Failed to tokenize");
        let ast = DefaultEU4Txt::parse(tokens).expect("Failed to parse");
        let tg: Tradegood = from_node(&ast).expect("Failed to deserialize");

        // Verify
        let color = tg.color.expect("color should be present");
        assert_eq!(color.len(), 3);
        assert!((color[0] - 0.5).abs() < 0.001);

        let modifier = tg.modifier.expect("modifier should be present");
        assert!(modifier.contains_key("trade_efficiency"));
    }
}
