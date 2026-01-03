//\! Unit tests for ideas.rs idea group system.
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

    // Use a modifier that definitely doesn't exist
    let entry = ModifierEntry::from_f32("unknown_test_modifier_xyz", 0.1);
    let applied = apply_modifier(&mut modifiers, "FRA", &entry, &tracker);

    assert!(!applied);
    assert!(tracker
        .unimplemented_keys()
        .contains(&"unknown_test_modifier_xyz".to_string()));
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

    let stats = recalculate_idea_modifiers(&mut modifiers, "TST", &country, &registry, &tracker);

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

#[test]
fn test_apply_diplomacy_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();

    // Test improve_relation_modifier
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry {
            key: "improve_relation_modifier".to_string(),
            value: Fixed::from_f32(0.25),
        },
        &stubs,
    );
    assert_eq!(
        modifiers.country_improve_relation_modifier.get("TST"),
        Some(&Fixed::from_f32(0.25))
    );

    // Test diplomats (additive)
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry {
            key: "diplomats".to_string(),
            value: Fixed::from_int(1),
        },
        &stubs,
    );
    assert_eq!(
        modifiers.country_diplomats.get("TST"),
        Some(&Fixed::from_int(1))
    );

    // Test diplomatic_annexation_cost
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry {
            key: "diplomatic_annexation_cost".to_string(),
            value: Fixed::from_f32(-0.25),
        },
        &stubs,
    );
    assert_eq!(
        modifiers.country_diplomatic_annexation_cost.get("TST"),
        Some(&Fixed::from_f32(-0.25))
    );
}

#[test]
fn test_apply_technology_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();

    // Test technology_cost
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry {
            key: "technology_cost".to_string(),
            value: Fixed::from_f32(-0.10),
        },
        &stubs,
    );
    assert_eq!(
        modifiers.country_technology_cost.get("TST"),
        Some(&Fixed::from_f32(-0.10))
    );

    // Test adm_tech_cost_modifier
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry {
            key: "adm_tech_cost_modifier".to_string(),
            value: Fixed::from_f32(-0.05),
        },
        &stubs,
    );
    assert_eq!(
        modifiers.country_adm_tech_cost.get("TST"),
        Some(&Fixed::from_f32(-0.05))
    );
}

#[test]
fn test_apply_force_limit_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();

    // Test land_forcelimit_modifier
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry {
            key: "land_forcelimit_modifier".to_string(),
            value: Fixed::from_f32(0.50),
        },
        &stubs,
    );
    assert_eq!(
        modifiers.country_land_forcelimit.get("TST"),
        Some(&Fixed::from_f32(0.50))
    );

    // Test naval_forcelimit_modifier
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry {
            key: "naval_forcelimit_modifier".to_string(),
            value: Fixed::from_f32(0.33),
        },
        &stubs,
    );
    assert_eq!(
        modifiers.country_naval_forcelimit.get("TST"),
        Some(&Fixed::from_f32(0.33))
    );
}

#[test]
fn test_apply_tradition_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();

    // Test army_tradition
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry {
            key: "army_tradition".to_string(),
            value: Fixed::from_int(1),
        },
        &stubs,
    );
    assert_eq!(
        modifiers.country_army_tradition.get("TST"),
        Some(&Fixed::from_int(1))
    );

    // Test army_tradition_decay
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry {
            key: "army_tradition_decay".to_string(),
            value: Fixed::from_f32(-0.01),
        },
        &stubs,
    );
    assert_eq!(
        modifiers.country_army_tradition_decay.get("TST"),
        Some(&Fixed::from_f32(-0.01))
    );

    // Test navy_tradition
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry {
            key: "navy_tradition".to_string(),
            value: Fixed::from_int(1),
        },
        &stubs,
    );
    assert_eq!(
        modifiers.country_navy_tradition.get("TST"),
        Some(&Fixed::from_int(1))
    );
}

