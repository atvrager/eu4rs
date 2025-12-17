//! Simulation systems.

pub mod combat;
pub mod expenses;
pub mod manpower;
pub mod production;
pub mod taxation;

pub use combat::run_combat_tick;
pub use expenses::run_expenses_tick;
pub use manpower::run_manpower_tick;
pub use production::{run_production_tick, EconomyConfig};
pub use taxation::run_taxation_tick;
