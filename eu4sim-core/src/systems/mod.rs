//! Simulation systems.

pub mod advisors;
pub mod attrition;
pub mod buildings;
pub mod coalitions;
pub mod colonization;
pub mod combat;
pub mod coring;
pub mod development;
pub mod estates;
pub mod expenses;
pub mod ideas;
pub mod institutions;
pub mod mana;
pub mod manpower;
pub mod movement;
pub mod naval_combat;
pub mod policies;
pub mod production;
pub mod reformation;
pub mod siege;
pub mod stats;
pub mod taxation;
pub mod tech;
pub mod trade_income;
pub mod tribute;
pub mod trade_power;
pub mod trade_value;
pub mod war_score;

pub use advisors::run_advisor_cost_tick;
pub use attrition::run_attrition_tick;
pub use buildings::{
    available_buildings, can_build, cancel_construction_conquest, cancel_construction_manual,
    demolish_building, max_building_slots, recompute_fort_level, recompute_province_modifiers,
    start_construction, tick_building_construction, transfer_construction_diplomatic,
    validate_manufactory_on_goods_change, BuildingError,
};
pub use coalitions::run_coalition_tick;
pub use colonization::run_colonization_tick;
pub use combat::run_combat_tick;
pub use coring::{
    calculate_coring_cost, effective_autonomy, recalculate_overextension, start_coring, tick_coring,
};
pub use development::develop_province;
pub use estates::{
    grant_privilege, revoke_privilege, run_estate_tick, sale_land, seize_land, CrownLandError,
    PrivilegeError,
};
pub use expenses::run_expenses_tick;
pub use ideas::{
    apply_modifier, print_modifier_report, recalculate_idea_modifiers, scan_all_modifiers,
    IdeaModifierStats, ModifierStubTracker,
};
pub use institutions::{embrace_institution, tick_institution_spread};
pub use mana::run_mana_tick;
pub use manpower::run_manpower_tick;
pub use movement::run_movement_tick;
pub use naval_combat::run_naval_combat_tick;
pub use policies::{
    apply_policy_modifiers, calculate_policy_slots, can_enable_policy, disable_policy,
    enable_policy, PolicyCategory, PolicyDef, PolicyError, PolicyId, PolicyRegistry,
};
pub use production::{run_production_tick, EconomyConfig};
pub use reformation::run_reformation_tick;
pub use siege::{run_siege_tick, start_occupation};
pub use stats::run_stats_tick;
pub use taxation::run_taxation_tick;
pub use tech::buy_tech;
pub use trade_income::run_trade_income_tick;
pub use tribute::run_tribute_payments;
pub use trade_power::{run_merchant_arrivals, run_trade_power_tick};
pub use trade_value::run_trade_value_tick;
pub use war_score::{award_battle_score, recalculate_war_scores, update_province_controller};
