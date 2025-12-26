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
            "global_tax_modifier"
                | "land_maintenance_modifier"
                | "fort_maintenance_modifier"
                | "production_efficiency"
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
                    modifiers: vec![ModifierEntry::from_f32("cavalry_power", 0.15)], // Stub
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

        let entry = ModifierEntry::from_f32("cavalry_power", 0.15);
        let applied = apply_modifier(&mut modifiers, "FRA", &entry, &tracker);

        assert!(!applied);
        assert!(tracker
            .unimplemented_keys()
            .contains(&"cavalry_power".to_string()));
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
        // idea_2 cavalry_power = stubbed
        assert_eq!(stats.applied, 3); // global_tax x2 + land_maintenance
        assert_eq!(stats.stubbed, 1); // cavalry_power

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
            .contains(&"cavalry_power".to_string()));
    }

    #[test]
    fn test_scan_all_modifiers() {
        let registry = make_test_registry();
        let counts = scan_all_modifiers(&registry);

        assert_eq!(counts.get("global_tax_modifier"), Some(&2)); // start + bonus
        assert_eq!(counts.get("land_maintenance_modifier"), Some(&1));
        assert_eq!(counts.get("cavalry_power"), Some(&1));
    }
}
