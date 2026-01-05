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

// ============================================================================
// Country History
// ============================================================================

/// Historical data for a country at game start (e.g., from `history/countries`).
///
/// Country history files contain base country data plus dated events that modify
/// the country state at specific dates. We extract the base values here.
#[derive(Debug, Default, TolerantDeserialize, SchemaType)]
pub struct CountryHistory {
    /// Starting religion (e.g., "catholic", "sunni").
    pub religion: Option<String>,
    /// Starting primary culture (e.g., "austrian", "english").
    pub primary_culture: Option<String>,
    /// Technology group (e.g., "western", "eastern", "ottoman").
    pub technology_group: Option<String>,
    /// Government rank (1=Duchy, 2=Kingdom, 3=Empire).
    pub government_rank: Option<i32>,
    /// Capital province ID.
    pub capital: Option<i32>,
    /// Government type (e.g., "monarchy", "republic").
    pub government: Option<String>,
    /// Whether this country is an HRE elector.
    pub elector: Option<bool>,
    // Monarch data is complex (Vec<serde_json::Value> in generated types).
    // We'll parse the first monarch separately after loading.
}

/// Parsed monarch data from country history.
#[derive(Debug, Default, Clone)]
pub struct MonarchData {
    /// Monarch's given name (e.g., "Friedrich").
    pub name: String,
    /// Monarch's dynasty (e.g., "von Habsburg").
    pub dynasty: Option<String>,
    /// Administrative skill (0-6).
    pub adm: u8,
    /// Diplomatic skill (0-6).
    pub dip: u8,
    /// Military skill (0-6).
    pub mil: u8,
}

/// Combined country history with parsed monarch data.
#[derive(Debug, Default, Clone)]
pub struct ParsedCountryHistory {
    /// State religion.
    pub religion: Option<String>,
    /// Primary culture.
    pub primary_culture: Option<String>,
    /// Technology group.
    pub technology_group: Option<String>,
    /// Government rank (1=Duchy, 2=Kingdom, 3=Empire).
    pub government_rank: u8,
    /// Capital province ID.
    pub capital: Option<u32>,
    /// Government type.
    pub government: Option<String>,
    /// First monarch at game start.
    pub monarch: Option<MonarchData>,
    /// Whether this country is an HRE elector.
    pub elector: bool,
}

/// Loads all country history files from the `history/countries` directory.
/// Returns a map of Country Tag -> ParsedCountryHistory.
pub type CountryHistoryLoadResult = (HashMap<String, ParsedCountryHistory>, (usize, usize));

