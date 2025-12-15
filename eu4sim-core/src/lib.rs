pub mod input;
pub mod state;
pub mod step;
pub mod testing;

pub use input::{Command, PlayerInputs};
pub use state::WorldState;
pub use step::{step_world, ActionError};

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
