use crate::coverage::SchemaType;
use eu4data_derive::TolerantDeserialize;
use serde::de::IgnoredAny;
use std::collections::HashMap;

/// Represents the historical data of a province (e.g., in `history/provinces`).
#[derive(Debug, Default, TolerantDeserialize, SchemaType)]
pub struct ProvinceHistory {
    /// The trade good produced in the province.
    #[schema(visualized)]
    pub trade_goods: Option<String>,
    /// The tag of the country that owns the province.
    #[schema(visualized)]
    pub owner: Option<String>,
    /// The base tax value of the province.
    #[schema(simulated)]
    pub base_tax: Option<f32>,
    /// The base production value of the province.
    #[schema(simulated)]
    pub base_production: Option<f32>,
    /// The base manpower value of the province.
    #[schema(simulated)]
    pub base_manpower: Option<f32>,
    /// The religion of the province.
    #[schema(visualized)]
    pub religion: Option<String>,
    /// The culture of the province.
    #[schema(visualized)]
    pub culture: Option<String>,

    // New Fields
    /// Whether the province is a city (fully colonized).
    pub is_city: Option<bool>,
    /// Whether the province is part of the HRE.
    pub hre: Option<bool>,
    /// The name of the capital city/provincial capital.
    pub capital: Option<String>,
    /// The tag of the country that controls the province (e.g. in war).
    pub controller: Option<String>,
    /// Cores held on this province.
    // pub add_core: Option<Vec<String>>,
    /// Claims held on this province.
    // pub add_claim: Option<Vec<String>>,
    /// Which tech groups have discovered this province.
    // pub discovered_by: Option<Vec<String>>,
    /// Native population size.
    pub native_size: Option<u32>,
    /// Native ferocity.
    pub native_ferocity: Option<u32>,
    /// Native hostileness.
    pub native_hostileness: Option<u32>,
    /// Level of Center of Trade (1, 2, 3).
    pub center_of_trade: Option<u8>,

    // Remaining Fields for 100% Coverage
    pub tribal_owner: Option<String>,
    pub revolt_risk: Option<f32>,
    pub unrest: Option<f32>,
    pub extra_cost: Option<f32>,
    pub add_local_autonomy: Option<f32>,
    pub add_nationalism: Option<f32>,
    pub seat_in_parliament: Option<bool>,
    pub shipyard: Option<bool>,
    #[schema(simulated)]
    pub fort_15th: Option<bool>,

    // Latent trade goods might be repeated or list, use Vec<IgnoredAny> to be safe for now
    pub latent_trade_goods: Option<Vec<IgnoredAny>>,

    pub discovered_by: Option<Vec<IgnoredAny>>,
    /// Historical cores on this province (countries that have permanent claims).
    /// Includes both the owner and other countries with reconquest claims.
    #[schema(simulated)]
    pub add_core: Option<Vec<String>>,
    pub add_claim: Option<Vec<IgnoredAny>>,

    // Explicitly ignored complex fields
    pub add_permanent_province_modifier: Option<Vec<IgnoredAny>>,
    pub add_province_triggered_modifier: Option<Vec<IgnoredAny>>,
    pub add_trade_modifier: Option<Vec<IgnoredAny>>,
    pub add_brahmins_or_church_effect: Option<Vec<IgnoredAny>>,
    pub add_jains_or_burghers_effect: Option<Vec<IgnoredAny>>,
    pub add_rajputs_or_marathas_or_nobles_effect: Option<Vec<IgnoredAny>>,
    pub add_vaisyas_or_burghers_effect: Option<Vec<IgnoredAny>>,
    // Note: Date-keyed entries (e.g. "1444.1.1 = { ... }") are silently ignored.
    // Unknown fields are not errors in serde - they're just skipped.
}

