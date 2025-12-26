//! Parser for EU4 diplomatic history from `history/diplomacy/`.
//!
//! These files define initial diplomatic relationships at game start:
//! vassals, alliances, personal unions, royal marriages, guarantees, etc.

use eu4txt::{DefaultEU4Txt, EU4Txt, EU4TxtAstItem, EU4TxtParseNode};
use rayon::prelude::*;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::sync::Mutex;

/// A raw diplomatic relationship parsed from history files.
#[derive(Debug, Clone)]
pub struct RawDiplomacy {
    /// Type of relationship (vassal, alliance, union, royal_marriage, guarantee, march, dependency)
    pub relation_type: String,
    /// First country (overlord for subjects, either party for bilateral)
    pub first: String,
    /// Second country (subject for subjects, other party for bilateral)
    pub second: String,
    /// When relationship starts
    pub start_date: Option<String>,
    /// When relationship ends (if any)
    pub end_date: Option<String>,
    /// For `dependency`: explicit subject type (appanage, tributary_state, etc.)
    pub subject_type: Option<String>,
}

/// Loads all diplomatic relationships from `history/diplomacy/`.
pub fn load_diplomacy_history(base_path: &Path) -> Result<Vec<RawDiplomacy>, Box<dyn Error>> {
    let diplomacy_dir = base_path.join("history/diplomacy");
    let results = Mutex::new(Vec::new());

    if !diplomacy_dir.exists() {
        log::warn!("Diplomacy history directory not found: {:?}", diplomacy_dir);
        return Ok(Vec::new());
    }

    let entries: Vec<_> = std::fs::read_dir(diplomacy_dir)?
        .filter_map(|e| e.ok())
        .collect();

    entries.par_iter().for_each(|entry| {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "txt")
            && let Err(e) = load_file(&path, &results)
        {
            log::warn!("Failed to parse diplomacy from {:?}: {}", path, e);
        }
    });

    Ok(results.into_inner().unwrap())
}

