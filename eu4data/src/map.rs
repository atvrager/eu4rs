use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

/// A mapping between a Province ID and its color on the map bitmap.
#[derive(Debug, Deserialize)]
pub struct ProvinceDefinition {
    /// The unique Province ID.
    pub id: u32,
    /// Red component of the province color (0-255).
    pub r: u8,
    /// Green component of the province color (0-255).
    pub g: u8,
    /// Blue component of the province color (0-255).
    pub b: u8,
    /// The name of the province.
    pub name: String,
    /// Unused field (often 'x' in definition.csv).
    pub x: String,
}

/// Loads province definitions from a CSV file.
pub fn load_definitions(path: &Path) -> Result<HashMap<u32, ProvinceDefinition>, Box<dyn Error>> {
    // Read raw bytes
    let raw_data = std::fs::read(path)?;
    // Decode from Windows-1252 to UTF-8
    let (decoded, _, _) = encoding_rs::WINDOWS_1252.decode(&raw_data);

    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(false)
        .flexible(true)
        .from_reader(decoded.as_bytes()); // csv::Reader takes bytes or a file

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
///
/// Note: Wasteland provinces are NOT defined in default.map.
/// They are defined in terrain.txt as the "wasteland" terrain type.
#[derive(Debug, Deserialize)]
pub struct DefaultMap {
    /// List of province IDs that are sea zones.
    #[serde(default)]
    pub sea_starts: Vec<u32>,
    /// List of province IDs that are lakes.
    #[serde(default)]
    pub lakes: Vec<u32>,
}

/// Loads the default.map file to get sea zones and lake definitions
pub fn load_default_map(game_path: &Path) -> Result<DefaultMap, Box<dyn Error>> {
    use eu4txt::{DefaultEU4Txt, EU4Txt};

    let path = game_path.join("map/default.map");
    let tokens = DefaultEU4Txt::open_txt(path.to_str().ok_or("Invalid path")?)
        .map_err(|e| format!("Failed to read default.map: {}", e))?;
    let ast =
        DefaultEU4Txt::parse(tokens).map_err(|e| format!("Failed to parse default.map: {}", e))?;

    let default_map = eu4txt::from_node::<DefaultMap>(&ast)
        .map_err(|e| format!("Failed to deserialize default.map: {}", e))?;

    Ok(default_map)
}

pub struct ProvinceLookup {
    pub by_id: HashMap<u32, ProvinceDefinition>,
    pub by_color: HashMap<(u8, u8, u8), u32>,
}

impl ProvinceLookup {
    pub fn load(path: &Path) -> Result<Self, Box<dyn Error>> {
        let defs = load_definitions(path)?;
        let mut by_color = HashMap::new();
        for def in defs.values() {
            by_color.insert((def.r, def.g, def.b), def.id);
        }
        Ok(Self {
            by_id: defs,
            by_color,
        })
    }
}

/// Loads the province map bitmap (provinces.bmp).
pub fn load_province_map(game_path: &Path) -> Result<image::RgbaImage, Box<dyn Error>> {
    let map_path = game_path.join("map/provinces.bmp");
    let img = image::open(&map_path)
        .map_err(|e| format!("Failed to open map file at {:?}: {}", map_path, e))?
        .to_rgba8();
    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_load_definitions() {
        let data = "1;10;10;10;Stockholm;x\n2;20;20;20;Paris;x";
        let mut file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
        write!(file, "{}", data).expect("Failed to write to temp file");
        let path = file.path();

        let defs = load_definitions(path).expect("Failed to load definitions");
        assert_eq!(defs.len(), 2);

        let stockholm = defs.get(&1).unwrap();
        assert_eq!(stockholm.name, "Stockholm");
        assert_eq!(stockholm.r, 10);

        let paris = defs.get(&2).unwrap();
        assert_eq!(paris.name, "Paris");
        assert_eq!(paris.b, 20);
    }

    #[test]
    fn test_load_definitions_broken() {
        // Test resilience against empty lines or bad rows
        let data = "1;10;10;10;Stockholm;x\n;;;;;\n3;30;30;30;Berlin;x";
        let mut file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
        write!(file, "{}", data).expect("Failed to write to temp file");
        let path = file.path();

        // depending on CSV parser strictness, this might fail or skip.
        // flexible(true) should handle some, but deserializing empty strings to integer might fail.
        // Our current logic 'continue's on error.
        let defs = load_definitions(path).unwrap();
        // Should have 1 and 3. Line 2 fails deserialize.
        assert!(defs.contains_key(&1));
        assert!(defs.contains_key(&3));
    }
}
