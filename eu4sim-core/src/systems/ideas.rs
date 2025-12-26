//! Idea system for modifier application and stub tracking.
//!
//! This module handles:
//! - Applying implemented modifiers from ideas to `GameModifiers`
//! - Tracking which modifiers are referenced but not yet implemented (stubs)
//!
//! The stub tracker serves as a roadmap for future mechanics implementation.

use crate::fixed::Fixed;
use crate::ideas::{IdeaGroupRegistry, ModifierEntry};
use crate::modifiers::GameModifiers;
use crate::state::CountryState;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

/// Tracks which modifiers are referenced but not implemented.
///
/// Thread-safe for parallel idea loading. Use `report()` at startup
/// to identify which mechanics need implementation.
#[derive(Debug, Default)]
pub struct ModifierStubTracker {
    /// Set of modifier keys that have been encountered but not implemented.
    unimplemented: Mutex<HashSet<String>>,
    /// Count of how many times each unimplemented modifier was referenced.
    reference_counts: Mutex<HashMap<String, u32>>,
}

impl ModifierStubTracker {
    /// Create a new tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Track an unimplemented modifier.
    pub fn track(&self, key: &str) {
        {
            let mut unimplemented = self.unimplemented.lock().unwrap();
            unimplemented.insert(key.to_string());
        }
        {
            let mut counts = self.reference_counts.lock().unwrap();
            *counts.entry(key.to_string()).or_default() += 1;
        }
    }

    /// Get all unimplemented modifier keys.
    pub fn unimplemented_keys(&self) -> Vec<String> {
        let unimplemented = self.unimplemented.lock().unwrap();
        unimplemented.iter().cloned().collect()
    }

    /// Get the number of unimplemented modifiers.
    pub fn unimplemented_count(&self) -> usize {
        let unimplemented = self.unimplemented.lock().unwrap();
        unimplemented.len()
    }

    /// Get reference counts for unimplemented modifiers.
    pub fn reference_counts(&self) -> HashMap<String, u32> {
        let counts = self.reference_counts.lock().unwrap();
        counts.clone()
    }

    /// Generate a report of unimplemented modifiers, sorted by frequency.
    pub fn report(&self) -> String {
        let counts = self.reference_counts.lock().unwrap();
        let mut sorted: Vec<_> = counts.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1)); // Descending by count

        let mut report = format!("Unimplemented Modifiers ({} unique):\n", sorted.len());
        for (key, count) in sorted.iter().take(50) {
            report.push_str(&format!("  {:40} {:4} references\n", key, count));
        }
        if sorted.len() > 50 {
            report.push_str(&format!("  ... and {} more\n", sorted.len() - 50));
        }
        report
    }

    /// Check if a specific modifier is implemented.
    pub fn is_implemented(key: &str) -> bool {
        matches!(
            key,
            // Tax
            "global_tax_modifier"
            // Maintenance
                | "land_maintenance_modifier"
                | "fort_maintenance_modifier"
            // Production
                | "production_efficiency"
            // Combat
                | "discipline"
                | "morale_of_armies"
                | "land_morale"
                | "infantry_power"
                | "infantry_combat_ability"
                | "cavalry_power"
                | "cavalry_combat_ability"
                | "artillery_power"
            // Trade/Production
                | "goods_produced_modifier"
                | "goods_produced"
                | "trade_efficiency"
                | "global_trade_power"
                | "trade_steering"
            // Administrative
                | "development_cost"
                | "core_creation"
                | "ae_impact"
                | "diplomatic_reputation"
            // Military Maintenance
                | "infantry_cost"
                | "cavalry_cost"
                | "mercenary_cost"
                | "mercenary_maintenance"
        )
    }
}

