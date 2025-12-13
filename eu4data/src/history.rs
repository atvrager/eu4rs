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
