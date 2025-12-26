//! Action inference by comparing sequential saves
//!
//! Compares two save states to detect what actions occurred between them.
//! This enables Phase 3 action validation by:
//! 1. Detecting state changes between saves
//! 2. Inferring the actions that caused those changes
//! 3. Validating action costs against sim calculations

use crate::{ExtractedAdvisor, ExtractedState};
use std::collections::HashSet;

/// An action inferred from comparing two saves
#[derive(Debug, Clone, PartialEq)]
pub enum InferredAction {
    /// A building was constructed in a province
    BuildBuilding {
        province_id: u32,
        province_name: Option<String>,
        building: String,
        owner: Option<String>,
    },

    /// Development was increased (dev click)
    DevelopProvince {
        province_id: u32,
        province_name: Option<String>,
        dev_type: DevType,
        from: f64,
        to: f64,
        owner: Option<String>,
    },

    /// An advisor was hired
    HireAdvisor {
        country: String,
        advisor_type: String,
        skill: u8,
    },

    /// An advisor was dismissed
    DismissAdvisor {
        country: String,
        advisor_type: String,
        skill: u8,
    },

    /// Monarch power was spent (generic, when we can't determine specific use)
    SpendMana {
        country: String,
        mana_type: ManaType,
        amount: f64,
    },

    /// Treasury changed significantly (spending or income event)
    TreasuryChange {
        country: String,
        delta: f64,
        likely_cause: String,
    },

    /// A new subject relationship was created (vassalize, PU, etc.)
    Vassalize {
        overlord: String,
        subject: String,
        subject_type: String,
    },

    /// A subject relationship ended (release, independence, integration)
    ReleaseSubject {
        overlord: String,
        subject: String,
        subject_type: String,
        likely_cause: String,
    },
}

/// Type of development
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevType {
    Tax,
    Production,
    Manpower,
}

impl std::fmt::Display for DevType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DevType::Tax => write!(f, "tax"),
            DevType::Production => write!(f, "production"),
            DevType::Manpower => write!(f, "manpower"),
        }
    }
}

/// Type of monarch power
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManaType {
    Admin,
    Diplo,
    Mil,
}

impl std::fmt::Display for ManaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManaType::Admin => write!(f, "ADM"),
            ManaType::Diplo => write!(f, "DIP"),
            ManaType::Mil => write!(f, "MIL"),
        }
    }
}

/// Result of comparing two saves
#[derive(Debug)]
pub struct DiffResult {
    /// Date of the "before" save
    pub from_date: String,
    /// Date of the "after" save
    pub to_date: String,
    /// Actions inferred from state changes
    pub actions: Vec<InferredAction>,
    /// Countries that were analyzed
    pub analyzed_countries: Vec<String>,
}

/// Infer actions by comparing two sequential saves
pub fn infer_actions(before: &ExtractedState, after: &ExtractedState) -> DiffResult {
    let mut actions = Vec::new();
    let mut analyzed_countries = Vec::new();

    // Find buildings built
    actions.extend(infer_building_actions(before, after));

    // Find development changes
    actions.extend(infer_development_actions(before, after));

    // Find subject relationship changes
    actions.extend(infer_subject_actions(before, after));

    // Find advisor changes per country
    for (tag, after_country) in &after.countries {
        if let Some(before_country) = before.countries.get(tag) {
            analyzed_countries.push(tag.clone());

            // Advisor changes
            actions.extend(infer_advisor_actions(tag, before_country, after_country));

            // Significant mana changes
            actions.extend(infer_mana_changes(tag, before_country, after_country));

            // Significant treasury changes
            if let (Some(before_treasury), Some(after_treasury)) =
                (before_country.treasury, after_country.treasury)
            {
                let delta = after_treasury - before_treasury;
                // Only report large changes (>50 ducats)
                if delta.abs() > 50.0 {
                    let likely_cause = if delta < 0.0 {
                        if delta < -200.0 {
                            "large purchase (building, army, peace deal?)".to_string()
                        } else {
                            "moderate spending".to_string()
                        }
                    } else if delta > 100.0 {
                        "windfall income (war reparations? loot?)".to_string()
                    } else {
                        "accumulated income".to_string()
                    };

                    actions.push(InferredAction::TreasuryChange {
                        country: tag.clone(),
                        delta,
                        likely_cause,
                    });
                }
            }
        }
    }

    DiffResult {
        from_date: before.meta.date.clone(),
        to_date: after.meta.date.clone(),
        actions,
        analyzed_countries,
    }
}

