//! Parser for EU4 idea groups from `common/ideas/`.
//!
//! Handles both generic idea groups (aristocracy_ideas) and
//! country-specific national ideas (FRA_ideas).

use eu4txt::{DefaultEU4Txt, EU4Txt, EU4TxtAstItem, EU4TxtParseNode};
use rayon::prelude::*;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::sync::Mutex;

/// Raw modifier entry parsed from game files.
#[derive(Debug, Clone)]
pub struct RawModifierEntry {
    pub key: String,
    pub value: f32,
}

/// Raw individual idea definition.
#[derive(Debug, Clone)]
pub struct RawIdea {
    pub name: String,
    pub modifiers: Vec<RawModifierEntry>,
}

/// Idea category (mana type spent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawIdeaCategory {
    Adm,
    Dip,
    Mil,
}

/// Raw idea group definition parsed from game files.
///
/// This is the intermediate representation before being converted to
/// [`eu4sim_core::ideas::IdeaGroupDef`].
#[derive(Debug, Clone)]
pub struct RawIdeaGroup {
    /// Name of the idea group (e.g., "aristocracy_ideas", "FRA_ideas")
    pub name: String,

    /// Category for generic idea groups (ADM/DIP/MIL)
    pub category: Option<RawIdeaCategory>,

    /// Whether this is a national idea (detected via `free = yes` or `start` block)
    pub is_national: bool,

    /// Required tag for national ideas (from `trigger = { tag = XXX }`)
    pub required_tag: Option<String>,

    /// Whether the ideas are auto-granted (national ideas have `free = yes`)
    pub is_free: bool,

    /// Starting modifiers (national ideas use `start = {}`)
    pub start_modifiers: Vec<RawModifierEntry>,

    /// Bonus modifiers when all 7 ideas are unlocked
    pub bonus_modifiers: Vec<RawModifierEntry>,

    /// The 7 individual ideas
    pub ideas: Vec<RawIdea>,

    /// AI willingness to pick this group (factor value)
    pub ai_will_do_factor: f32,
}

impl Default for RawIdeaGroup {
    fn default() -> Self {
        Self {
            name: String::new(),
            category: None,
            is_national: false,
            required_tag: None,
            is_free: false,
            start_modifiers: Vec::new(),
            bonus_modifiers: Vec::new(),
            ideas: Vec::new(),
            ai_will_do_factor: 1.0,
        }
    }
}

/// Reserved block names that aren't individual ideas.
const RESERVED_BLOCKS: &[&str] = &[
    "category",
    "bonus",
    "trigger",
    "free",
    "start",
    "ai_will_do",
    "important",
];

/// Loads all idea groups from `common/ideas/`.
pub fn load_idea_groups(base_path: &Path) -> Result<HashMap<String, RawIdeaGroup>, Box<dyn Error>> {
    let ideas_dir = base_path.join("common/ideas");
    let results = Mutex::new(HashMap::new());

    if !ideas_dir.exists() {
        log::warn!("Ideas directory not found: {:?}", ideas_dir);
        return Ok(HashMap::new());
    }

    let entries: Vec<_> = std::fs::read_dir(ideas_dir)?
        .filter_map(|e| e.ok())
        .collect();

    entries.par_iter().for_each(|entry| {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "txt")
            && let Err(e) = load_file(&path, &results)
        {
            log::warn!("Failed to parse ideas from {:?}: {}", path, e);
        }
    });

    Ok(results.into_inner().unwrap())
}

