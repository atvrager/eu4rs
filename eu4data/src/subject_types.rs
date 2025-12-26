//! Parser for EU4 subject type definitions from `common/subject_types/`.
//!
//! Subject types define relationships like vassal, march, personal union, etc.
//! The format uses inheritance via `copy_from` and equivalence via `count`.

use eu4txt::{DefaultEU4Txt, EU4Txt, EU4TxtAstItem, EU4TxtParseNode};
use rayon::prelude::*;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::sync::Mutex;

/// Raw subject type definition parsed from game files.
///
/// This is the intermediate representation before being converted to
/// [`eu4sim_core::subjects::SubjectTypeDef`].
#[derive(Debug, Clone, Default)]
pub struct RawSubjectType {
    /// Name of the subject type (e.g., "vassal", "appanage", "tributary_state")
    pub name: String,

    // === Inheritance ===
    /// Parent type to copy properties from
    pub copy_from: Option<String>,
    /// Type this "counts as" for trigger equivalence
    pub count: Option<String>,

    // === War behavior ===
    /// Subject auto-joins overlord's wars (default true)
    pub joins_overlords_wars: Option<bool>,
    /// Subject can declare independence war
    pub can_fight_independence_war: Option<bool>,
    /// Overlord defends subject against external attacks (optional for tributaries)
    pub overlord_protects_external: Option<bool>,

    // === Diplomacy ===
    /// Uses one of overlord's diplomatic relation slots
    pub takes_diplo_slot: Option<bool>,
    /// Can be diplomatically integrated/annexed
    pub can_be_integrated: Option<bool>,
    /// Shares overlord's ruler (personal unions)
    pub has_overlords_ruler: Option<bool>,
    /// Subject can leave voluntarily (tributaries)
    pub is_voluntary: Option<bool>,

    // === Liberty desire ===
    /// Base liberty desire modifier
    pub base_liberty_desire: Option<f32>,
    /// Liberty desire per development ratio
    pub liberty_desire_development_ratio: Option<f32>,

    // === Income/contributions ===
    /// Fraction of income paid to overlord (1.0 for vassals)
    pub pays_overlord: Option<f32>,
    /// Fraction of subject's forcelimit added to overlord's
    pub forcelimit_to_overlord: Option<f32>,
    /// Fraction of subject's manpower contributed
    pub manpower_to_overlord: Option<f32>,

    // === Other properties ===
    /// Maximum government rank (0 = no limit)
    pub max_government_rank: Option<i32>,
    /// Trust at relationship start
    pub trust_on_start: Option<i32>,
    /// Is this a march type
    pub is_march: Option<bool>,
    /// Is this a colony subtype
    pub is_colony_subtype: Option<bool>,
}

/// Loads all subject type definitions from `common/subject_types/`.
pub fn load_subject_types(
    base_path: &Path,
) -> Result<HashMap<String, RawSubjectType>, Box<dyn Error>> {
    let types_dir = base_path.join("common/subject_types");
    let results = Mutex::new(HashMap::new());

    if !types_dir.exists() {
        log::warn!("Subject types directory not found: {:?}", types_dir);
        return Ok(HashMap::new());
    }

    let entries: Vec<_> = std::fs::read_dir(types_dir)?
        .filter_map(|e| e.ok())
        .collect();

    entries.par_iter().for_each(|entry| {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "txt")
            && let Err(e) = load_file(&path, &results)
        {
            log::warn!("Failed to parse subject types from {:?}: {}", path, e);
        }
    });

    Ok(results.into_inner().unwrap())
}

fn load_file(
    path: &Path,
    results: &Mutex<HashMap<String, RawSubjectType>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let tokens = DefaultEU4Txt::open_txt(path.to_str().unwrap()).map_err(|e| format!("{}", e))?;
    if tokens.is_empty() {
        return Ok(());
    }

    let ast = DefaultEU4Txt::parse(tokens).map_err(|e| format!("{}", e))?;

    // Structure: subject_type_name = { properties... }
    if let EU4TxtAstItem::AssignmentList = ast.entry {
        for node in &ast.children {
            if let EU4TxtAstItem::Assignment = node.entry {
                let name_node = node.children.first().ok_or("Missing name node")?;
                let body_node = node.children.get(1);

                if let EU4TxtAstItem::Identifier(name) = &name_node.entry {
                    // Skip forward declarations (empty blocks like "vassal = {}")
                    // and the "default" template
                    if name == "default" {
                        continue;
                    }

                    // Check if body is empty (forward declaration)
                    let is_empty = body_node.map(|b| b.children.is_empty()).unwrap_or(true);

                    if is_empty {
                        continue;
                    }

                    let body = body_node.ok_or("Missing body node")?;
                    let subject_type = parse_subject_type(name, body);

                    let mut lock = results.lock().unwrap();
                    lock.insert(name.clone(), subject_type);
                }
            }
        }
    }

    Ok(())
}

