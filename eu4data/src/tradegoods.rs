use eu4txt::{DefaultEU4Txt, EU4Txt, EU4TxtAstItem, from_node};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::sync::Mutex;

/// Trade good price definition from common/prices/*.txt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradegoodPrice {
    /// Base price in ducats
    pub base_price: Option<f32>,
    /// Whether this tradegood uses gold-like pricing (from mine value)
    pub goldtype: Option<bool>,
}

/// Loads all trade goods prices from `common/prices`.
/// The file structure is flat: `tradegood_name = { base_price = X }`.
pub fn load_tradegoods(
    base_path: &Path,
) -> Result<HashMap<String, TradegoodPrice>, Box<dyn Error>> {
    let prices_dir = base_path.join("common/prices");
    let results = Mutex::new(HashMap::new());

    if !prices_dir.exists() {
        return Ok(HashMap::new());
    }

    let entries: Vec<_> = std::fs::read_dir(prices_dir)?
        .filter_map(|e| e.ok())
        .collect();

    entries.par_iter().for_each(|entry| {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "txt") {
            let _ = load_file(&path, &results);
        }
    });

    Ok(results.into_inner().unwrap())
}

fn load_file(
    path: &Path,
    results: &Mutex<HashMap<String, TradegoodPrice>>,
) -> Result<(), Box<dyn Error>> {
    let tokens = DefaultEU4Txt::open_txt(path.to_str().unwrap()).map_err(|e| e.to_string())?;
    if tokens.is_empty() {
        return Ok(());
    }

    let ast = DefaultEU4Txt::parse(tokens).map_err(|e| e.to_string())?;

    // Flat structure: tradegood = { base_price = X }
    if let EU4TxtAstItem::AssignmentList = ast.entry {
        for node in &ast.children {
            if let EU4TxtAstItem::Assignment = node.entry {
                let name_node = node.children.first().unwrap();
                let body_node = node.children.get(1).unwrap();

                if let EU4TxtAstItem::Identifier(name) = &name_node.entry {
                    let tradegood = match from_node::<TradegoodPrice>(body_node) {
                        Ok(tg) => tg,
                        Err(e) => {
                            log::warn!(
                                "Failed to parse tradegood '{}' in {}: {}",
                                name,
                                path.display(),
                                e
                            );
                            continue;
                        }
                    };

                    let mut lock = results.lock().unwrap();
                    lock.insert(name.clone(), tradegood);
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::detect_game_path;

    #[test]
    fn test_load_tradegoods() {
        let Some(game_path) = detect_game_path() else {
            eprintln!("Skipping test: EU4 not found");
            return;
        };

        let tradegoods = load_tradegoods(&game_path).expect("Failed to load tradegoods");

        // Check that we loaded some trade goods
        assert!(!tradegoods.is_empty(), "Should load trade goods");

        // Check grain exists and has a price
        let grain = tradegoods.get("grain").expect("grain should exist");
        assert!(grain.base_price.is_some(), "grain should have a base_price");
        assert_eq!(
            grain.base_price.unwrap(),
            2.5,
            "grain base_price should be 2.5"
        );

        // Check another common one
        let cloth = tradegoods.get("cloth").expect("cloth should exist");
        assert_eq!(
            cloth.base_price.unwrap(),
            3.0,
            "cloth base_price should be 3.0"
        );

        println!("Loaded {} trade goods", tradegoods.len());
    }
}
