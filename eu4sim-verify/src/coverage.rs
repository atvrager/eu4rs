//! Save file field coverage analysis
//!
//! Scans save files to discover all field names and compares against
//! what we currently extract, generating coverage reports.

use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Discovery result for a single field
#[derive(Debug, Clone)]
pub struct FieldDiscovery {
    /// Field name (e.g., "treasury", "base_tax")
    pub name: String,
    /// How many saves contain this field
    pub frequency: usize,
    /// Example value for type inference
    pub sample_value: Option<String>,
    /// Inferred type from sample values
    pub inferred_type: FieldType,
    /// Does it appear multiple times per save?
    pub appears_multiple: bool,
    /// Which category this field belongs to
    pub category: FieldCategory,
}

/// Inferred field type from sample values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    /// "yes" / "no"
    Bool,
    /// "123" (no decimal point)
    Integer,
    /// "1.234" (has decimal point)
    Float,
    /// "quoted string"
    String,
    /// "1444.11.11" (date format)
    Date,
    /// { 1 2 3 } (space-separated values)
    List,
    /// { key=value } (nested structure)
    Block,
    /// Could not determine
    Unknown,
}

impl std::fmt::Display for FieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldType::Bool => write!(f, "bool"),
            FieldType::Integer => write!(f, "int"),
            FieldType::Float => write!(f, "float"),
            FieldType::String => write!(f, "string"),
            FieldType::Date => write!(f, "date"),
            FieldType::List => write!(f, "list"),
            FieldType::Block => write!(f, "block"),
            FieldType::Unknown => write!(f, "?"),
        }
    }
}

/// Category of fields in save files
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldCategory {
    /// Top-level meta fields (date, player, etc.)
    Meta,
    /// Fields inside countries={...}
    Countries,
    /// Fields inside provinces={...}
    Provinces,
    /// Trade-related fields
    Trade,
    /// Army/navy units
    Military,
    /// Diplomatic relations
    Diplomacy,
    /// Other/unknown category
    Other,
}

impl std::fmt::Display for FieldCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldCategory::Meta => write!(f, "meta"),
            FieldCategory::Countries => write!(f, "countries"),
            FieldCategory::Provinces => write!(f, "provinces"),
            FieldCategory::Trade => write!(f, "trade"),
            FieldCategory::Military => write!(f, "military"),
            FieldCategory::Diplomacy => write!(f, "diplomacy"),
            FieldCategory::Other => write!(f, "other"),
        }
    }
}

/// Coverage status for a category
#[derive(Debug, Clone)]
pub struct CategoryCoverage {
    /// Category name
    pub category: FieldCategory,
    /// Fields found in saves
    pub discovered: usize,
    /// Fields we extract to ExtractedState
    pub extracted: usize,
    /// Coverage percentage (extracted / discovered)
    pub coverage_pct: f64,
    /// High-frequency fields we don't extract
    pub missing: Vec<FieldDiscovery>,
    /// Fields we do extract
    pub extracted_fields: Vec<String>,
}

/// Registry of fields we currently extract from saves
#[derive(Debug, Clone)]
pub struct ExtractedFieldRegistry {
    pub country_fields: HashSet<&'static str>,
    pub province_fields: HashSet<&'static str>,
}

impl Default for ExtractedFieldRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtractedFieldRegistry {
    /// Create registry from hardcoded list of extracted fields
    ///
    /// These fields match what we extract in parse.rs:
    /// - Country fields: parse_country_block()
    /// - Province fields: parse_province_block()
    pub fn new() -> Self {
        Self {
            country_fields: [
                // Core economic/military values
                "treasury",
                "manpower",
                "max_manpower",
                // Monarch power points
                "adm_power",
                "dip_power",
                "mil_power",
                // Advisors
                "advisors",
                "active_advisors",
            ]
            .into_iter()
            .collect(),
            province_fields: [
                // Ownership and identity
                "owner",
                "name",
                // Development
                "base_tax",
                "base_production",
                "base_manpower",
                // Modifiers
                "local_autonomy",
                // Buildings - extracted via extract_buildings()
                "marketplace",
                "trade_depot",
                "stock_exchange",
                "temple",
                "cathedral",
                "workshop",
                "counting_house",
                "barracks",
                "training_fields",
                "fort_15th",
                "fort_16th",
                "fort_17th",
                "fort_18th",
                "shipyard",
                "grand_shipyard",
                "dock",
                "drydock",
                "courthouse",
                "town_hall",
                "university",
            ]
            .into_iter()
            .collect(),
        }
    }

    /// Check if a field is extracted for its category
    pub fn is_extracted(&self, field: &str, category: FieldCategory) -> bool {
        match category {
            FieldCategory::Countries => self.country_fields.contains(field),
            FieldCategory::Provinces => self.province_fields.contains(field),
            _ => false,
        }
    }
}