/// Infer building construction actions
fn infer_building_actions(before: &ExtractedState, after: &ExtractedState) -> Vec<InferredAction> {
    let mut actions = Vec::new();

    for (id, after_prov) in &after.provinces {
        let before_buildings: HashSet<&String> = if let Some(before_prov) = before.provinces.get(id)
        {
            before_prov.buildings.iter().collect()
        } else {
            HashSet::new()
        };

        let after_buildings: HashSet<&String> = after_prov.buildings.iter().collect();

        // New buildings = in after but not in before
        for building in after_buildings.difference(&before_buildings) {
            actions.push(InferredAction::BuildBuilding {
                province_id: *id,
                province_name: after_prov.name.clone(),
                building: (*building).clone(),
                owner: after_prov.owner.clone(),
            });
        }
    }

    actions
}

/// Infer development actions
fn infer_development_actions(
    before: &ExtractedState,
    after: &ExtractedState,
) -> Vec<InferredAction> {
    let mut actions = Vec::new();

    for (id, after_prov) in &after.provinces {
        if let Some(before_prov) = before.provinces.get(id) {
            // Check tax dev
            if let (Some(before_tax), Some(after_tax)) = (before_prov.base_tax, after_prov.base_tax)
            {
                if after_tax > before_tax {
                    actions.push(InferredAction::DevelopProvince {
                        province_id: *id,
                        province_name: after_prov.name.clone(),
                        dev_type: DevType::Tax,
                        from: before_tax,
                        to: after_tax,
                        owner: after_prov.owner.clone(),
                    });
                }
            }

            // Check production dev
            if let (Some(before_prod), Some(after_prod)) =
                (before_prov.base_production, after_prov.base_production)
            {
                if after_prod > before_prod {
                    actions.push(InferredAction::DevelopProvince {
                        province_id: *id,
                        province_name: after_prov.name.clone(),
                        dev_type: DevType::Production,
                        from: before_prod,
                        to: after_prod,
                        owner: after_prov.owner.clone(),
                    });
                }
            }

            // Check manpower dev
            if let (Some(before_mp), Some(after_mp)) =
                (before_prov.base_manpower, after_prov.base_manpower)
            {
                if after_mp > before_mp {
                    actions.push(InferredAction::DevelopProvince {
                        province_id: *id,
                        province_name: after_prov.name.clone(),
                        dev_type: DevType::Manpower,
                        from: before_mp,
                        to: after_mp,
                        owner: after_prov.owner.clone(),
                    });
                }
            }
        }
    }

    actions
}

/// Infer advisor hire/dismiss actions
fn infer_advisor_actions(
    tag: &str,
    before: &crate::ExtractedCountry,
    after: &crate::ExtractedCountry,
) -> Vec<InferredAction> {
    let mut actions = Vec::new();

    // Get hired advisors before and after
    let before_hired: Vec<&ExtractedAdvisor> =
        before.advisors.iter().filter(|a| a.is_hired).collect();
    let after_hired: Vec<&ExtractedAdvisor> =
        after.advisors.iter().filter(|a| a.is_hired).collect();

    // Find newly hired (in after but not in before)
    for after_adv in &after_hired {
        let was_hired = before_hired
            .iter()
            .any(|b| b.advisor_type == after_adv.advisor_type && b.skill == after_adv.skill);

        if !was_hired {
            actions.push(InferredAction::HireAdvisor {
                country: tag.to_string(),
                advisor_type: after_adv.advisor_type.clone(),
                skill: after_adv.skill,
            });
        }
    }

    // Find dismissed (in before but not in after)
    for before_adv in &before_hired {
        let still_hired = after_hired
            .iter()
            .any(|a| a.advisor_type == before_adv.advisor_type && a.skill == before_adv.skill);

        if !still_hired {
            actions.push(InferredAction::DismissAdvisor {
                country: tag.to_string(),
                advisor_type: before_adv.advisor_type.clone(),
                skill: before_adv.skill,
            });
        }
    }

    actions
}