fn load_file(
    path: &Path,
    results: &Mutex<HashMap<String, RawIdeaGroup>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let tokens = DefaultEU4Txt::open_txt(path.to_str().unwrap()).map_err(|e| format!("{}", e))?;
    if tokens.is_empty() {
        return Ok(());
    }

    let ast = DefaultEU4Txt::parse(tokens).map_err(|e| format!("{}", e))?;

    // Structure: idea_group_name = { properties... }
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
                    let idea_group = parse_idea_group(name, body);

                    // Only add if we found actual ideas
                    if !idea_group.ideas.is_empty() {
                        let mut lock = results.lock().unwrap();
                        lock.insert(name.clone(), idea_group);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Parse an idea group definition from an AST node.
fn parse_idea_group(name: &str, node: &EU4TxtParseNode) -> RawIdeaGroup {
    let mut group = RawIdeaGroup {
        name: name.to_string(),
        ..Default::default()
    };

    // Detect if this is likely a national idea by name pattern (TAG_ideas)
    if name.len() >= 6 && name.ends_with("_ideas") {
        let prefix = &name[..name.len() - 6];
        // Check if prefix looks like a 3-letter tag (all uppercase)
        if prefix.len() == 3 && prefix.chars().all(|c| c.is_ascii_uppercase()) {
            group.required_tag = Some(prefix.to_string());
            group.is_national = true;
        }
    }

    for child in &node.children {
        if let EU4TxtAstItem::Assignment = child.entry
            && child.children.len() >= 2
        {
            let key_node = &child.children[0];
            let value_node = &child.children[1];

            if let EU4TxtAstItem::Identifier(key) = &key_node.entry {
                match key.as_str() {
                    "category" => {
                        group.category = parse_category(value_node);
                    }
                    "free" => {
                        group.is_free = get_bool(value_node).unwrap_or(false);
                        if group.is_free {
                            group.is_national = true;
                        }
                    }
                    "start" => {
                        group.start_modifiers = parse_modifier_block(value_node);
                        group.is_national = true;
                    }
                    "bonus" => {
                        group.bonus_modifiers = parse_modifier_block(value_node);
                    }
                    "trigger" => {
                        // Try to extract tag from trigger = { tag = XXX }
                        if let Some(tag) = extract_tag_from_trigger(value_node) {
                            group.required_tag = Some(tag);
                            group.is_national = true;
                        }
                    }
                    "ai_will_do" => {
                        group.ai_will_do_factor = parse_ai_will_do(value_node);
                    }
                    "important" => {
                        // Ignored
                    }
                    _ => {
                        // Not a reserved block - this is an individual idea
                        if !RESERVED_BLOCKS.contains(&key.as_str()) {
                            let idea = parse_idea(key, value_node);
                            group.ideas.push(idea);
                        }
                    }
                }
            }
        }
    }

    group
}

/// Parse a single idea definition.
fn parse_idea(name: &str, node: &EU4TxtParseNode) -> RawIdea {
    RawIdea {
        name: name.to_string(),
        modifiers: parse_modifier_block(node),
    }
}

/// Parse a block of modifiers (e.g., bonus = { global_tax_modifier = 0.1 }).
fn parse_modifier_block(node: &EU4TxtParseNode) -> Vec<RawModifierEntry> {
    let mut modifiers = Vec::new();

    for child in &node.children {
        if let EU4TxtAstItem::Assignment = child.entry
            && child.children.len() >= 2
        {
            let key_node = &child.children[0];
            let value_node = &child.children[1];

            if let EU4TxtAstItem::Identifier(key) = &key_node.entry {
                // Skip non-modifier fields like "effect", "removed_effect"
                if key == "effect" || key == "removed_effect" {
                    continue;
                }

                if let Some(value) = get_float(value_node) {
                    modifiers.push(RawModifierEntry {
                        key: key.clone(),
                        value,
                    });
                }
            }
        }
    }

    modifiers
}

/// Parse the category field (ADM/DIP/MIL).
fn parse_category(node: &EU4TxtParseNode) -> Option<RawIdeaCategory> {
    match &node.entry {
        EU4TxtAstItem::Identifier(s) => match s.to_uppercase().as_str() {
            "ADM" => Some(RawIdeaCategory::Adm),
            "DIP" => Some(RawIdeaCategory::Dip),
            "MIL" => Some(RawIdeaCategory::Mil),
            _ => None,
        },
        _ => None,
    }
}

/// Extract tag from trigger = { tag = XXX } pattern.
fn extract_tag_from_trigger(node: &EU4TxtParseNode) -> Option<String> {
    for child in &node.children {
        if let EU4TxtAstItem::Assignment = child.entry
            && child.children.len() >= 2
        {
            let key_node = &child.children[0];
            let value_node = &child.children[1];

            if let EU4TxtAstItem::Identifier(key) = &key_node.entry
                && key == "tag"
            {
                return get_string(value_node);
            }
        }
    }
    None
}

/// Parse ai_will_do block to extract the factor value.
fn parse_ai_will_do(node: &EU4TxtParseNode) -> f32 {
    for child in &node.children {
        if let EU4TxtAstItem::Assignment = child.entry
            && child.children.len() >= 2
        {
            let key_node = &child.children[0];
            let value_node = &child.children[1];

            if let EU4TxtAstItem::Identifier(key) = &key_node.entry
                && key == "factor"
            {
                return get_float(value_node).unwrap_or(1.0);
            }
        }
    }
    1.0
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::detect_game_path;

    #[test]
    fn test_load_idea_groups() {
        let Some(game_path) = detect_game_path() else {
            eprintln!("Skipping test: EU4 not found");
            return;
        };

        let groups = load_idea_groups(&game_path).expect("Failed to load idea groups");

        // Should have loaded many groups
        assert!(!groups.is_empty(), "Should load idea groups");
        println!("Loaded {} idea groups", groups.len());

        // Count nationals vs generics
        let national_count = groups.values().filter(|g| g.is_national).count();
        let generic_count = groups.len() - national_count;
        println!("  Generic: {}, National: {}", generic_count, national_count);

        // Check aristocracy_ideas exists (generic)
        let aristo = groups
            .get("aristocracy_ideas")
            .expect("aristocracy_ideas should exist");
        assert_eq!(aristo.category, Some(RawIdeaCategory::Mil));
        assert!(!aristo.is_national);
        assert!(!aristo.is_free);
        assert_eq!(aristo.ideas.len(), 7, "Should have 7 ideas");
        assert!(
            !aristo.bonus_modifiers.is_empty(),
            "Should have bonus modifiers"
        );
        println!(
            "  aristocracy: {} ideas, {} bonus modifiers",
            aristo.ideas.len(),
            aristo.bonus_modifiers.len()
        );

        // Check FRA_ideas exists (national)
        if let Some(fra) = groups.get("FRA_ideas") {
            assert!(fra.is_national);
            assert!(fra.is_free);
            assert_eq!(fra.required_tag.as_deref(), Some("FRA"));
            assert!(!fra.start_modifiers.is_empty(), "FRA should have start");
            println!(
                "  FRA_ideas: {} start, {} ideas",
                fra.start_modifiers.len(),
                fra.ideas.len()
            );
        }

        // Check TUR_ideas exists (national)
        if let Some(tur) = groups.get("TUR_ideas") {
            assert!(tur.is_national);
            assert_eq!(tur.required_tag.as_deref(), Some("TUR"));
            println!(
                "  TUR_ideas: {} start, {} ideas",
                tur.start_modifiers.len(),
                tur.ideas.len()
            );
        }

        // Sample some ideas to verify modifier parsing
        let first_idea = &aristo.ideas[0];
        println!(
            "  First idea: {} with {} modifiers",
            first_idea.name,
            first_idea.modifiers.len()
        );
        for m in &first_idea.modifiers {
            println!("    {} = {}", m.key, m.value);
        }

        // Collect all unique modifier keys for reference
        let mut all_modifiers = std::collections::HashSet::new();
        for group in groups.values() {
            for m in &group.start_modifiers {
                all_modifiers.insert(m.key.clone());
            }
            for m in &group.bonus_modifiers {
                all_modifiers.insert(m.key.clone());
            }
            for idea in &group.ideas {
                for m in &idea.modifiers {
                    all_modifiers.insert(m.key.clone());
                }
            }
        }
        println!("\nFound {} unique modifier keys", all_modifiers.len());

        // Print some samples
        let mut sorted: Vec<_> = all_modifiers.into_iter().collect();
        sorted.sort();
        for key in sorted.iter().take(20) {
            println!("  {}", key);
        }
    }

    #[test]
    fn test_idea_counts() {
        let Some(game_path) = detect_game_path() else {
            eprintln!("Skipping test: EU4 not found");
            return;
        };

        let groups = load_idea_groups(&game_path).expect("Failed to load idea groups");

        // Verify most groups have exactly 7 ideas
        let mut counts: HashMap<usize, usize> = HashMap::new();
        for group in groups.values() {
            *counts.entry(group.ideas.len()).or_default() += 1;
        }

        println!("Idea counts per group:");
        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by_key(|(k, _)| *k);
        for (count, num_groups) in sorted {
            println!("  {} ideas: {} groups", count, num_groups);
        }

        // Most should have 7
        let seven_count = groups.values().filter(|g| g.ideas.len() == 7).count();
        assert!(
            seven_count > groups.len() / 2,
            "Most groups should have 7 ideas"
        );
    }
}