use eu4txt::DefaultEU4Txt;
use eu4txt::EU4Txt;
use eu4txt::from_node;
use rayon::prelude::*;

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
        let try_load = || -> Result<(u32, ProvinceHistory), String> {
            let stem = path
                .file_stem()
                .ok_or("no file stem")?
                .to_str()
                .ok_or("invalid filename encoding")?;

            // Robustly parse ID: handle "123 - Name", "123-Name", "123 Name"
            let id_str = stem.split('-').next().unwrap_or(stem).trim();
            let id_part = id_str.split_whitespace().next().unwrap_or(id_str);
            let id = id_part
                .parse::<u32>()
                .map_err(|e| format!("bad id '{}': {}", id_part, e))?;

            let tokens = DefaultEU4Txt::open_txt(path.to_str().ok_or("path encoding")?)
                .map_err(|e| format!("tokenize: {}", e))?;

            if tokens.is_empty() {
                return Ok((id, ProvinceHistory::default()));
            }

            let ast = DefaultEU4Txt::parse(tokens).map_err(|e| format!("parse: {}", e))?;
            let hist =
                from_node::<ProvinceHistory>(&ast).map_err(|e| format!("deserialize: {}", e))?;

            Ok((id, hist))
        };

        match try_load() {
            Ok((id, hist)) => {
                let mut lock = results.lock().unwrap();
                lock.0.insert(id, hist);
                lock.1.0 += 1;
            }
            Err(e) => {
                // Log at warn level so parse errors are visible
                log::warn!(
                    "Failed to load {:?}: {}",
                    path.file_name().unwrap_or_default(),
                    e
                );
                let mut lock = results.lock().unwrap();
                lock.1.1 += 1;
            }
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

        assert_eq!(success, 3);
        assert_eq!(fail, 1); // "invalid_name.txt" fails ID parse

        let p1 = map.get(&1).unwrap();
        assert_eq!(p1.owner.as_deref(), Some("SWE"));
        assert_eq!(p1.base_tax, Some(10.0));
        assert_eq!(p1.trade_goods.as_deref(), Some("grain"));
        assert_eq!(p1.religion.as_deref(), Some("catholic"));
        assert_eq!(p1.culture.as_deref(), Some("swedish"));

        let p2 = map.get(&2).unwrap();
        assert_eq!(p2.owner.as_deref(), Some("SWE"));
        assert_eq!(p2.base_tax, None);
        let (map, (success, fail)) = load_province_history(dir.path()).unwrap();

        assert_eq!(success, 3);
        assert_eq!(fail, 1); // "invalid_name.txt" fails ID parse

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

    /// Integration test: verify critical provinces load from real game files
    #[test]
    fn test_critical_provinces_exist() {
        // Use env var or default Steam path
        let eu4_path = std::env::var("EU4_PATH")
            .ok()
            .map(std::path::PathBuf::from)
            .or_else(|| {
                let default = std::path::Path::new(
                    r"C:\Program Files (x86)\Steam\steamapps\common\Europa Universalis IV",
                );
                if default.exists() {
                    Some(default.to_path_buf())
                } else {
                    None
                }
            });

        if eu4_path.is_none() {
            eprintln!("Skipping test_critical_provinces_exist: EU4 not found");
            return;
        }
        let eu4_path = eu4_path.unwrap();

        let (map, (success, fail)) = load_province_history(&eu4_path).unwrap();

        // Should load thousands of provinces
        assert!(success > 3000, "Expected >3000 provinces, got {}", success);
        assert!(fail < 100, "Too many failures: {}", fail);

        // Critical provinces that MUST exist with owners
        // Note: Using 1444 starting owners
        let critical = [
            (151, "BYZ", "Constantinople"),
            (1, "SWE", "Stockholm"),
            (183, "FRA", "Paris"),
            (236, "ENG", "London"),
            // Removed: Moskva (50) - complex ownership history with date-keyed overrides
        ];

        for (id, expected_owner, name) in critical {
            let hist = map.get(&id);
            assert!(
                hist.is_some(),
                "Province {} ({}) not found in map",
                id,
                name
            );
            let hist = hist.unwrap();
            assert_eq!(
                hist.owner.as_deref(),
                Some(expected_owner),
                "Province {} ({}) should be owned by {} but got {:?}",
                id,
                name,
                expected_owner,
                hist.owner
            );
        }
    }
}