/// Full coverage report across all categories
#[derive(Debug, Clone)]
pub struct CoverageReport {
    /// Number of save files scanned
    pub files_scanned: usize,
    /// Coverage by category
    pub categories: Vec<CategoryCoverage>,
    /// All discovered fields
    pub all_fields: HashMap<String, FieldDiscovery>,
}

impl CoverageReport {
    /// Calculate total coverage percentage
    pub fn total_coverage(&self) -> f64 {
        let total_discovered: usize = self.categories.iter().map(|c| c.discovered).sum();
        let total_extracted: usize = self.categories.iter().map(|c| c.extracted).sum();
        if total_discovered == 0 {
            0.0
        } else {
            100.0 * total_extracted as f64 / total_discovered as f64
        }
    }
}

/// Scan a text save file for all field names
pub fn scan_text_save(path: &Path) -> Result<HashMap<String, FieldDiscovery>> {
    let data =
        std::fs::read(path).with_context(|| format!("Failed to read: {}", path.display()))?;

    // Handle ZIP archives
    let text = if data.starts_with(b"PK") {
        let cursor = std::io::Cursor::new(&data);
        let mut archive = zip::ZipArchive::new(cursor)?;
        let mut gamestate = archive.by_name("gamestate")?;
        let mut content = Vec::new();
        std::io::Read::read_to_end(&mut gamestate, &mut content)?;

        // Check if binary
        if content.starts_with(b"EU4bin") {
            anyhow::bail!("Binary saves not yet supported for field scanning");
        }

        // Strip header if present
        let content = if content.starts_with(b"EU4txt") {
            &content[6..]
        } else {
            &content[..]
        };
        String::from_utf8_lossy(content).into_owned()
    } else if data.starts_with(b"EU4bin") {
        anyhow::bail!("Binary saves not yet supported for field scanning");
    } else {
        // Plain text
        let content = if data.starts_with(b"EU4txt") {
            &data[6..]
        } else {
            &data[..]
        };
        String::from_utf8_lossy(content).into_owned()
    };

    scan_text_content(&text)
}

/// Scan text content for field names
fn scan_text_content(text: &str) -> Result<HashMap<String, FieldDiscovery>> {
    let mut fields: HashMap<String, FieldDiscovery> = HashMap::new();

    // Track which section we're in using depth tracking
    // Section starts when we see "sectionname={" and ends when brace depth returns
    let mut section_category = FieldCategory::Meta;
    let mut section_start_depth = 0;
    let mut current_depth = 0;

    // Regex to match field=value patterns
    let field_re = regex::Regex::new(r"^\s*([a-z_][a-z_0-9]*)=(.*)$")?;

    // Track section boundaries
    for line in text.lines() {
        let trimmed = line.trim();

        // Check for section starts BEFORE counting braces
        // Top-level sections: countries={, provinces={, trade={, etc.
        if current_depth == 0 {
            if trimmed.starts_with("countries={") {
                section_category = FieldCategory::Countries;
                section_start_depth = 0;
            } else if trimmed.starts_with("provinces={") {
                section_category = FieldCategory::Provinces;
                section_start_depth = 0;
            } else if trimmed.starts_with("trade={") {
                section_category = FieldCategory::Trade;
                section_start_depth = 0;
            } else if trimmed.starts_with("diplomacy={")
                || trimmed.starts_with("active_war=")
                || trimmed.starts_with("previous_war=")
                || trimmed.starts_with("active_relations=")
            {
                section_category = FieldCategory::Diplomacy;
                section_start_depth = 0;
            }
        }

        // Count braces
        let open_count = trimmed.matches('{').count();
        let close_count = trimmed.matches('}').count();
        current_depth += open_count;
        current_depth = current_depth.saturating_sub(close_count);

        // Check if we've exited the section
        if current_depth <= section_start_depth && section_category != FieldCategory::Meta {
            section_category = FieldCategory::Meta;
        }

        // Extract field names from this line
        if let Some(caps) = field_re.captures(trimmed) {
            let field_name = caps.get(1).unwrap().as_str().to_string();
            let value = caps.get(2).map(|m| m.as_str().to_string());

            // Skip the section header keys themselves
            if matches!(
                field_name.as_str(),
                "countries" | "provinces" | "trade" | "diplomacy" | "active_relations"
            ) {
                continue;
            }

            // Skip very generic structural fields at top level
            if matches!(field_name.as_str(), "id" | "type")
                && section_category == FieldCategory::Meta
            {
                continue;
            }

            // Infer type from value
            let inferred_type = value
                .as_ref()
                .map(|v| infer_type(v))
                .unwrap_or(FieldType::Unknown);

            // Use a composite key for proper categorization
            let key = format!("{}:{}", section_category, field_name);

            let entry = fields.entry(key).or_insert_with(|| FieldDiscovery {
                name: field_name,
                frequency: 0,
                sample_value: value.clone(),
                inferred_type,
                appears_multiple: false,
                category: section_category,
            });

            entry.frequency += 1;
            if entry.frequency > 1 {
                entry.appears_multiple = true;
            }
        }
    }

    Ok(fields)
}

