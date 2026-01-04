use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

pub type TerrainMap = HashMap<u32, String>;

#[derive(Debug, Deserialize)]
pub struct TerrainTxt {
    pub categories: HashMap<String, TerrainCategory>,
    #[serde(default)]
    pub terrain: HashMap<String, GraphicalTerrainDef>,
}

#[derive(Debug, Deserialize)]
pub struct TerrainCategory {
    #[serde(default)]
    pub terrain_override: Vec<u32>,
    #[serde(rename = "type")]
    pub terrain_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GraphicalTerrainDef {
    #[serde(rename = "type")]
    pub terrain_type: String,
    pub color: Vec<u32>,
}

/// Parse terrain.txt and extract province ID → terrain name mappings.
pub fn load_terrain_overrides(base_path: &Path) -> Result<TerrainMap, Box<dyn std::error::Error>> {
    let terrain_txt = load_terrain_txt(base_path)?;
    let mut terrain_map = HashMap::new();

    for (name, category) in terrain_txt.categories {
        for province_id in category.terrain_override {
            terrain_map.insert(province_id, name.clone());
        }
    }

    Ok(terrain_map)
}

/// Loads the graphical terrain definitions from terrain.txt.
/// Maps color indices in terrain.bmp to terrain names.
pub fn load_graphical_terrain(
    base_path: &Path,
) -> Result<HashMap<u8, String>, Box<dyn std::error::Error>> {
    let terrain_txt = load_terrain_txt(base_path)?;
    let mut graphical_map = HashMap::new();

    for (name, def) in terrain_txt.terrain {
        if let Some(&index) = def.color.first() {
            graphical_map.insert(index as u8, name);
        }
    }

    Ok(graphical_map)
}

fn load_terrain_txt(base_path: &Path) -> Result<TerrainTxt, Box<dyn std::error::Error>> {
    use eu4txt::{DefaultEU4Txt, EU4Txt};

    let terrain_path = base_path.join("map/terrain.txt");

    if !terrain_path.exists() {
        return Ok(TerrainTxt {
            categories: HashMap::new(),
            terrain: HashMap::new(),
        });
    }

    let tokens = DefaultEU4Txt::open_txt(terrain_path.to_str().unwrap())?;
    let ast =
        DefaultEU4Txt::parse(tokens).map_err(|e| format!("Failed to parse terrain.txt: {}", e))?;

    let terrain_txt = eu4txt::from_node::<TerrainTxt>(&ast)
        .map_err(|e| format!("Failed to deserialize terrain.txt: {}", e))?;

    Ok(terrain_txt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_terrain_txt() {
        let data = r#"
categories = {
    plains = {
        terrain_override = { 1 2 3 }
    }
}
terrain = {
    grasslands = {
        type = grasslands
        color = { 0 }
    }
}
"#;
        let mut file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
        write!(file, "{}", data).expect("Failed to write to temp file");
        // Since load_terrain_txt expects map/terrain.txt, we need to create the directory
        let dir = tempfile::tempdir().unwrap();
        let map_dir = dir.path().join("map");
        std::fs::create_dir(&map_dir).unwrap();
        std::fs::write(map_dir.join("terrain.txt"), data).unwrap();

        let terrain_txt = load_terrain_txt(dir.path()).expect("Failed to load terrain.txt");
        assert!(terrain_txt.categories.contains_key("plains"));
        assert_eq!(
            terrain_txt.categories["plains"].terrain_override,
            vec![1, 2, 3]
        );
        assert!(terrain_txt.terrain.contains_key("grasslands"));
        assert_eq!(terrain_txt.terrain["grasslands"].color, vec![0]);
    }
}
