use std::collections::HashMap;
use std::path::Path;

pub type TerrainMap = HashMap<u32, String>;

/// Parse terrain.txt and extract province ID → terrain name mappings.
///
/// EU4's terrain.txt format:
/// ```
/// categories = {
///     plains = { ... terrain_override = { 123 456 789 } }
///     mountains = { ... terrain_override = { 100 200 } }
/// }
/// ```
pub fn load_terrain_overrides(base_path: &Path) -> Result<TerrainMap, std::io::Error> {
    use eu4txt::{DefaultEU4Txt, EU4Txt, EU4TxtAstItem};

    let terrain_path = base_path.join("map/terrain.txt");

    if !terrain_path.exists() {
        return Ok(HashMap::new());
    }

    let tokens = DefaultEU4Txt::open_txt(terrain_path.to_str().unwrap())?;
    let ast = DefaultEU4Txt::parse(tokens)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let mut terrain_map = HashMap::new();

    // Look for "categories = { ... }" assignment in the root children
    for node in &ast.children {
        if let EU4TxtAstItem::Assignment = &node.entry
            && let Some(lhs) = node.children.first()
            && let EU4TxtAstItem::Identifier(key) = &lhs.entry
            && key == "categories"
            && let Some(rhs) = node.children.get(1)
        {
            // Iterate through each terrain category (e.g., "plains", "mountains")
            for terrain_node in &rhs.children {
                if let EU4TxtAstItem::Assignment = &terrain_node.entry {
                    // Get terrain name from LHS
                    if let Some(terrain_lhs) = terrain_node.children.first() {
                        let terrain_name = match &terrain_lhs.entry {
                            EU4TxtAstItem::Identifier(name) => name.clone(),
                            _ => continue,
                        };

                        // Get terrain definition from RHS
                        if let Some(terrain_rhs) = terrain_node.children.get(1) {
                            // Look for "terrain_override = { ... }" within this terrain
                            for field in &terrain_rhs.children {
                                if let EU4TxtAstItem::Assignment = &field.entry
                                    && let Some(field_lhs) = field.children.first()
                                    && let EU4TxtAstItem::Identifier(field_name) = &field_lhs.entry
                                    && field_name == "terrain_override"
                                    && let Some(override_rhs) = field.children.get(1)
                                {
                                    // Extract province IDs
                                    for id_node in &override_rhs.children {
                                        if let EU4TxtAstItem::IntValue(province_id) = &id_node.entry
                                        {
                                            terrain_map
                                                .insert(*province_id as u32, terrain_name.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(terrain_map)
}