/// Infer subject relationship changes (new vassals, releases, integrations)
fn infer_subject_actions(before: &ExtractedState, after: &ExtractedState) -> Vec<InferredAction> {
    let mut actions = Vec::new();

    // Check for new subject relationships (in after but not in before)
    for (subject_tag, after_rel) in &after.subjects {
        if !before.subjects.contains_key(subject_tag) {
            actions.push(InferredAction::Vassalize {
                overlord: after_rel.overlord.clone(),
                subject: after_rel.subject.clone(),
                subject_type: after_rel.subject_type.clone(),
            });
        }
    }

    // Check for ended subject relationships (in before but not in after)
    for (subject_tag, before_rel) in &before.subjects {
        if !after.subjects.contains_key(subject_tag) {
            // Determine likely cause based on whether the country still exists
            let likely_cause = if after.countries.contains_key(subject_tag) {
                // Country still exists - could be independence or release
                "independence or release".to_string()
            } else {
                // Country no longer exists - could be integration/annexation
                "integration or annexation".to_string()
            };

            actions.push(InferredAction::ReleaseSubject {
                overlord: before_rel.overlord.clone(),
                subject: before_rel.subject.clone(),
                subject_type: before_rel.subject_type.clone(),
                likely_cause,
            });
        }
    }

    actions
}

/// Infer significant monarch power spending
fn infer_mana_changes(
    tag: &str,
    before: &crate::ExtractedCountry,
    after: &crate::ExtractedCountry,
) -> Vec<InferredAction> {
    let mut actions = Vec::new();

    // Threshold for "significant" spending (beyond normal monthly gain)
    // Monthly gain is roughly 3-9 per type depending on monarch + advisors
    // So spending >50 in 20-30 days is noteworthy
    const SPENDING_THRESHOLD: f64 = 50.0;

    // Admin power
    if let (Some(before_adm), Some(after_adm)) = (before.adm_power, after.adm_power) {
        let delta = after_adm - before_adm;
        // Negative delta = spending; ignore small changes and gains
        if delta < -SPENDING_THRESHOLD {
            actions.push(InferredAction::SpendMana {
                country: tag.to_string(),
                mana_type: ManaType::Admin,
                amount: -delta,
            });
        }
    }

    // Diplo power
    if let (Some(before_dip), Some(after_dip)) = (before.dip_power, after.dip_power) {
        let delta = after_dip - before_dip;
        if delta < -SPENDING_THRESHOLD {
            actions.push(InferredAction::SpendMana {
                country: tag.to_string(),
                mana_type: ManaType::Diplo,
                amount: -delta,
            });
        }
    }

    // Mil power
    if let (Some(before_mil), Some(after_mil)) = (before.mil_power, after.mil_power) {
        let delta = after_mil - before_mil;
        if delta < -SPENDING_THRESHOLD {
            actions.push(InferredAction::SpendMana {
                country: tag.to_string(),
                mana_type: ManaType::Mil,
                amount: -delta,
            });
        }
    }

    actions
}

/// Format an inferred action for display
impl std::fmt::Display for InferredAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InferredAction::BuildBuilding {
                province_name,
                building,
                owner,
                province_id,
            } => {
                let default_name = format!("#{}", province_id);
                let name = province_name.as_deref().unwrap_or(&default_name);
                let owner = owner.as_deref().unwrap_or("???");
                write!(
                    f,
                    "{} built {} in {} ({})",
                    owner, building, name, province_id
                )
            }
            InferredAction::DevelopProvince {
                province_name,
                dev_type,
                from,
                to,
                owner,
                province_id,
            } => {
                let default_name = format!("#{}", province_id);
                let name = province_name.as_deref().unwrap_or(&default_name);
                let owner = owner.as_deref().unwrap_or("???");
                write!(
                    f,
                    "{} developed {} in {} ({} -> {})",
                    owner, dev_type, name, from, to
                )
            }
            InferredAction::HireAdvisor {
                country,
                advisor_type,
                skill,
            } => {
                write!(f, "{} hired {} (skill {})", country, advisor_type, skill)
            }
            InferredAction::DismissAdvisor {
                country,
                advisor_type,
                skill,
            } => {
                write!(
                    f,
                    "{} dismissed {} (skill {})",
                    country, advisor_type, skill
                )
            }
            InferredAction::SpendMana {
                country,
                mana_type,
                amount,
            } => {
                write!(
                    f,
                    "{} spent {} {} power",
                    country, *amount as i32, mana_type
                )
            }
            InferredAction::TreasuryChange {
                country,
                delta,
                likely_cause,
            } => {
                let sign = if *delta > 0.0 { "+" } else { "" };
                write!(
                    f,
                    "{} treasury {}{:.1} ({})",
                    country, sign, delta, likely_cause
                )
            }
            InferredAction::Vassalize {
                overlord,
                subject,
                subject_type,
            } => {
                write!(f, "{} made {} a {}", overlord, subject, subject_type)
            }
            InferredAction::ReleaseSubject {
                overlord,
                subject,
                subject_type,
                likely_cause,
            } => {
                write!(
                    f,
                    "{} lost {} ({}) - {}",
                    overlord, subject, subject_type, likely_cause
                )
            }
        }
    }
}

