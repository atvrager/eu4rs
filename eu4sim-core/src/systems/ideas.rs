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
            // Diplomacy & Relations
                | "improve_relation_modifier"
                | "diplomats"
                | "diplomatic_annexation_cost"
                | "vassal_income"
                | "fabricate_claims_cost"
                | "spy_offence"
            // Technology & Development
                | "technology_cost"
                | "adm_tech_cost_modifier"
                | "governing_capacity_modifier"
            // Force Limits & Manpower
                | "land_forcelimit_modifier"
                | "naval_forcelimit_modifier"
                | "global_sailors_modifier"
                | "sailor_maintenance_modifer"
            // Military Tradition & Leaders
                | "army_tradition"
                | "army_tradition_decay"
                | "navy_tradition"
                | "leader_land_shock"
                | "leader_land_manuever"
                | "prestige_decay"
            // Combat Modifiers
                | "fire_damage"
                | "shock_damage"
                | "shock_damage_received"
                | "naval_morale"
                | "siege_ability"
                | "movement_speed"
            // Attrition & War
                | "land_attrition"
                | "war_exhaustion"
            // Naval Costs & Power
                | "global_ship_cost"
                | "light_ship_cost"
                | "ship_durability"
                | "galley_power"
                | "privateer_efficiency"
                | "global_ship_trade_power"
                | "trade_range_modifier"
            // Trade Power
                | "global_own_trade_power"
                | "global_prov_trade_power_modifier"
            // Mercenary
                | "merc_maintenance_modifier"
            // Colonization
                | "colonists"
                | "global_colonial_growth"
                | "years_of_nationalism"
            // Religion & Tolerance
                | "tolerance_heretic"
                | "tolerance_heathen"
                | "religious_unity"
                | "global_heretic_missionary_strength"
                | "papal_influence"
                | "church_power_modifier"
            // Advisors
                | "advisor_cost"
                | "advisor_pool"
                | "culture_conversion_cost"
            // Economy & State
                | "inflation_reduction"
                | "global_autonomy"
                | "state_maintenance_modifier"
                | "garrison_size"
            // Special Mechanics
                | "global_institution_spread"
                | "heir_chance"
                | "caravan_power"
            // Missionary & Conversion
                | "missionaries"
            // Naval Power & Combat
                | "light_ship_power"
                | "heavy_ship_power"
                | "naval_maintenance_modifier"
                | "naval_attrition"
            // Mercenary Modifiers
                | "mercenary_discipline"
                | "mercenary_manpower"
            // War & Peace
                | "unjustified_demands"
                | "province_warscore_cost"
            // Diplomacy & Travel
                | "envoy_travel_time"
                | "reduced_liberty_desire"
            // Military Recruitment
                | "global_regiment_cost"
                | "global_regiment_recruit_speed"
            // Economy & Finance
                | "interest"
                | "prestige_from_land"
                | "loot_amount"
            // Military Leaders
                | "leader_land_fire"
                | "leader_siege"
                | "leader_naval_fire"
                | "leader_naval_manuever"
            // Naval Costs
                | "galley_cost"
                | "global_ship_recruit_speed"
            // Government & Reform
                | "reform_progress_growth"
                | "administrative_efficiency"
                | "yearly_absolutism"
            // Religion & Faith
                | "monthly_fervor_increase"
                | "monthly_piety"
            // Estate Loyalty
                | "burghers_loyalty_modifier"
                | "nobles_loyalty_modifier"
                | "church_loyalty_modifier"
            // Military Combat
                | "recover_army_morale_speed"
                | "fire_damage_received"
                | "cavalry_flanking"
                | "cav_to_inf_ratio"
                | "reinforce_speed"
            // Espionage & Defense
                | "global_spy_defence"
                | "rebel_support_efficiency"
            // Military Tradition & Decay
                | "navy_tradition_decay"
                | "army_tradition_from_battle"
            // Naval Combat
                | "embargo_efficiency"
                | "allowed_marine_fraction"
                | "capture_ship_chance"
            // Vassal & Subject
                | "vassal_forcelimit_bonus"
                | "same_culture_advisor_cost"
            // Siege & Fortification
                | "global_garrison_growth"
                | "war_exhaustion_cost"
            // Trade
                | "global_foreign_trade_power"
                | "range"
            // Miscellaneous
                | "female_advisor_chance"
                | "yearly_corruption"
                | "build_time"
                | "promote_culture_cost"
                | "liberty_desire_from_subject_development"
            // Naval Combat & Morale
                | "sunk_ship_morale_hit_recieved"
            // Naval Recovery
                | "sailors_recovery_speed"
            // Tech Costs
                | "mil_tech_cost_modifier"
                | "dip_tech_cost_modifier"
            // Government & Absolutism
                | "max_absolutism"
                | "num_of_pronoiars"
                | "max_revolutionary_zeal"
                | "possible_policy"
            // Power Projection
                | "power_projection_from_insults"
            // Rebellion & Unrest
                | "harsh_treatment_cost"
            // Leaders
                | "free_leader_pool"
            // Naval Combat Bonuses
                | "own_coast_naval_combat_bonus"
            // Technology & Innovation
                | "embracement_cost"
            // Military Costs
                | "artillery_cost"
            // Policy-Specific Modifiers (49 modifiers)
            // Colonization
                | "colonist_placement_chance"
                | "native_uprising_chance"
                | "native_assimilation"
            // Naval Combat & Morale
                | "recover_navy_morale_speed"
                | "global_naval_engagement_modifier"
                | "naval_tradition_from_battle"
                | "prestige_from_naval"
                | "disengagement_chance"
                | "leader_naval_shock"
                | "movement_speed_in_fleet_modifier"
                | "morale_damage_received"
            // Army Composition
                | "artillery_fraction"
                | "cavalry_fraction"
                | "infantry_fraction"
            // Economy & Trade
                | "mercantilism_cost"
                | "global_tariffs"
                | "monthly_favor_modifier"
            // Siege & Fortification
                | "siege_blockade_progress"
                | "blockade_efficiency"
                | "garrison_damage"
                | "artillery_level_modifier"
                | "artillery_levels_available_vs_fort"
            // Military Costs & Efficiency
                | "morale_damage"
                | "reinforce_cost_modifier"
                | "drill_gain_modifier"
                | "yearly_army_professionalism"
                | "special_unit_forcelimit"
            // Development & Culture
                | "development_cost_in_primary_culture"
                | "colony_development_boost"
            // Diplomacy & Subjects
                | "rival_border_fort_maintenance"
                | "reduced_liberty_desire_on_same_continent"
                | "years_to_integrate_personal_union"
                | "monthly_federation_favor_growth"
                | "all_estate_loyalty_equilibrium"
            // Religion & Authority
                | "prestige_per_development_from_conversion"
                | "yearly_patriarch_authority"
                | "yearly_harmony"
                | "yearly_karma_decay"
            // Government & Leaders
                | "innovativeness_gain"
                | "raze_power_gain"
                | "monarch_lifespan"
                | "reelection_cost"
                | "mil_advisor_cost"
            // War & Diplomacy
                | "warscore_cost_vs_other_religion"
                | "global_rebel_suppression_efficiency"
            // Naval Infrastructure
                | "global_ship_repair"
                | "transport_attrition"
            // Province Management
                | "manpower_in_true_faith_provinces"
                | "global_monthly_devastation"
            // Batch 1: Positions 21-25 (Frequency-Driven)
                | "monarch_military_power"
                | "center_of_trade_upgrade_cost"
                | "accept_vassalization_reasons"
                | "brahmins_hindu_loyalty_modifier"
                | "brahmins_muslim_loyalty_modifier"
            // Batch 2: Positions 26-30 (Frequency-Driven)
                | "tolerance_of_heathens_capacity"
                | "possible_mil_policy"
                | "curia_powers_cost"
                | "expand_administration_cost"
                | "loyalty_change_on_revoked"
            // Batch 3: Positions 31-35 (Frequency-Driven)
                | "great_project_upgrade_cost"
                | "gold_depletion_chance_modifier"
                | "global_supply_limit_modifier"
                | "general_cost"
                | "leader_cost"
            // Batch 4: Positions 36-40 (Frequency-Driven)
                | "cavalry_fire"
                | "war_taxes_cost_modifier"
                | "vaisyas_loyalty_modifier"
                | "max_hostile_attrition"
                | "nobles_influence_modifier"
            // Estate-Specific Modifiers (19 additional)
                | "dhimmi_loyalty_modifier"
                | "maratha_loyalty_modifier"
                | "rajput_loyalty_modifier"
                | "eunuchs_loyalty_modifier"
                | "ghulams_loyalty_modifier"
                | "janissaries_loyalty_modifier"
                | "qizilbash_loyalty_modifier"
                | "jains_loyalty_modifier"
                | "nomadic_tribes_loyalty_modifier"
                | "clergy_loyalty_modifier"
                | "burghers_influence_modifier"
                | "pr_captains_influence"
                | "all_estate_possible_privileges"
                | "estate_interaction_cooldown_modifier"
                | "cossacks_privilege_slots"
                | "ghulams_privilege_slots"
                | "qizilbash_privilege_slots"
                | "allowed_samurai_fraction"
                | "amount_of_banners"
            // Quick Wins Batch 1: Policy & Monarch Power
                | "free_mil_policy"
                | "free_adm_policy"
                | "free_dip_policy"
                | "possible_dip_policy"
                | "free_policy"
                | "monarch_diplomatic_power"
                | "monarch_admin_power"
                | "country_military_power"
                | "monarch_power_tribute"
            // Quick Wins Batch 2: Religion & Governance
                | "missionary_maintenance_cost"
                | "enforce_religion_cost"
                | "tolerance_of_heretics_capacity"
                | "overextension_impact_modifier"
                | "state_governing_cost"
                | "min_autonomy_in_territories"
                | "autonomy_change_time"
                | "expand_infrastructure_cost_modifier"
            // Quick Wins Batch 3: Advisors & Diplomacy
                | "adm_advisor_cost"
                | "dip_advisor_cost"
                | "same_religion_advisor_cost"
                | "reverse_relation_with_same_religion"
                | "reduced_liberty_desire_on_other_continent"
                | "rival_change_cost"
                | "stability_cost_to_declare_war"
            // Quick Wins Batch 4: Naval & Military
                | "ship_power_propagation"
                | "vassal_naval_forcelimit_bonus"
                | "admiral_cost"
                | "flagship_cost"
                | "heavy_ship_cost"
                | "artillery_fire"
                | "artillery_shock"
                | "infantry_shock"
                | "global_naval_barrage_cost"
                | "landing_penalty"
            // Quick Wins Batch 5: Miscellaneous
                | "monthly_gold_inflation_modifier"
                | "global_prosperity_growth"
                | "spy_action_cost_modifier"
                | "global_allowed_num_of_buildings"
                | "special_unit_cost_modifier"
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

        // === Diplomacy & Relations ===
        "improve_relation_modifier" => {
            let current = modifiers
                .country_improve_relation_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_improve_relation_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "diplomats" => {
            let current = modifiers
                .country_diplomats
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_diplomats
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "diplomatic_annexation_cost" => {
            let current = modifiers
                .country_diplomatic_annexation_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_diplomatic_annexation_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "vassal_income" => {
            let current = modifiers
                .country_vassal_income
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_vassal_income
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "fabricate_claims_cost" => {
            let current = modifiers
                .country_fabricate_claims_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_fabricate_claims_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "spy_offence" => {
            let current = modifiers
                .country_spy_offence
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_spy_offence
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Technology & Development ===
        "technology_cost" => {
            let current = modifiers
                .country_technology_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_technology_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "adm_tech_cost_modifier" => {
            let current = modifiers
                .country_adm_tech_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_adm_tech_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "governing_capacity_modifier" => {
            let current = modifiers
                .country_governing_capacity
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_governing_capacity
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Force Limits & Manpower ===
        "land_forcelimit_modifier" => {
            let current = modifiers
                .country_land_forcelimit
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_land_forcelimit
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "naval_forcelimit_modifier" => {
            let current = modifiers
                .country_naval_forcelimit
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_naval_forcelimit
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "global_sailors_modifier" => {
            let current = modifiers
                .country_global_sailors
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_sailors
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "sailor_maintenance_modifer" => {
            let current = modifiers
                .country_sailor_maintenance
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_sailor_maintenance
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Military Tradition & Leaders ===
        "army_tradition" => {
            let current = modifiers
                .country_army_tradition
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_army_tradition
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "army_tradition_decay" => {
            let current = modifiers
                .country_army_tradition_decay
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_army_tradition_decay
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "navy_tradition" => {
            let current = modifiers
                .country_navy_tradition
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_navy_tradition
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "leader_land_shock" => {
            let current = modifiers
                .country_leader_land_shock
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_leader_land_shock
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "leader_land_manuever" => {
            let current = modifiers
                .country_leader_land_manuever
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_leader_land_manuever
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "prestige_decay" => {
            let current = modifiers
                .country_prestige_decay
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_prestige_decay
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Combat Modifiers ===
        "fire_damage" => {
            let current = modifiers
                .country_fire_damage
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_fire_damage
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "shock_damage" => {
            let current = modifiers
                .country_shock_damage
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_shock_damage
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "shock_damage_received" => {
            let current = modifiers
                .country_shock_damage_received
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_shock_damage_received
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "naval_morale" => {
            let current = modifiers
                .country_naval_morale
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_naval_morale
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "siege_ability" => {
            let current = modifiers
                .country_siege_ability
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_siege_ability
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "movement_speed" => {
            let current = modifiers
                .country_movement_speed
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_movement_speed
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Attrition & War Exhaustion ===
        "land_attrition" => {
            let current = modifiers
                .country_land_attrition
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_land_attrition
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "war_exhaustion" => {
            let current = modifiers
                .country_war_exhaustion
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_war_exhaustion
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Naval Costs & Power ===
        "global_ship_cost" => {
            let current = modifiers
                .country_global_ship_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_ship_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "light_ship_cost" => {
            let current = modifiers
                .country_light_ship_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_light_ship_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "ship_durability" => {
            let current = modifiers
                .country_ship_durability
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_ship_durability
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "galley_power" => {
            let current = modifiers
                .country_galley_power
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_galley_power
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "privateer_efficiency" => {
            let current = modifiers
                .country_privateer_efficiency
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_privateer_efficiency
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "global_ship_trade_power" => {
            let current = modifiers
                .country_global_ship_trade_power
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_ship_trade_power
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "trade_range_modifier" => {
            let current = modifiers
                .country_trade_range
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_trade_range
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Trade Power ===
        "global_own_trade_power" => {
            let current = modifiers
                .country_global_own_trade_power
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_own_trade_power
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "global_prov_trade_power_modifier" => {
            let current = modifiers
                .country_global_prov_trade_power
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_prov_trade_power
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Mercenary Modifiers ===
        "merc_maintenance_modifier" => {
            let current = modifiers
                .country_merc_maintenance
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_merc_maintenance
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Colonization & Expansion ===
        "colonists" => {
            let current = modifiers
                .country_colonists
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_colonists
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "global_colonial_growth" => {
            let current = modifiers
                .country_global_colonial_growth
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_colonial_growth
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "years_of_nationalism" => {
            let current = modifiers
                .country_years_of_nationalism
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_years_of_nationalism
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Religion & Tolerance ===
        "tolerance_heretic" => {
            let current = modifiers
                .country_tolerance_heretic
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_tolerance_heretic
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "tolerance_heathen" => {
            let current = modifiers
                .country_tolerance_heathen
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_tolerance_heathen
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "religious_unity" => {
            let current = modifiers
                .country_religious_unity
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_religious_unity
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "global_heretic_missionary_strength" => {
            let current = modifiers
                .country_global_heretic_missionary_strength
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_heretic_missionary_strength
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "papal_influence" => {
            let current = modifiers
                .country_papal_influence
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_papal_influence
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "church_power_modifier" => {
            let current = modifiers
                .country_church_power
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_church_power
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Advisors ===
        "advisor_cost" => {
            let current = modifiers
                .country_advisor_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_advisor_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "advisor_pool" => {
            let current = modifiers
                .country_advisor_pool
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_advisor_pool
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "culture_conversion_cost" => {
            let current = modifiers
                .country_culture_conversion_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_culture_conversion_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Economy & State ===
        "inflation_reduction" => {
            let current = modifiers
                .country_inflation_reduction
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_inflation_reduction
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "global_autonomy" => {
            let current = modifiers
                .country_global_autonomy
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_autonomy
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "state_maintenance_modifier" => {
            let current = modifiers
                .country_state_maintenance
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_state_maintenance
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "garrison_size" => {
            let current = modifiers
                .country_garrison_size
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_garrison_size
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Special Mechanics ===
        "global_institution_spread" => {
            let current = modifiers
                .country_global_institution_spread
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_institution_spread
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "heir_chance" => {
            let current = modifiers
                .country_heir_chance
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_heir_chance
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "caravan_power" => {
            let current = modifiers
                .country_caravan_power
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_caravan_power
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Missionary & Conversion ===
        "missionaries" => {
            let current = modifiers
                .country_missionaries
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_missionaries
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Naval Power & Combat ===
        "light_ship_power" => {
            let current = modifiers
                .country_light_ship_power
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_light_ship_power
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "heavy_ship_power" => {
            let current = modifiers
                .country_heavy_ship_power
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_heavy_ship_power
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "naval_maintenance_modifier" => {
            let current = modifiers
                .country_naval_maintenance
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_naval_maintenance
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "naval_attrition" => {
            let current = modifiers
                .country_naval_attrition
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_naval_attrition
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Mercenary Modifiers ===
        "mercenary_discipline" => {
            let current = modifiers
                .country_mercenary_discipline
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_mercenary_discipline
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "mercenary_manpower" => {
            let current = modifiers
                .country_mercenary_manpower
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_mercenary_manpower
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === War & Peace ===
        "unjustified_demands" => {
            let current = modifiers
                .country_unjustified_demands
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_unjustified_demands
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "province_warscore_cost" => {
            let current = modifiers
                .country_province_warscore_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_province_warscore_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Diplomacy & Travel ===
        "envoy_travel_time" => {
            let current = modifiers
                .country_envoy_travel_time
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_envoy_travel_time
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "reduced_liberty_desire" => {
            let current = modifiers
                .country_reduced_liberty_desire
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_reduced_liberty_desire
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Military Recruitment ===
        "global_regiment_cost" => {
            let current = modifiers
                .country_global_regiment_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_regiment_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "global_regiment_recruit_speed" => {
            let current = modifiers
                .country_global_regiment_recruit_speed
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_regiment_recruit_speed
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Economy & Finance ===
        "interest" => {
            let current = modifiers
                .country_interest
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_interest
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "prestige_from_land" => {
            let current = modifiers
                .country_prestige_from_land
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_prestige_from_land
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "loot_amount" => {
            let current = modifiers
                .country_loot_amount
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_loot_amount
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Military Leaders ===
        "leader_land_fire" => {
            let current = modifiers
                .country_leader_land_fire
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_leader_land_fire
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "leader_siege" => {
            let current = modifiers
                .country_leader_siege
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_leader_siege
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "leader_naval_fire" => {
            let current = modifiers
                .country_leader_naval_fire
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_leader_naval_fire
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "leader_naval_manuever" => {
            let current = modifiers
                .country_leader_naval_manuever
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_leader_naval_manuever
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Naval Costs ===
        "galley_cost" => {
            let current = modifiers
                .country_galley_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_galley_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "global_ship_recruit_speed" => {
            let current = modifiers
                .country_global_ship_recruit_speed
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_ship_recruit_speed
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Government & Reform ===
        "reform_progress_growth" => {
            let current = modifiers
                .country_reform_progress_growth
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_reform_progress_growth
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "administrative_efficiency" => {
            let current = modifiers
                .country_administrative_efficiency
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_administrative_efficiency
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "yearly_absolutism" => {
            let current = modifiers
                .country_yearly_absolutism
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_yearly_absolutism
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Religion & Faith ===
        "monthly_fervor_increase" => {
            let current = modifiers
                .country_monthly_fervor_increase
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_monthly_fervor_increase
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "monthly_piety" => {
            let current = modifiers
                .country_monthly_piety
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_monthly_piety
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Estate Loyalty ===
        "burghers_loyalty_modifier" => {
            let current = modifiers
                .country_burghers_loyalty
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_burghers_loyalty
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "nobles_loyalty_modifier" => {
            let current = modifiers
                .country_nobles_loyalty
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_nobles_loyalty
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "church_loyalty_modifier" => {
            let current = modifiers
                .country_church_loyalty
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_church_loyalty
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Estate-Specific Loyalty Modifiers ===
        "clergy_loyalty_modifier" => {
            let current = modifiers
                .country_clergy_loyalty_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_clergy_loyalty_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "dhimmi_loyalty_modifier" => {
            let current = modifiers
                .country_dhimmi_loyalty_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_dhimmi_loyalty_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "maratha_loyalty_modifier" => {
            let current = modifiers
                .country_maratha_loyalty_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_maratha_loyalty_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "rajput_loyalty_modifier" => {
            let current = modifiers
                .country_rajput_loyalty_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_rajput_loyalty_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "eunuchs_loyalty_modifier" => {
            let current = modifiers
                .country_eunuchs_loyalty_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_eunuchs_loyalty_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "ghulams_loyalty_modifier" => {
            let current = modifiers
                .country_ghulams_loyalty_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_ghulams_loyalty_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "janissaries_loyalty_modifier" => {
            let current = modifiers
                .country_janissaries_loyalty_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_janissaries_loyalty_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "qizilbash_loyalty_modifier" => {
            let current = modifiers
                .country_qizilbash_loyalty_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_qizilbash_loyalty_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "jains_loyalty_modifier" => {
            let current = modifiers
                .country_jains_loyalty_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_jains_loyalty_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "nomadic_tribes_loyalty_modifier" => {
            let current = modifiers
                .country_nomadic_tribes_loyalty_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_nomadic_tribes_loyalty_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Estate Influence Modifiers ===
        "burghers_influence_modifier" => {
            let current = modifiers
                .country_burghers_influence_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_burghers_influence_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "pr_captains_influence" => {
            let current = modifiers
                .country_pr_captains_influence
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_pr_captains_influence
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Estate Privilege Slots ===
        "all_estate_possible_privileges" => {
            let current = modifiers
                .country_all_estate_possible_privileges
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_all_estate_possible_privileges
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "estate_interaction_cooldown_modifier" => {
            let current = modifiers
                .country_estate_interaction_cooldown_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_estate_interaction_cooldown_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "cossacks_privilege_slots" => {
            let current = modifiers
                .country_cossacks_privilege_slots
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_cossacks_privilege_slots
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "ghulams_privilege_slots" => {
            let current = modifiers
                .country_ghulams_privilege_slots
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_ghulams_privilege_slots
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "qizilbash_privilege_slots" => {
            let current = modifiers
                .country_qizilbash_privilege_slots
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_qizilbash_privilege_slots
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Special Units ===
        "allowed_samurai_fraction" => {
            let current = modifiers
                .country_allowed_samurai_fraction
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_allowed_samurai_fraction
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "amount_of_banners" => {
            let current = modifiers
                .country_amount_of_banners
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_amount_of_banners
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Military Combat ===
        "recover_army_morale_speed" => {
            let current = modifiers
                .country_recover_army_morale_speed
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_recover_army_morale_speed
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "fire_damage_received" => {
            let current = modifiers
                .country_fire_damage_received
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_fire_damage_received
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "cavalry_flanking" => {
            let current = modifiers
                .country_cavalry_flanking
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_cavalry_flanking
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "cav_to_inf_ratio" => {
            let current = modifiers
                .country_cav_to_inf_ratio
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_cav_to_inf_ratio
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "reinforce_speed" => {
            let current = modifiers
                .country_reinforce_speed
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_reinforce_speed
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Espionage & Defense ===
        "global_spy_defence" => {
            let current = modifiers
                .country_global_spy_defence
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_spy_defence
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "rebel_support_efficiency" => {
            let current = modifiers
                .country_rebel_support_efficiency
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_rebel_support_efficiency
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Military Tradition & Decay ===
        "navy_tradition_decay" => {
            let current = modifiers
                .country_navy_tradition_decay
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_navy_tradition_decay
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "army_tradition_from_battle" => {
            let current = modifiers
                .country_army_tradition_from_battle
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_army_tradition_from_battle
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Naval Combat ===
        "embargo_efficiency" => {
            let current = modifiers
                .country_embargo_efficiency
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_embargo_efficiency
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "allowed_marine_fraction" => {
            let current = modifiers
                .country_allowed_marine_fraction
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_allowed_marine_fraction
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "capture_ship_chance" => {
            let current = modifiers
                .country_capture_ship_chance
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_capture_ship_chance
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Vassal & Subject ===
        "vassal_forcelimit_bonus" => {
            let current = modifiers
                .country_vassal_forcelimit_bonus
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_vassal_forcelimit_bonus
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "same_culture_advisor_cost" => {
            let current = modifiers
                .country_same_culture_advisor_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_same_culture_advisor_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Siege & Fortification ===
        "global_garrison_growth" => {
            let current = modifiers
                .country_global_garrison_growth
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_garrison_growth
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "war_exhaustion_cost" => {
            let current = modifiers
                .country_war_exhaustion_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_war_exhaustion_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Trade ===
        "global_foreign_trade_power" => {
            let current = modifiers
                .country_global_foreign_trade_power
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_foreign_trade_power
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "range" => {
            let current = modifiers
                .country_range
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_range
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Miscellaneous ===
        "female_advisor_chance" => {
            let current = modifiers
                .country_female_advisor_chance
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_female_advisor_chance
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "yearly_corruption" => {
            let current = modifiers
                .country_yearly_corruption
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_yearly_corruption
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "build_time" => {
            let current = modifiers
                .country_build_time
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_build_time
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "promote_culture_cost" => {
            let current = modifiers
                .country_promote_culture_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_promote_culture_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "liberty_desire_from_subject_development" => {
            let current = modifiers
                .country_liberty_desire_from_subject_development
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_liberty_desire_from_subject_development
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Naval Combat & Morale ===
        "sunk_ship_morale_hit_recieved" => {
            let current = modifiers
                .country_sunk_ship_morale_hit_recieved
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_sunk_ship_morale_hit_recieved
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Naval Recovery ===
        "sailors_recovery_speed" => {
            let current = modifiers
                .country_sailors_recovery_speed
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_sailors_recovery_speed
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Tech Costs ===
        "mil_tech_cost_modifier" => {
            let current = modifiers
                .country_mil_tech_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_mil_tech_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "dip_tech_cost_modifier" => {
            let current = modifiers
                .country_dip_tech_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_dip_tech_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Government & Absolutism ===
        "max_absolutism" => {
            let current = modifiers
                .country_max_absolutism
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_max_absolutism
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "num_of_pronoiars" => {
            let current = modifiers
                .country_num_of_pronoiars
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_num_of_pronoiars
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "max_revolutionary_zeal" => {
            let current = modifiers
                .country_max_revolutionary_zeal
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_max_revolutionary_zeal
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "possible_policy" => {
            let current = modifiers
                .country_possible_policy
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_possible_policy
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Power Projection ===
        "power_projection_from_insults" => {
            let current = modifiers
                .country_power_projection_from_insults
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_power_projection_from_insults
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Rebellion & Unrest ===
        "harsh_treatment_cost" => {
            let current = modifiers
                .country_harsh_treatment_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_harsh_treatment_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Leaders ===
        "free_leader_pool" => {
            let current = modifiers
                .country_free_leader_pool
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_free_leader_pool
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Naval Combat Bonuses ===
        "own_coast_naval_combat_bonus" => {
            let current = modifiers
                .country_own_coast_naval_combat_bonus
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_own_coast_naval_combat_bonus
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Technology & Innovation ===
        "embracement_cost" => {
            let current = modifiers
                .country_embracement_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_embracement_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Military Costs ===
        "artillery_cost" => {
            let current = modifiers
                .country_artillery_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_artillery_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Policy-Specific Modifiers (49 modifiers) ===

        // === Colonization ===
        "colonist_placement_chance" => {
            let current = modifiers
                .country_colonist_placement_chance
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_colonist_placement_chance
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "native_uprising_chance" => {
            let current = modifiers
                .country_native_uprising_chance
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_native_uprising_chance
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "native_assimilation" => {
            let current = modifiers
                .country_native_assimilation
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_native_assimilation
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Naval Combat & Morale ===
        "recover_navy_morale_speed" => {
            let current = modifiers
                .country_recover_navy_morale_speed
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_recover_navy_morale_speed
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "global_naval_engagement_modifier" => {
            let current = modifiers
                .country_global_naval_engagement_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_naval_engagement_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "naval_tradition_from_battle" => {
            let current = modifiers
                .country_naval_tradition_from_battle
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_naval_tradition_from_battle
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "prestige_from_naval" => {
            let current = modifiers
                .country_prestige_from_naval
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_prestige_from_naval
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "disengagement_chance" => {
            let current = modifiers
                .country_disengagement_chance
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_disengagement_chance
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "leader_naval_shock" => {
            let current = modifiers
                .country_leader_naval_shock
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_leader_naval_shock
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "movement_speed_in_fleet_modifier" => {
            let current = modifiers
                .country_movement_speed_in_fleet_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_movement_speed_in_fleet_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "morale_damage_received" => {
            let current = modifiers
                .country_morale_damage_received
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_morale_damage_received
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Army Composition ===
        "artillery_fraction" => {
            let current = modifiers
                .country_artillery_fraction
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_artillery_fraction
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "cavalry_fraction" => {
            let current = modifiers
                .country_cavalry_fraction
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_cavalry_fraction
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "infantry_fraction" => {
            let current = modifiers
                .country_infantry_fraction
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_infantry_fraction
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Economy & Trade ===
        "mercantilism_cost" => {
            let current = modifiers
                .country_mercantilism_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_mercantilism_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "global_tariffs" => {
            let current = modifiers
                .country_global_tariffs
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_tariffs
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "monthly_favor_modifier" => {
            let current = modifiers
                .country_monthly_favor_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_monthly_favor_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Siege & Fortification ===
        "siege_blockade_progress" => {
            let current = modifiers
                .country_siege_blockade_progress
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_siege_blockade_progress
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "blockade_efficiency" => {
            let current = modifiers
                .country_blockade_efficiency
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_blockade_efficiency
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "garrison_damage" => {
            let current = modifiers
                .country_garrison_damage
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_garrison_damage
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "artillery_level_modifier" => {
            let current = modifiers
                .country_artillery_level_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_artillery_level_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "artillery_levels_available_vs_fort" => {
            let current = modifiers
                .country_artillery_levels_available_vs_fort
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_artillery_levels_available_vs_fort
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Military Costs & Efficiency ===
        "morale_damage" => {
            let current = modifiers
                .country_morale_damage
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_morale_damage
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "reinforce_cost_modifier" => {
            let current = modifiers
                .country_reinforce_cost_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_reinforce_cost_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "drill_gain_modifier" => {
            let current = modifiers
                .country_drill_gain_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_drill_gain_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "yearly_army_professionalism" => {
            let current = modifiers
                .country_yearly_army_professionalism
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_yearly_army_professionalism
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "special_unit_forcelimit" => {
            let current = modifiers
                .country_special_unit_forcelimit
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_special_unit_forcelimit
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Development & Culture ===
        "development_cost_in_primary_culture" => {
            let current = modifiers
                .country_development_cost_in_primary_culture
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_development_cost_in_primary_culture
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "colony_development_boost" => {
            let current = modifiers
                .country_colony_development_boost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_colony_development_boost
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Diplomacy & Subjects ===
        "rival_border_fort_maintenance" => {
            let current = modifiers
                .country_rival_border_fort_maintenance
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_rival_border_fort_maintenance
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "reduced_liberty_desire_on_same_continent" => {
            let current = modifiers
                .country_reduced_liberty_desire_on_same_continent
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_reduced_liberty_desire_on_same_continent
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "years_to_integrate_personal_union" => {
            let current = modifiers
                .country_years_to_integrate_personal_union
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_years_to_integrate_personal_union
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "monthly_federation_favor_growth" => {
            let current = modifiers
                .country_monthly_federation_favor_growth
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_monthly_federation_favor_growth
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "all_estate_loyalty_equilibrium" => {
            let current = modifiers
                .country_all_estate_loyalty_equilibrium
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_all_estate_loyalty_equilibrium
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Religion & Authority ===
        "prestige_per_development_from_conversion" => {
            let current = modifiers
                .country_prestige_per_development_from_conversion
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_prestige_per_development_from_conversion
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "yearly_patriarch_authority" => {
            let current = modifiers
                .country_yearly_patriarch_authority
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_yearly_patriarch_authority
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "yearly_harmony" => {
            let current = modifiers
                .country_yearly_harmony
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_yearly_harmony
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "yearly_karma_decay" => {
            let current = modifiers
                .country_yearly_karma_decay
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_yearly_karma_decay
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Government & Leaders ===
        "innovativeness_gain" => {
            let current = modifiers
                .country_innovativeness_gain
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_innovativeness_gain
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "raze_power_gain" => {
            let current = modifiers
                .country_raze_power_gain
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_raze_power_gain
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "monarch_lifespan" => {
            let current = modifiers
                .country_monarch_lifespan
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_monarch_lifespan
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "reelection_cost" => {
            let current = modifiers
                .country_reelection_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_reelection_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "mil_advisor_cost" => {
            let current = modifiers
                .country_mil_advisor_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_mil_advisor_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === War & Diplomacy ===
        "warscore_cost_vs_other_religion" => {
            let current = modifiers
                .country_warscore_cost_vs_other_religion
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_warscore_cost_vs_other_religion
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "global_rebel_suppression_efficiency" => {
            let current = modifiers
                .country_global_rebel_suppression_efficiency
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_rebel_suppression_efficiency
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Naval Infrastructure ===
        "global_ship_repair" => {
            let current = modifiers
                .country_global_ship_repair
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_ship_repair
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "transport_attrition" => {
            let current = modifiers
                .country_transport_attrition
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_transport_attrition
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Province Management ===
        "manpower_in_true_faith_provinces" => {
            let current = modifiers
                .country_manpower_in_true_faith_provinces
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_manpower_in_true_faith_provinces
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "global_monthly_devastation" => {
            let current = modifiers
                .country_global_monthly_devastation
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_monthly_devastation
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Batch 1: Positions 21-25 (Frequency-Driven) ===
        "monarch_military_power" => {
            let current = modifiers
                .country_monarch_military_power
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_monarch_military_power
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "center_of_trade_upgrade_cost" => {
            let current = modifiers
                .country_center_of_trade_upgrade_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_center_of_trade_upgrade_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "accept_vassalization_reasons" => {
            let current = modifiers
                .country_accept_vassalization_reasons
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_accept_vassalization_reasons
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "brahmins_hindu_loyalty_modifier" => {
            let current = modifiers
                .country_brahmins_hindu_loyalty_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_brahmins_hindu_loyalty_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "brahmins_muslim_loyalty_modifier" => {
            let current = modifiers
                .country_brahmins_muslim_loyalty_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_brahmins_muslim_loyalty_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Batch 2: Positions 26-30 (Frequency-Driven) ===
        "tolerance_of_heathens_capacity" => {
            let current = modifiers
                .country_tolerance_of_heathens_capacity
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_tolerance_of_heathens_capacity
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "possible_mil_policy" => {
            let current = modifiers
                .country_possible_mil_policy
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_possible_mil_policy
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "curia_powers_cost" => {
            let current = modifiers
                .country_curia_powers_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_curia_powers_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "expand_administration_cost" => {
            let current = modifiers
                .country_expand_administration_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_expand_administration_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "loyalty_change_on_revoked" => {
            let current = modifiers
                .country_loyalty_change_on_revoked
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_loyalty_change_on_revoked
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Batch 3: Positions 31-35 (Frequency-Driven) ===
        "great_project_upgrade_cost" => {
            let current = modifiers
                .country_great_project_upgrade_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_great_project_upgrade_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "gold_depletion_chance_modifier" => {
            let current = modifiers
                .country_gold_depletion_chance_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_gold_depletion_chance_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "global_supply_limit_modifier" => {
            let current = modifiers
                .country_global_supply_limit_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_supply_limit_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "general_cost" => {
            let current = modifiers
                .country_general_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_general_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "leader_cost" => {
            let current = modifiers
                .country_leader_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_leader_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Batch 4: Positions 36-40 (Frequency-Driven) ===
        "cavalry_fire" => {
            let current = modifiers
                .country_cavalry_fire
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_cavalry_fire
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "war_taxes_cost_modifier" => {
            let current = modifiers
                .country_war_taxes_cost_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_war_taxes_cost_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "vaisyas_loyalty_modifier" => {
            let current = modifiers
                .country_vaisyas_loyalty_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_vaisyas_loyalty_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "max_hostile_attrition" => {
            let current = modifiers
                .country_max_hostile_attrition
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_max_hostile_attrition
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "nobles_influence_modifier" => {
            let current = modifiers
                .country_nobles_influence_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_nobles_influence_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Quick Wins Batch 1: Policy & Monarch Power ===
        "free_mil_policy" => {
            let current = modifiers
                .country_free_mil_policy
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_free_mil_policy
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "free_adm_policy" => {
            let current = modifiers
                .country_free_adm_policy
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_free_adm_policy
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "free_dip_policy" => {
            let current = modifiers
                .country_free_dip_policy
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_free_dip_policy
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "possible_dip_policy" => {
            let current = modifiers
                .country_possible_dip_policy_alt
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_possible_dip_policy_alt
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "free_policy" => {
            let current = modifiers
                .country_free_policy
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_free_policy
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "monarch_diplomatic_power" => {
            let current = modifiers
                .country_monarch_diplomatic_power
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_monarch_diplomatic_power
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "monarch_admin_power" => {
            let current = modifiers
                .country_monarch_admin_power
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_monarch_admin_power
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "country_military_power" => {
            let current = modifiers
                .country_country_military_power
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_country_military_power
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "monarch_power_tribute" => {
            let current = modifiers
                .country_monarch_power_tribute
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_monarch_power_tribute
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Quick Wins Batch 2: Religion & Governance ===
        "missionary_maintenance_cost" => {
            let current = modifiers
                .country_missionary_maintenance_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_missionary_maintenance_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "enforce_religion_cost" => {
            let current = modifiers
                .country_enforce_religion_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_enforce_religion_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "tolerance_of_heretics_capacity" => {
            let current = modifiers
                .country_tolerance_of_heretics_capacity
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_tolerance_of_heretics_capacity
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "overextension_impact_modifier" => {
            let current = modifiers
                .country_overextension_impact_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_overextension_impact_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "state_governing_cost" => {
            let current = modifiers
                .country_state_governing_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_state_governing_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "min_autonomy_in_territories" => {
            let current = modifiers
                .country_min_autonomy_in_territories
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_min_autonomy_in_territories
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "autonomy_change_time" => {
            let current = modifiers
                .country_autonomy_change_time
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_autonomy_change_time
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "expand_infrastructure_cost_modifier" => {
            let current = modifiers
                .country_expand_infrastructure_cost_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_expand_infrastructure_cost_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Quick Wins Batch 3: Advisors & Diplomacy ===
        "adm_advisor_cost" => {
            let current = modifiers
                .country_adm_advisor_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_adm_advisor_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "dip_advisor_cost" => {
            let current = modifiers
                .country_dip_advisor_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_dip_advisor_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "same_religion_advisor_cost" => {
            let current = modifiers
                .country_same_religion_advisor_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_same_religion_advisor_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "reverse_relation_with_same_religion" => {
            let current = modifiers
                .country_reverse_relation_with_same_religion
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_reverse_relation_with_same_religion
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "reduced_liberty_desire_on_other_continent" => {
            let current = modifiers
                .country_reduced_liberty_desire_on_other_continent
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_reduced_liberty_desire_on_other_continent
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "rival_change_cost" => {
            let current = modifiers
                .country_rival_change_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_rival_change_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "stability_cost_to_declare_war" => {
            let current = modifiers
                .country_stability_cost_to_declare_war
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_stability_cost_to_declare_war
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Quick Wins Batch 4: Naval & Military ===
        "ship_power_propagation" => {
            let current = modifiers
                .country_ship_power_propagation
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_ship_power_propagation
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "vassal_naval_forcelimit_bonus" => {
            let current = modifiers
                .country_vassal_naval_forcelimit_bonus
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_vassal_naval_forcelimit_bonus
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "admiral_cost" => {
            let current = modifiers
                .country_admiral_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_admiral_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "flagship_cost" => {
            let current = modifiers
                .country_flagship_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_flagship_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "heavy_ship_cost" => {
            let current = modifiers
                .country_heavy_ship_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_heavy_ship_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "artillery_fire" => {
            let current = modifiers
                .country_artillery_fire
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_artillery_fire
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "artillery_shock" => {
            let current = modifiers
                .country_artillery_shock
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_artillery_shock
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "infantry_shock" => {
            let current = modifiers
                .country_infantry_shock
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_infantry_shock
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "global_naval_barrage_cost" => {
            let current = modifiers
                .country_global_naval_barrage_cost
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_naval_barrage_cost
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "landing_penalty" => {
            let current = modifiers
                .country_landing_penalty
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_landing_penalty
                .insert(tag.to_string(), current + entry.value);
            true
        }

        // === Quick Wins Batch 5: Miscellaneous ===
        "monthly_gold_inflation_modifier" => {
            let current = modifiers
                .country_monthly_gold_inflation_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_monthly_gold_inflation_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "global_prosperity_growth" => {
            let current = modifiers
                .country_global_prosperity_growth
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_prosperity_growth
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "spy_action_cost_modifier" => {
            let current = modifiers
                .country_spy_action_cost_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_spy_action_cost_modifier
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "global_allowed_num_of_buildings" => {
            let current = modifiers
                .country_global_allowed_num_of_buildings
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_global_allowed_num_of_buildings
                .insert(tag.to_string(), current + entry.value);
            true
        }
        "special_unit_cost_modifier" => {
            let current = modifiers
                .country_special_unit_cost_modifier
                .get(tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            modifiers
                .country_special_unit_cost_modifier
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
#[path = "ideas_tests.rs"]
mod tests;
