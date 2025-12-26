//! Modifier system for dynamic game state mutations.
//!
//! Events, decisions, and other game mechanics modify these values.
//! All values use [`Fixed`] for deterministic simulation.

use crate::fixed::Fixed;
use crate::state::{ProvinceId, Tag};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type-safe trade good identifier.
///
/// Prevents mixing up trade good IDs with province IDs or other numeric types.
#[derive(Hash, Eq, PartialEq, Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct TradegoodId(pub u16);

/// Type-safe building identifier.
///
/// Sequential IDs (0..N) enable efficient bitmask storage via [`crate::buildings::BuildingSet`].
/// EU4 has ~70 buildings, so `u8` is sufficient.
#[derive(
    Hash, Eq, PartialEq, Clone, Copy, Debug, Default, Serialize, Deserialize, PartialOrd, Ord,
)]
pub struct BuildingId(pub u8);

impl BuildingId {
    /// Convert to bitmask position for [`crate::buildings::BuildingSet`].
    #[inline]
    pub fn as_mask(self) -> u128 {
        1u128 << self.0
    }
}

/// Dynamic game state modifiable by events.
///
/// Keys are typed IDs for safety; values are [`Fixed`] for determinism.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GameModifiers {
    /// Price modifiers for trade goods (from events like "Price Change: Cotton").
    /// Added to base price: effective = base + modifier.
    pub goods_price_mods: HashMap<TradegoodId, Fixed>,

    /// Province-level production efficiency bonuses.
    /// Applied as: (1 + efficiency) multiplier.
    pub province_production_efficiency: HashMap<ProvinceId, Fixed>,

    /// Province-level autonomy values.
    /// Applied as: (1 - autonomy) multiplier.
    pub province_autonomy: HashMap<ProvinceId, Fixed>,

    /// Country-level tax efficiency (national tax modifier).
    /// Applied as: (1 + modifier) multiplier.
    pub country_tax_modifier: HashMap<Tag, Fixed>,

    /// Province-level tax modifier.
    /// Applied to base tax.
    pub province_tax_modifier: HashMap<ProvinceId, Fixed>,

    /// Country-level land maintenance modifier.
    /// Applied as: (1 + modifier) multiplier for army cost.
    pub land_maintenance_modifier: HashMap<Tag, Fixed>,

    /// Country-level fort maintenance modifier.
    /// Applied as: (1 + modifier) multiplier for fort cost.
    pub fort_maintenance_modifier: HashMap<Tag, Fixed>,

    /// Country-level discipline modifier.
    /// Applied as: (1 + modifier) multiplier to damage dealt in combat.
    pub country_discipline: HashMap<Tag, Fixed>,

    /// Country-level morale bonus.
    /// Applied as: (1 + modifier) multiplier to base morale.
    pub country_morale: HashMap<Tag, Fixed>,

    /// Country-level infantry power modifier.
    /// Applied as: (1 + modifier) multiplier to infantry damage.
    pub country_infantry_power: HashMap<Tag, Fixed>,

    /// Country-level cavalry power modifier.
    /// Applied as: (1 + modifier) multiplier to cavalry damage.
    pub country_cavalry_power: HashMap<Tag, Fixed>,

    /// Country-level artillery power modifier.
    /// Applied as: (1 + modifier) multiplier to artillery damage.
    pub country_artillery_power: HashMap<Tag, Fixed>,

    /// Country-level goods produced modifier.
    /// Applied as: (1 + modifier) multiplier to province goods production.
    pub country_goods_produced: HashMap<Tag, Fixed>,

    /// Country-level trade efficiency modifier.
    /// Applied as: (1 + modifier) multiplier to trade income collection.
    pub country_trade_efficiency: HashMap<Tag, Fixed>,

    /// Country-level global trade power modifier.
    /// Applied as: (1 + modifier) multiplier to provincial trade power.
    pub country_trade_power: HashMap<Tag, Fixed>,

    /// Country-level trade steering modifier.
    /// Applied as: (1 + modifier) multiplier to trade value steering.
    pub country_trade_steering: HashMap<Tag, Fixed>,

    /// Country-level development cost modifier.
    /// Applied as: (1 + modifier) multiplier to monarch point cost for development.
    /// Negative values make development cheaper.
    pub country_development_cost: HashMap<Tag, Fixed>,

    /// Country-level core creation modifier.
    /// Applied as: (1 + modifier) multiplier to coring time and cost.
    /// Negative values make coring faster/cheaper.
    pub country_core_creation: HashMap<Tag, Fixed>,

    /// Country-level aggressive expansion impact modifier.
    /// Applied as: (1 + modifier) multiplier to AE gained from conquest.
    /// Negative values reduce AE impact.
    pub country_ae_impact: HashMap<Tag, Fixed>,

    /// Country-level diplomatic reputation modifier.
    /// Applied as additive bonus to diplomatic actions.
    pub country_diplomatic_reputation: HashMap<Tag, Fixed>,

    /// Country-level infantry cost modifier.
    /// Applied as: (1 + modifier) multiplier to infantry maintenance cost.
    /// Negative values reduce cost.
    pub country_infantry_cost: HashMap<Tag, Fixed>,

    /// Country-level cavalry cost modifier.
    /// Applied as: (1 + modifier) multiplier to cavalry maintenance cost.
    /// Negative values reduce cost.
    pub country_cavalry_cost: HashMap<Tag, Fixed>,

    /// Country-level mercenary cost modifier.
    /// Applied as: (1 + modifier) multiplier to mercenary maintenance cost.
    /// Negative values reduce cost.
    pub country_mercenary_cost: HashMap<Tag, Fixed>,

    /// Country-level global manpower modifier.
    /// Applied as: (1 + modifier) multiplier to maximum manpower pool.
    pub country_manpower: HashMap<Tag, Fixed>,

    /// Country-level monthly prestige gain.
    /// Applied as additive bonus to prestige per month.
    pub country_prestige: HashMap<Tag, Fixed>,

    /// Country-level devotion gain (for Theocracy governments).
    /// Applied as additive bonus to monthly devotion.
    pub country_devotion: HashMap<Tag, Fixed>,

    /// Country-level horde unity gain (for Steppe Horde governments).
    /// Applied as additive bonus to monthly horde unity.
    pub country_horde_unity: HashMap<Tag, Fixed>,

    /// Country-level legitimacy gain (for Monarchy governments).
    /// Applied as additive bonus to monthly legitimacy.
    pub country_legitimacy: HashMap<Tag, Fixed>,

    /// Country-level republican tradition gain (for Republic governments).
    /// Applied as additive bonus to monthly republican tradition.
    pub country_republican_tradition: HashMap<Tag, Fixed>,

    /// Country-level meritocracy gain (for Celestial Empire government).
    /// Applied as additive bonus to monthly meritocracy.
    pub country_meritocracy: HashMap<Tag, Fixed>,

    /// Country-level defensiveness modifier.
    /// Applied as: (1 + modifier) multiplier to fort defense strength.
    pub country_defensiveness: HashMap<Tag, Fixed>,

    /// Country-level global unrest modifier.
    /// Applied as additive penalty/bonus to province unrest.
    pub country_unrest: HashMap<Tag, Fixed>,

    /// Country-level stability cost modifier.
    /// Applied as: (1 + modifier) multiplier to stability increase cost.
    /// Negative values make stability cheaper.
    pub country_stability_cost: HashMap<Tag, Fixed>,

    /// Country-level tolerance of the true faith.
    /// Applied as additive bonus to tolerance (reduces unrest from same religion provinces).
    pub country_tolerance_own: HashMap<Tag, Fixed>,

    /// Country-level global trade goods size modifier.
    /// Applied as: (1 + modifier) multiplier to goods produced.
    /// Functionally equivalent to goods_produced_modifier.
    pub country_trade_goods_size: HashMap<Tag, Fixed>,

    /// Country-level build cost modifier.
    /// Applied as: (1 + modifier) multiplier to building construction cost.
    /// Negative values make buildings cheaper.
    pub country_build_cost: HashMap<Tag, Fixed>,

    /// Country-level manpower recovery speed modifier.
    /// Applied as: (1 + modifier) multiplier to monthly manpower recovery.
    pub country_manpower_recovery_speed: HashMap<Tag, Fixed>,

    /// Country-level hostile attrition modifier.
    /// Applied to enemy armies in your territory.
    pub country_hostile_attrition: HashMap<Tag, Fixed>,

    /// Country-level diplomatic relations limit.
    /// Applied as additive bonus to maximum diplomatic relations.
    pub country_diplomatic_upkeep: HashMap<Tag, Fixed>,

    /// Country-level idea cost modifier.
    /// Applied as: (1 + modifier) multiplier to idea group unlock cost.
    /// Negative values make ideas cheaper.
    pub country_idea_cost: HashMap<Tag, Fixed>,

    /// Country-level merchant bonus.
    /// Applied as additive bonus to number of available merchants.
    pub country_merchants: HashMap<Tag, Fixed>,

    /// Country-level global missionary strength.
    /// Applied as additive bonus to missionary conversion strength.
    pub country_missionary_strength: HashMap<Tag, Fixed>,

    /// Country-level accepted cultures limit.
    /// Applied as additive bonus to maximum accepted cultures.
    pub country_num_accepted_cultures: HashMap<Tag, Fixed>,

    // === Diplomacy & Relations (6 modifiers) ===
    /// Country-level improve relations modifier.
    /// Applied as: (1 + modifier) multiplier to improve relations speed.
    pub country_improve_relation_modifier: HashMap<Tag, Fixed>,

    /// Country-level diplomat count bonus.
    /// Applied as additive bonus to number of available diplomats.
    pub country_diplomats: HashMap<Tag, Fixed>,

    /// Country-level diplomatic annexation cost modifier.
    /// Applied as: (1 + modifier) multiplier to diplomatic annexation cost.
    pub country_diplomatic_annexation_cost: HashMap<Tag, Fixed>,

    /// Country-level vassal income modifier.
    /// Applied as: (1 + modifier) multiplier to vassal income.
    pub country_vassal_income: HashMap<Tag, Fixed>,

    /// Country-level fabricate claims cost modifier.
    /// Applied as: (1 + modifier) multiplier to fabricate claims cost.
    pub country_fabricate_claims_cost: HashMap<Tag, Fixed>,

    /// Country-level spy offense modifier.
    /// Applied as additive bonus to spy network construction.
    pub country_spy_offence: HashMap<Tag, Fixed>,

    // === Technology & Development (3 modifiers) ===
    /// Country-level general technology cost modifier.
    /// Applied as: (1 + modifier) multiplier to all tech costs.
    pub country_technology_cost: HashMap<Tag, Fixed>,

    /// Country-level administrative technology cost modifier.
    /// Applied as: (1 + modifier) multiplier to ADM tech cost.
    pub country_adm_tech_cost: HashMap<Tag, Fixed>,

    /// Country-level governing capacity modifier.
    /// Applied as: (1 + modifier) multiplier to governing capacity.
    pub country_governing_capacity: HashMap<Tag, Fixed>,

    // === Military Force Limits & Manpower (4 modifiers) ===
    /// Country-level land force limit modifier.
    /// Applied as: (1 + modifier) multiplier to land force limit.
    pub country_land_forcelimit: HashMap<Tag, Fixed>,

    /// Country-level naval force limit modifier.
    /// Applied as: (1 + modifier) multiplier to naval force limit.
    pub country_naval_forcelimit: HashMap<Tag, Fixed>,

    /// Country-level global sailors modifier.
    /// Applied as: (1 + modifier) multiplier to maximum sailors.
    pub country_global_sailors: HashMap<Tag, Fixed>,

    /// Country-level sailor maintenance modifier.
    /// Applied as: (1 + modifier) multiplier to sailor maintenance cost.
    pub country_sailor_maintenance: HashMap<Tag, Fixed>,

    // === Military Tradition & Leaders (6 modifiers) ===
    /// Country-level army tradition gain.
    /// Applied as additive bonus to monthly army tradition.
    pub country_army_tradition: HashMap<Tag, Fixed>,

    /// Country-level army tradition decay.
    /// Applied as: (1 + modifier) multiplier to army tradition decay.
    pub country_army_tradition_decay: HashMap<Tag, Fixed>,

    /// Country-level navy tradition gain.
    /// Applied as additive bonus to monthly navy tradition.
    pub country_navy_tradition: HashMap<Tag, Fixed>,

    /// Country-level land leader shock bonus.
    /// Applied as additive bonus to land leader shock skill.
    pub country_leader_land_shock: HashMap<Tag, Fixed>,

    /// Country-level land leader maneuver bonus.
    /// Applied as additive bonus to land leader maneuver skill.
    pub country_leader_land_manuever: HashMap<Tag, Fixed>,

    /// Country-level prestige decay modifier.
    /// Applied as: (1 + modifier) multiplier to prestige decay.
    pub country_prestige_decay: HashMap<Tag, Fixed>,

    // === Combat Modifiers (6 modifiers) ===
    /// Country-level fire damage modifier.
    /// Applied as: (1 + modifier) multiplier to fire phase damage dealt.
    pub country_fire_damage: HashMap<Tag, Fixed>,

    /// Country-level shock damage modifier.
    /// Applied as: (1 + modifier) multiplier to shock phase damage dealt.
    pub country_shock_damage: HashMap<Tag, Fixed>,

    /// Country-level shock damage received modifier.
    /// Applied as: (1 + modifier) multiplier to shock damage taken.
    pub country_shock_damage_received: HashMap<Tag, Fixed>,

    /// Country-level naval morale modifier.
    /// Applied as: (1 + modifier) multiplier to base naval morale.
    pub country_naval_morale: HashMap<Tag, Fixed>,

    /// Country-level siege ability modifier.
    /// Applied as: (1 + modifier) multiplier to siege progress.
    pub country_siege_ability: HashMap<Tag, Fixed>,

    /// Country-level movement speed modifier.
    /// Applied as: (1 + modifier) multiplier to army movement speed.
    pub country_movement_speed: HashMap<Tag, Fixed>,

    // === Attrition & War Exhaustion (2 modifiers) ===
    /// Country-level land attrition modifier.
    /// Applied as: (1 + modifier) multiplier to land attrition.
    pub country_land_attrition: HashMap<Tag, Fixed>,

    /// Country-level war exhaustion modifier.
    /// Applied as: (1 + modifier) multiplier to war exhaustion gain.
    pub country_war_exhaustion: HashMap<Tag, Fixed>,

    // === Naval Costs & Power (7 modifiers) ===
    /// Country-level global ship cost modifier.
    /// Applied as: (1 + modifier) multiplier to all ship costs.
    pub country_global_ship_cost: HashMap<Tag, Fixed>,

    /// Country-level light ship cost modifier.
    /// Applied as: (1 + modifier) multiplier to light ship cost.
    pub country_light_ship_cost: HashMap<Tag, Fixed>,

    /// Country-level ship durability modifier.
    /// Applied as: (1 + modifier) multiplier to ship durability.
    pub country_ship_durability: HashMap<Tag, Fixed>,

    /// Country-level galley power modifier.
    /// Applied as: (1 + modifier) multiplier to galley combat ability.
    pub country_galley_power: HashMap<Tag, Fixed>,

    /// Country-level privateer efficiency modifier.
    /// Applied as: (1 + modifier) multiplier to privateer income.
    pub country_privateer_efficiency: HashMap<Tag, Fixed>,

    /// Country-level global ship trade power modifier.
    /// Applied as: (1 + modifier) multiplier to ship trade power.
    pub country_global_ship_trade_power: HashMap<Tag, Fixed>,

    /// Country-level trade range modifier.
    /// Applied as: (1 + modifier) multiplier to trade range.
    pub country_trade_range: HashMap<Tag, Fixed>,

    // === Trade Power (2 modifiers) ===
    /// Country-level own trade power modifier.
    /// Applied as: (1 + modifier) multiplier to trade power in owned provinces.
    pub country_global_own_trade_power: HashMap<Tag, Fixed>,

    /// Country-level provincial trade power modifier.
    /// Applied as: (1 + modifier) multiplier to provincial trade power.
    pub country_global_prov_trade_power: HashMap<Tag, Fixed>,

    // === Mercenary Modifiers (1 modifier) ===
    /// Country-level mercenary maintenance modifier.
    /// Applied as: (1 + modifier) multiplier to mercenary maintenance cost.
    pub country_merc_maintenance: HashMap<Tag, Fixed>,

    // === Colonization & Expansion (3 modifiers) ===
    /// Country-level colonist count bonus.
    /// Applied as additive bonus to number of available colonists.
    pub country_colonists: HashMap<Tag, Fixed>,

    /// Country-level global colonial growth modifier.
    /// Applied as additive bonus to colonial growth rate.
    pub country_global_colonial_growth: HashMap<Tag, Fixed>,

    /// Country-level years of nationalism modifier.
    /// Applied as: (1 + modifier) multiplier to years of nationalism.
    pub country_years_of_nationalism: HashMap<Tag, Fixed>,

    // === Religion & Tolerance (6 modifiers) ===
    /// Country-level tolerance of heretics.
    /// Applied as additive bonus to tolerance (reduces unrest from heretic provinces).
    pub country_tolerance_heretic: HashMap<Tag, Fixed>,

    /// Country-level tolerance of heathens.
    /// Applied as additive bonus to tolerance (reduces unrest from heathen provinces).
    pub country_tolerance_heathen: HashMap<Tag, Fixed>,

    /// Country-level religious unity modifier.
    /// Applied as: (1 + modifier) multiplier to religious unity.
    pub country_religious_unity: HashMap<Tag, Fixed>,

    /// Country-level global heretic missionary strength.
    /// Applied as additive bonus to missionary strength against heretics.
    pub country_global_heretic_missionary_strength: HashMap<Tag, Fixed>,

    /// Country-level papal influence gain.
    /// Applied as additive bonus to monthly papal influence (Catholic nations).
    pub country_papal_influence: HashMap<Tag, Fixed>,

    /// Country-level church power modifier.
    /// Applied as: (1 + modifier) multiplier to church power gain (Protestant nations).
    pub country_church_power: HashMap<Tag, Fixed>,

    // === Advisors (3 modifiers) ===
    /// Country-level advisor cost modifier.
    /// Applied as: (1 + modifier) multiplier to advisor maintenance cost.
    pub country_advisor_cost: HashMap<Tag, Fixed>,

    /// Country-level advisor pool modifier.
    /// Applied as additive bonus to advisor pool size.
    pub country_advisor_pool: HashMap<Tag, Fixed>,

    /// Country-level culture conversion cost modifier.
    /// Applied as: (1 + modifier) multiplier to culture conversion cost.
    pub country_culture_conversion_cost: HashMap<Tag, Fixed>,

    // === Economy & State (4 modifiers) ===
    /// Country-level inflation reduction.
    /// Applied as additive bonus to yearly inflation reduction.
    pub country_inflation_reduction: HashMap<Tag, Fixed>,

    /// Country-level global autonomy modifier.
    /// Applied as additive bonus/penalty to all province autonomy.
    pub country_global_autonomy: HashMap<Tag, Fixed>,

    /// Country-level state maintenance modifier.
    /// Applied as: (1 + modifier) multiplier to state maintenance cost.
    pub country_state_maintenance: HashMap<Tag, Fixed>,

    /// Country-level garrison size modifier.
    /// Applied as: (1 + modifier) multiplier to fort garrison size.
    pub country_garrison_size: HashMap<Tag, Fixed>,

    // === Special Mechanics (4 modifiers) ===
    /// Country-level institution spread modifier.
    /// Applied as: (1 + modifier) multiplier to institution spread rate.
    pub country_global_institution_spread: HashMap<Tag, Fixed>,

    /// Country-level heir chance modifier.
    /// Applied as: (1 + modifier) multiplier to heir chance.
    pub country_heir_chance: HashMap<Tag, Fixed>,

    /// Country-level caravan power modifier.
    /// Applied as: (1 + modifier) multiplier to caravan trade power.
    pub country_caravan_power: HashMap<Tag, Fixed>,

    // === Missionary & Conversion (1 modifier) ===
    /// Country-level missionary count bonus.
    /// Applied as additive bonus to number of available missionaries.
    pub country_missionaries: HashMap<Tag, Fixed>,

    // === Naval Power & Combat (4 modifiers) ===
    /// Country-level light ship power modifier.
    /// Applied as: (1 + modifier) multiplier to light ship combat ability.
    pub country_light_ship_power: HashMap<Tag, Fixed>,

    /// Country-level heavy ship power modifier.
    /// Applied as: (1 + modifier) multiplier to heavy ship combat ability.
    pub country_heavy_ship_power: HashMap<Tag, Fixed>,

    /// Country-level naval maintenance modifier.
    /// Applied as: (1 + modifier) multiplier to naval maintenance cost.
    pub country_naval_maintenance: HashMap<Tag, Fixed>,

    /// Country-level naval attrition modifier.
    /// Applied as: (1 + modifier) multiplier to naval attrition.
    pub country_naval_attrition: HashMap<Tag, Fixed>,

    // === Mercenary Modifiers (2 modifiers) ===
    /// Country-level mercenary discipline modifier.
    /// Applied as: (1 + modifier) multiplier to mercenary discipline.
    pub country_mercenary_discipline: HashMap<Tag, Fixed>,

    /// Country-level mercenary manpower modifier.
    /// Applied as: (1 + modifier) multiplier to mercenary manpower pool.
    pub country_mercenary_manpower: HashMap<Tag, Fixed>,

    // === War & Peace (2 modifiers) ===
    /// Country-level unjustified demands penalty.
    /// Applied as: (1 + modifier) multiplier to unjustified demands AE/cost.
    pub country_unjustified_demands: HashMap<Tag, Fixed>,

    /// Country-level province warscore cost modifier.
    /// Applied as: (1 + modifier) multiplier to province warscore cost.
    pub country_province_warscore_cost: HashMap<Tag, Fixed>,

    // === Diplomacy & Travel (2 modifiers) ===
    /// Country-level envoy travel time modifier.
    /// Applied as: (1 + modifier) multiplier to envoy travel time.
    pub country_envoy_travel_time: HashMap<Tag, Fixed>,

    /// Country-level reduced liberty desire modifier.
    /// Applied as additive reduction to subject liberty desire.
    pub country_reduced_liberty_desire: HashMap<Tag, Fixed>,

    // === Military Recruitment (2 modifiers) ===
    /// Country-level global regiment cost modifier.
    /// Applied as: (1 + modifier) multiplier to regiment recruitment cost.
    pub country_global_regiment_cost: HashMap<Tag, Fixed>,

    /// Country-level global regiment recruit speed modifier.
    /// Applied as: (1 + modifier) multiplier to regiment recruitment speed.
    pub country_global_regiment_recruit_speed: HashMap<Tag, Fixed>,

    // === Economy & Finance (3 modifiers) ===
    /// Country-level interest modifier.
    /// Applied as: (1 + modifier) multiplier to loan interest rate.
    pub country_interest: HashMap<Tag, Fixed>,

    /// Country-level prestige from land battles modifier.
    /// Applied as: (1 + modifier) multiplier to prestige gained from land battles.
    pub country_prestige_from_land: HashMap<Tag, Fixed>,

    /// Country-level loot amount modifier.
    /// Applied as: (1 + modifier) multiplier to loot from sieges.
    pub country_loot_amount: HashMap<Tag, Fixed>,

    // === Military Leaders (4 modifiers) ===
    /// Country-level land leader fire bonus.
    /// Applied as additive bonus to land leader fire skill.
    pub country_leader_land_fire: HashMap<Tag, Fixed>,

    /// Country-level land leader siege bonus.
    /// Applied as additive bonus to land leader siege skill.
    pub country_leader_siege: HashMap<Tag, Fixed>,

    /// Country-level naval leader fire bonus.
    /// Applied as additive bonus to naval leader fire skill.
    pub country_leader_naval_fire: HashMap<Tag, Fixed>,

    /// Country-level naval leader maneuver bonus.
    /// Applied as additive bonus to naval leader maneuver skill.
    pub country_leader_naval_manuever: HashMap<Tag, Fixed>,

    // === Naval Costs (2 modifiers) ===
    /// Country-level galley cost modifier.
    /// Applied as: (1 + modifier) multiplier to galley cost.
    pub country_galley_cost: HashMap<Tag, Fixed>,

    /// Country-level global ship recruit speed modifier.
    /// Applied as: (1 + modifier) multiplier to ship build speed.
    pub country_global_ship_recruit_speed: HashMap<Tag, Fixed>,

    // === Government & Reform (3 modifiers) ===
    /// Country-level reform progress growth modifier.
    /// Applied as: (1 + modifier) multiplier to monthly reform progress.
    pub country_reform_progress_growth: HashMap<Tag, Fixed>,

    /// Country-level administrative efficiency modifier.
    /// Applied as additive bonus to administrative efficiency.
    pub country_administrative_efficiency: HashMap<Tag, Fixed>,

    /// Country-level yearly absolutism gain.
    /// Applied as additive bonus to yearly absolutism.
    pub country_yearly_absolutism: HashMap<Tag, Fixed>,

    // === Religion & Faith (2 modifiers) ===
    /// Country-level monthly fervor increase.
    /// Applied as additive bonus to monthly fervor (Reformed).
    pub country_monthly_fervor_increase: HashMap<Tag, Fixed>,

    /// Country-level monthly piety gain.
    /// Applied as additive bonus to monthly piety (Muslim).
    pub country_monthly_piety: HashMap<Tag, Fixed>,

    // === Estate Loyalty (3 modifiers) ===
    /// Country-level burghers loyalty modifier.
    /// Applied as additive bonus to burghers estate loyalty.
    pub country_burghers_loyalty: HashMap<Tag, Fixed>,

    /// Country-level nobles loyalty modifier.
    /// Applied as additive bonus to nobles estate loyalty.
    pub country_nobles_loyalty: HashMap<Tag, Fixed>,

    /// Country-level church loyalty modifier.
    /// Applied as additive bonus to church estate loyalty.
    pub country_church_loyalty: HashMap<Tag, Fixed>,

    // === Military Combat (5 modifiers) ===
    /// Country-level army morale recovery speed modifier.
    /// Applied as: (1 + modifier) multiplier to army morale recovery.
    pub country_recover_army_morale_speed: HashMap<Tag, Fixed>,

    /// Country-level fire damage received modifier.
    /// Applied as: (1 + modifier) multiplier to fire damage taken.
    pub country_fire_damage_received: HashMap<Tag, Fixed>,

    /// Country-level cavalry flanking ability modifier.
    /// Applied as: (1 + modifier) multiplier to cavalry flanking range.
    pub country_cavalry_flanking: HashMap<Tag, Fixed>,

    /// Country-level cavalry to infantry ratio modifier.
    /// Applied as additive bonus to maximum cavalry ratio.
    pub country_cav_to_inf_ratio: HashMap<Tag, Fixed>,

    /// Country-level reinforce speed modifier.
    /// Applied as: (1 + modifier) multiplier to army reinforcement speed.
    pub country_reinforce_speed: HashMap<Tag, Fixed>,

    // === Espionage & Defense (2 modifiers) ===
    /// Country-level global spy defense modifier.
    /// Applied as additive bonus to spy defense.
    pub country_global_spy_defence: HashMap<Tag, Fixed>,

    /// Country-level rebel support efficiency modifier.
    /// Applied as: (1 + modifier) multiplier to rebel support efficiency.
    pub country_rebel_support_efficiency: HashMap<Tag, Fixed>,

    // === Military Tradition & Decay (2 modifiers) ===
    /// Country-level navy tradition decay modifier.
    /// Applied as: (1 + modifier) multiplier to navy tradition decay.
    pub country_navy_tradition_decay: HashMap<Tag, Fixed>,

    /// Country-level army tradition from battle modifier.
    /// Applied as: (1 + modifier) multiplier to army tradition from battles.
    pub country_army_tradition_from_battle: HashMap<Tag, Fixed>,

    // === Naval Combat (3 modifiers) ===
    /// Country-level embargo efficiency modifier.
    /// Applied as: (1 + modifier) multiplier to embargo effectiveness.
    pub country_embargo_efficiency: HashMap<Tag, Fixed>,

    /// Country-level allowed marines fraction.
    /// Applied as additive bonus to maximum marines ratio.
    pub country_allowed_marine_fraction: HashMap<Tag, Fixed>,

    /// Country-level capture ship chance modifier.
    /// Applied as: (1 + modifier) multiplier to capture ship chance.
    pub country_capture_ship_chance: HashMap<Tag, Fixed>,

    // === Vassal & Subject (2 modifiers) ===
    /// Country-level vassal force limit bonus.
    /// Applied as: (1 + modifier) multiplier to vassal force limit contribution.
    pub country_vassal_forcelimit_bonus: HashMap<Tag, Fixed>,

    /// Country-level same culture advisor cost modifier.
    /// Applied as: (1 + modifier) multiplier to same-culture advisor costs.
    pub country_same_culture_advisor_cost: HashMap<Tag, Fixed>,

    // === Siege & Fortification (2 modifiers) ===
    /// Country-level global garrison growth modifier.
    /// Applied as: (1 + modifier) multiplier to garrison growth rate.
    pub country_global_garrison_growth: HashMap<Tag, Fixed>,

    /// Country-level war exhaustion cost modifier.
    /// Applied as: (1 + modifier) multiplier to war exhaustion reduction cost.
    pub country_war_exhaustion_cost: HashMap<Tag, Fixed>,

    // === Trade (2 modifiers) ===
    /// Country-level global foreign trade power modifier.
    /// Applied as: (1 + modifier) multiplier to foreign trade power.
    pub country_global_foreign_trade_power: HashMap<Tag, Fixed>,

    /// Country-level artillery range modifier.
    /// Applied as additive bonus to artillery range.
    pub country_range: HashMap<Tag, Fixed>,

    // === Miscellaneous (5 modifiers) ===
    /// Country-level female advisor chance modifier.
    /// Applied as: (1 + modifier) multiplier to female advisor chance.
    pub country_female_advisor_chance: HashMap<Tag, Fixed>,

    /// Country-level yearly corruption modifier.
    /// Applied as additive bonus to yearly corruption (negative = reduction).
    pub country_yearly_corruption: HashMap<Tag, Fixed>,

    /// Country-level build time modifier.
    /// Applied as: (1 + modifier) multiplier to building construction time.
    pub country_build_time: HashMap<Tag, Fixed>,

    /// Country-level promote culture cost modifier.
    /// Applied as: (1 + modifier) multiplier to promote culture cost.
    pub country_promote_culture_cost: HashMap<Tag, Fixed>,

    /// Country-level liberty desire from subject development.
    /// Applied as: (1 + modifier) multiplier to liberty desire from development.
    pub country_liberty_desire_from_subject_development: HashMap<Tag, Fixed>,
}

