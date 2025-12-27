//! Parser for EU4 estates and privileges from `common/estates/` and `common/estate_privileges/`.
//!
//! Loads estate definitions (Nobles, Clergy, Burghers, etc.) and their privileges.

use eu4txt::{DefaultEU4Txt, EU4Txt, EU4TxtAstItem, EU4TxtParseNode};
use rayon::prelude::*;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::sync::Mutex;

/// Raw modifier entry (key-value pair).
#[derive(Debug, Clone)]
pub struct RawModifierEntry {
    pub key: String,
    pub value: f32,
}

/// Raw estate definition from game files.
#[derive(Debug, Clone, Default)]
pub struct RawEstate {
    pub name: String,
    pub icon: u8,
    /// Modifiers when loyalty > 60 (happy)
    pub happy_modifiers: Vec<RawModifierEntry>,
    /// Modifiers when loyalty 30-60 (neutral)
    pub neutral_modifiers: Vec<RawModifierEntry>,
    /// Modifiers when loyalty < 30 (angry)
    pub angry_modifiers: Vec<RawModifierEntry>,
}

/// Raw privilege definition from game files.
#[derive(Debug, Clone)]
pub struct RawPrivilege {
    pub name: String,
    pub estate_name: String, // Which estate this privilege belongs to
    pub land_share: f32,
    pub max_absolutism: i8,
    pub loyalty: f32,
    pub influence: f32,
    pub benefits: Vec<RawModifierEntry>,
    pub penalties: Vec<RawModifierEntry>,
}

impl Default for RawPrivilege {
    fn default() -> Self {
        Self {
            name: String::new(),
            estate_name: String::new(),
            land_share: 0.0,
            max_absolutism: 0,
            loyalty: 0.0,
            influence: 0.0,
            benefits: Vec::new(),
            penalties: Vec::new(),
        }
    }
}

/// Extract float value from AST node.
fn get_float(node: &EU4TxtParseNode) -> Option<f32> {
    match &node.entry {
        EU4TxtAstItem::FloatValue(n) => Some(*n),
        EU4TxtAstItem::IntValue(n) => Some(*n as f32),
        EU4TxtAstItem::Identifier(s) => s.parse().ok(),
        _ => None,
    }
}

/// Extract i8 value from AST node.
fn get_i8(node: &EU4TxtParseNode) -> Option<i8> {
    match &node.entry {
        EU4TxtAstItem::IntValue(n) => Some(*n as i8),
        EU4TxtAstItem::FloatValue(n) => Some(*n as i8),
        EU4TxtAstItem::Identifier(s) => s.parse().ok(),
        _ => None,
    }
}

/// Extract u8 value from AST node.
fn get_u8(node: &EU4TxtParseNode) -> Option<u8> {
    match &node.entry {
        EU4TxtAstItem::IntValue(n) => Some(*n as u8),
        EU4TxtAstItem::FloatValue(n) => Some(*n as u8),
        EU4TxtAstItem::Identifier(s) => s.parse().ok(),
        _ => None,
    }
}

/// Parse modifiers from a block (e.g., `country_modifier_happy = { ... }`).
fn parse_modifiers(node: &EU4TxtParseNode) -> Vec<RawModifierEntry> {
    let mut modifiers = Vec::new();

    for child in &node.children {
        if let EU4TxtAstItem::Assignment = child.entry {
            let key_node = child.children.first();
            let value_node = child.children.get(1);

            if let (Some(k), Some(v)) = (key_node, value_node)
                && let EU4TxtAstItem::Identifier(key) = &k.entry
                && let Some(value) = get_float(v)
            {
                modifiers.push(RawModifierEntry {
                    key: key.clone(),
                    value,
                });
            }
        }
    }

    modifiers
}