#[test]
fn test_apply_combat_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();

    // Test fire_damage
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry {
            key: "fire_damage".to_string(),
            value: Fixed::from_f32(0.10),
        },
        &stubs,
    );
    assert_eq!(
        modifiers.country_fire_damage.get("TST"),
        Some(&Fixed::from_f32(0.10))
    );

    // Test shock_damage
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry {
            key: "shock_damage".to_string(),
            value: Fixed::from_f32(0.10),
        },
        &stubs,
    );
    assert_eq!(
        modifiers.country_shock_damage.get("TST"),
        Some(&Fixed::from_f32(0.10))
    );

    // Test naval_morale
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry {
            key: "naval_morale".to_string(),
            value: Fixed::from_f32(0.15),
        },
        &stubs,
    );
    assert_eq!(
        modifiers.country_naval_morale.get("TST"),
        Some(&Fixed::from_f32(0.15))
    );
}

#[test]
fn test_apply_tolerance_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();

    // Test tolerance_heretic
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry {
            key: "tolerance_heretic".to_string(),
            value: Fixed::from_int(2),
        },
        &stubs,
    );
    assert_eq!(
        modifiers.country_tolerance_heretic.get("TST"),
        Some(&Fixed::from_int(2))
    );

    // Test tolerance_heathen
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry {
            key: "tolerance_heathen".to_string(),
            value: Fixed::from_int(1),
        },
        &stubs,
    );
    assert_eq!(
        modifiers.country_tolerance_heathen.get("TST"),
        Some(&Fixed::from_int(1))
    );
}

#[test]
fn test_apply_colonization_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();

    // Test colonists
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry {
            key: "colonists".to_string(),
            value: Fixed::from_int(1),
        },
        &stubs,
    );
    assert_eq!(
        modifiers.country_colonists.get("TST"),
        Some(&Fixed::from_int(1))
    );

    // Test global_colonial_growth
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry {
            key: "global_colonial_growth".to_string(),
            value: Fixed::from_int(10),
        },
        &stubs,
    );
    assert_eq!(
        modifiers.country_global_colonial_growth.get("TST"),
        Some(&Fixed::from_int(10))
    );
}

#[test]
fn test_modifier_stacking_new_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();

    // Apply army_tradition twice
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry {
            key: "army_tradition".to_string(),
            value: Fixed::from_int(1),
        },
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry {
            key: "army_tradition".to_string(),
            value: Fixed::from_f32(0.5),
        },
        &stubs,
    );

    // Should sum to 1.5
    assert_eq!(
        modifiers.country_army_tradition.get("TST"),
        Some(&Fixed::from_f32(1.5))
    );
}

#[test]
fn test_all_56_new_modifiers_implemented() {
    // Verify all 56 new modifiers are in is_implemented()
    let new_modifiers = vec![
        "improve_relation_modifier",
        "diplomats",
        "diplomatic_annexation_cost",
        "vassal_income",
        "fabricate_claims_cost",
        "spy_offence",
        "technology_cost",
        "adm_tech_cost_modifier",
        "governing_capacity_modifier",
        "land_forcelimit_modifier",
        "naval_forcelimit_modifier",
        "global_sailors_modifier",
        "sailor_maintenance_modifer",
        "army_tradition",
        "army_tradition_decay",
        "navy_tradition",
        "leader_land_shock",
        "leader_land_manuever",
        "prestige_decay",
        "fire_damage",
        "shock_damage",
        "shock_damage_received",
        "naval_morale",
        "siege_ability",
        "movement_speed",
        "land_attrition",
        "war_exhaustion",
        "global_ship_cost",
        "light_ship_cost",
        "ship_durability",
        "galley_power",
        "privateer_efficiency",
        "global_ship_trade_power",
        "trade_range_modifier",
        "global_own_trade_power",
        "global_prov_trade_power_modifier",
        "merc_maintenance_modifier",
        "colonists",
        "global_colonial_growth",
        "years_of_nationalism",
        "tolerance_heretic",
        "tolerance_heathen",
        "religious_unity",
        "global_heretic_missionary_strength",
        "papal_influence",
        "church_power_modifier",
        "advisor_cost",
        "advisor_pool",
        "culture_conversion_cost",
        "inflation_reduction",
        "global_autonomy",
        "state_maintenance_modifier",
        "garrison_size",
        "global_institution_spread",
        "heir_chance",
        "caravan_power",
    ];

    for modifier in new_modifiers {
        assert!(
            ModifierStubTracker::is_implemented(modifier),
            "Modifier {} should be implemented",
            modifier
        );
    }
}