fn load_file(
    path: &Path,
    results: &Mutex<Vec<RawDiplomacy>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let tokens = DefaultEU4Txt::open_txt(path.to_str().unwrap()).map_err(|e| format!("{}", e))?;
    if tokens.is_empty() {
        return Ok(());
    }

    let ast = DefaultEU4Txt::parse(tokens).map_err(|e| format!("{}", e))?;

    // Structure: relation_type = { first = TAG second = TAG ... }
    if let EU4TxtAstItem::AssignmentList = ast.entry {
        for node in &ast.children {
            if let EU4TxtAstItem::Assignment = node.entry {
                let name_node = node.children.first().ok_or("Missing name node")?;
                let body_node = node.children.get(1);

                if let EU4TxtAstItem::Identifier(relation_type) = &name_node.entry {
                    // Skip empty blocks
                    let body = match body_node {
                        Some(b) if !b.children.is_empty() => b,
                        _ => continue,
                    };

                    if let Some(diplomacy) = parse_diplomacy(relation_type, body) {
                        let mut lock = results.lock().unwrap();
                        lock.push(diplomacy);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Parse a diplomacy entry from an AST node.
fn parse_diplomacy(relation_type: &str, node: &EU4TxtParseNode) -> Option<RawDiplomacy> {
    let mut first = None;
    let mut second = None;
    let mut start_date = None;
    let mut end_date = None;
    let mut subject_type = None;

    for child in &node.children {
        if let EU4TxtAstItem::Assignment = child.entry
            && child.children.len() >= 2
        {
            let key_node = &child.children[0];
            let value_node = &child.children[1];

            if let EU4TxtAstItem::Identifier(key) = &key_node.entry {
                match key.as_str() {
                    "first" => first = get_string(value_node),
                    "second" => second = get_string(value_node),
                    "start_date" => start_date = get_date(value_node),
                    "end_date" => end_date = get_date(value_node),
                    "subject_type" => subject_type = get_string(value_node),
                    _ => {}
                }
            }
        }
    }

    // Both first and second are required
    let first = first?;
    let second = second?;

    Some(RawDiplomacy {
        relation_type: relation_type.to_string(),
        first,
        second,
        start_date,
        end_date,
        subject_type,
    })
}

fn get_string(node: &EU4TxtParseNode) -> Option<String> {
    match &node.entry {
        EU4TxtAstItem::Identifier(s) | EU4TxtAstItem::StringValue(s) => Some(s.clone()),
        _ => None,
    }
}

fn get_date(node: &EU4TxtParseNode) -> Option<String> {
    match &node.entry {
        EU4TxtAstItem::Identifier(s) | EU4TxtAstItem::StringValue(s) => Some(s.clone()),
        // Dates may also be parsed as nested structures, so try to extract the string
        _ => None,
    }
}

/// Filter diplomacy entries to get only those active at a given date.
///
/// Returns entries where:
/// - `start_date` is None or <= target_date
/// - `end_date` is None or > target_date
pub fn filter_active_at_date<'a>(
    entries: &'a [RawDiplomacy],
    target_date: &str,
) -> Vec<&'a RawDiplomacy> {
    entries
        .iter()
        .filter(|e| {
            // Check start_date
            let started = e
                .start_date
                .as_ref()
                .map(|d| d.as_str() <= target_date)
                .unwrap_or(true);

            // Check end_date (exclusive - relationship ends ON end_date)
            let not_ended = e
                .end_date
                .as_ref()
                .map(|d| d.as_str() > target_date)
                .unwrap_or(true);

            started && not_ended
        })
        .collect()
}

/// Categorize diplomacy entries by relationship type.
pub fn categorize_diplomacy(entries: &[RawDiplomacy]) -> HashMap<&str, Vec<&RawDiplomacy>> {
    let mut result: HashMap<&str, Vec<&RawDiplomacy>> = HashMap::new();
    for entry in entries {
        result
            .entry(entry.relation_type.as_str())
            .or_default()
            .push(entry);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::detect_game_path;

    #[test]
    fn test_load_diplomacy() {
        let Some(game_path) = detect_game_path() else {
            eprintln!("Skipping test: EU4 not found");
            return;
        };

        let diplomacy = load_diplomacy_history(&game_path).expect("Failed to load diplomacy");

        // Should have loaded many entries
        assert!(!diplomacy.is_empty(), "Should load diplomacy entries");
        println!("Loaded {} diplomacy entries", diplomacy.len());

        // Count by type
        let by_type = categorize_diplomacy(&diplomacy);
        for (rel_type, entries) in by_type.iter() {
            println!("  {}: {} entries", rel_type, entries.len());
        }

        // Check for known vassals at game start
        let active = filter_active_at_date(&diplomacy, "1444.11.11");
        println!("Active at 1444.11.11: {} entries", active.len());

        // France should have appanage vassals (Orleans, Armagnac, etc.)
        let french_subjects: Vec<_> = active
            .iter()
            .filter(|e| {
                e.first == "FRA" && (e.relation_type == "vassal" || e.relation_type == "dependency")
            })
            .collect();
        assert!(
            !french_subjects.is_empty(),
            "France should have subjects at game start"
        );
        println!("French subjects at 1444:");
        for subj in &french_subjects {
            println!(
                "  {} -> {} (type: {:?})",
                subj.first, subj.second, subj.subject_type
            );
        }

        // Check for alliances
        let alliances: Vec<_> = active
            .iter()
            .filter(|e| e.relation_type == "alliance")
            .collect();
        println!("Alliances at 1444: {}", alliances.len());
    }

    #[test]
    fn test_filter_active_at_date() {
        let entries = vec![
            RawDiplomacy {
                relation_type: "vassal".to_string(),
                first: "FRA".to_string(),
                second: "ORL".to_string(),
                start_date: Some("1444.1.1".to_string()),
                end_date: Some("1500.1.1".to_string()),
                subject_type: None,
            },
            RawDiplomacy {
                relation_type: "vassal".to_string(),
                first: "FRA".to_string(),
                second: "BOU".to_string(),
                start_date: Some("1444.1.1".to_string()),
                end_date: None, // Never ends
                subject_type: None,
            },
            RawDiplomacy {
                relation_type: "vassal".to_string(),
                first: "ENG".to_string(),
                second: "NRM".to_string(),
                start_date: Some("1600.1.1".to_string()), // Starts later
                end_date: None,
                subject_type: None,
            },
        ];

        // At game start
        let active = filter_active_at_date(&entries, "1444.11.11");
        assert_eq!(active.len(), 2, "Two relationships active at game start");
        assert!(active.iter().any(|e| e.second == "ORL"));
        assert!(active.iter().any(|e| e.second == "BOU"));

        // After ORL relationship ends
        let active = filter_active_at_date(&entries, "1500.1.1");
        assert_eq!(active.len(), 1, "Only BOU still active");
        assert!(active.iter().any(|e| e.second == "BOU"));

        // Much later
        let active = filter_active_at_date(&entries, "1600.6.1");
        assert_eq!(active.len(), 2, "BOU and NRM active");
    }
}
