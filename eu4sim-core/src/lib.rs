pub mod ai;
pub mod bounded;
pub mod config;
pub mod fixed;
pub mod input;
pub mod metrics;
pub mod observer;
pub use ai::{AiPlayer, RandomAi, VisibilityMode, VisibleWorldState};
pub mod modifiers;
pub mod state;
pub mod step;
pub mod systems;
pub mod testing;

pub use bounded::{new_prestige, new_stability, new_tradition, BoundedFixed, BoundedInt};
pub use config::SimConfig;
pub use fixed::Fixed;
pub use input::{Command, PlayerInputs};
pub use metrics::SimMetrics;
pub use modifiers::{GameModifiers, TradegoodId};
pub use observer::{ObserverConfig, ObserverError, ObserverRegistry, SimObserver, Snapshot};
pub use state::WorldState;
pub use step::{step_world, ActionError};
pub use systems::{run_production_tick, EconomyConfig};

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