#[test]
fn test_apply_naval_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();

    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("light_ship_power", 0.20),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("heavy_ship_power", 0.15),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("naval_maintenance_modifier", -0.10),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("naval_attrition", -0.25),
        &stubs,
    );

    assert_eq!(
        modifiers.country_light_ship_power.get("TST"),
        Some(&Fixed::from_f32(0.20))
    );
    assert_eq!(
        modifiers.country_heavy_ship_power.get("TST"),
        Some(&Fixed::from_f32(0.15))
    );
    assert_eq!(
        modifiers.country_naval_maintenance.get("TST"),
        Some(&Fixed::from_f32(-0.10))
    );
    assert_eq!(
        modifiers.country_naval_attrition.get("TST"),
        Some(&Fixed::from_f32(-0.25))
    );
}

#[test]
fn test_apply_mercenary_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();

    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("mercenary_discipline", 0.05),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("mercenary_manpower", 0.25),
        &stubs,
    );

    assert_eq!(
        modifiers.country_mercenary_discipline.get("TST"),
        Some(&Fixed::from_f32(0.05))
    );
    assert_eq!(
        modifiers.country_mercenary_manpower.get("TST"),
        Some(&Fixed::from_f32(0.25))
    );
}

#[test]
fn test_apply_war_and_diplomacy_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();

    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("unjustified_demands", -0.50),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("province_warscore_cost", -0.10),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("envoy_travel_time", -0.25),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::new("reduced_liberty_desire", Fixed::from_int(-10)),
        &stubs,
    );

    assert_eq!(
        modifiers.country_unjustified_demands.get("TST"),
        Some(&Fixed::from_f32(-0.50))
    );
    assert_eq!(
        modifiers.country_province_warscore_cost.get("TST"),
        Some(&Fixed::from_f32(-0.10))
    );
    assert_eq!(
        modifiers.country_envoy_travel_time.get("TST"),
        Some(&Fixed::from_f32(-0.25))
    );
    assert_eq!(
        modifiers.country_reduced_liberty_desire.get("TST"),
        Some(&Fixed::from_int(-10))
    );
}

#[test]
fn test_apply_recruitment_and_economy_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();

    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("global_regiment_cost", -0.10),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("global_regiment_recruit_speed", 0.25),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("interest", -0.50),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("prestige_from_land", 0.10),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("loot_amount", 0.50),
        &stubs,
    );

    assert_eq!(
        modifiers.country_global_regiment_cost.get("TST"),
        Some(&Fixed::from_f32(-0.10))
    );
    assert_eq!(
        modifiers.country_global_regiment_recruit_speed.get("TST"),
        Some(&Fixed::from_f32(0.25))
    );
    assert_eq!(
        modifiers.country_interest.get("TST"),
        Some(&Fixed::from_f32(-0.50))
    );
    assert_eq!(
        modifiers.country_prestige_from_land.get("TST"),
        Some(&Fixed::from_f32(0.10))
    );
    assert_eq!(
        modifiers.country_loot_amount.get("TST"),
        Some(&Fixed::from_f32(0.50))
    );
}

#[test]
fn test_apply_leader_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();

    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::new("leader_land_fire", Fixed::from_int(1)),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::new("leader_siege", Fixed::from_int(1)),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::new("leader_naval_fire", Fixed::from_int(1)),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::new("leader_naval_manuever", Fixed::from_int(1)),
        &stubs,
    );

    assert_eq!(
        modifiers.country_leader_land_fire.get("TST"),
        Some(&Fixed::from_int(1))
    );
    assert_eq!(
        modifiers.country_leader_siege.get("TST"),
        Some(&Fixed::from_int(1))
    );
    assert_eq!(
        modifiers.country_leader_naval_fire.get("TST"),
        Some(&Fixed::from_int(1))
    );
    assert_eq!(
        modifiers.country_leader_naval_manuever.get("TST"),
        Some(&Fixed::from_int(1))
    );
}

