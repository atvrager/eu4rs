use std::collections::HashMap;
use std::path::Path;

/// Categories of EU4 game data we track
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataCategory {
    Countries,       // common/countries/, common/country_tags/
    Religions,       // common/religions/
    Cultures,        // common/cultures/
    TradeGoods,      // common/tradegoods/
    ProvinceHistory, // history/provinces/
    Map,             // map/ (definitions, default, terrain)
}

impl DataCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            DataCategory::Countries => "Countries",
            DataCategory::Religions => "Religions",
            DataCategory::Cultures => "Cultures",
            DataCategory::TradeGoods => "Trade Goods",
            DataCategory::ProvinceHistory => "Province History",
            DataCategory::Map => "Map Data",
        }
    }

    pub fn path_suffix(&self) -> &'static str {
        match self {
            DataCategory::Countries => "common/countries",
            DataCategory::Religions => "common/religions",
            DataCategory::Cultures => "common/cultures",
            DataCategory::TradeGoods => "common/tradegoods",
            DataCategory::ProvinceHistory => "history/provinces",
            DataCategory::Map => "map",
        }
    }
}

/// Coverage status for a single field within a struct
#[derive(Debug, Clone)]
pub struct FieldCoverage {
    pub name: &'static str,
    pub parsed: bool,                // Do we have this in our struct?
    pub used: bool,                  // Is it accessed somewhere in code?
    pub notes: Option<&'static str>, // Why is it missing/unused?
}

/// Coverage status for an entire data category
#[derive(Debug)]
pub struct CategoryCoverage {
    pub category: DataCategory,
    pub game_files: usize,   // Files found in EU4 install
    pub parsed_files: usize, // Files we successfully parse
    pub total_fields: usize, // Estimated total distinct fields in this category
    pub our_fields: usize,   // Fields we define in structs
    pub used_fields: usize,  // Fields actually referenced in code
    pub details: Vec<FieldCoverage>,
}

/// The complete coverage report
pub struct CoverageReport {
    pub categories: Vec<CategoryCoverage>,
    pub timestamp: String,
}

impl CoverageReport {
    pub fn to_terminal(&self) -> String {
        let mut output = String::new();
        output.push_str("📊 EU4 Data Coverage Report\n");
        output.push_str("==========================\n\n");

        for cat in &self.categories {
            let coverage_pct = if cat.total_fields > 0 {
                (cat.our_fields as f64 / cat.total_fields as f64) * 100.0
            } else {
                0.0
            };

            let bar_len = 20;
            let filled_len = (coverage_pct / 100.0 * bar_len as f64).round() as usize;
            let bar: String = "█".repeat(filled_len) + &"░".repeat(bar_len - filled_len);

            output.push_str(&format!(
                "{:<20} {} {:>5.1}% (Parsed: {}/{})\n",
                cat.category.as_str(),
                bar,
                coverage_pct,
                cat.our_fields,
                cat.total_fields
            ));
        }

        output
    }
}

/// Generates the static "Supported Fields" documentation.
/// This depends ONLY on the code definitions, not on any game files.
pub fn generate_static_docs() -> String {
    let mut output = String::new();
    output.push_str("# EU4 Data Support Matrix\n\n");
    output.push_str("This document is auto-generated from `eu4data/src/coverage.rs`. **Do not edit manually.**\n");
    output
        .push_str("It defines which EU4 data fields are currently parsed and used by `eu4rs`.\n\n");

    let registry = get_gold_standard_registry();
    let mut categories: Vec<_> = registry.keys().collect();
    categories.sort_by_key(|c| c.as_str());

    for cat in categories {
        // Summary for category
        let fields = &registry[cat];
        let total = fields.len();
        let parsed = fields.iter().filter(|f| f.parsed).count();
        let used = fields.iter().filter(|f| f.used).count();

        output.push_str(&format!("## {}\n\n", cat.as_str()));
        output.push_str(&format!("- **Total Known Fields:** {}\n", total));
        output.push_str(&format!(
            "- **Parsed:** {} ({:.1}%)\n",
            parsed,
            (parsed as f64 / total as f64) * 100.0
        ));
        output.push_str(&format!(
            "- **Used:** {} ({:.1}%)\n\n",
            used,
            (used as f64 / total as f64) * 100.0
        ));

        output.push_str("| Field | Parsed | Used | Notes |\n");
        output.push_str("|-------|--------|------|-------|\n");

        for field in fields {
            output.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                field.name,
                if field.parsed { "✅" } else { "❌" },
                if field.used { "✅" } else { "-" },
                field.notes.unwrap_or("")
            ));
        }
        output.push('\n');
    }

    output
}

