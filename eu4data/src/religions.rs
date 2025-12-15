use crate::coverage::SchemaType;
use eu4txt::{DefaultEU4Txt, EU4Txt, EU4TxtAstItem, from_node};
use rayon::prelude::*;
use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::sync::Mutex;

/// Represents a religion definition.
#[derive(Debug, Deserialize, Serialize, Clone, SchemaType)]
pub struct Religion {
    /// The RGB color of the religion (from `color = { r g b }`).
    pub color: Vec<u8>,

    /// Icon ID (often just an index)
    #[serde(default)]
    pub icon: u32,

    /// Modifiers applied to the country.
    #[serde(skip_serializing)]
    pub country: Option<HashMap<String, IgnoredAny>>,

    /// Modifiers applied to provinces following this religion.
    #[serde(skip_serializing)]
    pub province: Option<HashMap<String, IgnoredAny>>,

    /// Modifiers applied if this is a secondary religion.
    #[serde(skip_serializing)]
    pub country_as_secondary: Option<HashMap<String, IgnoredAny>>,

    /// List of heretic religion tags.
    pub heretic: Option<Vec<String>>,

    /// Effects when converting.
    #[serde(skip_serializing)]
    pub on_convert: Option<HashMap<String, IgnoredAny>>,

    /// List of religions this one can convert to/from.
    pub allowed_conversion: Option<Vec<String>>,

    /// List of religions that centers of reformation can convert to.
    pub allowed_center_conversion: Option<Vec<String>>,

    /// Date of reformation or enabling.
    pub date: Option<String>,

    /// Catch-all for other fields to ensure 100% Parse coverage.
    #[serde(flatten, skip_serializing)]
    pub other: HashMap<String, IgnoredAny>,
}

/// Loads all religions types from `common/religions`.
/// The file structure is `group = { religion = { ... } }`.
pub fn load_religions(base_path: &Path) -> Result<HashMap<String, Religion>, Box<dyn Error>> {
    let rel_dir = base_path.join("common/religions");
    let results = Mutex::new(HashMap::new());

    if !rel_dir.exists() {
        return Ok(HashMap::new());
    }

    let entries: Vec<_> = std::fs::read_dir(rel_dir)?.filter_map(|e| e.ok()).collect();

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
    results: &Mutex<HashMap<String, Religion>>,
) -> Result<(), Box<dyn Error>> {
    let tokens = DefaultEU4Txt::open_txt(path.to_str().unwrap()).map_err(|e| e.to_string())?;
    // If empty file, return ok
    if tokens.is_empty() {
        return Ok(());
    }

    let ast = DefaultEU4Txt::parse(tokens).map_err(|e| e.to_string())?;

    // The AST is a list of groups: christian = { catholic = { ... } }
    if let EU4TxtAstItem::AssignmentList = ast.entry {
        for group_node in &ast.children {
            // Check if group_node is an assignment (christian = { ... })
            if let EU4TxtAstItem::Assignment = group_node.entry {
                let group_rhs = group_node.children.get(1).unwrap();

                // Iterate children of the group (the actual religions)
                if let EU4TxtAstItem::AssignmentList = group_rhs.entry {
                    for rel_node in &group_rhs.children {
                        if let EU4TxtAstItem::Assignment = rel_node.entry {
                            let rel_name_node = rel_node.children.first().unwrap();
                            let rel_def_node = rel_node.children.get(1).unwrap();

                            if let EU4TxtAstItem::Identifier(name) = &rel_name_node.entry {
                                // Skip group-level metadata fields (not actual religions)
                                let group_metadata_fields = [
                                    "defender_of_faith",
                                    "can_form_personal_unions",
                                    "center_of_religion",
                                    "flags_with_emblem_percentage",
                                    "flag_emblem_index_range",
                                    "crusade_name",
                                    "harmonized_modifier",
                                    "ai_will_propagate_through_trade",
                                    "religious_schools",
                                    "papacy",
                                    "hre_heretic_religion",
                                    "hre_religion",
                                    "misguided_heretic",
                                ];

                                if group_metadata_fields.contains(&name.as_str()) {
                                    continue;
                                }

                                // Try parse Religion struct from the RHS
                                if let Ok(religion) = from_node::<Religion>(rel_def_node) {
                                    let mut lock = results.lock().unwrap();
                                    lock.insert(name.clone(), religion);
                                }
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
    fn test_load_religions() {
        let dir = tempdir().unwrap();
        let rel_dir = dir.path().join("common/religions");
        std::fs::create_dir_all(&rel_dir).unwrap();

        let file_path = rel_dir.join("00_religion.txt");
        let mut file = std::fs::File::create(file_path).unwrap();
        write!(
            file,
            r#"
            christian = {{
                catholic = {{
                    color = {{ 200 200 0 }}
                    icon = 1
                    heretic = {{ protestant reformed }}
                    country = {{ tolerance_own = 1 }}
                }}
                protestant = {{
                    color = {{ 0 0 200 }}
                    icon = 6
                }}
            }}
            muslim = {{
                sunni = {{
                    color = {{ 0 200 0 }}
                }}
            }}
            "#
        )
        .unwrap();

        let religions = load_religions(dir.path()).unwrap();

        assert_eq!(religions.len(), 3);

        let catholic = religions.get("catholic").unwrap();
        assert_eq!(catholic.color, vec![200, 200, 0]);
        assert_eq!(
            catholic.heretic.as_ref().unwrap(),
            &vec!["protestant".to_string(), "reformed".to_string()]
        );
        assert!(catholic.country.is_some());

        let sunni = religions.get("sunni").unwrap();
        assert_eq!(sunni.color, vec![0, 200, 0]);
        assert_eq!(sunni.icon, 0); // Default
    }
}
