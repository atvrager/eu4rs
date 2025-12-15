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
use rayon::prelude::*;
use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

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
    let results = Mutex::new(HashMap::new());

    // We are going to be tolerant here. If a country fails to load, we just skip it.
    // In a real game engine, we might want to log this.

    tags.par_iter().for_each(|(tag, rel_path)| {
        let full_path = base_path.join("common").join(rel_path);

        if !full_path.exists() {
            return;
        }

        // Try to load the country, using a block to easily bail out on any error
        let maybe_country = (|| {
            let tokens = DefaultEU4Txt::open_txt(full_path.to_str()?).ok()?;
            let ast = DefaultEU4Txt::parse(tokens).ok()?;
            from_node::<Country>(&ast).ok()
        })();

        if let Some(country) = maybe_country {
            results.lock().unwrap().insert(tag.clone(), country);
        }
    });

    results.into_inner().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_load_tags() {
        let dir = tempfile::tempdir().unwrap();
        let tags_dir = dir.path().join("common/country_tags");
        std::fs::create_dir_all(&tags_dir).unwrap();

        let file_path = tags_dir.join("00_countries.txt");
        let mut file = std::fs::File::create(file_path).unwrap();
        write!(
            file,
            r#"
            SWE = "countries/Sweden.txt"
            ENG = "countries/England.txt"
            "#
        )
        .unwrap();

        let tags = load_tags(dir.path()).unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(
            tags.get("SWE")
                .unwrap()
                .to_str()
                .unwrap()
                .replace("\\", "/"),
            "countries/Sweden.txt"
        ); // normalize separators
        assert_eq!(
            tags.get("ENG")
                .unwrap()
                .to_str()
                .unwrap()
                .replace("\\", "/"),
            "countries/England.txt"
        );
    }

    #[test]
    fn test_load_country_map() {
        let dir = tempfile::tempdir().unwrap();
        let common_dir = dir.path().join("common");
        std::fs::create_dir_all(common_dir.join("countries")).unwrap();

        // 1. Create mock country files
        let sweden_path = common_dir.join("countries/Sweden.txt");
        let mut file = std::fs::File::create(&sweden_path).unwrap();
        write!(
            file,
            r#"
            color = {{ 10 20 200 }}
            "#
        )
        .unwrap();

        let england_path = common_dir.join("countries/England.txt");
        let mut file = std::fs::File::create(&england_path).unwrap();
        write!(
            file,
            r#"
            color = {{ 200 10 10 }}
            "#
        )
        .unwrap();

        // 2. Create tag map
        let mut tags = TagMap::new();
        tags.insert(
            "SWE".to_string(),
            std::path::PathBuf::from("countries/Sweden.txt"),
        );
        tags.insert(
            "ENG".to_string(),
            std::path::PathBuf::from("countries/England.txt"),
        );
        tags.insert(
            "FRA".to_string(),
            std::path::PathBuf::from("countries/France.txt"),
        ); // Does not exist

        // 3. Load
        let countries = load_country_map(dir.path(), &tags);

        assert_eq!(countries.len(), 2);

        let swe = countries.get("SWE").unwrap();
        assert_eq!(swe.color, vec![10, 20, 200]);

        let eng = countries.get("ENG").unwrap();
        assert_eq!(eng.color, vec![200, 10, 10]);

        assert!(!countries.contains_key("FRA"));
    }
}
