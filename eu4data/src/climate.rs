use std::collections::HashSet;
use std::path::Path;

/// Load impassable (wasteland) provinces from climate.txt.
///
/// EU4's climate.txt contains an `impassable = { ... }` block listing
/// all province IDs that are wastelands (Sahara, Amazon rainforest cores, etc.)
pub fn load_impassable_provinces(base_path: &Path) -> Result<HashSet<u32>, std::io::Error> {
    use eu4txt::{DefaultEU4Txt, EU4Txt, EU4TxtAstItem};

    let climate_path = base_path.join("map/climate.txt");

    if !climate_path.exists() {
        return Ok(HashSet::new());
    }

    let tokens = DefaultEU4Txt::open_txt(climate_path.to_str().unwrap())?;
    let ast = DefaultEU4Txt::parse(tokens)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let mut impassable = HashSet::new();

    // Look for "impassable = { ... }" assignment
    for node in &ast.children {
        if let EU4TxtAstItem::Assignment = &node.entry
            && let Some(lhs) = node.children.first()
            && let EU4TxtAstItem::Identifier(key) = &lhs.entry
            && key == "impassable"
            && let Some(rhs) = node.children.get(1)
        {
            // Extract province IDs from the RHS block
            for id_node in &rhs.children {
                if let EU4TxtAstItem::IntValue(province_id) = &id_node.entry {
                    impassable.insert(*province_id as u32);
                }
            }
        }
    }

    Ok(impassable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_impassable_from_game() {
        // Only run if game path is available
        if let Some(path) = crate::path::detect_game_path() {
            let impassable = load_impassable_provinces(&path).unwrap();
            // EU4 has many wasteland provinces (Sahara, etc.)
            assert!(
                impassable.len() > 50,
                "Expected many impassable provinces, got {}",
                impassable.len()
            );
            // Known Sahara wasteland province
            assert!(
                impassable.contains(&1779),
                "Province 1779 should be impassable"
            );
        }
    }
}
