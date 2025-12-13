use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

// use encoding_rs::WINDOWS_1252;
use encoding_rs_io::DecodeReaderBytesBuilder;

/// content of a localisation file is typically:
/// l_english:
///  KEY:0 "Value"
///  KEY2: "Value" # Optional comment
/// Manages game localisation data, mapping keys to text values.
///
/// Supports loading from standard Paradox `.yml` files, handling UTF-8 BOM,
/// and filtering by language (e.g. `l_english`).
#[derive(Debug, Default)]
pub struct Localisation {
    map: HashMap<String, String>,
}

impl Localisation {
    /// Creates a new empty localisation store.
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Inserts a key-value pair directly.
    pub fn insert(&mut self, key: String, value: String) {
        self.map.insert(key, value);
    }

    /// Retrieves the localised value for a given key.
    pub fn get(&self, key: &str) -> Option<&String> {
        self.map.get(key)
    }

    /// Loads all `.yml` files in a directory that match the specified language.
    ///
    /// # Arguments
    ///
    /// * `dir` - The directory path to search.
    /// * `language` - The language tag to filter for (e.g., "english", "l_spanish").
    ///
    /// # Returns
    ///
    /// The number of keys successfully loaded.
    pub fn load_from_dir<P: AsRef<Path>>(
        &mut self,
        dir: P,
        language: &str,
    ) -> std::io::Result<usize> {
        let mut count = 0;
        let dir = dir.as_ref();
        if !dir.exists() {
            return Ok(0);
        }

        // Expected header format: "l_english:"
        // We strip "l_" if the user provided it, to be safe.
        // And we handle case insensitivity by just using the language name for checking.
        let language = language.trim_start_matches("l_").trim_start_matches("L_");
        let header_suffix = format!("_{}:", language).to_lowercase();
        // We will check if "l" + header_suffix matches "l_english:" or "L_ENGLISH:" (lowercased)

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yml") {
                count += self.load_file(&path, &header_suffix)?;
            }
        }
        Ok(count)
    }

    fn load_file(&mut self, path: &Path, header_suffix: &str) -> std::io::Result<usize> {
        let file = File::open(path)?;
        // Use default encoding (UTF-8) with BOM sniffing.
        // Localisation files are typically UTF-8 BOM.
        let reader = BufReader::new(DecodeReaderBytesBuilder::new().build(file));

        let mut count = 0;
        let mut correct_language = false;

        for (i, line) in reader.lines().enumerate() {
            let line = line?;
            // Handle parsing artifacts from BOM or whitespace
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Check first non-empty/comment line for language tag
            // We check if it starts with "l_" or "L_" case-insensitively
            let line_lower = line.to_lowercase();
            if line_lower.starts_with("l_") {
                // Check if it ends with our expected suffix (e.g. "_english:")
                if line_lower.ends_with(header_suffix) {
                    correct_language = true;
                } else {
                    // Wrong language, skip
                    return Ok(0);
                }
                continue;
            }

            // If we haven't seen the language tag yet...
            if !correct_language {
                if i > 5 {
                    return Ok(0);
                }
                continue;
            }

            // Parse: KEY:0 "Value"
            if let Some((key_part, val_part)) = line.split_once(':') {
                let key = key_part.trim().to_string();

                // val_part is like `0 "Value"` or `"Value"`
                if let Some(start_quote) = val_part.find('"')
                    && let Some(end_quote) = val_part[start_quote + 1..].rfind('"')
                {
                    let value = &val_part[start_quote + 1..start_quote + 1 + end_quote];
                    self.map.insert(key, value.to_string());
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    /// Scans the directory for available languages by checking `l_<language>:` headers.
    ///
    /// This function reads the first few lines of every `.yml` file in the directory
    /// to identify the language tag relative to "l_".
    pub fn list_languages<P: AsRef<Path>>(dir: P) -> std::io::Result<Vec<String>> {
        let dir = dir.as_ref();
        let mut languages = std::collections::HashSet::new();

        if !dir.exists() {
            return Ok(vec![]);
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yml")
                && let Ok(file) = File::open(&path)
            {
                let reader = BufReader::new(DecodeReaderBytesBuilder::new().build(file));

                for line in reader.lines().take(5).flatten() {
                    let line = line.trim();
                    let line_lower = line.to_lowercase();
                    if line_lower.starts_with("l_") && line_lower.ends_with(':') {
                        // Extract "english" from "l_english:"
                        let lang = &line_lower[2..line_lower.len() - 1];
                        languages.insert(lang.to_string());
                        break;
                    }
                }
            }
        }
        let mut result: Vec<String> = languages.into_iter().collect();
        result.sort();
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_simple() {
        let content = r#"
l_english:
 KEY:0 "Value"
 KEY_TWO: "Value Two"
 BROKEN "No colon"
        "#;

        let mut file = tempfile::NamedTempFile::new().expect("create temp");
        write!(file, "{}", content).expect("write temp");

        let mut loc = Localisation::new();
        loc.load_file(file.path(), "_english:").expect("load");

        assert_eq!(loc.get("KEY"), Some(&"Value".to_string()));
        assert_eq!(loc.get("KEY_TWO"), Some(&"Value Two".to_string()));
        assert_eq!(loc.get("BROKEN"), None);
    }
}
