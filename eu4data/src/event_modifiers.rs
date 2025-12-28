//! Event modifier definitions loader
//!
//! Loads event modifiers from `common/event_modifiers/*.txt` files

use eu4txt::{DefaultEU4Txt, EU4Txt, EU4TxtAstItem, EU4TxtParseNode};
use rayon::prelude::*;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::sync::Mutex;

/// Definition of an event modifier (e.g., "tripitaka_koreana", "tax_reform")
#[derive(Debug, Clone, Default)]
pub struct EventModifierDef {
    pub name: String,

    // Economic modifiers
    pub global_tax_modifier: Option<f32>,
    pub production_efficiency: Option<f32>,
    pub trade_efficiency: Option<f32>,
    pub global_trade_goods_size_modifier: Option<f32>,

    // Other common modifiers (can expand as needed)
    pub land_morale: Option<f32>,
    pub naval_morale: Option<f32>,
    pub discipline: Option<f32>,
}

/// Registry of all event modifier definitions
#[derive(Debug, Clone, Default)]
pub struct EventModifiersRegistry {
    pub modifiers: HashMap<String, EventModifierDef>,
}

impl EventModifiersRegistry {
    /// Load event modifiers from game directory
    pub fn load_from_game(game_path: &Path) -> Result<Self, Box<dyn Error>> {
        let modifiers_dir = game_path.join("common/event_modifiers");
        let results = Mutex::new(HashMap::new());

        if !modifiers_dir.exists() {
            log::warn!("Event modifiers directory not found: {:?}", modifiers_dir);
            return Ok(Self::default());
        }

        let entries: Vec<_> = std::fs::read_dir(modifiers_dir)?
            .filter_map(|e| e.ok())
            .collect();

        entries.par_iter().for_each(|entry| {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "txt")
                && let Err(e) = load_file(&path, &results)
            {
                log::warn!("Failed to parse event modifiers from {:?}: {}", path, e);
            }
        });

        let modifiers = results.into_inner().unwrap();
        log::info!("Loaded {} event modifier definitions", modifiers.len());

        Ok(Self { modifiers })
    }

    /// Get a modifier definition by name
    pub fn get(&self, name: &str) -> Option<&EventModifierDef> {
        self.modifiers.get(name)
    }
}

fn load_file(
    path: &Path,
    results: &Mutex<HashMap<String, EventModifierDef>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let tokens = DefaultEU4Txt::open_txt(path.to_str().unwrap()).map_err(|e| format!("{}", e))?;
    if tokens.is_empty() {
        return Ok(());
    }

    let ast = DefaultEU4Txt::parse(tokens).map_err(|e| format!("{}", e))?;

    // Structure: modifier_name = { field=value field=value ... }
    if let EU4TxtAstItem::AssignmentList = ast.entry {
        for node in &ast.children {
            if let EU4TxtAstItem::Assignment = node.entry {
                let modifier_name = get_key_str(node);
                let mut def = EventModifierDef {
                    name: modifier_name.clone(),
                    ..Default::default()
                };

                // Parse modifier fields
                if let Some(value_node) = node.children.first() {
                    if let EU4TxtAstItem::AssignmentList = value_node.entry {
                        for field_node in &value_node.children {
                            if let EU4TxtAstItem::Assignment = field_node.entry {
                                let field_name = get_key_str(field_node);
                                let field_value = get_f32(field_node);

                                match field_name.as_str() {
                                    "global_tax_modifier" => {
                                        def.global_tax_modifier = field_value;
                                    }
                                    "production_efficiency" => {
                                        def.production_efficiency = field_value;
                                    }
                                    "trade_efficiency" => {
                                        def.trade_efficiency = field_value;
                                    }
                                    "global_trade_goods_size_modifier" => {
                                        def.global_trade_goods_size_modifier = field_value;
                                    }
                                    "land_morale" => {
                                        def.land_morale = field_value;
                                    }
                                    "naval_morale" => {
                                        def.naval_morale = field_value;
                                    }
                                    "discipline" => {
                                        def.discipline = field_value;
                                    }
                                    _ => {
                                        // Ignore other fields for now
                                    }
                                }
                            }
                        }
                    }
                }

                results.lock().unwrap().insert(modifier_name, def);
            }
        }
    }

    Ok(())
}

/// Extract key string from an assignment node
fn get_key_str(node: &EU4TxtParseNode) -> String {
    node.children
        .first()
        .and_then(|n| {
            if let EU4TxtAstItem::Identifier(s) = &n.entry {
                Some(s.clone())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// Extract f32 value from an AST node
pub(crate) fn get_f32(node: &EU4TxtParseNode) -> Option<f32> {
    match &node.entry {
        EU4TxtAstItem::IntValue(n) => Some(*n as f32),
        EU4TxtAstItem::FloatValue(f) => Some(*f),
        EU4TxtAstItem::Identifier(s) => s.parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_event_modifiers() {
        let game_path = crate::path::detect_game_path().expect("Game path not found");
        let registry = EventModifiersRegistry::load_from_game(&game_path).expect("Failed to load");

        assert!(registry.modifiers.len() > 100, "Should load many modifiers");

        // Check a known modifier
        if let Some(tax_reform) = registry.get("tax_reform") {
            assert!(tax_reform.global_tax_modifier.is_some());
            assert!(tax_reform.global_tax_modifier.unwrap() > 0.0);
        }
    }
}
