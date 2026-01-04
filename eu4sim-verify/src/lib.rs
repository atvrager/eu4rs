pub mod coverage;
pub mod diff;
pub mod extract;
pub mod hydrate;
pub mod ledger_comparison;
pub mod melt;
pub mod parse;
pub mod predict;
pub mod report;
pub mod verify;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents state extracted from a save file for verification
#[derive(Debug, Clone)]
pub struct ExtractedState {
    pub meta: SaveMeta,
    pub countries: HashMap<String, ExtractedCountry>,
    pub provinces: HashMap<u32, ExtractedProvince>,
    /// Subject relationships: vassal tag -> relationship details
    pub subjects: HashMap<String, ExtractedSubject>,
    /// Celestial Empire (Emperor of China) state
    pub celestial_empire: Option<ExtractedCelestialEmpire>,
}

/// Celestial Empire (Emperor of China) state extracted from save
#[derive(Debug, Clone, Default)]
pub struct ExtractedCelestialEmpire {
    /// Current Emperor of China tag (e.g., "MNG" for Ming)
    pub emperor: Option<String>,
    /// Current mandate value (0-100)
    pub mandate: Option<f64>,
    /// Whether the celestial empire has been dismantled
    pub dismantled: bool,
    /// Reforms that have been passed (reform IDs as strings)
    pub reforms_passed: Vec<String>,
}

/// Subject relationship extracted from save
#[derive(Debug, Clone, Default)]
pub struct ExtractedSubject {
    /// Overlord country tag (e.g., "FRA")
    pub overlord: String,
    /// Subject country tag (e.g., "PRO")
    pub subject: String,
    /// Subject type name (e.g., "vassal", "march", "personal_union")
    pub subject_type: String,
    /// When the relationship started (YYYY.M.D format)
    pub start_date: Option<String>,
}

/// Save file metadata
#[derive(Debug, Clone)]
pub struct SaveMeta {
    pub date: String,
    pub player: Option<String>,
    pub ironman: bool,
    pub save_version: Option<String>,
}

/// Country state extracted from save
#[derive(Debug, Clone, Default)]
pub struct ExtractedCountry {
    /// Country tag (e.g., "FRA", "TUR")
    pub tag: String,

    // Cached values from save (what we verify against)
    pub max_manpower: Option<f64>,
    pub current_manpower: Option<f64>,
    pub treasury: Option<f64>,

    // Monarch power points (current stored amounts)
    pub adm_power: Option<f64>,
    pub dip_power: Option<f64>,
    pub mil_power: Option<f64>,

    // Ruler stats (0-6, determines monthly power generation)
    pub ruler_adm: Option<u16>,
    pub ruler_dip: Option<u16>,
    pub ruler_mil: Option<u16>,
    /// Ruler's dynasty (for HRE re-election bonus and PU mechanics)
    pub ruler_dynasty: Option<String>,

    // Tribute type (for tributary states)
    pub tribute_type: Option<i32>,

    // Income breakdown
    pub monthly_income: Option<MonthlyIncome>,

    // Expense breakdown (from ledger)
    pub total_monthly_expenses: Option<f64>,
    pub army_maintenance: Option<f64>,
    pub navy_maintenance: Option<f64>,
    pub fort_maintenance: Option<f64>,
    pub state_maintenance: Option<f64>,
    pub root_out_corruption: Option<f64>,

    // Advisors (type -> skill level)
    pub advisors: Vec<ExtractedAdvisor>,

    // Ideas
    pub ideas: ExtractedIdeas,

    // Active country modifiers (event modifiers, government bonuses, etc.)
    pub active_modifiers: Vec<String>,

    // For recalculation
    pub owned_province_ids: Vec<u32>,
}

/// Advisor state extracted from save
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExtractedAdvisor {
    /// Advisor type (e.g., "philosopher", "trader", "army_reformer")
    pub advisor_type: String,
    /// Skill level (1-5)
    pub skill: u8,
    /// Is this advisor currently hired?
    pub is_hired: bool,
}

/// Idea group state extracted from save
#[derive(Debug, Clone, Default)]
pub struct ExtractedIdeaGroup {
    /// Idea group name (e.g., "aristocracy_ideas", "FRA_ideas")
    pub name: String,
    /// Number of ideas unlocked in this group (0-7)
    pub ideas_unlocked: u8,
}

