//! # EU4 Simulation Core
//!
//! Deterministic game simulation engine for Europa Universalis IV.
//!
//! This crate implements the core game loop: state → commands → state transitions.
//! It is designed for lockstep multiplayer and replay determinism.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐     ┌──────────────┐     ┌─────────────┐
//! │  AI Players │────▶│ PlayerInputs │────▶│ step_world  │
//! │  (decide)   │     │ (commands)   │     │ (pure fn)   │
//! └─────────────┘     └──────────────┘     └──────┬──────┘
//!                                                 │
//!                     ┌──────────────┐     ┌──────▼──────┐
//!                     │  Observers   │◀────│ WorldState  │
//!                     │  (side fx)   │     │ (new state) │
//!                     └──────────────┘     └─────────────┘
//! ```
//!
//! ## Key Types
//!
//! | Type | Purpose |
//! |------|---------|
//! | [`WorldState`] | Complete simulation state (countries, provinces, wars) |
//! | [`Command`] | Player actions (Move, DeclareWar, BuyTech, etc.) |
//! | [`step_world`] | Pure function: `(state, inputs) -> state` |
//! | [`AiPlayer`] | Trait for AI decision making |
//! | [`SimObserver`] | Trait for observing state changes (training data, metrics) |
//!
//! ## AI System
//!
//! Built-in AI implementations:
//! - [`GreedyAI`]: Deterministic priority-based decisions
//! - [`RandomAi`]: Randomized decisions for exploration
//!
//! Multi-command support: AI can submit multiple commands per tick via
//! [`CommandCategory`](ai::CommandCategory) routing.
//!
//! ## Observers
//!
//! Side effects are isolated to the observer layer:
//! - [`DataGenObserver`]: Generates ML training data (Cap'n Proto format)
//! - [`EventLogObserver`]: Records game events for replay/debugging
//! - [`SimMetrics`]: Performance and game statistics

pub mod ai;
pub mod bounded;
pub mod buildings;
pub mod config;
pub mod trade;

// Cap'n Proto generated schema for training data serialization.
// Included at crate root because capnpc generates self-referential code.
#[allow(dead_code)]
#[allow(clippy::all)]
pub mod training_capnp {
    include!(concat!(env!("OUT_DIR"), "/training_capnp.rs"));
}
pub mod fixed;
pub mod input;
pub mod metrics;
pub mod observer;
pub use ai::{AiPlayer, GreedyAI, RandomAi, VisibilityMode, VisibleWorldState};
pub mod modifiers;
pub mod state;
pub mod step;
pub mod systems;
pub mod testing;

pub use bounded::{new_prestige, new_stability, new_tradition, BoundedFixed, BoundedInt};
pub use buildings::{BuildingConstruction, BuildingDef, BuildingSet, BuildingSlotSource};
pub use config::SimConfig;
pub use fixed::Fixed;
pub use input::{Command, PlayerInputs};
pub use metrics::SimMetrics;
pub use modifiers::{BuildingId, GameModifiers, TradegoodId};
pub use observer::datagen::{DataGenObserver, TrainingSample};
pub use observer::event_log::{EventLogObserver, GameEvent};
pub use observer::{ObserverConfig, ObserverError, ObserverRegistry, SimObserver, Snapshot};
pub use state::{InstitutionId, TechType, WorldState};
pub use step::{step_world, ActionError};
pub use systems::{run_production_tick, EconomyConfig};
pub use trade::{
    CountryTradeState, MerchantAction, MerchantState, ProvinceTradeState, TradeNodeId,
    TradeNodeState, TradeTopology,
};

/// Check if a command can be executed in the current state.
pub fn can_execute(_state: &WorldState, _country: &str, _cmd: &Command) -> Result<(), ActionError> {
    // Re-use logic or implement separate validation
    // Ideally this shares code with execute_command but without mutation
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_execute_stub() {
        // Verify the stub returns Ok
        let state = WorldState::default();
        let cmd = Command::Quit;
        assert!(can_execute(&state, "TAG", &cmd).is_ok());
    }
}