/// Returns the "Gold Standard" registry of fields for each category.
/// This includes what we *should* have support for.
pub fn get_gold_standard_registry() -> HashMap<DataCategory, Vec<FieldCoverage>> {
    let mut registry = HashMap::new();

    // 1. Countries (common/countries/xxx.txt)
    registry.insert(
        DataCategory::Countries,
        vec![
            FieldCoverage {
                name: "color",
                parsed: true,
                used: true,
                notes: Some("Essential for political map"),
            },
            FieldCoverage {
                name: "graphical_culture",
                parsed: false,
                used: false,
                notes: Some("For unit models and city graphics"),
            },
            FieldCoverage {
                name: "historical_idea_groups",
                parsed: false,
                used: false,
                notes: None,
            },
            FieldCoverage {
                name: "historical_units",
                parsed: false,
                used: false,
                notes: None,
            },
            FieldCoverage {
                name: "monarch_names",
                parsed: false,
                used: false,
                notes: None,
            },
            FieldCoverage {
                name: "leader_names",
                parsed: false,
                used: false,
                notes: None,
            },
            FieldCoverage {
                name: "ship_names",
                parsed: false,
                used: false,
                notes: None,
            },
            FieldCoverage {
                name: "army_names",
                parsed: false,
                used: false,
                notes: None,
            },
            FieldCoverage {
                name: "fleet_names",
                parsed: false,
                used: false,
                notes: None,
            },
        ],
    );

    // 2. Province History (history/provinces/xxx.txt)
    registry.insert(
        DataCategory::ProvinceHistory,
        vec![
            FieldCoverage {
                name: "owner",
                parsed: true,
                used: true,
                notes: Some("Political map ownership"),
            },
            FieldCoverage {
                name: "controller",
                parsed: false,
                used: false,
                notes: Some("Wartime occupation"),
            },
            FieldCoverage {
                name: "add_core",
                parsed: false,
                used: false,
                notes: None,
            },
            FieldCoverage {
                name: "culture",
                parsed: true,
                used: true,
                notes: Some("Culture map mode"),
            },
            FieldCoverage {
                name: "religion",
                parsed: true,
                used: true,
                notes: Some("Religion map mode"),
            },
            FieldCoverage {
                name: "base_tax",
                parsed: true,
                used: false,
                notes: Some("Parsed but not visualized yet"),
            },
            FieldCoverage {
                name: "base_production",
                parsed: true,
                used: false,
                notes: None,
            },
            FieldCoverage {
                name: "base_manpower",
                parsed: true,
                used: false,
                notes: None,
            },
            FieldCoverage {
                name: "trade_goods",
                parsed: true,
                used: true,
                notes: Some("Trade goods map mode"),
            },
            FieldCoverage {
                name: "capital",
                parsed: false,
                used: false,
                notes: Some("Province capital name"),
            },
            FieldCoverage {
                name: "is_city",
                parsed: false,
                used: false,
                notes: None,
            },
            FieldCoverage {
                name: "hre", // HRE membership
                parsed: false,
                used: false,
                notes: None,
            },
            FieldCoverage {
                name: "discovered_by",
                parsed: false,
                used: false,
                notes: None,
            },
        ],
    );

    // 3. Trade Goods (common/tradegoods/xxx.txt)
    registry.insert(
        DataCategory::TradeGoods,
        vec![
            FieldCoverage {
                name: "color",
                parsed: true,
                used: true,
                notes: Some("Map color"),
            },
            FieldCoverage {
                name: "modifier",
                parsed: true,
                used: false,
                notes: Some("Production bonuses"),
            },
            FieldCoverage {
                name: "province",
                parsed: true,
                used: false,
                notes: Some("Province scope modifiers"),
            },
            FieldCoverage {
                name: "chance",
                parsed: true,
                used: false,
                notes: Some("Spawn chance (scripted)"),
            },
            FieldCoverage {
                name: "base_price",
                parsed: false,
                used: false,
                notes: None,
            },
            FieldCoverage {
                name: "gold_type", // boolean
                parsed: false,
                used: false,
                notes: None,
            },
        ],
    );

    // 4. Religions (common/religions/xxx.txt)
    // Note: Religions are nested in groups, but individual religions have fields
    registry.insert(
        DataCategory::Religions,
        vec![
            FieldCoverage {
                name: "color",
                parsed: true, // Indirectly via RGBA
                used: true,
                notes: Some("Map color"),
            },
            FieldCoverage {
                name: "icon",
                parsed: true,
                used: false,
                notes: None,
            },
            FieldCoverage {
                name: "allowed_conversion",
                parsed: false,
                used: false,
                notes: None,
            },
            FieldCoverage {
                name: "country",
                parsed: false,
                used: false,
                notes: Some("Country modifiers"),
            },
            FieldCoverage {
                name: "province",
                parsed: false,
                used: false,
                notes: Some("Province modifiers"),
            },
            FieldCoverage {
                name: "heretic",
                parsed: false, // List of heretic religions
                used: false,
                notes: None,
            },
        ],
    );

    // 5. Cultures (common/cultures/xxx.txt)
    registry.insert(
        DataCategory::Cultures,
        vec![
            FieldCoverage {
                name: "primary",
                parsed: false, // Tag of primary nation
                used: false,
                notes: None,
            },
            FieldCoverage {
                name: "dynasty_names",
                parsed: false,
                used: false,
                notes: None,
            },
            FieldCoverage {
                name: "male_names",
                parsed: false,
                used: false,
                notes: None,
            },
            FieldCoverage {
                name: "female_names",
                parsed: false,
                used: false,
                notes: None,
            },
        ],
    );

    registry
}