/// Country idea state extracted from save
#[derive(Debug, Clone, Default)]
pub struct ExtractedIdeas {
    /// National idea group name (if any)
    pub national_ideas: Option<String>,
    /// National ideas unlocked (0-7)
    pub national_ideas_progress: u8,
    /// Picked generic idea groups with unlock progress
    pub idea_groups: Vec<ExtractedIdeaGroup>,
}

/// Monthly income breakdown from save
#[derive(Debug, Clone, Default)]
pub struct MonthlyIncome {
    pub tax: f64,
    pub production: f64,
    pub trade: f64,
    pub gold: f64,
    pub tariffs: f64,
    pub subsidies: f64,
    pub total: f64,
}

/// Province state extracted from save
#[derive(Debug, Clone, Default)]
pub struct ExtractedProvince {
    pub id: u32,
    pub name: Option<String>,
    pub owner: Option<String>,

    // Development
    pub base_tax: Option<f64>,
    pub base_production: Option<f64>,
    pub base_manpower: Option<f64>,

    // Institution spread
    pub institutions: HashMap<String, f64>,

    // Modifiers
    pub local_autonomy: Option<f64>,

    // Buildings present in province
    pub buildings: Vec<String>,

    // Trade good produced (e.g., "grain", "cloth", "silk")
    pub trade_good: Option<String>,
}

/// Result of verifying a single metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub metric: MetricType,
    pub expected: f64,
    pub actual: f64,
    pub delta: f64,
    pub status: VerifyStatus,
    pub details: Option<String>,
}

impl VerificationResult {
    pub fn pass(metric: MetricType, expected: f64, actual: f64) -> Self {
        let delta = (actual - expected).abs();
        Self {
            metric,
            expected,
            actual,
            delta,
            status: VerifyStatus::Pass,
            details: None,
        }
    }

    pub fn fail(metric: MetricType, expected: f64, actual: f64, reason: impl Into<String>) -> Self {
        let delta = (actual - expected).abs();
        Self {
            metric,
            expected,
            actual,
            delta,
            status: VerifyStatus::Fail,
            details: Some(reason.into()),
        }
    }

    pub fn skip(metric: MetricType, reason: impl Into<String>) -> Self {
        Self {
            metric,
            expected: 0.0,
            actual: 0.0,
            delta: 0.0,
            status: VerifyStatus::Skip,
            details: Some(reason.into()),
        }
    }
}

/// Verification status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerifyStatus {
    /// Values match within tolerance
    Pass,
    /// Significant delta detected (potential bug)
    Fail,
    /// Metric not implemented or data missing
    Skip,
}

/// Type of metric being verified
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricType {
    MaxManpower { country: String },
    MonthlyTaxIncome { country: String },
    MonthlyTradeIncome { country: String },
    MonthlyProductionIncome { country: String },
    ArmyMaintenance { country: String },
    NavyMaintenance { country: String },
    InstitutionSpread { province: u32, institution: String },
    ProvinceDevelopment { province: u32 },
}

impl std::fmt::Display for MetricType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetricType::MaxManpower { country } => write!(f, "MaxManpower({})", country),
            MetricType::MonthlyTaxIncome { country } => write!(f, "MonthlyTax({})", country),
            MetricType::MonthlyTradeIncome { country } => write!(f, "MonthlyTrade({})", country),
            MetricType::MonthlyProductionIncome { country } => {
                write!(f, "MonthlyProduction({})", country)
            }
            MetricType::ArmyMaintenance { country } => write!(f, "ArmyMaintenance({})", country),
            MetricType::NavyMaintenance { country } => write!(f, "NavyMaintenance({})", country),
            MetricType::InstitutionSpread {
                province,
                institution,
            } => write!(f, "Institution({}, {})", province, institution),
            MetricType::ProvinceDevelopment { province } => write!(f, "Development({})", province),
        }
    }
}

/// Summary of verification run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub results: Vec<VerificationResult>,
}

impl VerificationSummary {
    pub fn new(results: Vec<VerificationResult>) -> Self {
        let passed = results
            .iter()
            .filter(|r| r.status == VerifyStatus::Pass)
            .count();
        let failed = results
            .iter()
            .filter(|r| r.status == VerifyStatus::Fail)
            .count();
        let skipped = results
            .iter()
            .filter(|r| r.status == VerifyStatus::Skip)
            .count();

        Self {
            total: results.len(),
            passed,
            failed,
            skipped,
            results,
        }
    }

    pub fn pass_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.passed as f64 / (self.passed + self.failed) as f64 * 100.0
        }
    }
}