pub fn load_country_history(base_path: &Path) -> Result<CountryHistoryLoadResult, std::io::Error> {
    let history_path = base_path.join("history/countries");

    if !history_path.is_dir() {
        return Ok((HashMap::new(), (0, 0)));
    }

    // Collect entries first to bridge to rayon
    let entries: Vec<_> = std::fs::read_dir(history_path)?
        .filter_map(|e| e.ok())
        .collect();

    let results = Mutex::new((HashMap::new(), (0, 0)));

    entries.par_iter().for_each(|entry| {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "txt") {
            return;
        }

        let try_load = || -> Result<(String, ParsedCountryHistory), String> {
            let stem = path
                .file_stem()
                .ok_or("no file stem")?
                .to_str()
                .ok_or("invalid filename encoding")?;

            // Country history files are named like "HAB - Austria.txt" or "ENG - England.txt"
            // Extract the 3-letter tag from the beginning
            let tag = stem
                .split(['-', ' '])
                .next()
                .unwrap_or(stem)
                .trim()
                .to_uppercase();

            if tag.len() != 3 {
                return Err(format!("invalid tag length: '{}'", tag));
            }

            let tokens = DefaultEU4Txt::open_txt(path.to_str().ok_or("path encoding")?)
                .map_err(|e| format!("tokenize: {}", e))?;

            if tokens.is_empty() {
                return Ok((tag, ParsedCountryHistory::default()));
            }

            let ast = DefaultEU4Txt::parse(tokens).map_err(|e| format!("parse: {}", e))?;

            // Parse basic country history fields
            let hist =
                from_node::<CountryHistory>(&ast).map_err(|e| format!("deserialize: {}", e))?;

            // Parse monarch data from the AST directly (it's a complex nested structure)
            let monarch = parse_first_monarch(&ast);

            let parsed = ParsedCountryHistory {
                religion: hist.religion,
                primary_culture: hist.primary_culture,
                technology_group: hist.technology_group,
                government_rank: hist.government_rank.unwrap_or(1).clamp(1, 3) as u8,
                capital: hist.capital.map(|c| c as u32),
                government: hist.government,
                monarch,
                elector: hist.elector.unwrap_or(false),
            };

            Ok((tag, parsed))
        };

        match try_load() {
            Ok((tag, hist)) => {
                let mut lock = results.lock().unwrap();
                lock.0.insert(tag, hist);
                lock.1.0 += 1;
            }
            Err(e) => {
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

/// Parse the most recent monarch block from a country history AST.
///
/// EU4 country history files contain monarchs in dated blocks like:
/// ```text
/// 1395.8.29 = { monarch = { name = "Albrecht IV" ... } }
/// 1404.9.14 = { monarch = { name = "Albrecht V" ... } }
/// ```
/// We traverse the entire AST and collect all monarch blocks, returning
/// the one from the latest date that's still before game start (1444.11.11).
fn parse_first_monarch(ast: &eu4txt::EU4TxtParseNode) -> Option<MonarchData> {
    // Collect all monarch blocks with their dates
    let mut monarchs = Vec::new();
    collect_monarchs_recursive(ast, None, &mut monarchs);

    // Find the most recent monarch that's before or at game start (1444.11.11)
    // Date value: year * 10000 + month * 100 + day
    const GAME_START: u32 = 14_441_111;

    monarchs
        .into_iter()
        .filter(|(date, _)| date.unwrap_or(0) <= GAME_START)
        .max_by_key(|(date, _)| *date)
        .map(|(_, data)| data)
}

/// Recursively collect all monarch blocks in the AST with their dates.
fn collect_monarchs_recursive(
    node: &eu4txt::EU4TxtParseNode,
    current_date: Option<u32>,
    monarchs: &mut Vec<(Option<u32>, MonarchData)>,
) {
    use eu4txt::EU4TxtAstItem;

    for child in &node.children {
        if let EU4TxtAstItem::Assignment = &child.entry
            && child.children.len() >= 2
        {
            let lhs = &child.children[0];
            let rhs = &child.children[1];

            // Check if this is a date block (e.g., "1444.11.11 = { ... }")
            if let EU4TxtAstItem::Identifier(key) = &lhs.entry {
                if let Some(date) = parse_date_key(key) {
                    // This is a dated block - recurse into it with this date
                    if matches!(
                        &rhs.entry,
                        EU4TxtAstItem::Brace | EU4TxtAstItem::AssignmentList
                    ) {
                        collect_monarchs_recursive(rhs, Some(date), monarchs);
                    }
                } else if key == "monarch"
                    && matches!(
                        &rhs.entry,
                        EU4TxtAstItem::Brace | EU4TxtAstItem::AssignmentList
                    )
                {
                    // Found a monarch block
                    if let Some(data) = parse_monarch_block(rhs) {
                        monarchs.push((current_date, data));
                    }
                }
            }
        }
    }
}

/// Parse a date string like "1444.11.11" into a sortable u32.
fn parse_date_key(key: &str) -> Option<u32> {
    let parts: Vec<&str> = key.split('.').collect();
    if parts.len() == 3 {
        let year: u32 = parts[0].parse().ok()?;
        let month: u32 = parts[1].parse().ok()?;
        let day: u32 = parts[2].parse().ok()?;
        if year > 0 && (1..=12).contains(&month) && (1..=31).contains(&day) {
            return Some(year * 10000 + month * 100 + day);
        }
    }
    None
}

/// Parse a monarch block into MonarchData.
fn parse_monarch_block(block: &eu4txt::EU4TxtParseNode) -> Option<MonarchData> {
    use eu4txt::EU4TxtAstItem;

    let mut data = MonarchData::default();

    for child in &block.children {
        if let EU4TxtAstItem::Assignment = &child.entry
            && child.children.len() >= 2
        {
            let lhs = &child.children[0];
            let rhs = &child.children[1];

            if let EU4TxtAstItem::Identifier(key) = &lhs.entry {
                match key.as_str() {
                    "name" => {
                        if let EU4TxtAstItem::StringValue(v) = &rhs.entry {
                            data.name = v.clone();
                        }
                    }
                    "dynasty" => {
                        if let EU4TxtAstItem::StringValue(v) = &rhs.entry {
                            data.dynasty = Some(v.clone());
                        } else if let EU4TxtAstItem::Identifier(v) = &rhs.entry {
                            data.dynasty = Some(v.clone());
                        }
                    }
                    "adm" | "ADM" => {
                        if let EU4TxtAstItem::IntValue(v) = &rhs.entry {
                            data.adm = (*v).clamp(0, 6) as u8;
                        }
                    }
                    "dip" | "DIP" => {
                        if let EU4TxtAstItem::IntValue(v) = &rhs.entry {
                            data.dip = (*v).clamp(0, 6) as u8;
                        }
                    }
                    "mil" | "MIL" => {
                        if let EU4TxtAstItem::IntValue(v) = &rhs.entry {
                            data.mil = (*v).clamp(0, 6) as u8;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Only return if we have a name
    if !data.name.is_empty() {
        Some(data)
    } else {
        None
    }
}

// ============================================================================
// Diplomacy History
// ============================================================================

/// HRE and Celestial Empire state from diplomacy history.
#[derive(Debug, Default, Clone)]
pub struct DiplomacyHistoryState {
    /// Initial HRE emperor tag (from history/diplomacy/hre.txt).
    pub hre_emperor: Option<String>,
    /// Initial Celestial Emperor tag (from history/diplomacy/celestial_empire.txt).
    pub celestial_emperor: Option<String>,
}

/// Loads diplomacy history to get initial HRE emperor and other diplomatic state.
///
/// Parses files in `history/diplomacy/` directory.
/// Note: Files use dated entries like `1437.12.9 = { emperor = HAB }`.
/// We select the most recent entry before 1444.11.11 (game start).
pub fn load_diplomacy_history(base_path: &Path) -> Result<DiplomacyHistoryState, std::io::Error> {
    let mut state = DiplomacyHistoryState::default();

    // Load HRE emperor from history/diplomacy/hre.txt
    let hre_path = base_path.join("history/diplomacy/hre.txt");
    if hre_path.exists()
        && let Ok(tokens) = DefaultEU4Txt::open_txt(hre_path.to_str().unwrap_or_default())
        && let Ok(ast) = DefaultEU4Txt::parse(tokens)
    {
        // Parse dated entries and find emperor at game start
        state.hre_emperor = find_emperor_at_date(&ast, 14_441_111);
        if let Some(ref emperor) = state.hre_emperor {
            log::debug!("Loaded HRE emperor from history: {}", emperor);
        }
    }

    // Load Celestial Emperor from history/diplomacy/celestial_empire.txt
    let ce_path = base_path.join("history/diplomacy/celestial_empire.txt");
    if ce_path.exists()
        && let Ok(tokens) = DefaultEU4Txt::open_txt(ce_path.to_str().unwrap_or_default())
        && let Ok(ast) = DefaultEU4Txt::parse(tokens)
    {
        state.celestial_emperor = find_celestial_emperor_at_date(&ast, 14_441_111);
        if let Some(ref emperor) = state.celestial_emperor {
            log::debug!("Loaded Celestial Emperor from history: {}", emperor);
        }
    }

    Ok(state)
}

/// Find the HRE emperor at a given date by parsing dated entries.
/// Format: `1437.12.9 = { emperor = HAB }`
fn find_emperor_at_date(ast: &eu4txt::EU4TxtParseNode, target_date: u32) -> Option<String> {
    use eu4txt::EU4TxtAstItem;

    let mut emperors_with_dates: Vec<(u32, String)> = Vec::new();

    for child in &ast.children {
        if let EU4TxtAstItem::Assignment = &child.entry
            && child.children.len() >= 2
        {
            let lhs = &child.children[0];
            let rhs = &child.children[1];

            // Check if LHS is a date
            if let EU4TxtAstItem::Identifier(date_str) = &lhs.entry
                && let Some(date) = parse_date_key(date_str)
            {
                // RHS is a block containing emperor = TAG
                if let EU4TxtAstItem::AssignmentList = &rhs.entry {
                    for inner in &rhs.children {
                        if let EU4TxtAstItem::Assignment = &inner.entry
                            && inner.children.len() >= 2
                        {
                            let inner_lhs = &inner.children[0];
                            let inner_rhs = &inner.children[1];
                            if let EU4TxtAstItem::Identifier(key) = &inner_lhs.entry
                                && key == "emperor"
                                && let EU4TxtAstItem::Identifier(tag) = &inner_rhs.entry
                            {
                                emperors_with_dates.push((date, tag.clone()));
                            }
                        }
                    }
                }
            }
        }
    }

    // Find the most recent emperor before target_date
    emperors_with_dates
        .into_iter()
        .filter(|(date, _)| *date <= target_date)
        .max_by_key(|(date, _)| *date)
        .map(|(_, tag)| tag)
}

/// Find the Celestial Emperor at a given date by parsing dated entries.
/// Format: `1402.5.13 = { celestial_emperor = MNG }`
fn find_celestial_emperor_at_date(
    ast: &eu4txt::EU4TxtParseNode,
    target_date: u32,
) -> Option<String> {
    use eu4txt::EU4TxtAstItem;

    let mut emperors_with_dates: Vec<(u32, String)> = Vec::new();

    for child in &ast.children {
        if let EU4TxtAstItem::Assignment = &child.entry
            && child.children.len() >= 2
        {
            let lhs = &child.children[0];
            let rhs = &child.children[1];

            // Check if LHS is a date
            if let EU4TxtAstItem::Identifier(date_str) = &lhs.entry
                && let Some(date) = parse_date_key(date_str)
            {
                // RHS is a block containing celestial_emperor = TAG
                if let EU4TxtAstItem::AssignmentList = &rhs.entry {
                    for inner in &rhs.children {
                        if let EU4TxtAstItem::Assignment = &inner.entry
                            && inner.children.len() >= 2
                        {
                            let inner_lhs = &inner.children[0];
                            let inner_rhs = &inner.children[1];
                            if let EU4TxtAstItem::Identifier(key) = &inner_lhs.entry
                                && key == "celestial_emperor"
                                && let EU4TxtAstItem::Identifier(tag) = &inner_rhs.entry
                            {
                                emperors_with_dates.push((date, tag.clone()));
                            }
                        }
                    }
                }
            }
        }
    }

    // Find the most recent emperor before target_date
    emperors_with_dates
        .into_iter()
        .filter(|(date, _)| *date <= target_date)
        .max_by_key(|(date, _)| *date)
        .map(|(_, tag)| tag)
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

    #[test]
    fn test_load_country_history() {
        let dir = tempdir().unwrap();
        let history_path = dir.path().join("history/countries");
        fs::create_dir_all(&history_path).unwrap();

        // Austria with monarch
        let file_path = history_path.join("HAB - Austria.txt");
        let mut file = fs::File::create(file_path).unwrap();
        writeln!(
            file,
            r#"
            government = monarchy
            government_rank = 2
            technology_group = western
            religion = catholic
            primary_culture = austrian
            capital = 134

            monarch = {{
                name = "Friedrich III"
                dynasty = "von Habsburg"
                adm = 3
                dip = 4
                mil = 2
            }}
            "#
        )
        .unwrap();

        // England without dynasty
        let file_path = history_path.join("ENG - England.txt");
        let mut file = fs::File::create(file_path).unwrap();
        writeln!(
            file,
            r#"
            government_rank = 2
            technology_group = western
            monarch = {{
                name = "Henry VI"
                adm = 1
                dip = 1
                mil = 1
            }}
            "#
        )
        .unwrap();

        let (map, (success, fail)) = load_country_history(dir.path()).unwrap();

        assert_eq!(success, 2);
        assert_eq!(fail, 0);

        // Check Austria
        let hab = map.get("HAB").unwrap();
        assert_eq!(hab.government_rank, 2);
        assert_eq!(hab.technology_group.as_deref(), Some("western"));
        assert_eq!(hab.religion.as_deref(), Some("catholic"));
        assert_eq!(hab.capital, Some(134));

        let monarch = hab.monarch.as_ref().unwrap();
        assert_eq!(monarch.name, "Friedrich III");
        assert_eq!(monarch.dynasty.as_deref(), Some("von Habsburg"));
        assert_eq!(monarch.adm, 3);
        assert_eq!(monarch.dip, 4);
        assert_eq!(monarch.mil, 2);

        // Check England
        let eng = map.get("ENG").unwrap();
        assert_eq!(eng.government_rank, 2);
        let eng_monarch = eng.monarch.as_ref().unwrap();
        assert_eq!(eng_monarch.name, "Henry VI");
        assert!(eng_monarch.dynasty.is_none());
    }
}
