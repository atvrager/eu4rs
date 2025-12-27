//! Parser for EU4 policies from `common/policies/`.
//!
//! Policies are synergies between two fully-unlocked idea groups that
//! grant bonus modifiers.

use eu4txt::{DefaultEU4Txt, EU4Txt, EU4TxtAstItem, EU4TxtParseNode};
use rayon::prelude::*;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::sync::Mutex;

/// Policy category (monarch power type).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawPolicyCategory {
    Adm,
    Dip,
    Mil,
}

/// Raw modifier entry parsed from game files.
#[derive(Debug, Clone)]
pub struct RawModifierEntry {
    pub key: String,
    pub value: f32,
}

/// Raw policy definition parsed from game files.
///
/// This is the intermediate representation before being converted to
/// [`eu4sim_core::systems::PolicyDef`].
#[derive(Debug, Clone)]
pub struct RawPolicy {
    /// Name of the policy (e.g., "the_combination_act")
    pub name: String,

    /// Category (ADM/DIP/MIL) - determines monarch power type
    pub category: RawPolicyCategory,

    /// First required idea group (from `allow` block)
    pub idea_group_1: String,

    /// Second required idea group (from `allow` block)
    pub idea_group_2: String,

    /// Modifiers granted by this policy
    pub modifiers: Vec<RawModifierEntry>,
}

/// Reserved block names that aren't modifiers.
const RESERVED_BLOCKS: &[&str] = &[
    "monarch_power",
    "potential",
    "allow",
    "effect",
    "removed_effect",
    "ai_will_do",
];

/// Loads all policies from `common/policies/`.
pub fn load_policies(base_path: &Path) -> Result<HashMap<String, RawPolicy>, Box<dyn Error>> {
    let policies_dir = base_path.join("common/policies");
    let results = Mutex::new(HashMap::new());

    if !policies_dir.exists() {
        log::warn!("Policies directory not found: {:?}", policies_dir);
        return Ok(HashMap::new());
    }

    let entries: Vec<_> = std::fs::read_dir(policies_dir)?
        .filter_map(|e| e.ok())
        .collect();

    entries.par_iter().for_each(|entry| {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "txt")
            && let Err(e) = load_file(&path, &results)
        {
            log::warn!("Failed to parse policies from {:?}: {}", path, e);
        }
    });

    Ok(results.into_inner().unwrap())
}

