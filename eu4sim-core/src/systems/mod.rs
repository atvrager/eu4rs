//! Economy simulation systems.

pub mod production;
pub mod taxation;
pub mod manpower;

pub use production::{run_production_tick, EconomyConfig};
pub use taxation::run_taxation_tick;
pub use manpower::run_manpower_tick;
