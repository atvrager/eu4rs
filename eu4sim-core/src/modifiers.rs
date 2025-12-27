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

    /// Province-level trade power modifier (from buildings like Marketplace).
    /// Applied as: (1 + modifier) multiplier to provincial trade power.
    pub province_trade_power: HashMap<ProvinceId, Fixed>,

    /// Province-level manpower modifier (from buildings like Barracks).
    /// Applied as: (1 + modifier) multiplier to provincial manpower.
    pub province_manpower_modifier: HashMap<ProvinceId, Fixed>,

    /// Province-level sailors modifier (from buildings like Dock).
    /// Applied as: (1 + modifier) multiplier to provincial sailors.
    pub province_sailors_modifier: HashMap<ProvinceId, Fixed>,

    /// Province-level trade goods size modifier (from Manufactories).
    /// Applied as: additive bonus to trade goods produced.
    pub province_trade_goods_size: HashMap<ProvinceId, Fixed>,

    /// Province-level defensiveness modifier (from forts, ramparts).
    /// Applied as: (1 + modifier) multiplier to fort defense.
    pub province_defensiveness: HashMap<ProvinceId, Fixed>,

    /// Province-level ship repair modifier (from Shipyards, Drydocks).
    /// Applied as: (1 + modifier) multiplier to ship repair speed.
    pub province_ship_repair: HashMap<ProvinceId, Fixed>,

    /// Province-level ship cost modifier (from Dock, Drydock).
    /// Applied as: (1 + modifier) multiplier to ship construction cost.
    pub province_ship_cost: HashMap<ProvinceId, Fixed>,

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

    // === Naval Combat & Morale (1 modifier) ===
    /// Country-level sunk ship morale hit received modifier.
    /// Applied as: (1 + modifier) multiplier to morale damage when ships are sunk.
    pub country_sunk_ship_morale_hit_recieved: HashMap<Tag, Fixed>,

    // === Naval Recovery (1 modifier) ===
    /// Country-level sailors recovery speed modifier.
    /// Applied as: (1 + modifier) multiplier to sailors recovery rate.
    pub country_sailors_recovery_speed: HashMap<Tag, Fixed>,

    // === Tech Costs (2 modifiers) ===
    /// Country-level military tech cost modifier.
    /// Applied as: (1 + modifier) multiplier to military technology cost.
    pub country_mil_tech_cost: HashMap<Tag, Fixed>,

    /// Country-level diplomatic tech cost modifier.
    /// Applied as: (1 + modifier) multiplier to diplomatic technology cost.
    pub country_dip_tech_cost: HashMap<Tag, Fixed>,

    // === Government & Absolutism (4 modifiers) ===
    /// Country-level max absolutism modifier.
    /// Applied as: additive bonus to maximum absolutism cap.
    pub country_max_absolutism: HashMap<Tag, Fixed>,

    /// Country-level number of pronoiars modifier.
    /// Applied as: additive bonus to pronoia (Byzantine land grant) count.
    pub country_num_of_pronoiars: HashMap<Tag, Fixed>,

    /// Country-level max revolutionary zeal modifier.
    /// Applied as: additive bonus to maximum revolutionary zeal cap.
    pub country_max_revolutionary_zeal: HashMap<Tag, Fixed>,

    /// Country-level possible policy slots modifier.
    /// Applied as: additive bonus to number of policy slots.
    pub country_possible_policy: HashMap<Tag, Fixed>,

    // === Power Projection (1 modifier) ===
    /// Country-level power projection from insults modifier.
    /// Applied as: (1 + modifier) multiplier to power projection gained from insults.
    pub country_power_projection_from_insults: HashMap<Tag, Fixed>,

    // === Rebellion & Unrest (1 modifier) ===
    /// Country-level harsh treatment cost modifier.
    /// Applied as: (1 + modifier) multiplier to harsh treatment cost.
    pub country_harsh_treatment_cost: HashMap<Tag, Fixed>,

    // === Leaders (1 modifier) ===
    /// Country-level free leader pool modifier.
    /// Applied as: additive bonus to free leader pool size.
    pub country_free_leader_pool: HashMap<Tag, Fixed>,

    // === Naval Combat Bonuses (1 modifier) ===
    /// Country-level own coast naval combat bonus.
    /// Applied as: additive bonus to naval combat when fighting in own coastal waters.
    pub country_own_coast_naval_combat_bonus: HashMap<Tag, Fixed>,

    // === Technology & Innovation (1 modifier) ===
    /// Country-level institution embracement cost modifier.
    /// Applied as: (1 + modifier) multiplier to institution embracement cost.
    pub country_embracement_cost: HashMap<Tag, Fixed>,

    // === Military Costs (1 modifier) ===
    /// Country-level artillery cost modifier.
    /// Applied as: (1 + modifier) multiplier to artillery recruitment cost.
    pub country_artillery_cost: HashMap<Tag, Fixed>,

    // === Policy-Specific Modifiers (49 modifiers) ===
    // These modifiers are primarily used by policies (combinations of idea groups).

    // === Colonization (3 modifiers) ===
    /// Country-level colonist placement chance modifier.
    /// Applied as: (1 + modifier) multiplier to colonist success chance.
    pub country_colonist_placement_chance: HashMap<Tag, Fixed>,

    /// Country-level native uprising chance modifier.
    /// Applied as: (1 + modifier) multiplier to native uprising chance.
    pub country_native_uprising_chance: HashMap<Tag, Fixed>,

    /// Country-level native assimilation modifier.
    /// Applied as: (1 + modifier) multiplier to native assimilation rate.
    pub country_native_assimilation: HashMap<Tag, Fixed>,

    // === Naval Combat & Morale (8 modifiers) ===
    /// Country-level navy morale recovery speed modifier.
    /// Applied as: (1 + modifier) multiplier to navy morale recovery rate.
    pub country_recover_navy_morale_speed: HashMap<Tag, Fixed>,

    /// Country-level global naval engagement modifier.
    /// Applied as additive bonus to naval engagement width.
    pub country_global_naval_engagement_modifier: HashMap<Tag, Fixed>,

    /// Country-level naval tradition from battle modifier.
    /// Applied as: (1 + modifier) multiplier to navy tradition from battles.
    pub country_naval_tradition_from_battle: HashMap<Tag, Fixed>,

    /// Country-level prestige from naval battles modifier.
    /// Applied as: (1 + modifier) multiplier to prestige from naval battles.
    pub country_prestige_from_naval: HashMap<Tag, Fixed>,

    /// Country-level disengagement chance modifier.
    /// Applied as: (1 + modifier) multiplier to chance of disengaging from combat.
    pub country_disengagement_chance: HashMap<Tag, Fixed>,

    /// Country-level naval leader shock bonus.
    /// Applied as additive bonus to naval leader shock skill.
    pub country_leader_naval_shock: HashMap<Tag, Fixed>,

    /// Country-level movement speed in fleet modifier.
    /// Applied as: (1 + modifier) multiplier to army movement when transported.
    pub country_movement_speed_in_fleet_modifier: HashMap<Tag, Fixed>,

    /// Country-level morale damage received modifier.
    /// Applied as: (1 + modifier) multiplier to morale damage taken.
    pub country_morale_damage_received: HashMap<Tag, Fixed>,

    // === Army Composition (3 modifiers) ===
    /// Country-level artillery fraction modifier.
    /// Applied as additive bonus to artillery ratio in armies.
    pub country_artillery_fraction: HashMap<Tag, Fixed>,

    /// Country-level cavalry fraction modifier.
    /// Applied as additive bonus to cavalry ratio in armies.
    pub country_cavalry_fraction: HashMap<Tag, Fixed>,

    /// Country-level infantry fraction modifier.
    /// Applied as additive bonus to infantry ratio in armies.
    pub country_infantry_fraction: HashMap<Tag, Fixed>,

    // === Economy & Trade (3 modifiers) ===
    /// Country-level mercantilism cost modifier.
    /// Applied as: (1 + modifier) multiplier to mercantilism increase cost.
    pub country_mercantilism_cost: HashMap<Tag, Fixed>,

    /// Country-level global tariffs modifier.
    /// Applied as: (1 + modifier) multiplier to tariff income from subjects.
    pub country_global_tariffs: HashMap<Tag, Fixed>,

    /// Country-level monthly favor modifier.
    /// Applied as: (1 + modifier) multiplier to monthly favor gain from allies.
    pub country_monthly_favor_modifier: HashMap<Tag, Fixed>,

    // === Siege & Fortification (5 modifiers) ===
    /// Country-level siege blockade progress modifier.
    /// Applied as: (1 + modifier) multiplier to siege progress from naval blockade.
    pub country_siege_blockade_progress: HashMap<Tag, Fixed>,

    /// Country-level blockade efficiency modifier.
    /// Applied as: (1 + modifier) multiplier to naval blockade effectiveness.
    pub country_blockade_efficiency: HashMap<Tag, Fixed>,

    /// Country-level garrison damage modifier.
    /// Applied as: (1 + modifier) multiplier to damage dealt to garrison.
    pub country_garrison_damage: HashMap<Tag, Fixed>,

    /// Country-level artillery level modifier.
    /// Applied as additive bonus to effective artillery level in sieges.
    pub country_artillery_level_modifier: HashMap<Tag, Fixed>,

    /// Country-level artillery levels available vs fort modifier.
    /// Applied as additive bonus to artillery bonus against forts.
    pub country_artillery_levels_available_vs_fort: HashMap<Tag, Fixed>,

    // === Military Costs & Efficiency (5 modifiers) ===
    /// Country-level morale damage modifier.
    /// Applied as: (1 + modifier) multiplier to morale damage dealt.
    pub country_morale_damage: HashMap<Tag, Fixed>,

    /// Country-level reinforce cost modifier.
    /// Applied as: (1 + modifier) multiplier to reinforcement cost.
    pub country_reinforce_cost_modifier: HashMap<Tag, Fixed>,

    /// Country-level drill gain modifier.
    /// Applied as: (1 + modifier) multiplier to monthly drill gain.
    pub country_drill_gain_modifier: HashMap<Tag, Fixed>,

    /// Country-level yearly army professionalism modifier.
    /// Applied as additive bonus to yearly army professionalism.
    pub country_yearly_army_professionalism: HashMap<Tag, Fixed>,

    /// Country-level special unit force limit modifier.
    /// Applied as: (1 + modifier) multiplier to special unit force limit.
    pub country_special_unit_forcelimit: HashMap<Tag, Fixed>,

    // === Development & Culture (2 modifiers) ===
    /// Country-level development cost in primary culture modifier.
    /// Applied as: (1 + modifier) multiplier to development cost in primary culture provinces.
    pub country_development_cost_in_primary_culture: HashMap<Tag, Fixed>,

    /// Country-level colony development boost modifier.
    /// Applied as additive bonus to development in colonial nations.
    pub country_colony_development_boost: HashMap<Tag, Fixed>,

    // === Diplomacy & Subjects (5 modifiers) ===
    /// Country-level rival border fort maintenance modifier.
    /// Applied as: (1 + modifier) multiplier to fort maintenance on rival borders.
    pub country_rival_border_fort_maintenance: HashMap<Tag, Fixed>,

    /// Country-level reduced liberty desire on same continent modifier.
    /// Applied as additive reduction to liberty desire for subjects on same continent.
    pub country_reduced_liberty_desire_on_same_continent: HashMap<Tag, Fixed>,

    /// Country-level years to integrate personal union modifier.
    /// Applied as: (1 + modifier) multiplier to integration time for personal unions.
    pub country_years_to_integrate_personal_union: HashMap<Tag, Fixed>,

    /// Country-level monthly federation favor growth modifier.
    /// Applied as: (1 + modifier) multiplier to monthly federation favor growth.
    pub country_monthly_federation_favor_growth: HashMap<Tag, Fixed>,

    /// Country-level all estate loyalty equilibrium modifier.
    /// Applied as additive bonus to all estate loyalty equilibrium.
    pub country_all_estate_loyalty_equilibrium: HashMap<Tag, Fixed>,

    // === Religion & Authority (4 modifiers) ===
    /// Country-level prestige per development from conversion modifier.
    /// Applied as: (1 + modifier) multiplier to prestige from converting provinces.
    pub country_prestige_per_development_from_conversion: HashMap<Tag, Fixed>,

    /// Country-level yearly patriarch authority modifier.
    /// Applied as additive bonus to yearly patriarch authority (Orthodox).
    pub country_yearly_patriarch_authority: HashMap<Tag, Fixed>,

    /// Country-level yearly harmony modifier.
    /// Applied as additive bonus to yearly harmony (Confucian).
    pub country_yearly_harmony: HashMap<Tag, Fixed>,

    /// Country-level yearly karma decay modifier.
    /// Applied as: (1 + modifier) multiplier to yearly karma decay (Buddhist).
    pub country_yearly_karma_decay: HashMap<Tag, Fixed>,

    // === Government & Leaders (5 modifiers) ===
    /// Country-level innovativeness gain modifier.
    /// Applied as: (1 + modifier) multiplier to innovativeness gain.
    pub country_innovativeness_gain: HashMap<Tag, Fixed>,

    /// Country-level raze power gain modifier.
    /// Applied as: (1 + modifier) multiplier to raze power gain (Hordes).
    pub country_raze_power_gain: HashMap<Tag, Fixed>,

    /// Country-level monarch lifespan modifier.
    /// Applied as: (1 + modifier) multiplier to monarch lifespan.
    pub country_monarch_lifespan: HashMap<Tag, Fixed>,

    /// Country-level reelection cost modifier.
    /// Applied as: (1 + modifier) multiplier to reelection cost (Republics).
    pub country_reelection_cost: HashMap<Tag, Fixed>,

    /// Country-level military advisor cost modifier.
    /// Applied as: (1 + modifier) multiplier to military advisor hiring cost.
    pub country_mil_advisor_cost: HashMap<Tag, Fixed>,

    // === War & Diplomacy (2 modifiers) ===
    /// Country-level warscore cost vs other religion modifier.
    /// Applied as: (1 + modifier) multiplier to warscore cost for different religion.
    pub country_warscore_cost_vs_other_religion: HashMap<Tag, Fixed>,

    /// Country-level global rebel suppression efficiency modifier.
    /// Applied as: (1 + modifier) multiplier to rebel suppression efficiency.
    pub country_global_rebel_suppression_efficiency: HashMap<Tag, Fixed>,

    // === Naval Infrastructure (2 modifiers) ===
    /// Country-level global ship repair modifier.
    /// Applied as: (1 + modifier) multiplier to ship repair speed globally.
    pub country_global_ship_repair: HashMap<Tag, Fixed>,

    /// Country-level transport attrition modifier.
    /// Applied as: (1 + modifier) multiplier to attrition for transported armies.
    pub country_transport_attrition: HashMap<Tag, Fixed>,

    // === Province Management (2 modifiers) ===
    /// Country-level manpower in true faith provinces modifier.
    /// Applied as: (1 + modifier) multiplier to manpower in true faith provinces.
    pub country_manpower_in_true_faith_provinces: HashMap<Tag, Fixed>,

    /// Country-level global monthly devastation modifier.
    /// Applied as additive modifier to monthly devastation change.
    pub country_global_monthly_devastation: HashMap<Tag, Fixed>,

    // === Batch 1: Positions 21-25 (Frequency-Driven) ===
    /// Country-level monarch military power bonus.
    /// Applied as additive bonus to ruler's military skill.
    pub country_monarch_military_power: HashMap<Tag, Fixed>,

    /// Country-level center of trade upgrade cost modifier.
    /// Applied as: (1 + modifier) multiplier to upgrade cost.
    pub country_center_of_trade_upgrade_cost: HashMap<Tag, Fixed>,

    /// Country-level accept vassalization reasons modifier.
    /// Applied as: additive bonus to AI acceptance of vassalization.
    pub country_accept_vassalization_reasons: HashMap<Tag, Fixed>,

    /// Country-level Brahmins (Hindu) estate loyalty modifier.
    /// Applied as: additive bonus to estate loyalty.
    pub country_brahmins_hindu_loyalty_modifier: HashMap<Tag, Fixed>,

    /// Country-level Brahmins (Muslim) estate loyalty modifier.
    /// Applied as: additive bonus to estate loyalty.
    pub country_brahmins_muslim_loyalty_modifier: HashMap<Tag, Fixed>,

    // === Batch 2: Positions 26-30 (Frequency-Driven) ===
    /// Country-level tolerance of heathens capacity modifier.
    /// Applied as: additive bonus to max tolerance of heathens.
    pub country_tolerance_of_heathens_capacity: HashMap<Tag, Fixed>,

    /// Country-level possible military policy slots modifier.
    /// Applied as: additive bonus to available MIL policy slots.
    pub country_possible_mil_policy: HashMap<Tag, Fixed>,

    /// Country-level curia powers cost modifier.
    /// Applied as: (1 + modifier) multiplier to curia power costs.
    pub country_curia_powers_cost: HashMap<Tag, Fixed>,

    /// Country-level expand administration cost modifier.
    /// Applied as: (1 + modifier) multiplier to expansion cost.
    pub country_expand_administration_cost: HashMap<Tag, Fixed>,

    /// Country-level loyalty change on revoked privilege modifier.
    /// Applied as: additive modifier to loyalty penalty when revoking.
    pub country_loyalty_change_on_revoked: HashMap<Tag, Fixed>,

    // === Batch 3: Positions 31-35 (Frequency-Driven) ===
    /// Country-level great project upgrade cost modifier.
    /// Applied as: (1 + modifier) multiplier to upgrade cost.
    pub country_great_project_upgrade_cost: HashMap<Tag, Fixed>,

    /// Country-level gold depletion chance modifier.
    /// Applied as: (1 + modifier) multiplier to depletion chance.
    pub country_gold_depletion_chance_modifier: HashMap<Tag, Fixed>,

    /// Country-level global supply limit modifier.
    /// Applied as: (1 + modifier) multiplier to province supply limits.
    pub country_global_supply_limit_modifier: HashMap<Tag, Fixed>,

    /// Country-level general cost modifier.
    /// Applied as: (1 + modifier) multiplier to general recruitment cost.
    pub country_general_cost: HashMap<Tag, Fixed>,

    /// Country-level leader cost modifier (generals + admirals).
    /// Applied as: (1 + modifier) multiplier to all leader costs.
    pub country_leader_cost: HashMap<Tag, Fixed>,

    // === Batch 4: Positions 36-40 (Frequency-Driven) ===
    /// Country-level cavalry fire pips bonus.
    /// Applied as: additive bonus to cavalry fire pips.
    pub country_cavalry_fire: HashMap<Tag, Fixed>,

    /// Country-level war taxes cost modifier.
    /// Applied as: (1 + modifier) multiplier to war taxes cost.
    pub country_war_taxes_cost_modifier: HashMap<Tag, Fixed>,

    /// Country-level Vaisyas estate loyalty modifier.
    /// Applied as: additive bonus to estate loyalty.
    pub country_vaisyas_loyalty_modifier: HashMap<Tag, Fixed>,

    /// Country-level max hostile attrition modifier.
    /// Applied as: additive bonus to attrition dealt to enemies.
    pub country_max_hostile_attrition: HashMap<Tag, Fixed>,

    /// Country-level nobles influence modifier.
    /// Applied as: additive modifier to nobles estate influence.
    pub country_nobles_influence_modifier: HashMap<Tag, Fixed>,
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