fn load_file(
    path: &Path,
    results: &Mutex<HashMap<String, RawPolicy>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let tokens = DefaultEU4Txt::open_txt(path.to_str().unwrap()).map_err(|e| format!("{}", e))?;
    if tokens.is_empty() {
        return Ok(());
    }

    let ast = DefaultEU4Txt::parse(tokens).map_err(|e| format!("{}", e))?;

    // Structure: policy_name = { properties... }
    if let EU4TxtAstItem::AssignmentList = ast.entry {
        for node in &ast.children {
            if let EU4TxtAstItem::Assignment = node.entry {
                let name_node = node.children.first().ok_or("Missing name node")?;
                let body_node = node.children.get(1);

                if let EU4TxtAstItem::Identifier(name) = &name_node.entry {
                    // Skip empty blocks
                    let is_empty = body_node.map(|b| b.children.is_empty()).unwrap_or(true);
                    if is_empty {
                        continue;
                    }

                    let body = body_node.ok_or("Missing body node")?;
                    if let Some(policy) = parse_policy(name, body) {
                        let mut lock = results.lock().unwrap();
                        lock.insert(name.clone(), policy);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Parse a policy definition from an AST node.
fn parse_policy(name: &str, node: &EU4TxtParseNode) -> Option<RawPolicy> {
    let mut category = None;
    let mut idea_groups = Vec::new();
    let mut modifiers = Vec::new();

    for child in &node.children {
        if let EU4TxtAstItem::Assignment = child.entry
            && child.children.len() >= 2
        {
            let key_node = &child.children[0];
            let value_node = &child.children[1];

            if let EU4TxtAstItem::Identifier(key) = &key_node.entry {
                match key.as_str() {
                    "monarch_power" => {
                        category = parse_category(value_node);
                    }
                    "allow" => {
                        // Extract required idea groups from allow block
                        idea_groups = extract_idea_groups(value_node);
                    }
                    _ if !RESERVED_BLOCKS.contains(&key.as_str()) => {
                        // Not a reserved block - this is a modifier
                        if let Some(value) = get_f32(value_node) {
                            modifiers.push(RawModifierEntry {
                                key: key.clone(),
                                value,
                            });
                        }
                    }
                    _ => {
                        // Skip other reserved blocks
                    }
                }
            }
        }
    }

    // Validate we have all required fields
    if let Some(cat) = category
        && idea_groups.len() == 2
    {
        Some(RawPolicy {
            name: name.to_string(),
            category: cat,
            idea_group_1: idea_groups[0].clone(),
            idea_group_2: idea_groups[1].clone(),
            modifiers,
        })
    } else {
        if category.is_none() {
            log::warn!("Policy '{}' missing monarch_power category", name);
        }
        if idea_groups.len() != 2 {
            log::warn!(
                "Policy '{}' has {} idea groups (expected 2)",
                name,
                idea_groups.len()
            );
        }
        None
    }
}

/// Parse policy category from AST node.
fn parse_category(node: &EU4TxtParseNode) -> Option<RawPolicyCategory> {
    if let EU4TxtAstItem::Identifier(cat) = &node.entry {
        match cat.as_str() {
            "ADM" => Some(RawPolicyCategory::Adm),
            "DIP" => Some(RawPolicyCategory::Dip),
            "MIL" => Some(RawPolicyCategory::Mil),
            _ => None,
        }
    } else {
        None
    }
}

/// Extract idea group names from `allow` block.
///
/// Looks for `full_idea_group = <name>` entries.
fn extract_idea_groups(node: &EU4TxtParseNode) -> Vec<String> {
    let mut groups = Vec::new();

    for child in &node.children {
        if let EU4TxtAstItem::Assignment = child.entry
            && child.children.len() >= 2
        {
            let key_node = &child.children[0];
            let value_node = &child.children[1];

            if let EU4TxtAstItem::Identifier(key) = &key_node.entry
                && key == "full_idea_group"
                && let EU4TxtAstItem::Identifier(value) = &value_node.entry
            {
                groups.push(value.clone());
            }
        }
    }

    groups
}

/// Extract f32 value from AST node.
fn get_f32(node: &EU4TxtParseNode) -> Option<f32> {
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
    fn test_parse_category() {
        assert_eq!(
            parse_category(&EU4TxtParseNode {
                entry: EU4TxtAstItem::Identifier("ADM".to_string()),
                children: vec![]
            }),
            Some(RawPolicyCategory::Adm)
        );

        assert_eq!(
            parse_category(&EU4TxtParseNode {
                entry: EU4TxtAstItem::Identifier("DIP".to_string()),
                children: vec![]
            }),
            Some(RawPolicyCategory::Dip)
        );

        assert_eq!(
            parse_category(&EU4TxtParseNode {
                entry: EU4TxtAstItem::Identifier("MIL".to_string()),
                children: vec![]
            }),
            Some(RawPolicyCategory::Mil)
        );
    }

    #[test]
    fn test_get_f32() {
        assert_eq!(
            get_f32(&EU4TxtParseNode {
                entry: EU4TxtAstItem::FloatValue(0.25),
                children: vec![]
            }),
            Some(0.25)
        );

        assert_eq!(
            get_f32(&EU4TxtParseNode {
                entry: EU4TxtAstItem::IntValue(5),
                children: vec![]
            }),
            Some(5.0)
        );

        assert_eq!(
            get_f32(&EU4TxtParseNode {
                entry: EU4TxtAstItem::Identifier("foo".to_string()),
                children: vec![]
            }),
            None
        );
    }
}