/// Print a diff report to stdout
pub fn print_diff_report(result: &DiffResult) {
    println!();
    println!(
        "=== Action Diff: {} -> {} ===",
        result.from_date, result.to_date
    );
    println!("Analyzed {} countries", result.analyzed_countries.len());
    println!();

    if result.actions.is_empty() {
        println!("No significant actions detected.");
    } else {
        println!("Detected {} actions:", result.actions.len());
        println!();
        for (i, action) in result.actions.iter().enumerate() {
            println!("  {}. {}", i + 1, action);
        }
    }
    println!();
}

/// Filter actions by country
pub fn filter_by_country<'a>(result: &'a DiffResult, country: &str) -> Vec<&'a InferredAction> {
    result
        .actions
        .iter()
        .filter(|action| match action {
            InferredAction::BuildBuilding { owner, .. } => owner.as_deref() == Some(country),
            InferredAction::DevelopProvince { owner, .. } => owner.as_deref() == Some(country),
            InferredAction::HireAdvisor { country: c, .. } => c == country,
            InferredAction::DismissAdvisor { country: c, .. } => c == country,
            InferredAction::SpendMana { country: c, .. } => c == country,
            InferredAction::TreasuryChange { country: c, .. } => c == country,
            InferredAction::Vassalize { overlord, .. } => overlord == country,
            InferredAction::ReleaseSubject { overlord, .. } => overlord == country,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExtractedProvince, SaveMeta};
    use std::collections::HashMap;

    fn make_test_state(date: &str) -> ExtractedState {
        ExtractedState {
            meta: SaveMeta {
                date: date.to_string(),
                player: Some("TST".to_string()),
                ironman: false,
                save_version: None,
            },
            countries: HashMap::new(),
            provinces: HashMap::new(),
            subjects: HashMap::new(),
        }
    }

    #[test]
    fn test_detect_new_building() {
        let mut before = make_test_state("1444.11.11");
        let mut after = make_test_state("1444.12.01");

        // Add a province without building in "before"
        before.provinces.insert(
            730,
            ExtractedProvince {
                id: 730,
                name: Some("Seoul".to_string()),
                owner: Some("KOR".to_string()),
                buildings: vec![],
                ..Default::default()
            },
        );

        // Add same province with marketplace in "after"
        after.provinces.insert(
            730,
            ExtractedProvince {
                id: 730,
                name: Some("Seoul".to_string()),
                owner: Some("KOR".to_string()),
                buildings: vec!["marketplace".to_string()],
                ..Default::default()
            },
        );

        let result = infer_actions(&before, &after);
        assert_eq!(result.actions.len(), 1);

        match &result.actions[0] {
            InferredAction::BuildBuilding {
                province_id,
                building,
                owner,
                ..
            } => {
                assert_eq!(*province_id, 730);
                assert_eq!(building, "marketplace");
                assert_eq!(owner.as_deref(), Some("KOR"));
            }
            _ => panic!("Expected BuildBuilding action"),
        }
    }

    #[test]
    fn test_detect_development() {
        let mut before = make_test_state("1444.11.11");
        let mut after = make_test_state("1444.12.01");

        before.provinces.insert(
            730,
            ExtractedProvince {
                id: 730,
                name: Some("Seoul".to_string()),
                owner: Some("KOR".to_string()),
                base_tax: Some(5.0),
                base_production: Some(5.0),
                base_manpower: Some(5.0),
                ..Default::default()
            },
        );

        after.provinces.insert(
            730,
            ExtractedProvince {
                id: 730,
                name: Some("Seoul".to_string()),
                owner: Some("KOR".to_string()),
                base_tax: Some(6.0), // +1 dev
                base_production: Some(5.0),
                base_manpower: Some(5.0),
                ..Default::default()
            },
        );

        let result = infer_actions(&before, &after);
        assert_eq!(result.actions.len(), 1);

        match &result.actions[0] {
            InferredAction::DevelopProvince {
                dev_type, from, to, ..
            } => {
                assert_eq!(*dev_type, DevType::Tax);
                assert_eq!(*from, 5.0);
                assert_eq!(*to, 6.0);
            }
            _ => panic!("Expected DevelopProvince action"),
        }
    }

    #[test]
    fn test_no_actions_when_unchanged() {
        let before = make_test_state("1444.11.11");
        let after = make_test_state("1444.12.01");

        let result = infer_actions(&before, &after);
        assert!(result.actions.is_empty());
    }

    #[test]
    fn test_detect_new_vassal() {
        let before = make_test_state("1444.11.11");
        let mut after = make_test_state("1445.01.01");

        // Add a new vassal in "after"
        after.subjects.insert(
            "PRO".to_string(),
            crate::ExtractedSubject {
                overlord: "FRA".to_string(),
                subject: "PRO".to_string(),
                subject_type: "vassal".to_string(),
                start_date: Some("1445.1.1".to_string()),
            },
        );

        let result = infer_actions(&before, &after);
        assert_eq!(result.actions.len(), 1);

        match &result.actions[0] {
            InferredAction::Vassalize {
                overlord,
                subject,
                subject_type,
            } => {
                assert_eq!(overlord, "FRA");
                assert_eq!(subject, "PRO");
                assert_eq!(subject_type, "vassal");
            }
            _ => panic!("Expected Vassalize action"),
        }
    }

    #[test]
    fn test_detect_subject_released() {
        let mut before = make_test_state("1444.11.11");
        let mut after = make_test_state("1445.01.01");

        // Add vassal in "before"
        before.subjects.insert(
            "ORL".to_string(),
            crate::ExtractedSubject {
                overlord: "FRA".to_string(),
                subject: "ORL".to_string(),
                subject_type: "appanage".to_string(),
                start_date: Some("1444.1.1".to_string()),
            },
        );

        // Subject still exists as independent country in "after"
        after.countries.insert(
            "ORL".to_string(),
            crate::ExtractedCountry {
                tag: "ORL".to_string(),
                ..Default::default()
            },
        );

        let result = infer_actions(&before, &after);
        assert_eq!(result.actions.len(), 1);

        match &result.actions[0] {
            InferredAction::ReleaseSubject {
                overlord,
                subject,
                likely_cause,
                ..
            } => {
                assert_eq!(overlord, "FRA");
                assert_eq!(subject, "ORL");
                assert!(likely_cause.contains("independence") || likely_cause.contains("release"));
            }
            _ => panic!("Expected ReleaseSubject action"),
        }
    }

    #[test]
    fn test_detect_subject_integrated() {
        let mut before = make_test_state("1444.11.11");
        let after = make_test_state("1470.01.01");

        // Add vassal in "before"
        before.subjects.insert(
            "ORL".to_string(),
            crate::ExtractedSubject {
                overlord: "FRA".to_string(),
                subject: "ORL".to_string(),
                subject_type: "vassal".to_string(),
                start_date: Some("1444.1.1".to_string()),
            },
        );

        // Subject no longer exists in "after" (integrated)
        // Note: ORL not in after.countries

        let result = infer_actions(&before, &after);
        assert_eq!(result.actions.len(), 1);

        match &result.actions[0] {
            InferredAction::ReleaseSubject {
                overlord,
                subject,
                likely_cause,
                ..
            } => {
                assert_eq!(overlord, "FRA");
                assert_eq!(subject, "ORL");
                assert!(
                    likely_cause.contains("integration") || likely_cause.contains("annexation")
                );
            }
            _ => panic!("Expected ReleaseSubject action"),
        }
    }
}