/// Parse a subject type definition from an AST node.
fn parse_subject_type(name: &str, node: &EU4TxtParseNode) -> RawSubjectType {
    let mut st = RawSubjectType {
        name: name.to_string(),
        ..Default::default()
    };

    for child in &node.children {
        if let EU4TxtAstItem::Assignment = child.entry
            && child.children.len() >= 2
        {
            let key_node = &child.children[0];
            let value_node = &child.children[1];

            if let EU4TxtAstItem::Identifier(key) = &key_node.entry {
                match key.as_str() {
                    // Inheritance
                    "copy_from" => st.copy_from = get_string(value_node),
                    "count" => st.count = get_string(value_node),

                    // War behavior
                    "joins_overlords_wars" => st.joins_overlords_wars = get_bool(value_node),
                    "can_fight_independence_war" => {
                        st.can_fight_independence_war = get_bool(value_node)
                    }
                    "overlord_protects_external" => {
                        st.overlord_protects_external = get_bool(value_node)
                    }

                    // Diplomacy
                    "takes_diplo_slot" => st.takes_diplo_slot = get_bool(value_node),
                    "can_be_integrated" => st.can_be_integrated = get_bool(value_node),
                    "has_overlords_ruler" => st.has_overlords_ruler = get_bool(value_node),
                    "is_voluntary" => st.is_voluntary = get_bool(value_node),

                    // Liberty desire
                    "base_liberty_desire" => st.base_liberty_desire = get_float(value_node),
                    "liberty_desire_development_ratio" => {
                        st.liberty_desire_development_ratio = get_float(value_node)
                    }

                    // Income/contributions
                    "pays_overlord" => st.pays_overlord = get_float(value_node),
                    "forcelimit_to_overlord" => st.forcelimit_to_overlord = get_float(value_node),
                    "manpower_to_overlord" => st.manpower_to_overlord = get_float(value_node),

                    // Other
                    "max_government_rank" => st.max_government_rank = get_int(value_node),
                    "trust_on_start" => st.trust_on_start = get_int(value_node),
                    "is_march" => st.is_march = get_bool(value_node),
                    "is_colony_subtype" => st.is_colony_subtype = get_bool(value_node),

                    _ => {} // Ignore unknown fields
                }
            }
        }
    }

    st
}

fn get_string(node: &EU4TxtParseNode) -> Option<String> {
    match &node.entry {
        EU4TxtAstItem::Identifier(s) | EU4TxtAstItem::StringValue(s) => Some(s.clone()),
        _ => None,
    }
}

fn get_bool(node: &EU4TxtParseNode) -> Option<bool> {
    match &node.entry {
        EU4TxtAstItem::Identifier(s) => match s.as_str() {
            "yes" => Some(true),
            "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn get_float(node: &EU4TxtParseNode) -> Option<f32> {
    match &node.entry {
        EU4TxtAstItem::FloatValue(n) => Some(*n),
        EU4TxtAstItem::IntValue(n) => Some(*n as f32),
        EU4TxtAstItem::Identifier(s) => s.parse().ok(),
        _ => None,
    }
}

fn get_int(node: &EU4TxtParseNode) -> Option<i32> {
    match &node.entry {
        EU4TxtAstItem::IntValue(n) => Some(*n),
        EU4TxtAstItem::FloatValue(n) => Some(*n as i32),
        EU4TxtAstItem::Identifier(s) => s.parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::detect_game_path;

    #[test]
    fn test_load_subject_types() {
        let Some(game_path) = detect_game_path() else {
            eprintln!("Skipping test: EU4 not found");
            return;
        };

        let types = load_subject_types(&game_path).expect("Failed to load subject types");

        // Should have loaded some types
        assert!(!types.is_empty(), "Should load subject types");
        println!("Loaded {} subject types", types.len());

        // Check vassal exists and has expected properties
        // Note: joins_overlords_wars is inherited from "default", not set on vassal directly
        let vassal = types.get("vassal").expect("vassal should exist");
        assert_eq!(vassal.name, "vassal");
        assert_eq!(
            vassal.copy_from.as_deref(),
            Some("default"),
            "vassal copies from default"
        );
        assert_eq!(
            vassal.pays_overlord.unwrap_or(0.0),
            1.0,
            "vassals pay full tribute"
        );
        assert_eq!(
            vassal.liberty_desire_development_ratio.unwrap_or(0.0),
            0.25,
            "vassal liberty desire ratio"
        );
        assert_eq!(
            vassal.max_government_rank.unwrap_or(0),
            1,
            "vassals capped at duchy rank"
        );

        // Check march exists
        let march = types.get("march").expect("march should exist");
        assert_eq!(
            march.copy_from.as_deref(),
            Some("vassal"),
            "march copies from vassal"
        );

        // Check tributary has key properties set
        let tributary = types
            .get("tributary_state")
            .expect("tributary should exist");
        assert!(
            !tributary.joins_overlords_wars.unwrap_or(true),
            "tributaries don't join wars"
        );
        assert!(
            tributary.is_voluntary.unwrap_or(false),
            "tributaries are voluntary"
        );

        // Check appanage counts as vassal
        let appanage = types.get("appanage").expect("appanage should exist");
        assert_eq!(
            appanage.count.as_deref(),
            Some("vassal"),
            "appanage counts as vassal"
        );

        // Print some discovered types for visibility
        for (name, st) in types.iter().take(5) {
            println!(
                "  {} (copy_from={:?}, count={:?}, joins_wars={:?})",
                name, st.copy_from, st.count, st.joins_overlords_wars
            );
        }
    }
}
