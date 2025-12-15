use crate::coverage::SchemaType;
use eu4data_derive::TolerantDeserialize;
use eu4txt::{DefaultEU4Txt, EU4Txt, EU4TxtAstItem, from_node};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Mutex;

/// Represents a culture definition.
#[derive(Debug, Clone, Default, Serialize, TolerantDeserialize, SchemaType)]
pub struct Culture {
    /// The RGB color. Often generated.
    #[serde(skip)]
    pub color: [u8; 3],

    pub primary: Option<String>,
    pub graphical_culture: Option<String>,
    pub second_graphical_culture: Option<String>,

    pub male_names: Option<Vec<String>>,
    pub female_names: Option<Vec<String>>,
    pub dynasty_names: Option<Vec<String>>,

    #[serde(skip_serializing)]
    pub country: Option<HashMap<String, serde::de::IgnoredAny>>,
    #[serde(skip_serializing)]
    pub province: Option<HashMap<String, serde::de::IgnoredAny>>,

    // Mechanics
    pub has_samurai: Option<bool>,
    pub local_has_samurai: Option<bool>,
    pub local_has_tercio: Option<bool>,

    // Catch all
    #[serde(flatten, skip_serializing)]
    pub other: std::collections::HashMap<String, serde::de::IgnoredAny>,
}

/// Generates a deterministic color from a string.
pub fn hash_color(s: &str) -> [u8; 3] {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    let hash = hasher.finish();

    // Simple way to get 3 bytes
    let r = (hash & 0xFF) as u8;
    let g = ((hash >> 8) & 0xFF) as u8;
    let b = ((hash >> 16) & 0xFF) as u8;

    // Boost saturation/brightness to avoid muddy colors
    [
        r.saturating_add(50),
        g.saturating_add(50),
        b.saturating_add(50),
    ]
}

/// Loads all cultures from `common/cultures`.
/// The file structure is `group = { culture = { ... } }`.
pub fn load_cultures(base_path: &Path) -> Result<HashMap<String, Culture>, Box<dyn Error>> {
    let culture_dir = base_path.join("common/cultures");
    let results = Mutex::new(HashMap::new());

    if !culture_dir.exists() {
        return Ok(HashMap::new());
    }

    let entries: Vec<_> = std::fs::read_dir(culture_dir)?
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

fn load_file(path: &Path, results: &Mutex<HashMap<String, Culture>>) -> Result<(), Box<dyn Error>> {
    let tokens = DefaultEU4Txt::open_txt(path.to_str().unwrap()).map_err(|e| e.to_string())?;
    if tokens.is_empty() {
        return Ok(());
    }

    let ast = DefaultEU4Txt::parse(tokens).map_err(|e| e.to_string())?;

    // Group level
    if let EU4TxtAstItem::AssignmentList = ast.entry {
        for group_node in &ast.children {
            if let EU4TxtAstItem::Assignment = group_node.entry {
                let group_rhs = group_node.children.get(1).unwrap();

                // Culture level
                if let EU4TxtAstItem::AssignmentList = group_rhs.entry {
                    for cult_node in &group_rhs.children {
                        if let EU4TxtAstItem::Assignment = cult_node.entry {
                            let name_node = cult_node.children.first().unwrap();
                            // We don't really parse the body since we are generating color
                            // But usually there isn't a color field.

                            if let EU4TxtAstItem::Identifier(name) = &name_node.entry {
                                if name == "graphical_culture" {
                                    continue;
                                } // non-culture keys

                                let color = hash_color(name);

                                // Parse the body
                                let body_node = cult_node.children.get(1).unwrap();
                                let mut culture = match from_node::<Culture>(body_node) {
                                    Ok(c) => c,
                                    Err(e) => {
                                        log::warn!(
                                            "Failed to parse culture '{}' in {}: {}",
                                            name,
                                            path.display(),
                                            e
                                        );
                                        Culture::default()
                                    }
                                };

                                culture.color = color; // Inject generated color
                                let mut lock = results.lock().unwrap();
                                lock.insert(name.clone(), culture);
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_load_cultures() {
        let dir = tempdir().unwrap();
        let c_dir = dir.path().join("common/cultures");
        std::fs::create_dir_all(&c_dir).unwrap();

        // Mock file
        let mut f = std::fs::File::create(c_dir.join("00_cultures.txt")).unwrap();
        write!(
            f,
            r#"
        germanic = {{
            swedish = {{
                primary = SWE
            }}
            danish = {{
                primary = DAN
            }}
        }}
        "#
        )
        .unwrap();

        let cultures = load_cultures(dir.path()).unwrap();
        assert_eq!(cultures.len(), 2);
        assert!(cultures.contains_key("swedish"));
        assert!(cultures.contains_key("danish"));

        // Deterministic check
        let swedish = cultures.get("swedish").unwrap();
        let expected = hash_color("swedish");
        assert_eq!(swedish.color, expected);
        assert_eq!(swedish.primary.as_deref(), Some("SWE"));
    }
}