/// Parse a single estate file.
fn parse_estate(path: &Path) -> Result<Vec<RawEstate>, Box<dyn Error + Send + Sync>> {
    let filename = path.file_name().unwrap().to_str().unwrap();
    let tokens = DefaultEU4Txt::open_txt(path.to_str().unwrap()).map_err(|e| format!("{}", e))?;

    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    let ast = DefaultEU4Txt::parse(tokens).map_err(|e| format!("{}", e))?;
    let mut estates = Vec::new();

    if let EU4TxtAstItem::AssignmentList = ast.entry {
        for node in &ast.children {
            if let EU4TxtAstItem::Assignment = node.entry {
                let name_node = node.children.first().ok_or("Missing name node")?;
                let body_node = node.children.get(1);

                if let EU4TxtAstItem::Identifier(name) = &name_node.entry {
                    let body = body_node.ok_or("Missing body node")?;
                    let mut estate = RawEstate {
                        name: name.clone(),
                        ..Default::default()
                    };

                    // Parse estate properties
                    for child in &body.children {
                        if let EU4TxtAstItem::Assignment = child.entry {
                            let key_node = child.children.first();
                            let value_node = child.children.get(1);

                            if let Some(k) = key_node
                                && let EU4TxtAstItem::Identifier(key) = &k.entry
                            {
                                match key.as_str() {
                                    "icon" => {
                                        if let Some(v) = value_node
                                            && let Some(icon) = get_u8(v)
                                        {
                                            estate.icon = icon;
                                        }
                                    }
                                    "country_modifier_happy" => {
                                        if let Some(v) = value_node {
                                            estate.happy_modifiers = parse_modifiers(v);
                                        }
                                    }
                                    "country_modifier_neutral" => {
                                        if let Some(v) = value_node {
                                            estate.neutral_modifiers = parse_modifiers(v);
                                        }
                                    }
                                    "country_modifier_angry" => {
                                        if let Some(v) = value_node {
                                            estate.angry_modifiers = parse_modifiers(v);
                                        }
                                    }
                                    _ => {} // Skip trigger, ai_will_do, etc.
                                }
                            }
                        }
                    }

                    estates.push(estate);
                }
            }
        }
    }

    if estates.is_empty() {
        log::warn!("No estates parsed from {}", filename);
    }

    Ok(estates)
}

/// Parse a single privilege file.
fn parse_privileges(path: &Path) -> Result<Vec<RawPrivilege>, Box<dyn Error + Send + Sync>> {
    let filename = path.file_name().unwrap().to_str().unwrap();
    let tokens = DefaultEU4Txt::open_txt(path.to_str().unwrap()).map_err(|e| format!("{}", e))?;

    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    let ast = DefaultEU4Txt::parse(tokens).map_err(|e| format!("{}", e))?;
    let mut privileges = Vec::new();

    // Extract estate name from filename (e.g., "02_noble_privileges.txt" -> "nobles")
    let estate_name = extract_estate_from_filename(filename);

    if let EU4TxtAstItem::AssignmentList = ast.entry {
        for node in &ast.children {
            if let EU4TxtAstItem::Assignment = node.entry {
                let name_node = node.children.first().ok_or("Missing name node")?;
                let body_node = node.children.get(1);

                if let EU4TxtAstItem::Identifier(name) = &name_node.entry {
                    let body = body_node.ok_or("Missing body node")?;
                    let mut privilege = RawPrivilege {
                        name: name.clone(),
                        estate_name: estate_name.clone(),
                        ..Default::default()
                    };

                    // Parse privilege properties
                    for child in &body.children {
                        if let EU4TxtAstItem::Assignment = child.entry {
                            let key_node = child.children.first();
                            let value_node = child.children.get(1);

                            if let Some(k) = key_node
                                && let EU4TxtAstItem::Identifier(key) = &k.entry
                            {
                                match key.as_str() {
                                    "land_share" => {
                                        if let Some(v) = value_node
                                            && let Some(value) = get_float(v)
                                        {
                                            privilege.land_share = value;
                                        }
                                    }
                                    "max_absolutism" => {
                                        if let Some(v) = value_node
                                            && let Some(value) = get_i8(v)
                                        {
                                            privilege.max_absolutism = value;
                                        }
                                    }
                                    "loyalty" => {
                                        if let Some(v) = value_node
                                            && let Some(value) = get_float(v)
                                        {
                                            privilege.loyalty = value;
                                        }
                                    }
                                    "influence" => {
                                        if let Some(v) = value_node
                                            && let Some(value) = get_float(v)
                                        {
                                            privilege.influence = value;
                                        }
                                    }
                                    "benefits" => {
                                        if let Some(v) = value_node {
                                            privilege.benefits = parse_modifiers(v);
                                        }
                                    }
                                    "penalties" => {
                                        if let Some(v) = value_node {
                                            privilege.penalties = parse_modifiers(v);
                                        }
                                    }
                                    _ => {} // Skip can_select, on_granted, ai_will_do, etc.
                                }
                            }
                        }
                    }

                    privileges.push(privilege);
                }
            }
        }
    }

    Ok(privileges)
}

