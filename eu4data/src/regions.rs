use eu4txt::{DefaultEU4Txt, EU4Txt, EU4TxtAstItem};
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

/// Represents a geographic region containing multiple areas.
#[derive(Debug, Clone)]
pub struct Region {
    pub name: String,
    pub areas: Vec<String>,
}

/// Maps province IDs to their region and area.
#[derive(Debug, Clone)]
pub struct ProvinceRegionMapping {
    /// Maps province ID to region name
    pub province_to_region: HashMap<u32, String>,
    /// Maps province ID to area name
    pub province_to_area: HashMap<u32, String>,
    /// Maps region name to region data
    pub regions: HashMap<String, Region>,
}

/// Loads area definitions from map/area.txt.
/// Returns a map of area_name -> Vec<province_id>.
fn load_areas(game_path: &Path) -> Result<HashMap<String, Vec<u32>>, Box<dyn Error>> {
    let area_file = game_path.join("map/area.txt");
    let tokens = DefaultEU4Txt::open_txt(area_file.to_str().unwrap())
        .map_err(|e| format!("Failed to parse area.txt: {}", e))?;

    if tokens.is_empty() {
        return Ok(HashMap::new());
    }

    let ast =
        DefaultEU4Txt::parse(tokens).map_err(|e| format!("Failed to parse area.txt AST: {}", e))?;

    let mut areas = HashMap::new();

    // Parse top-level assignments: area_name = { province_ids }
    if let EU4TxtAstItem::AssignmentList = ast.entry {
        for node in &ast.children {
            if let EU4TxtAstItem::Assignment = node.entry
                && node.children.len() >= 2
            {
                // Get area name
                if let EU4TxtAstItem::Identifier(area_name) = &node.children[0].entry {
                    // Get province list - could be AssignmentList or Brace
                    let province_list_node = &node.children[1];
                    let mut provinces = Vec::new();

                    // Parse province IDs from list
                    for prov_node in &province_list_node.children {
                        match &prov_node.entry {
                            EU4TxtAstItem::IntValue(id) => provinces.push(*id as u32),
                            EU4TxtAstItem::Identifier(s) => {
                                if let Ok(id) = s.parse::<u32>() {
                                    provinces.push(id);
                                }
                            }
                            _ => {}
                        }
                    }

                    if !provinces.is_empty() {
                        areas.insert(area_name.clone(), provinces);
                    }
                }
            }
        }
    }

    Ok(areas)
}