#[test]
fn test_apply_government_reform_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();

    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("reform_progress_growth", 0.25),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("administrative_efficiency", 0.05),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("yearly_absolutism", 0.50),
        &stubs,
    );

    assert_eq!(
        modifiers.country_reform_progress_growth.get("TST"),
        Some(&Fixed::from_f32(0.25))
    );
    assert_eq!(
        modifiers.country_administrative_efficiency.get("TST"),
        Some(&Fixed::from_f32(0.05))
    );
    assert_eq!(
        modifiers.country_yearly_absolutism.get("TST"),
        Some(&Fixed::from_f32(0.50))
    );
}

#[test]
fn test_apply_estate_loyalty_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();

    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("burghers_loyalty_modifier", 0.10),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("nobles_loyalty_modifier", 0.10),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("church_loyalty_modifier", 0.10),
        &stubs,
    );

    assert_eq!(
        modifiers.country_burghers_loyalty.get("TST"),
        Some(&Fixed::from_f32(0.10))
    );
    assert_eq!(
        modifiers.country_nobles_loyalty.get("TST"),
        Some(&Fixed::from_f32(0.10))
    );
    assert_eq!(
        modifiers.country_church_loyalty.get("TST"),
        Some(&Fixed::from_f32(0.10))
    );
}

#[test]
fn test_apply_combat_and_reinforcement_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();

    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("recover_army_morale_speed", 0.10),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("fire_damage_received", -0.10),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("cavalry_flanking", 0.50),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("cav_to_inf_ratio", 0.25),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("reinforce_speed", 0.33),
        &stubs,
    );

    assert_eq!(
        modifiers.country_recover_army_morale_speed.get("TST"),
        Some(&Fixed::from_f32(0.10))
    );
    assert_eq!(
        modifiers.country_fire_damage_received.get("TST"),
        Some(&Fixed::from_f32(-0.10))
    );
    assert_eq!(
        modifiers.country_cavalry_flanking.get("TST"),
        Some(&Fixed::from_f32(0.50))
    );
    assert_eq!(
        modifiers.country_cav_to_inf_ratio.get("TST"),
        Some(&Fixed::from_f32(0.25))
    );
    assert_eq!(
        modifiers.country_reinforce_speed.get("TST"),
        Some(&Fixed::from_f32(0.33))
    );
}

#[test]
fn test_apply_espionage_and_tradition_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();

    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("global_spy_defence", 0.20),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("rebel_support_efficiency", 0.50),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("navy_tradition_decay", -0.01),
        &stubs,
    );
    apply_modifier(
        &mut modifiers,
        "TST",
        &ModifierEntry::from_f32("army_tradition_from_battle", 0.50),
        &stubs,
    );

    assert_eq!(
        modifiers.country_global_spy_defence.get("TST"),
        Some(&Fixed::from_f32(0.20))
    );
    assert_eq!(
        modifiers.country_rebel_support_efficiency.get("TST"),
        Some(&Fixed::from_f32(0.50))
    );
    assert_eq!(
        modifiers.country_navy_tradition_decay.get("TST"),
        Some(&Fixed::from_f32(-0.01))
    );
    assert_eq!(
        modifiers.country_army_tradition_from_battle.get("TST"),
        Some(&Fixed::from_f32(0.50))
    );
}