/// Apply a single modifier to GameModifiers.
///
/// Returns `true` if the modifier was applied, `false` if it's a stub.
pub fn apply_modifier(
    modifiers: &mut GameModifiers,
    tag: &str,
    entry: &ModifierEntry,
    stubs: &ModifierStubTracker,
) -> bool {
    match entry.key.as_str() {
        // === Tax modifiers ===
        "global_tax_modifier" => {
            let current = modifiers
                .country_tax_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_tax_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Maintenance modifiers ===
        "land_maintenance_modifier" => {
            let current = modifiers
                .land_maintenance_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .land_maintenance_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "fort_maintenance_modifier" => {
            let current = modifiers
                .fort_maintenance_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .fort_maintenance_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Combat modifiers ===
        "discipline" => {
            let current = modifiers
                .country_discipline
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_discipline
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "morale_of_armies" | "land_morale" => {
            let current = modifiers
                .country_morale
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_morale
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "infantry_power" | "infantry_combat_ability" => {
            let current = modifiers
                .country_infantry_power
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_infantry_power
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "cavalry_power" | "cavalry_combat_ability" => {
            let current = modifiers
                .country_cavalry_power
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_cavalry_power
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "artillery_power" => {
            let current = modifiers
                .country_artillery_power
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_artillery_power
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Production/Trade modifiers ===
        "goods_produced_modifier" | "goods_produced" => {
            let current = modifiers
                .country_goods_produced
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_goods_produced
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "trade_efficiency" => {
            let current = modifiers
                .country_trade_efficiency
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_trade_efficiency
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "global_trade_power" => {
            let current = modifiers
                .country_trade_power
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_trade_power
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "trade_steering" => {
            let current = modifiers
                .country_trade_steering
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_trade_steering
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Administrative modifiers ===
        "development_cost" => {
            let current = modifiers
                .country_development_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_development_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "core_creation" => {
            let current = modifiers
                .country_core_creation
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_core_creation
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "ae_impact" => {
            let current = modifiers
                .country_ae_impact
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_ae_impact
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "diplomatic_reputation" => {
            let current = modifiers
                .country_diplomatic_reputation
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_diplomatic_reputation
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Military maintenance modifiers ===
        "infantry_cost" => {
            let current = modifiers
                .country_infantry_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_infantry_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "cavalry_cost" => {
            let current = modifiers
                .country_cavalry_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_cavalry_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "mercenary_cost" | "mercenary_maintenance" => {
            let current = modifiers
                .country_mercenary_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_mercenary_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === All other modifiers are stubs ===
        _ => {
            stubs.track(&entry.key);
            false
        }
    }
}

/// Recalculate all idea modifiers for a country.
///
/// Clears existing modifiers and reapplies based on:
/// - National ideas (start + unlocked ideas + bonus if complete)
/// - Picked idea groups (unlocked ideas + bonus if complete)
pub fn recalculate_idea_modifiers(
    modifiers: &mut GameModifiers,
    tag: &str,
    country: &CountryState,
    registry: &IdeaGroupRegistry,
    stubs: &ModifierStubTracker,
) -> IdeaModifierStats {
    let mut stats = IdeaModifierStats::default();

    // Clear existing country-level modifiers from ideas
    // (In a full implementation, we'd need to track which modifiers came from ideas
    // vs other sources. For now, we just add to existing values.)

    // Apply national ideas
    if let Some(national_id) = country.ideas.national_ideas {
        if let Some(national) = registry.get(national_id) {
            // Start modifiers always apply
            for entry in &national.start_modifiers {
                if apply_modifier(modifiers, tag, entry, stubs) {
                    stats.applied += 1;
                } else {
                    stats.stubbed += 1;
                }
            }

            // Apply unlocked ideas (0 to national_ideas_progress)
            let progress = country.ideas.national_ideas_progress as usize;
            for idea in national.ideas.iter().take(progress) {
                for entry in &idea.modifiers {
                    if apply_modifier(modifiers, tag, entry, stubs) {
                        stats.applied += 1;
                    } else {
                        stats.stubbed += 1;
                    }
                }
            }

            // Bonus if complete
            if progress >= 7 {
                for entry in &national.bonus_modifiers {
                    if apply_modifier(modifiers, tag, entry, stubs) {
                        stats.applied += 1;
                    } else {
                        stats.stubbed += 1;
                    }
                }
            }
        }
    }

    // Apply picked idea groups
    for (&group_id, &ideas_unlocked) in &country.ideas.groups {
        if let Some(group) = registry.get(group_id) {
            // Apply unlocked ideas
            let progress = ideas_unlocked as usize;
            for idea in group.ideas.iter().take(progress) {
                for entry in &idea.modifiers {
                    if apply_modifier(modifiers, tag, entry, stubs) {
                        stats.applied += 1;
                    } else {
                        stats.stubbed += 1;
                    }
                }
            }

            // Bonus if complete
            if progress >= 7 {
                for entry in &group.bonus_modifiers {
                    if apply_modifier(modifiers, tag, entry, stubs) {
                        stats.applied += 1;
                    } else {
                        stats.stubbed += 1;
                    }
                }
            }
        }
    }

    stats
}

/// Statistics from applying idea modifiers.
#[derive(Debug, Default, Clone, Copy)]
pub struct IdeaModifierStats {
    /// Number of modifiers successfully applied.
    pub applied: u32,
    /// Number of modifiers that were stubs (not implemented).
    pub stubbed: u32,
}

/// Scan all ideas and count modifier references.
///
/// Useful at startup to understand which modifiers are most commonly used.
pub fn scan_all_modifiers(registry: &IdeaGroupRegistry) -> HashMap<String, u32> {
    let mut counts: HashMap<String, u32> = HashMap::new();

    for group in registry.iter() {
        for entry in &group.start_modifiers {
            *counts.entry(entry.key.clone()).or_default() += 1;
        }
        for entry in &group.bonus_modifiers {
            *counts.entry(entry.key.clone()).or_default() += 1;
        }
        for idea in &group.ideas {
            for entry in &idea.modifiers {
                *counts.entry(entry.key.clone()).or_default() += 1;
            }
        }
    }

    counts
}

/// Print modifier usage report, highlighting which are implemented.
pub fn print_modifier_report(registry: &IdeaGroupRegistry) {
    let counts = scan_all_modifiers(registry);
    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1)); // Descending by count

    log::info!("=== Idea Modifier Usage Report ===");
    log::info!("Total unique modifiers: {}", sorted.len());

    let mut implemented_count = 0;
    let mut unimplemented_count = 0;

    for (key, count) in &sorted {
        let status = if ModifierStubTracker::is_implemented(key) {
            implemented_count += 1;
            "[OK]"
        } else {
            unimplemented_count += 1;
            "[STUB]"
        };
        if *count >= 5 {
            // Only log frequently used modifiers
            log::debug!("  {} {:40} {:4} refs", status, key, count);
        }
    }

    log::info!(
        "Implemented: {}, Stubbed: {}",
        implemented_count,
        unimplemented_count
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ideas::{CountryIdeaState, IdeaDef, IdeaGroupDef};

    fn make_test_registry() -> IdeaGroupRegistry {
        let mut registry = IdeaGroupRegistry::new();

        // Add a test national idea with implemented modifier
        registry.add(IdeaGroupDef {
            name: "TEST_ideas".into(),
            is_national: true,
            required_tag: Some("TST".into()),
            is_free: true,
            start_modifiers: vec![ModifierEntry::from_f32("global_tax_modifier", 0.10)],
            bonus_modifiers: vec![ModifierEntry::from_f32("global_tax_modifier", 0.05)],
            ideas: vec![
                IdeaDef {
                    name: "idea_1".into(),
                    position: 0,
                    modifiers: vec![ModifierEntry::from_f32("land_maintenance_modifier", -0.10)],
                },
                IdeaDef {
                    name: "idea_2".into(),
                    position: 1,
                    modifiers: vec![ModifierEntry::from_f32("global_manpower_modifier", 0.15)], // Stub
                },
            ],
            ..Default::default()
        });

        registry
    }

    #[test]
    fn test_stub_tracker() {
        let tracker = ModifierStubTracker::new();

        tracker.track("cavalry_power");
        tracker.track("cavalry_power");
        tracker.track("global_manpower_modifier");

        assert_eq!(tracker.unimplemented_count(), 2);
        assert!(tracker
            .unimplemented_keys()
            .contains(&"cavalry_power".to_string()));
        assert_eq!(tracker.reference_counts().get("cavalry_power"), Some(&2));
    }

    #[test]
    fn test_apply_modifier_implemented() {
        let mut modifiers = GameModifiers::default();
        let tracker = ModifierStubTracker::new();

        let entry = ModifierEntry::from_f32("global_tax_modifier", 0.10);
        let applied = apply_modifier(&mut modifiers, "FRA", &entry, &tracker);

        assert!(applied);
        assert_eq!(
            modifiers.country_tax_modifier.get("FRA"),
            Some(&Fixed::from_f32(0.10))
        );
    }

    #[test]
    fn test_apply_modifier_stub() {
        let mut modifiers = GameModifiers::default();
        let tracker = ModifierStubTracker::new();

        let entry = ModifierEntry::from_f32("global_manpower_modifier", 0.25);
        let applied = apply_modifier(&mut modifiers, "FRA", &entry, &tracker);

        assert!(!applied);
        assert!(tracker
            .unimplemented_keys()
            .contains(&"global_manpower_modifier".to_string()));
    }

    #[test]
    fn test_apply_discipline_modifier() {
        let mut modifiers = GameModifiers::default();
        let tracker = ModifierStubTracker::new();

        let entry = ModifierEntry::from_f32("discipline", 0.05);
        let applied = apply_modifier(&mut modifiers, "PRU", &entry, &tracker);

        assert!(applied);
        assert_eq!(
            modifiers.country_discipline.get("PRU"),
            Some(&Fixed::from_f32(0.05))
        );
    }

    #[test]
    fn test_apply_morale_modifier() {
        let mut modifiers = GameModifiers::default();
        let tracker = ModifierStubTracker::new();

        // Test morale_of_armies
        let entry1 = ModifierEntry::from_f32("morale_of_armies", 0.15);
        let applied1 = apply_modifier(&mut modifiers, "FRA", &entry1, &tracker);
        assert!(applied1);

        // Test land_morale alias
        let entry2 = ModifierEntry::from_f32("land_morale", 0.10);
        let applied2 = apply_modifier(&mut modifiers, "FRA", &entry2, &tracker);
        assert!(applied2);

        // Both should sum
        assert_eq!(
            modifiers.country_morale.get("FRA"),
            Some(&Fixed::from_f32(0.25))
        );
    }

    #[test]
    fn test_apply_unit_power_modifiers() {
        let mut modifiers = GameModifiers::default();
        let tracker = ModifierStubTracker::new();

        // Infantry power
        let inf_entry = ModifierEntry::from_f32("infantry_power", 0.10);
        assert!(apply_modifier(&mut modifiers, "SWE", &inf_entry, &tracker));
        assert_eq!(
            modifiers.country_infantry_power.get("SWE"),
            Some(&Fixed::from_f32(0.10))
        );

        // Cavalry power with alias
        let cav_entry1 = ModifierEntry::from_f32("cavalry_power", 0.15);
        let cav_entry2 = ModifierEntry::from_f32("cavalry_combat_ability", 0.10);
        assert!(apply_modifier(&mut modifiers, "POL", &cav_entry1, &tracker));
        assert!(apply_modifier(&mut modifiers, "POL", &cav_entry2, &tracker));
        assert_eq!(
            modifiers.country_cavalry_power.get("POL"),
            Some(&Fixed::from_f32(0.25))
        );

        // Artillery power
        let art_entry = ModifierEntry::from_f32("artillery_power", 0.05);
        assert!(apply_modifier(&mut modifiers, "FRA", &art_entry, &tracker));
        assert_eq!(
            modifiers.country_artillery_power.get("FRA"),
            Some(&Fixed::from_f32(0.05))
        );
    }

    #[test]
    fn test_apply_trade_modifiers() {
        let mut modifiers = GameModifiers::default();
        let tracker = ModifierStubTracker::new();

        // goods_produced_modifier (with alias)
        let entry1 = ModifierEntry::from_f32("goods_produced_modifier", 0.10);
        assert!(apply_modifier(&mut modifiers, "NED", &entry1, &tracker));
        let entry2 = ModifierEntry::from_f32("goods_produced", 0.05);
        assert!(apply_modifier(&mut modifiers, "NED", &entry2, &tracker));
        assert_eq!(
            modifiers.country_goods_produced.get("NED"),
            Some(&Fixed::from_f32(0.15))
        );

        // trade_efficiency
        let entry3 = ModifierEntry::from_f32("trade_efficiency", 0.20);
        assert!(apply_modifier(&mut modifiers, "VEN", &entry3, &tracker));
        assert_eq!(
            modifiers.country_trade_efficiency.get("VEN"),
            Some(&Fixed::from_f32(0.20))
        );

        // global_trade_power
        let entry4 = ModifierEntry::from_f32("global_trade_power", 0.15);
        assert!(apply_modifier(&mut modifiers, "POR", &entry4, &tracker));
        assert_eq!(
            modifiers.country_trade_power.get("POR"),
            Some(&Fixed::from_f32(0.15))
        );

        // trade_steering
        let entry5 = ModifierEntry::from_f32("trade_steering", 0.25);
        assert!(apply_modifier(&mut modifiers, "GEN", &entry5, &tracker));
        assert_eq!(
            modifiers.country_trade_steering.get("GEN"),
            Some(&Fixed::from_f32(0.25))
        );
    }

    #[test]
    fn test_apply_administrative_modifiers() {
        let mut modifiers = GameModifiers::default();
        let tracker = ModifierStubTracker::new();

        // development_cost
        let entry1 = ModifierEntry::from_f32("development_cost", -0.10);
        assert!(apply_modifier(&mut modifiers, "FRA", &entry1, &tracker));
        assert_eq!(
            modifiers.country_development_cost.get("FRA"),
            Some(&Fixed::from_f32(-0.10))
        );

        // core_creation
        let entry2 = ModifierEntry::from_f32("core_creation", -0.25);
        assert!(apply_modifier(&mut modifiers, "ADM", &entry2, &tracker));
        assert_eq!(
            modifiers.country_core_creation.get("ADM"),
            Some(&Fixed::from_f32(-0.25))
        );

        // ae_impact
        let entry3 = ModifierEntry::from_f32("ae_impact", -0.20);
        assert!(apply_modifier(&mut modifiers, "DIP", &entry3, &tracker));
        assert_eq!(
            modifiers.country_ae_impact.get("DIP"),
            Some(&Fixed::from_f32(-0.20))
        );

        // diplomatic_reputation
        let entry4 = ModifierEntry::from_f32("diplomatic_reputation", 2.0);
        assert!(apply_modifier(&mut modifiers, "AUS", &entry4, &tracker));
        assert_eq!(
            modifiers.country_diplomatic_reputation.get("AUS"),
            Some(&Fixed::from_f32(2.0))
        );
    }

    #[test]
    fn test_apply_maintenance_modifiers() {
        let mut modifiers = GameModifiers::default();
        let tracker = ModifierStubTracker::new();

        // infantry_cost
        let entry1 = ModifierEntry::from_f32("infantry_cost", -0.10);
        assert!(apply_modifier(&mut modifiers, "PRU", &entry1, &tracker));
        assert_eq!(
            modifiers.country_infantry_cost.get("PRU"),
            Some(&Fixed::from_f32(-0.10))
        );

        // cavalry_cost
        let entry2 = ModifierEntry::from_f32("cavalry_cost", -0.15);
        assert!(apply_modifier(&mut modifiers, "POL", &entry2, &tracker));
        assert_eq!(
            modifiers.country_cavalry_cost.get("POL"),
            Some(&Fixed::from_f32(-0.15))
        );

        // mercenary_cost (with alias)
        let entry3 = ModifierEntry::from_f32("mercenary_cost", -0.25);
        assert!(apply_modifier(&mut modifiers, "VEN", &entry3, &tracker));
        let entry4 = ModifierEntry::from_f32("mercenary_maintenance", -0.10);
        assert!(apply_modifier(&mut modifiers, "VEN", &entry4, &tracker));
        assert_eq!(
            modifiers.country_mercenary_cost.get("VEN"),
            Some(&Fixed::from_f32(-0.35))
        );
    }

    #[test]
    fn test_recalculate_idea_modifiers() {
        let registry = make_test_registry();
        let mut modifiers = GameModifiers::default();
        let tracker = ModifierStubTracker::new();

        // Get the TEST_ideas group ID
        let test_id = registry.id_by_name("TEST_ideas").unwrap();

        let country = CountryState {
            ideas: CountryIdeaState {
                national_ideas: Some(test_id),
                national_ideas_progress: 7, // Full unlock
                ..Default::default()
            },
            ..Default::default()
        };

        let stats =
            recalculate_idea_modifiers(&mut modifiers, "TST", &country, &registry, &tracker);

        // Start (0.10) + idea_1 (-0.10 land maintenance) + bonus (0.05) = applied
        // idea_2 global_manpower_modifier = stubbed
        assert_eq!(stats.applied, 3); // global_tax x2 + land_maintenance
        assert_eq!(stats.stubbed, 1); // global_manpower_modifier

        // Check tax modifier was applied (0.10 start + 0.05 bonus = 0.15)
        assert_eq!(
            modifiers.country_tax_modifier.get("TST"),
            Some(&Fixed::from_f32(0.15))
        );

        // Check land maintenance was applied
        assert_eq!(
            modifiers.land_maintenance_modifier.get("TST"),
            Some(&Fixed::from_f32(-0.10))
        );

        // Check stubs were tracked
        assert!(tracker
            .unimplemented_keys()
            .contains(&"global_manpower_modifier".to_string()));
    }

    #[test]
    fn test_scan_all_modifiers() {
        let registry = make_test_registry();
        let counts = scan_all_modifiers(&registry);

        assert_eq!(counts.get("global_tax_modifier"), Some(&2)); // start + bonus
        assert_eq!(counts.get("land_maintenance_modifier"), Some(&1));
        assert_eq!(counts.get("global_manpower_modifier"), Some(&1));
    }
}
