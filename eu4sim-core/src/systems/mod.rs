//! Simulation systems.

pub mod colonization;
pub mod combat;
pub mod development;
pub mod expenses;
pub mod institutions;
pub mod mana;
pub mod manpower;
pub mod movement;
pub mod production;
pub mod reformation;
pub mod stats;
pub mod taxation;
pub mod tech;
pub mod war_score;

pub use colonization::run_colonization_tick;
pub use combat::run_combat_tick;
pub use development::develop_province;
pub use expenses::run_expenses_tick;
pub use institutions::{embrace_institution, tick_institution_spread};
pub use mana::run_mana_tick;
pub use manpower::run_manpower_tick;
pub use movement::run_movement_tick;
pub use production::{run_production_tick, EconomyConfig};
pub use reformation::run_reformation_tick;
pub use stats::run_stats_tick;
pub use taxation::run_taxation_tick;
pub use tech::buy_tech;
pub use war_score::{award_battle_score, recalculate_war_scores, update_province_controller};
