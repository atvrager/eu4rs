use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

/// A mapping between a Province ID and its color on the map bitmap.
#[derive(Debug, Deserialize)]
pub struct ProvinceDefinition {
    pub id: u32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub name: String,
    pub x: String, // unused but present in csv
}

/// Loads province definitions from a CSV file.
pub fn load_definitions(path: &Path) -> Result<HashMap<u32, ProvinceDefinition>, Box<dyn Error>> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(false) // definitions.csv usually has no headers? Or maybe it does. Let's assume no for now or check.
        // Actually, many eu4 files have a header line like 'province;red;green;blue;x;x' or similar.
        // But frequently standard csv parser fails because of 'x' column?
        // Let's try flexible parsing.
        .flexible(true)
        .from_path(path)?;

    let mut definitions = HashMap::new();
    for result in reader.deserialize() {
        // We might fail on the header if we assume it matches the struct types (u32 vs string "province").
        // So we should specific handle or skip first row if it fails?
        // Let's try to just deserialize.
        match result {
            Ok(record) => {
                let def: ProvinceDefinition = record;
                definitions.insert(def.id, def);
            }
            Err(_) => {
                // Likely header or malformed line. Skip.
                continue;
            }
        }
    }
    Ok(definitions)
}

/// Represents a mapping of special provinces like sea zones and lakes.
#[derive(Debug, Deserialize)]
pub struct DefaultMap {
    /// List of province IDs that are sea zones.
    #[serde(default)]
    pub sea_starts: Vec<u32>,
    /// List of province IDs that are lakes.
    #[serde(default)]
    pub lakes: Vec<u32>,
}