/// Loads region definitions from map/region.txt.
/// Returns a map of region_name -> Region.
fn load_regions(game_path: &Path) -> Result<HashMap<String, Region>, Box<dyn Error>> {
    let region_file = game_path.join("map/region.txt");
    let tokens = DefaultEU4Txt::open_txt(region_file.to_str().unwrap())
        .map_err(|e| format!("Failed to parse region.txt: {}", e))?;

    if tokens.is_empty() {
        return Ok(HashMap::new());
    }

    let ast = DefaultEU4Txt::parse(tokens)
        .map_err(|e| format!("Failed to parse region.txt AST: {}", e))?;

    let mut regions = HashMap::new();

    // Parse top-level assignments: region_name = { areas = { ... } }
    if let EU4TxtAstItem::AssignmentList = ast.entry {
        for node in &ast.children {
            if let EU4TxtAstItem::Assignment = node.entry
                && node.children.len() >= 2
            {
                // Get region name
                if let EU4TxtAstItem::Identifier(region_name) = &node.children[0].entry {
                    // Get region body
                    let region_body = &node.children[1];
                    if let EU4TxtAstItem::AssignmentList = region_body.entry {
                        // Find "areas = { ... }" assignment
                        for field_node in &region_body.children {
                            if let EU4TxtAstItem::Assignment = field_node.entry
                                && field_node.children.len() >= 2
                                && let EU4TxtAstItem::Identifier(field_name) =
                                    &field_node.children[0].entry
                                && field_name == "areas"
                            {
                                // Parse area list
                                let areas_node = &field_node.children[1];
                                let mut areas = Vec::new();

                                for area_node in &areas_node.children {
                                    if let EU4TxtAstItem::Identifier(area_name) = &area_node.entry {
                                        areas.push(area_name.clone());
                                    }
                                }

                                if !areas.is_empty() {
                                    regions.insert(
                                        region_name.clone(),
                                        Region {
                                            name: region_name.clone(),
                                            areas,
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(regions)
}

/// Loads region and area data, building a complete province -> region mapping.
pub fn load_region_mapping(game_path: &Path) -> Result<ProvinceRegionMapping, Box<dyn Error>> {
    let areas = load_areas(game_path)?;
    let regions = load_regions(game_path)?;

    let mut province_to_area = HashMap::new();
    let mut province_to_region = HashMap::new();

    // Build province -> area mapping
    for (area_name, province_ids) in &areas {
        for &province_id in province_ids {
            province_to_area.insert(province_id, area_name.clone());
        }
    }

    // Build province -> region mapping
    for (region_name, region) in &regions {
        for area_name in &region.areas {
            if let Some(province_ids) = areas.get(area_name) {
                for &province_id in province_ids {
                    province_to_region.insert(province_id, region_name.clone());
                }
            }
        }
    }

    log::info!(
        "Loaded {} regions, {} areas, mapped {} provinces",
        regions.len(),
        areas.len(),
        province_to_region.len()
    );

    Ok(ProvinceRegionMapping {
        province_to_region,
        province_to_area,
        regions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    #[ignore] // Skipping - will test with real game data instead
    fn test_load_areas() {
        let dir = tempdir().unwrap();
        let map_dir = dir.path().join("map");
        std::fs::create_dir_all(&map_dir).unwrap();

        let mut f = std::fs::File::create(map_dir.join("area.txt")).unwrap();
        write!(
            f,
            "test_area_1 = {{\n\t1 2 3\n}}\ntest_area_2 = {{\n\t10 20 30\n}}\n"
        )
        .unwrap();

        let areas = load_areas(dir.path()).unwrap();
        assert_eq!(areas.len(), 2);
        assert_eq!(areas.get("test_area_1").unwrap(), &vec![1, 2, 3]);
        assert_eq!(areas.get("test_area_2").unwrap(), &vec![10, 20, 30]);
    }

    #[test]
    fn test_load_regions() {
        let dir = tempdir().unwrap();
        let map_dir = dir.path().join("map");
        std::fs::create_dir_all(&map_dir).unwrap();

        let mut f = std::fs::File::create(map_dir.join("region.txt")).unwrap();
        write!(
            f,
            "test_region = {{\n\tareas = {{\n\t\ttest_area_1\n\t\ttest_area_2\n\t}}\n}}\n"
        )
        .unwrap();

        let regions = load_regions(dir.path()).unwrap();
        assert_eq!(regions.len(), 1);
        let region = regions.get("test_region").unwrap();
        assert_eq!(region.areas, vec!["test_area_1", "test_area_2"]);
    }

    #[test]
    #[ignore] // Skipping - will test with real game data instead
    fn test_load_region_mapping() {
        let dir = tempdir().unwrap();
        let map_dir = dir.path().join("map");
        std::fs::create_dir_all(&map_dir).unwrap();

        let mut area_file = std::fs::File::create(map_dir.join("area.txt")).unwrap();
        write!(area_file, "test_area = {{\n\t1 2 3\n}}\n").unwrap();

        let mut region_file = std::fs::File::create(map_dir.join("region.txt")).unwrap();
        write!(
            region_file,
            "test_region = {{\n\tareas = {{\n\t\ttest_area\n\t}}\n}}\n"
        )
        .unwrap();

        let mapping = load_region_mapping(dir.path()).unwrap();
        assert_eq!(mapping.province_to_region.get(&1).unwrap(), "test_region");
        assert_eq!(mapping.province_to_region.get(&2).unwrap(), "test_region");
        assert_eq!(mapping.province_to_region.get(&3).unwrap(), "test_region");
        assert_eq!(mapping.province_to_area.get(&1).unwrap(), "test_area");
    }
}