use eu4txt::{DefaultEU4Txt, EU4Txt};
use rayon::prelude::*;
use walkdir::WalkDir;

/// Main entry point: scan EU4 directory and produce coverage report
pub fn analyze_coverage(eu4_path: &Path) -> Result<CoverageReport, std::io::Error> {
    let mut categories = Vec::new();
    let registry = get_gold_standard_registry();
    let now = std::time::SystemTime::now();
    let timestamp = humantime::format_rfc3339(now).to_string();

    // Iterate over our defined categories
    let cats = vec![
        DataCategory::Countries,
        DataCategory::ProvinceHistory,
        DataCategory::TradeGoods,
        DataCategory::Religions,
        DataCategory::Cultures,
    ];

    for cat in cats {
        let fields = registry.get(&cat).cloned().unwrap_or_default();
        let our_fields = fields.iter().filter(|f| f.parsed).count();
        let used_fields = fields.iter().filter(|f| f.used).count();
        let total_fields = fields.len();

        let dir_path = eu4_path.join(cat.path_suffix());
        let game_files = count_txt_files(&dir_path);
        let parsed_files = count_parsable_files(&dir_path);

        categories.push(CategoryCoverage {
            category: cat,
            game_files,
            parsed_files,
            total_fields,
            our_fields,
            used_fields,
            details: fields,
        });
    }

    Ok(CoverageReport {
        categories,
        timestamp,
    })
}

fn count_txt_files(path: &Path) -> usize {
    if !path.exists() {
        return 0;
    }

    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "txt"))
        .count()
}

fn count_parsable_files(path: &Path) -> usize {
    if !path.exists() {
        return 0;
    }

    let files: Vec<_> = WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "txt"))
        .collect();

    files
        .par_iter()
        .filter(|entry| {
            let path = entry.path();
            // Try to open and parse
            if let Ok(tokens) = DefaultEU4Txt::open_txt(path.to_str().unwrap_or_default()) {
                if tokens.is_empty() {
                    true // Empty files are considered "parsable" (no errors)
                } else {
                    DefaultEU4Txt::parse(tokens).is_ok()
                }
            } else {
                false
            }
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_analyze_coverage_mock() {
        let dir = tempdir().unwrap();
        let common = dir.path().join("common");
        let countries_dir = common.join("countries");
        let history_dir = dir.path().join("history/provinces");

        fs::create_dir_all(&countries_dir).unwrap();
        fs::create_dir_all(&history_dir).unwrap();

        // Create 2 country files
        fs::write(countries_dir.join("Sweden.txt"), "color = { 1 1 1 }").unwrap();
        // Empty file parses OK (it's just empty AST)
        fs::write(countries_dir.join("Denmark.txt"), "").unwrap();

        // Create 1 valid history file and 1 invalid
        let p1 = history_dir.join("1.txt");
        fs::write(&p1, "owner = SWE").unwrap();

        let p2 = history_dir.join("2.txt");
        // This fails tokenization or parsing (Missing RHS)
        fs::write(&p2, "key =").unwrap();

        let report = analyze_coverage(dir.path()).unwrap();

        // Find Countries category
        let country_cat = report
            .categories
            .iter()
            .find(|c| c.category == DataCategory::Countries)
            .unwrap();
        assert_eq!(country_cat.game_files, 2);
        assert_eq!(country_cat.parsed_files, 2); // Both valid (empty is valid)

        // Find ProvinceHistory category
        let hist_cat = report
            .categories
            .iter()
            .find(|c| c.category == DataCategory::ProvinceHistory)
            .unwrap();
        assert_eq!(hist_cat.game_files, 2);
        assert_eq!(hist_cat.parsed_files, 1);
    }
}
