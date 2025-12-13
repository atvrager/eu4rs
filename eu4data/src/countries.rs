use serde::{Deserialize, Serialize};

/// Represents a country definition in EU4.
#[derive(Debug, Deserialize, Serialize)]
pub struct Country {
    /// The RGB color of the country on the political map.
    #[serde(default)]
    pub color: Vec<u8>,
    // There are many other fields (graphical_culture, etc.) but we only need color for now.
}

use eu4txt::{DefaultEU4Txt, EU4Txt, EU4TxtAstItem};
use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};

pub type TagMap = HashMap<String, PathBuf>;

/// Loads country tags from the `common/country_tags` directory.
/// Returns a map of Tag -> Path (relative to game root).
pub fn load_tags(base_path: &Path) -> Result<TagMap, Box<dyn Error>> {
    let tags_dir = base_path.join("common/country_tags");
    let mut tags = HashMap::new();

    if tags_dir.is_dir() {
        for entry in std::fs::read_dir(tags_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "txt") {
                let tokens = DefaultEU4Txt::open_txt(path.to_str().unwrap())?;
                let ast = DefaultEU4Txt::parse(tokens).map_err(|e| e.to_string())?;

                // country_tags files are usually lists of assignments:
                // SWE = "countries/Sweden.txt"
                // ENG = "countries/England.txt"
                if let EU4TxtAstItem::AssignmentList = ast.entry {
                    for child in ast.children {
                        if let EU4TxtAstItem::Assignment = child.entry {
                            let lhs = child.children.first().unwrap();
                            let rhs = child.children.get(1).unwrap();

                            let key = match &lhs.entry {
                                EU4TxtAstItem::Identifier(s) => Some(s.clone()),
                                EU4TxtAstItem::StringValue(s) => Some(s.clone()),
                                _ => None,
                            };

                            let val = match &rhs.entry {
                                EU4TxtAstItem::StringValue(s) => Some(s.clone()),
                                _ => None,
                            };

                            if let (Some(k), Some(v)) = (key, val) {
                                tags.insert(k, PathBuf::from(v));
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(tags)
}

use eu4txt::from_node;

/// Loads all country definitions based on the provided TagMap.
/// Returns a map of Tag -> Country.
pub fn load_country_map(base_path: &Path, tags: &TagMap) -> HashMap<String, Country> {
    let mut countries = HashMap::new();

    // We are going to be tolerant here. If a country fails to load, we just skip it.
    // In a real game engine, we might want to log this.

    for (tag, rel_path) in tags {
        let full_path = base_path.join("common").join(rel_path);
        // Note: rel_path from country_tags usually starts with "countries/", so join("common").join(...) is correct because
        // country_tags entries are relative to "common". E.g. "countries/Sweden.txt".

        if !full_path.exists() {
            continue;
        }

        let tokens = match DefaultEU4Txt::open_txt(full_path.to_str().unwrap()) {
            Ok(t) => t,
            Err(_) => continue,
        };

        let ast = match DefaultEU4Txt::parse(tokens) {
            Ok(a) => a,
            Err(_) => continue,
        };

        if let Ok(country) = from_node::<Country>(&ast) {
            countries.insert(tag.clone(), country);
        }
    }

    countries
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_country_mock() {
        let data = r#"
            color = { 10 20 200 }
            graphical_culture = westerngfx
        "#;

        // Write to temp file
        let mut file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
        write!(file, "{}", data).expect("Failed to write");
        let path = file.path().to_str().unwrap().to_string();

        // Testing direct parsing, so tags map is irrelevant here.
        // But if I wanted to test load_country_map, I would need it.
        // For now, let's just delete the unused map.

        use eu4txt::{DefaultEU4Txt, EU4Txt, from_node};
        let tokens = DefaultEU4Txt::open_txt(&path).expect("Tok");
        let ast = DefaultEU4Txt::parse(tokens).expect("Parse");
        let country: Country = from_node(&ast).expect("De");

        assert_eq!(country.color.len(), 3);
        assert_eq!(country.color[0], 10);
        assert_eq!(country.color[2], 200);
    }
}
