use serde::Deserialize;

/// Represents the historical data of a province (e.g., in `history/provinces`).
#[derive(Debug, Deserialize)]
pub struct ProvinceHistory {
    /// The trade good produced in the province.
    pub trade_goods: Option<String>,
    /// The tag of the country that owns the province.
    pub owner: Option<String>,
    /// The base tax value of the province.
    pub base_tax: Option<f32>,
    /// The base production value of the province.
    pub base_production: Option<f32>,
    /// The base manpower value of the province.
    pub base_manpower: Option<f32>,
    /// The religion of the province.
    pub religion: Option<String>,
    /// The culture of the province.
    pub culture: Option<String>,
}

use eu4txt::DefaultEU4Txt;
use eu4txt::EU4Txt;
use eu4txt::from_node;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

/// Loads all province history files from the `history/provinces` directory.
/// Returns a map of Province ID -> ProvinceHistory.
pub type HistoryLoadResult = (HashMap<u32, ProvinceHistory>, (usize, usize));

pub fn load_province_history(base_path: &Path) -> Result<HistoryLoadResult, std::io::Error> {
    let history_path = base_path.join("history/provinces");

    if !history_path.is_dir() {
        return Ok((HashMap::new(), (0, 0)));
    }

    // Collect entries first to bridge to rayon (read_dir is not Send)
    let entries: Vec<_> = std::fs::read_dir(history_path)?
        .filter_map(|e| e.ok())
        .collect();

    let results = Mutex::new((HashMap::new(), (0, 0)));

    entries.par_iter().for_each(|entry| {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "txt") {
            return;
        }

        // Helper closure for the "happy path" to allow early exit on failure
        let try_load = || -> Option<(u32, ProvinceHistory)> {
            let stem = path.file_stem()?.to_str()?;

            // Robustly parse ID: handle "123 - Name", "123-Name", "123 Name"
            let id_str = stem.split('-').next().unwrap_or(stem).trim();
            let id_part = id_str.split_whitespace().next().unwrap_or(id_str);
            let id = id_part.parse::<u32>().ok()?;

            let tokens = DefaultEU4Txt::open_txt(path.to_str()?).ok()?;

            if tokens.is_empty() {
                return Some((
                    id,
                    ProvinceHistory {
                        trade_goods: None,
                        owner: None,
                        base_tax: None,
                        base_production: None,
                        base_manpower: None,
                        religion: None,
                        culture: None,
                    },
                ));
            }

            let ast = DefaultEU4Txt::parse(tokens).ok()?;
            let hist = from_node::<ProvinceHistory>(&ast).ok()?;

            Some((id, hist))
        };

        if let Some((id, hist)) = try_load() {
            let mut lock = results.lock().unwrap();
            lock.0.insert(id, hist);
            lock.1.0 += 1;
        } else {
            let mut lock = results.lock().unwrap();
            lock.1.1 += 1;
        }
    });

    Ok(results.into_inner().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_load_province_history() {
        let dir = tempdir().unwrap();
        let history_path = dir.path().join("history/provinces");
        fs::create_dir_all(&history_path).unwrap();

        // 1. Valid file
        let file_path = history_path.join("1 - Stockholm.txt");
        let mut file = fs::File::create(file_path).unwrap();
        writeln!(
            file,
            r#"
            trade_goods = grain
            owner = SWE
            base_tax = 10.0
            base_production = 5.0
            religion = catholic
            culture = swedish
            "#
        )
        .unwrap();

        // 2. File with irregular name
        let file_path = history_path.join("2-Svealand.txt");
        let mut file = fs::File::create(file_path).unwrap();
        // Missing fields should be handled by Option::None
        writeln!(file, "owner = SWE").unwrap();

        // 3. Broken file (non-parsable ID)
        let file_path = history_path.join("invalid_name.txt");
        fs::File::create(file_path).unwrap();

        // 4. Broken file (bad syntax)
        let file_path = history_path.join("3 - Kalmar.txt");
        let mut file = fs::File::create(file_path).unwrap();
        writeln!(file, "this is not legitimate eu4 script").unwrap();

        let (map, (success, fail)) = load_province_history(dir.path()).unwrap();

        assert_eq!(success, 2);
        assert_eq!(fail, 2); // "invalid_name.txt" fails ID parse, "3 - Kalmar" fails content parse

        let p1 = map.get(&1).unwrap();
        assert_eq!(p1.owner.as_deref(), Some("SWE"));
        assert_eq!(p1.base_tax, Some(10.0));
        assert_eq!(p1.trade_goods.as_deref(), Some("grain"));
        assert_eq!(p1.religion.as_deref(), Some("catholic"));
        assert_eq!(p1.culture.as_deref(), Some("swedish"));

        let p2 = map.get(&2).unwrap();
        assert_eq!(p2.owner.as_deref(), Some("SWE"));
        assert_eq!(p2.base_tax, None);
    }
}
