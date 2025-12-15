use crate::input::{Command, PlayerInputs};
use crate::state::WorldState;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ActionError {
    #[error("Insufficient funds: required {required}, available {available}")]
    InsufficientFunds { required: f32, available: f32 },
    // Add other errors
}

/// Advance the world by one tick.
pub fn step_world(state: &WorldState, inputs: &[PlayerInputs]) -> WorldState {
    let mut new_state = state.clone();

    // 1. Advance Date
    new_state.date = state.date.add_days(1);

    // 2. Process Inputs
    for player_input in inputs {
        for cmd in &player_input.commands {
            if let Err(e) = execute_command(&mut new_state, &player_input.country, cmd) {
                log::warn!(
                    "Failed to execute command for {}: {}",
                    player_input.country,
                    e
                );
            }
        }
    }

    // 3. Run Systems (Economy, Pop growth, etc.)
    // run_economy_tick(&mut new_state);

    new_state
}

fn execute_command(
    state: &mut WorldState,
    country_tag: &str,
    cmd: &Command,
) -> Result<(), ActionError> {
    match cmd {
        Command::BuildInProvince {
            province: _,
            building: _,
        } => {
            // Stub implementation
            let _country =
                state
                    .countries
                    .get(country_tag)
                    .ok_or(ActionError::InsufficientFunds {
                        required: 0.0,
                        available: 0.0,
                    })?; // Better error needed

            // Validate Logic (Check cost vs treasury)
            // if country.treasury < cost ...

            // Apply Effect
            log::info!("Player {} building something (stub)", country_tag);

            Ok(())
        }
        Command::Quit => Ok(()), // Handled by outer loop usually, but harmless here
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Date;
    use crate::testing::WorldStateBuilder;

    #[test]
    fn test_step_world_advances_date() {
        let state = WorldStateBuilder::new().date(1444, 11, 11).build();

        let inputs = vec![];
        let new_state = step_world(&state, &inputs);

        assert_eq!(new_state.date, Date::new(1444, 11, 12));
    }

    #[test]
    fn test_step_world_command_execution() {
        let state = WorldStateBuilder::new()
            .date(1444, 11, 11)
            .with_country("SWE")
            .build();

        let inputs = vec![PlayerInputs {
            country: "SWE".to_string(),
            commands: vec![Command::BuildInProvince {
                province: 1,
                building: "temple".to_string(),
            }],
        }];

        // This should log (we can't easily assert logs without a capture, but we know it runs)
        // Ideally we'd inspect side effects on state, but the stub does nothing yet.
        let _new_state = step_world(&state, &inputs);

        // Assert no crash and logic ran
    }

    #[test]
    fn test_determinism() {
        let state = WorldStateBuilder::new()
            .date(1444, 1, 1)
            .with_country("SWE")
            .build();

        let inputs = vec![];

        let state_a = step_world(&state, &inputs);
        let state_b = step_world(&state, &inputs);

        // Serialize to compare fully or just debug format
        let json_a = serde_json::to_string(&state_a).unwrap();
        let json_b = serde_json::to_string(&state_b).unwrap();

        assert_eq!(json_a, json_b);
    }
}