impl GameModifiers {
    /// Get effective goods price: base + modifier.
    ///
    /// Returns the base price if no modifier exists.
    #[inline]
    pub fn effective_price(&self, id: TradegoodId, base: Fixed) -> Fixed {
        let modifier = self
            .goods_price_mods
            .get(&id)
            .copied()
            .unwrap_or(Fixed::ZERO);
        base + modifier
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tradegood_id_equality() {
        let grain = TradegoodId(0);
        let iron = TradegoodId(1);
        let grain2 = TradegoodId(0);

        assert_eq!(grain, grain2);
        assert_ne!(grain, iron);
    }

    #[test]
    fn test_effective_price_no_modifier() {
        let mods = GameModifiers::default();
        let base = Fixed::from_f32(2.5);
        let grain = TradegoodId(0);

        assert_eq!(mods.effective_price(grain, base), base);
    }

    #[test]
    fn test_effective_price_with_modifier() {
        let mut mods = GameModifiers::default();
        let grain = TradegoodId(0);
        let base = Fixed::from_f32(2.5);
        let bonus = Fixed::from_f32(0.5); // +0.5 price bonus

        mods.goods_price_mods.insert(grain, bonus);

        let expected = Fixed::from_f32(3.0);
        assert_eq!(mods.effective_price(grain, base), expected);
    }

    #[test]
    fn test_game_modifiers_default() {
        let mods = GameModifiers::default();
        assert!(mods.goods_price_mods.is_empty());
        assert!(mods.province_production_efficiency.is_empty());
        assert!(mods.province_autonomy.is_empty());
    }

    #[test]
    fn test_building_id_equality() {
        let temple = BuildingId(0);
        let workshop = BuildingId(1);
        let temple2 = BuildingId(0);

        assert_eq!(temple, temple2);
        assert_ne!(temple, workshop);
    }

    #[test]
    fn test_building_id_as_mask() {
        assert_eq!(BuildingId(0).as_mask(), 1);
        assert_eq!(BuildingId(1).as_mask(), 2);
        assert_eq!(BuildingId(7).as_mask(), 128);
        assert_eq!(BuildingId(63).as_mask(), 1u128 << 63);
    }
}