#[test]
fn test_all_50_new_modifiers_implemented() {
    let new_modifiers = vec![
        "missionaries",
        "light_ship_power",
        "heavy_ship_power",
        "naval_maintenance_modifier",
        "naval_attrition",
        "mercenary_discipline",
        "mercenary_manpower",
        "unjustified_demands",
        "province_warscore_cost",
        "envoy_travel_time",
        "reduced_liberty_desire",
        "global_regiment_cost",
        "global_regiment_recruit_speed",
        "interest",
        "prestige_from_land",
        "loot_amount",
        "leader_land_fire",
        "leader_siege",
        "leader_naval_fire",
        "leader_naval_manuever",
        "galley_cost",
        "global_ship_recruit_speed",
        "reform_progress_growth",
        "administrative_efficiency",
        "yearly_absolutism",
        "monthly_fervor_increase",
        "monthly_piety",
        "burghers_loyalty_modifier",
        "nobles_loyalty_modifier",
        "church_loyalty_modifier",
        "recover_army_morale_speed",
        "fire_damage_received",
        "cavalry_flanking",
        "cav_to_inf_ratio",
        "reinforce_speed",
        "global_spy_defence",
        "rebel_support_efficiency",
        "navy_tradition_decay",
        "army_tradition_from_battle",
        "embargo_efficiency",
        "allowed_marine_fraction",
        "capture_ship_chance",
        "vassal_forcelimit_bonus",
        "same_culture_advisor_cost",
        "global_garrison_growth",
        "war_exhaustion_cost",
        "global_foreign_trade_power",
        "range",
        "female_advisor_chance",
        "yearly_corruption",
        "build_time",
        "promote_culture_cost",
        "liberty_desire_from_subject_development",
    ];

    for modifier in new_modifiers {
        assert!(
            ModifierStubTracker::is_implemented(modifier),
            "Modifier {} should be implemented",
            modifier
        );
    }
}

#[test]
fn test_apply_final_naval_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();
    let tag = "ENG";

    // Test sunk_ship_morale_hit_recieved
    let entry = ModifierEntry::new("sunk_ship_morale_hit_recieved", Fixed::from_f32(-0.33));
    assert!(apply_modifier(&mut modifiers, tag, &entry, &stubs));
    assert_eq!(
        modifiers.country_sunk_ship_morale_hit_recieved.get(tag),
        Some(&Fixed::from_f32(-0.33))
    );

    // Test sailors_recovery_speed
    let entry = ModifierEntry::new("sailors_recovery_speed", Fixed::from_f32(0.2));
    assert!(apply_modifier(&mut modifiers, tag, &entry, &stubs));
    assert_eq!(
        modifiers.country_sailors_recovery_speed.get(tag),
        Some(&Fixed::from_f32(0.2))
    );

    // Test own_coast_naval_combat_bonus
    let entry = ModifierEntry::new("own_coast_naval_combat_bonus", Fixed::from_int(1));
    assert!(apply_modifier(&mut modifiers, tag, &entry, &stubs));
    assert_eq!(
        modifiers.country_own_coast_naval_combat_bonus.get(tag),
        Some(&Fixed::from_int(1))
    );
}

#[test]
fn test_apply_tech_cost_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();
    let tag = "PRU";

    // Test mil_tech_cost_modifier
    let entry = ModifierEntry::new("mil_tech_cost_modifier", Fixed::from_f32(-0.1));
    assert!(apply_modifier(&mut modifiers, tag, &entry, &stubs));
    assert_eq!(
        modifiers.country_mil_tech_cost.get(tag),
        Some(&Fixed::from_f32(-0.1))
    );

    // Test dip_tech_cost_modifier
    let entry = ModifierEntry::new("dip_tech_cost_modifier", Fixed::from_f32(-0.05));
    assert!(apply_modifier(&mut modifiers, tag, &entry, &stubs));
    assert_eq!(
        modifiers.country_dip_tech_cost.get(tag),
        Some(&Fixed::from_f32(-0.05))
    );

    // Test stacking multiple sources
    let entry2 = ModifierEntry::new("mil_tech_cost_modifier", Fixed::from_f32(-0.05));
    assert!(apply_modifier(&mut modifiers, tag, &entry2, &stubs));
    assert_eq!(
        modifiers.country_mil_tech_cost.get(tag),
        Some(&Fixed::from_f32(-0.15))
    );
}

