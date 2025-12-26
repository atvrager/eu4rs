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
            // Manpower/Stats
                | "global_manpower_modifier"
                | "prestige"
                | "devotion"
                | "horde_unity"
                | "legitimacy"
                | "republican_tradition"
                | "meritocracy"
            // Fort/Stability
                | "defensiveness"
                | "global_unrest"
                | "stability_cost_modifier"
            // Tolerance/Religion
                | "tolerance_own"
            // Economic
                | "global_trade_goods_size_modifier"
                | "build_cost"
            // Military
                | "manpower_recovery_speed"
                | "hostile_attrition"
            // Diplomatic/Culture
                | "diplomatic_upkeep"
                | "idea_cost"
                | "merchants"
                | "global_missionary_strength"
                | "num_accepted_cultures"
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

        // === Manpower/Stats modifiers ===
        "global_manpower_modifier" => {
            let current = modifiers
                .country_manpower
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_manpower
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "prestige" => {
            let current = modifiers
                .country_prestige
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_prestige
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "devotion" => {
            let current = modifiers
                .country_devotion
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_devotion
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "horde_unity" => {
            let current = modifiers
                .country_horde_unity
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_horde_unity
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "legitimacy" => {
            let current = modifiers
                .country_legitimacy
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_legitimacy
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "republican_tradition" => {
            let current = modifiers
                .country_republican_tradition
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_republican_tradition
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "meritocracy" => {
            let current = modifiers
                .country_meritocracy
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_meritocracy
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "defensiveness" => {
            let current = modifiers
                .country_defensiveness
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_defensiveness
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "global_unrest" => {
            let current = modifiers
                .country_unrest
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_unrest
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "stability_cost_modifier" => {
            let current = modifiers
                .country_stability_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_stability_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Tolerance/Religion modifiers ===
        "tolerance_own" => {
            let current = modifiers
                .country_tolerance_own
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_tolerance_own
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Economic modifiers ===
        "global_trade_goods_size_modifier" => {
            let current = modifiers
                .country_trade_goods_size
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_trade_goods_size
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "build_cost" => {
            let current = modifiers
                .country_build_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_build_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Military modifiers ===
        "manpower_recovery_speed" => {
            let current = modifiers
                .country_manpower_recovery_speed
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_manpower_recovery_speed
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "hostile_attrition" => {
            let current = modifiers
                .country_hostile_attrition
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_hostile_attrition
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Diplomatic/Culture modifiers ===
        "diplomatic_upkeep" => {
            let current = modifiers
                .country_diplomatic_upkeep
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_diplomatic_upkeep
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "idea_cost" => {
            let current = modifiers
                .country_idea_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_idea_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "merchants" => {
            let current = modifiers
                .country_merchants
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_merchants
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "global_missionary_strength" => {
            let current = modifiers
                .country_missionary_strength
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_missionary_strength
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "num_accepted_cultures" => {
            let current = modifiers
                .country_num_accepted_cultures
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_num_accepted_cultures
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

        let entry = ModifierEntry::from_f32("improve_relation_modifier", 0.25);
        let applied = apply_modifier(&mut modifiers, "FRA", &entry, &tracker);

        assert!(!applied);
        assert!(tracker
            .unimplemented_keys()
            .contains(&"improve_relation_modifier".to_string()));
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
    fn test_apply_manpower_stats_modifiers() {
        let mut modifiers = GameModifiers::default();
        let tracker = ModifierStubTracker::new();

        // global_manpower_modifier
        let entry1 = ModifierEntry::from_f32("global_manpower_modifier", 0.25);
        assert!(apply_modifier(&mut modifiers, "MOS", &entry1, &tracker));
        assert_eq!(
            modifiers.country_manpower.get("MOS"),
            Some(&Fixed::from_f32(0.25))
        );

        // prestige
        let entry2 = ModifierEntry::from_f32("prestige", 1.0);
        assert!(apply_modifier(&mut modifiers, "FRA", &entry2, &tracker));
        assert_eq!(
            modifiers.country_prestige.get("FRA"),
            Some(&Fixed::from_f32(1.0))
        );

        // devotion (theocracy government stat)
        let entry3 = ModifierEntry::from_f32("devotion", 0.5);
        assert!(apply_modifier(&mut modifiers, "PAP", &entry3, &tracker));
        assert_eq!(
            modifiers.country_devotion.get("PAP"),
            Some(&Fixed::from_f32(0.5))
        );

        // horde_unity (steppe horde government stat)
        let entry4 = ModifierEntry::from_f32("horde_unity", 1.0);
        assert!(apply_modifier(&mut modifiers, "KZH", &entry4, &tracker));
        assert_eq!(
            modifiers.country_horde_unity.get("KZH"),
            Some(&Fixed::from_f32(1.0))
        );

        // legitimacy (monarchy government stat)
        let entry5 = ModifierEntry::from_f32("legitimacy", 0.5);
        assert!(apply_modifier(&mut modifiers, "CAS", &entry5, &tracker));
        assert_eq!(
            modifiers.country_legitimacy.get("CAS"),
            Some(&Fixed::from_f32(0.5))
        );
    }

    #[test]
    fn test_apply_government_and_stability_modifiers() {
        let mut modifiers = GameModifiers::default();
        let tracker = ModifierStubTracker::new();

        // republican_tradition (republic government stat)
        let entry1 = ModifierEntry::from_f32("republican_tradition", 0.5);
        assert!(apply_modifier(&mut modifiers, "VEN", &entry1, &tracker));
        assert_eq!(
            modifiers.country_republican_tradition.get("VEN"),
            Some(&Fixed::from_f32(0.5))
        );

        // meritocracy (celestial empire government stat)
        let entry2 = ModifierEntry::from_f32("meritocracy", 1.0);
        assert!(apply_modifier(&mut modifiers, "MNG", &entry2, &tracker));
        assert_eq!(
            modifiers.country_meritocracy.get("MNG"),
            Some(&Fixed::from_f32(1.0))
        );

        // defensiveness (fort defense bonus)
        let entry3 = ModifierEntry::from_f32("defensiveness", 0.25);
        assert!(apply_modifier(&mut modifiers, "BYZ", &entry3, &tracker));
        assert_eq!(
            modifiers.country_defensiveness.get("BYZ"),
            Some(&Fixed::from_f32(0.25))
        );

        // global_unrest (province unrest modifier)
        let entry4 = ModifierEntry::from_f32("global_unrest", -2.0);
        assert!(apply_modifier(&mut modifiers, "PRU", &entry4, &tracker));
        assert_eq!(
            modifiers.country_unrest.get("PRU"),
            Some(&Fixed::from_f32(-2.0))
        );

        // stability_cost_modifier (stability increase cost)
        let entry5 = ModifierEntry::from_f32("stability_cost_modifier", -0.10);
        assert!(apply_modifier(&mut modifiers, "FRA", &entry5, &tracker));
        assert_eq!(
            modifiers.country_stability_cost.get("FRA"),
            Some(&Fixed::from_f32(-0.10))
        );
    }

    #[test]
    fn test_apply_tolerance_and_economy_modifiers() {
        let mut modifiers = GameModifiers::default();
        let tracker = ModifierStubTracker::new();

        // tolerance_own
        let entry1 = ModifierEntry::from_f32("tolerance_own", 2.0);
        assert!(apply_modifier(&mut modifiers, "SPA", &entry1, &tracker));
        assert_eq!(
            modifiers.country_tolerance_own.get("SPA"),
            Some(&Fixed::from_f32(2.0))
        );

        // global_trade_goods_size_modifier
        let entry2 = ModifierEntry::from_f32("global_trade_goods_size_modifier", 0.10);
        assert!(apply_modifier(&mut modifiers, "ENG", &entry2, &tracker));
        assert_eq!(
            modifiers.country_trade_goods_size.get("ENG"),
            Some(&Fixed::from_f32(0.10))
        );

        // build_cost
        let entry3 = ModifierEntry::from_f32("build_cost", -0.10);
        assert!(apply_modifier(&mut modifiers, "PRU", &entry3, &tracker));
        assert_eq!(
            modifiers.country_build_cost.get("PRU"),
            Some(&Fixed::from_f32(-0.10))
        );

        // manpower_recovery_speed
        let entry4 = ModifierEntry::from_f32("manpower_recovery_speed", 0.20);
        assert!(apply_modifier(&mut modifiers, "RUS", &entry4, &tracker));
        assert_eq!(
            modifiers.country_manpower_recovery_speed.get("RUS"),
            Some(&Fixed::from_f32(0.20))
        );

        // hostile_attrition
        let entry5 = ModifierEntry::from_f32("hostile_attrition", 1.0);
        assert!(apply_modifier(&mut modifiers, "SWE", &entry5, &tracker));
        assert_eq!(
            modifiers.country_hostile_attrition.get("SWE"),
            Some(&Fixed::from_f32(1.0))
        );
    }

    #[test]
    fn test_apply_diplomatic_and_culture_modifiers() {
        let mut modifiers = GameModifiers::default();
        let tracker = ModifierStubTracker::new();

        // diplomatic_upkeep
        let entry1 = ModifierEntry::from_f32("diplomatic_upkeep", 1.0);
        assert!(apply_modifier(&mut modifiers, "FRA", &entry1, &tracker));
        assert_eq!(
            modifiers.country_diplomatic_upkeep.get("FRA"),
            Some(&Fixed::from_f32(1.0))
        );

        // idea_cost
        let entry2 = ModifierEntry::from_f32("idea_cost", -0.10);
        assert!(apply_modifier(&mut modifiers, "PRU", &entry2, &tracker));
        assert_eq!(
            modifiers.country_idea_cost.get("PRU"),
            Some(&Fixed::from_f32(-0.10))
        );

        // merchants
        let entry3 = ModifierEntry::from_f32("merchants", 1.0);
        assert!(apply_modifier(&mut modifiers, "VEN", &entry3, &tracker));
        assert_eq!(
            modifiers.country_merchants.get("VEN"),
            Some(&Fixed::from_f32(1.0))
        );

        // global_missionary_strength
        let entry4 = ModifierEntry::from_f32("global_missionary_strength", 0.02);
        assert!(apply_modifier(&mut modifiers, "SPA", &entry4, &tracker));
        assert_eq!(
            modifiers.country_missionary_strength.get("SPA"),
            Some(&Fixed::from_f32(0.02))
        );

        // num_accepted_cultures
        let entry5 = ModifierEntry::from_f32("num_accepted_cultures", 2.0);
        assert!(apply_modifier(&mut modifiers, "TUR", &entry5, &tracker));
        assert_eq!(
            modifiers.country_num_accepted_cultures.get("TUR"),
            Some(&Fixed::from_f32(2.0))
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

        // Start (0.10) + idea_1 (-0.10 land maintenance) + bonus (0.05) + idea_2 (global_manpower) = applied
        assert_eq!(stats.applied, 4); // global_tax x2 + land_maintenance + global_manpower_modifier
        assert_eq!(stats.stubbed, 0);

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

        // Check global_manpower_modifier was applied
        assert_eq!(
            modifiers.country_manpower.get("TST"),
            Some(&Fixed::from_f32(0.15))
        );
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
