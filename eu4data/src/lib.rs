use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod map;
pub mod history;
pub mod countries;

/// Represents a trade good in EU4, determining the value and bonuses of a province's production.
#[derive(Debug, Deserialize, Serialize)]
pub struct Tradegood {
    /// The RGB color used to represent this trade good on the map.
    #[serde(default)]
    pub color: Vec<f32>,
    /// Modifiers applied to the province or country producing this good (e.g., trade efficiency).
    #[serde(default)]
    pub modifier: HashMap<String, f32>,
    /// Province-scope modifiers (e.g., manpower increase).
    #[serde(default)]
    pub province: Option<HashMap<String, f32>>,
    #[serde(default)] 
    pub chance: Option<HashMap<String, serde_json::Value>>, 
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
        let path = file.path().to_str().expect("Failed to get path").to_string();

        // Use parser
        // Note: DefaultEU4Txt::open_txt re-opens the file by path.
        // We must keep 'file' alive so tempfile doesn't delete it yet.
        let tokens = DefaultEU4Txt::open_txt(&path).expect("Failed to tokenize");
        let ast = DefaultEU4Txt::parse(tokens).expect("Failed to parse");
        let tg: Tradegood = from_node(&ast).expect("Failed to deserialize");

        // Verify
        assert_eq!(tg.color.len(), 3);
        assert!((tg.color[0] - 0.5).abs() < 0.001);
        assert!(tg.modifier.contains_key("trade_efficiency"));
    }
}