/// Extract estate name from privilege filename.
/// Examples:
/// - "02_noble_privileges.txt" -> "nobles"
/// - "01_church_privileges.txt" -> "church"
/// - "03_burgher_privileges.txt" -> "burghers"
fn extract_estate_from_filename(filename: &str) -> String {
    // Remove numbers, underscores at start, and file extension
    let name = filename
        .trim_start_matches(|c: char| c.is_ascii_digit() || c == '_')
        .trim_end_matches(".txt")
        .replace("_privileges", "");

    // Map common variants to estate names
    match name.as_str() {
        "noble" => "nobles".to_string(),
        "church" => "church".to_string(),
        "burgher" => "burghers".to_string(),
        "cossack" => "cossacks".to_string(),
        "dhimmi" => "dhimmi".to_string(),
        "brahmin" => "brahmins".to_string(),
        "maratha" => "maratha".to_string(),
        "rajput" => "rajput".to_string(),
        "vaisya" => "vaisyas".to_string(),
        "nomadic" | "nomadic_tribe" => "nomadic_tribes".to_string(),
        _ => name,
    }
}

/// Load all estates from `common/estates/`.
pub fn load_estates(base_path: &Path) -> Result<HashMap<String, RawEstate>, Box<dyn Error>> {
    let estates_dir = base_path.join("common/estates");
    let results = Mutex::new(HashMap::new());

    if !estates_dir.exists() {
        return Err(format!("Estates directory not found: {:?}", estates_dir).into());
    }

    let files: Vec<_> = std::fs::read_dir(&estates_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "txt")
                .unwrap_or(false)
        })
        .collect();

    files.par_iter().for_each(|entry| {
        let path = entry.path();

        match parse_estate(&path) {
            Ok(estates) => {
                let mut map = results.lock().unwrap();
                for estate in estates {
                    map.insert(estate.name.clone(), estate);
                }
            }
            Err(e) => {
                log::warn!("Failed to parse {:?}: {}", path, e);
            }
        }
    });

    let map = results.into_inner().unwrap();
    log::debug!("Loaded {} estates", map.len());

    Ok(map)
}

/// Load all privileges from `common/estate_privileges/`.
pub fn load_privileges(base_path: &Path) -> Result<Vec<RawPrivilege>, Box<dyn Error>> {
    let privileges_dir = base_path.join("common/estate_privileges");
    let results = Mutex::new(Vec::new());

    if !privileges_dir.exists() {
        return Err(format!("Privileges directory not found: {:?}", privileges_dir).into());
    }

    let files: Vec<_> = std::fs::read_dir(&privileges_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "txt")
                .unwrap_or(false)
        })
        .collect();

    files.par_iter().for_each(|entry| {
        let path = entry.path();

        match parse_privileges(&path) {
            Ok(privileges) => {
                let mut vec = results.lock().unwrap();
                vec.extend(privileges);
            }
            Err(e) => {
                log::warn!("Failed to parse {:?}: {}", path, e);
            }
        }
    });

    let vec = results.into_inner().unwrap();
    log::debug!("Loaded {} privileges", vec.len());

    Ok(vec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_estate_from_filename() {
        assert_eq!(
            extract_estate_from_filename("02_noble_privileges.txt"),
            "nobles"
        );
        assert_eq!(
            extract_estate_from_filename("01_church_privileges.txt"),
            "church"
        );
        assert_eq!(
            extract_estate_from_filename("03_burgher_privileges.txt"),
            "burghers"
        );
        assert_eq!(
            extract_estate_from_filename("04_cossack_privileges.txt"),
            "cossacks"
        );
    }

    #[test]
    fn test_get_float() {
        let node = EU4TxtParseNode {
            entry: EU4TxtAstItem::FloatValue(1.5),
            children: vec![],
        };
        assert_eq!(get_float(&node), Some(1.5));

        let node = EU4TxtParseNode {
            entry: EU4TxtAstItem::IntValue(42),
            children: vec![],
        };
        assert_eq!(get_float(&node), Some(42.0));
    }

    #[test]
    fn test_get_i8() {
        let node = EU4TxtParseNode {
            entry: EU4TxtAstItem::IntValue(-5),
            children: vec![],
        };
        assert_eq!(get_i8(&node), Some(-5));
    }

    #[test]
    fn test_get_u8() {
        let node = EU4TxtParseNode {
            entry: EU4TxtAstItem::IntValue(2),
            children: vec![],
        };
        assert_eq!(get_u8(&node), Some(2));
    }
}