#[test]
fn test_apply_government_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();
    let tag = "FRA";

    // Test max_absolutism
    let entry = ModifierEntry::new("max_absolutism", Fixed::from_int(5));
    assert!(apply_modifier(&mut modifiers, tag, &entry, &stubs));
    assert_eq!(
        modifiers.country_max_absolutism.get(tag),
        Some(&Fixed::from_int(5))
    );

    // Test num_of_pronoiars
    let entry = ModifierEntry::new("num_of_pronoiars", Fixed::from_int(2));
    assert!(apply_modifier(&mut modifiers, tag, &entry, &stubs));
    assert_eq!(
        modifiers.country_num_of_pronoiars.get(tag),
        Some(&Fixed::from_int(2))
    );

    // Test max_revolutionary_zeal
    let entry = ModifierEntry::new("max_revolutionary_zeal", Fixed::from_int(10));
    assert!(apply_modifier(&mut modifiers, tag, &entry, &stubs));
    assert_eq!(
        modifiers.country_max_revolutionary_zeal.get(tag),
        Some(&Fixed::from_int(10))
    );

    // Test possible_policy
    let entry = ModifierEntry::new("possible_policy", Fixed::from_int(1));
    assert!(apply_modifier(&mut modifiers, tag, &entry, &stubs));
    assert_eq!(
        modifiers.country_possible_policy.get(tag),
        Some(&Fixed::from_int(1))
    );
}

#[test]
fn test_apply_power_projection_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();
    let tag = "GBR";

    // Test power_projection_from_insults
    let entry = ModifierEntry::new("power_projection_from_insults", Fixed::from_f32(0.5));
    assert!(apply_modifier(&mut modifiers, tag, &entry, &stubs));
    assert_eq!(
        modifiers.country_power_projection_from_insults.get(tag),
        Some(&Fixed::from_f32(0.5))
    );
}

#[test]
fn test_apply_rebellion_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();
    let tag = "TUR";

    // Test harsh_treatment_cost
    let entry = ModifierEntry::new("harsh_treatment_cost", Fixed::from_f32(-0.25));
    assert!(apply_modifier(&mut modifiers, tag, &entry, &stubs));
    assert_eq!(
        modifiers.country_harsh_treatment_cost.get(tag),
        Some(&Fixed::from_f32(-0.25))
    );
}

#[test]
fn test_apply_leader_pool_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();
    let tag = "RUS";

    // Test free_leader_pool
    let entry = ModifierEntry::new("free_leader_pool", Fixed::from_int(1));
    assert!(apply_modifier(&mut modifiers, tag, &entry, &stubs));
    assert_eq!(
        modifiers.country_free_leader_pool.get(tag),
        Some(&Fixed::from_int(1))
    );
}

#[test]
fn test_apply_innovation_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();
    let tag = "SPA";

    // Test embracement_cost
    let entry = ModifierEntry::new("embracement_cost", Fixed::from_f32(-0.1));
    assert!(apply_modifier(&mut modifiers, tag, &entry, &stubs));
    assert_eq!(
        modifiers.country_embracement_cost.get(tag),
        Some(&Fixed::from_f32(-0.1))
    );
}

#[test]
fn test_apply_artillery_cost_modifiers() {
    let mut modifiers = GameModifiers::default();
    let stubs = ModifierStubTracker::new();
    let tag = "BRA";

    // Test artillery_cost
    let entry = ModifierEntry::new("artillery_cost", Fixed::from_f32(-0.15));
    assert!(apply_modifier(&mut modifiers, tag, &entry, &stubs));
    assert_eq!(
        modifiers.country_artillery_cost.get(tag),
        Some(&Fixed::from_f32(-0.15))
    );
}

#[test]
fn test_all_14_final_modifiers_implemented() {
    // Verify all 14 final modifiers are recognized as implemented
    let final_modifiers = [
        "sunk_ship_morale_hit_recieved",
        "sailors_recovery_speed",
        "mil_tech_cost_modifier",
        "dip_tech_cost_modifier",
        "max_absolutism",
        "num_of_pronoiars",
        "max_revolutionary_zeal",
        "possible_policy",
        "power_projection_from_insults",
        "harsh_treatment_cost",
        "free_leader_pool",
        "own_coast_naval_combat_bonus",
        "embracement_cost",
        "artillery_cost",
    ];

    for modifier in final_modifiers {
        assert!(
            ModifierStubTracker::is_implemented(modifier),
            "Modifier {} should be implemented",
            modifier
        );
    }
}