/// Infer field type from a sample value
fn infer_type(value: &str) -> FieldType {
    let value = value.trim();

    // Boolean
    if value == "yes" || value == "no" {
        return FieldType::Bool;
    }

    // String (quoted)
    if value.starts_with('"') && value.ends_with('"') {
        return FieldType::String;
    }

    // Block or list
    if value.starts_with('{') {
        // Check if it's a list (just values) or block (has key=)
        if value.contains('=') {
            return FieldType::Block;
        } else {
            return FieldType::List;
        }
    }

    // Date format (YYYY.M.D)
    if value.contains('.')
        && value
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
    {
        let parts: Vec<&str> = value.split('.').collect();
        if parts.len() == 3 && parts[0].len() == 4 {
            return FieldType::Date;
        }
    }

    // Number
    if value.chars().all(|c| c.is_ascii_digit() || c == '-') {
        return FieldType::Integer;
    }

    if value
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
    {
        return FieldType::Float;
    }

    FieldType::Unknown
}

/// Scan multiple save files and aggregate results
pub fn scan_saves(paths: &[&Path]) -> Result<CoverageReport> {
    let mut all_fields: HashMap<String, FieldDiscovery> = HashMap::new();
    let mut files_scanned = 0;

    for path in paths {
        log::info!("Scanning: {}", path.display());
        match scan_text_save(path) {
            Ok(fields) => {
                files_scanned += 1;
                for (key, discovery) in fields {
                    let entry = all_fields
                        .entry(key.clone())
                        .or_insert_with(|| FieldDiscovery {
                            name: discovery.name.clone(),
                            frequency: 0,
                            sample_value: discovery.sample_value.clone(),
                            inferred_type: discovery.inferred_type,
                            appears_multiple: discovery.appears_multiple,
                            category: discovery.category,
                        });
                    entry.frequency += 1;
                    if discovery.appears_multiple {
                        entry.appears_multiple = true;
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to scan {}: {}", path.display(), e);
            }
        }
    }

    // Generate coverage report
    let registry = ExtractedFieldRegistry::new();
    let categories = generate_category_coverage(&all_fields, &registry, files_scanned);

    Ok(CoverageReport {
        files_scanned,
        categories,
        all_fields,
    })
}

/// Generate coverage by category
fn generate_category_coverage(
    fields: &HashMap<String, FieldDiscovery>,
    registry: &ExtractedFieldRegistry,
    total_saves: usize,
) -> Vec<CategoryCoverage> {
    let mut by_category: HashMap<FieldCategory, Vec<&FieldDiscovery>> = HashMap::new();

    for field in fields.values() {
        by_category.entry(field.category).or_default().push(field);
    }

    let mut result = Vec::new();

    for category in [
        FieldCategory::Meta,
        FieldCategory::Countries,
        FieldCategory::Provinces,
        FieldCategory::Trade,
        FieldCategory::Military,
        FieldCategory::Diplomacy,
        FieldCategory::Other,
    ] {
        let cat_fields = by_category
            .get(&category)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        let discovered = cat_fields.len();
        let mut extracted = 0;
        let mut extracted_fields = Vec::new();
        let mut missing = Vec::new();

        for field in cat_fields {
            // Check if this field is extracted (using just the field name, not the composite key)
            if registry.is_extracted(&field.name, category) {
                extracted += 1;
                extracted_fields.push(field.name.clone());
            } else if field.frequency >= total_saves.max(2) / 2 {
                // High-frequency missing field (appears in at least half of saves)
                missing.push((*field).clone());
            }
        }

        // Sort missing by frequency (descending)
        missing.sort_by(|a, b| b.frequency.cmp(&a.frequency));
        missing.truncate(10); // Top 10 missing

        let coverage_pct = if discovered == 0 {
            0.0
        } else {
            100.0 * extracted as f64 / discovered as f64
        };

        result.push(CategoryCoverage {
            category,
            discovered,
            extracted,
            coverage_pct,
            missing,
            extracted_fields,
        });
    }

    result
}

/// Print coverage report to terminal
pub fn print_report(report: &CoverageReport, verbose: bool) {
    println!();
    println!("=== Save Field Coverage ===");
    println!("Scanned: {} save files", report.files_scanned);
    println!();
    println!(
        "{:16} {:>10}  {:>9}  {:>8}",
        "Category", "Discovered", "Extracted", "Coverage"
    );
    println!("{}", "─".repeat(50));

    for cat in &report.categories {
        if cat.discovered > 0 {
            println!(
                "{:16} {:>10}  {:>9}  {:>7.1}%",
                cat.category.to_string(),
                cat.discovered,
                cat.extracted,
                cat.coverage_pct
            );
        }
    }

    let total_discovered: usize = report.categories.iter().map(|c| c.discovered).sum();
    let total_extracted: usize = report.categories.iter().map(|c| c.extracted).sum();

    println!("{}", "─".repeat(50));
    println!(
        "{:16} {:>10}  {:>9}  {:>7.1}%",
        "TOTAL",
        total_discovered,
        total_extracted,
        report.total_coverage()
    );

    // Show missing fields per category
    for cat in &report.categories {
        if !cat.missing.is_empty() {
            println!();
            println!("High-frequency missing fields ({}):", cat.category);
            for field in &cat.missing {
                println!(
                    "  x {:24} (freq: {}/{}, type: {})",
                    &field.name, field.frequency, report.files_scanned, field.inferred_type
                );
            }
        }
    }

    // Show extracted fields
    for cat in &report.categories {
        if !cat.extracted_fields.is_empty() {
            println!();
            println!("Extracted fields ({}):", cat.category);
            for field in &cat.extracted_fields {
                println!("  + {}", field);
            }
        }
    }

    if verbose {
        println!();
        println!("=== All Discovered Fields ===");
        let mut sorted_fields: Vec<_> = report.all_fields.values().collect();
        sorted_fields.sort_by(|a, b| {
            a.category
                .to_string()
                .cmp(&b.category.to_string())
                .then(b.frequency.cmp(&a.frequency))
        });

        let mut current_cat = None;
        for field in sorted_fields {
            if current_cat != Some(field.category) {
                println!();
                println!("--- {} ---", field.category);
                current_cat = Some(field.category);
            }
            let extracted_marker =
                if ExtractedFieldRegistry::new().is_extracted(&field.name, field.category) {
                    "+"
                } else {
                    " "
                };
            println!(
                "  {} {:30} freq={:3}  type={:8}  sample={:?}",
                extracted_marker,
                field.name,
                field.frequency,
                field.inferred_type.to_string(),
                field.sample_value.as_ref().map(|s| if s.len() > 30 {
                    format!("{}...", &s[..30])
                } else {
                    s.clone()
                })
            );
        }
    }
}

/// Generate JSON report
pub fn json_report(report: &CoverageReport) -> Result<String> {
    use serde::Serialize;

    #[derive(Serialize)]
    struct JsonReport {
        files_scanned: usize,
        total_coverage_pct: f64,
        categories: Vec<JsonCategory>,
    }

    #[derive(Serialize)]
    struct JsonCategory {
        name: String,
        discovered: usize,
        extracted: usize,
        coverage_pct: f64,
        missing_fields: Vec<JsonField>,
    }

    #[derive(Serialize)]
    struct JsonField {
        name: String,
        frequency: usize,
        field_type: String,
    }

    let json = JsonReport {
        files_scanned: report.files_scanned,
        total_coverage_pct: report.total_coverage(),
        categories: report
            .categories
            .iter()
            .filter(|c| c.discovered > 0)
            .map(|c| JsonCategory {
                name: c.category.to_string(),
                discovered: c.discovered,
                extracted: c.extracted,
                coverage_pct: c.coverage_pct,
                missing_fields: c
                    .missing
                    .iter()
                    .map(|f| JsonField {
                        name: f.name.clone(),
                        frequency: f.frequency,
                        field_type: f.inferred_type.to_string(),
                    })
                    .collect(),
            })
            .collect(),
    };

    serde_json::to_string_pretty(&json).context("Failed to serialize JSON report")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_type() {
        assert_eq!(infer_type("yes"), FieldType::Bool);
        assert_eq!(infer_type("no"), FieldType::Bool);
        assert_eq!(infer_type("123"), FieldType::Integer);
        assert_eq!(infer_type("-45"), FieldType::Integer);
        assert_eq!(infer_type("1.234"), FieldType::Float);
        assert_eq!(infer_type("-0.5"), FieldType::Float);
        assert_eq!(infer_type("\"hello\""), FieldType::String);
        assert_eq!(infer_type("1444.11.11"), FieldType::Date);
        assert_eq!(infer_type("{ 1 2 3 }"), FieldType::List);
        assert_eq!(infer_type("{ key=value }"), FieldType::Block);
    }

    #[test]
    fn test_extracted_registry() {
        let registry = ExtractedFieldRegistry::new();
        assert!(registry.is_extracted("treasury", FieldCategory::Countries));
        assert!(registry.is_extracted("manpower", FieldCategory::Countries));
        assert!(registry.is_extracted("owner", FieldCategory::Provinces));
        assert!(!registry.is_extracted("prestige", FieldCategory::Countries));
    }
}
