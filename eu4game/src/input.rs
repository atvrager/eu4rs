//! Input state machine for player interaction.
//!
//! Handles modal input states like normal browsing, army movement, etc.

use eu4sim_core::state::{ArmyId, FleetId, ProvinceId};

/// Input mode state machine.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum InputMode {
    /// Normal mode - click to select provinces, view info.
    #[default]
    Normal,
    /// Moving an army - click to set destination.
    MovingArmy { army_id: ArmyId },
    /// Moving a fleet - click to set destination (sea zone).
    MovingFleet { fleet_id: FleetId },
    /// Declaring war - click on enemy territory to declare.
    DeclaringWar,
}

impl InputMode {
    /// Returns a description of the current mode for display.
    pub fn description(&self) -> &'static str {
        match self {
            InputMode::Normal => "Normal",
            InputMode::MovingArmy { .. } => "Move Army (click destination, ESC to cancel)",
            InputMode::MovingFleet { .. } => "Move Fleet (click sea zone, ESC to cancel)",
            InputMode::DeclaringWar => "Declare War (click enemy province, ESC to cancel)",
        }
    }

    /// Returns true if this mode can be cancelled with ESC.
    pub fn is_cancellable(&self) -> bool {
        !matches!(self, InputMode::Normal)
    }
}

/// Player command generated from input.
#[derive(Debug, Clone)]
#[allow(dead_code)] // All variants are used in match patterns
pub enum PlayerAction {
    /// Select a province (for viewing info).
    SelectProvince(ProvinceId),
    /// Move an army to a destination.
    MoveArmy {
        army_id: ArmyId,
        destination: ProvinceId,
    },
    /// Move a fleet to a destination (sea zone).
    MoveFleet {
        fleet_id: FleetId,
        destination: ProvinceId,
    },
    /// Declare war on a country.
    DeclareWar { target: String },
    /// Cancel current action.
    Cancel,
    /// No action.
    None,
}

/// Processes a province click based on current input mode.
pub fn handle_province_click(
    mode: &InputMode,
    province_id: ProvinceId,
    province_owner: Option<&str>,
    player_tag: &str,
) -> (InputMode, PlayerAction) {
    match mode {
        InputMode::Normal => {
            // Just select the province
            (InputMode::Normal, PlayerAction::SelectProvince(province_id))
        }
        InputMode::MovingArmy { army_id } => {
            // Issue move command
            let action = PlayerAction::MoveArmy {
                army_id: *army_id,
                destination: province_id,
            };
            (InputMode::Normal, action)
        }
        InputMode::MovingFleet { fleet_id } => {
            // Issue fleet move command (destination should be a sea zone)
            let action = PlayerAction::MoveFleet {
                fleet_id: *fleet_id,
                destination: province_id,
            };
            (InputMode::Normal, action)
        }
        InputMode::DeclaringWar => {
            // Check if clicking on enemy territory
            if let Some(owner) = province_owner
                && owner != player_tag
            {
                let action = PlayerAction::DeclareWar {
                    target: owner.to_string(),
                };
                return (InputMode::Normal, action);
            }
            // Clicked on own/neutral territory - stay in mode
            (InputMode::DeclaringWar, PlayerAction::None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_mode_selects_province() {
        let mode = InputMode::Normal;
        let (new_mode, action) = handle_province_click(&mode, 123, Some("FRA"), "ENG");
        assert_eq!(new_mode, InputMode::Normal);
        assert!(matches!(action, PlayerAction::SelectProvince(123)));
    }

    #[test]
    fn test_moving_army_issues_move() {
        let mode = InputMode::MovingArmy { army_id: 42 };
        let (new_mode, action) = handle_province_click(&mode, 100, Some("FRA"), "ENG");
        assert_eq!(new_mode, InputMode::Normal);
        assert!(matches!(
            action,
            PlayerAction::MoveArmy {
                army_id: 42,
                destination: 100
            }
        ));
    }

    #[test]
    fn test_declare_war_on_enemy() {
        let mode = InputMode::DeclaringWar;
        let (new_mode, action) = handle_province_click(&mode, 50, Some("FRA"), "ENG");
        assert_eq!(new_mode, InputMode::Normal);
        if let PlayerAction::DeclareWar { target } = action {
            assert_eq!(target, "FRA");
        } else {
            panic!("Expected DeclareWar action");
        }
    }

    #[test]
    fn test_declare_war_on_own_does_nothing() {
        let mode = InputMode::DeclaringWar;
        let (new_mode, action) = handle_province_click(&mode, 50, Some("ENG"), "ENG");
        assert_eq!(new_mode, InputMode::DeclaringWar);
        assert!(matches!(action, PlayerAction::None));
    }

    #[test]
    fn test_moving_fleet_issues_move() {
        let mode = InputMode::MovingFleet { fleet_id: 99 };
        let (new_mode, action) = handle_province_click(&mode, 1234, None, "ENG");
        assert_eq!(new_mode, InputMode::Normal);
        assert!(matches!(
            action,
            PlayerAction::MoveFleet {
                fleet_id: 99,
                destination: 1234
            }
        ));
    }

    #[test]
    fn test_mode_descriptions() {
        assert_eq!(InputMode::Normal.description(), "Normal");
        assert!(
            InputMode::MovingArmy { army_id: 1 }
                .description()
                .contains("Move")
        );
        assert!(
            InputMode::MovingFleet { fleet_id: 1 }
                .description()
                .contains("Fleet")
        );
        assert!(InputMode::DeclaringWar.description().contains("War"));
    }

    #[test]
    fn test_cancellable() {
        assert!(!InputMode::Normal.is_cancellable());
        assert!(InputMode::MovingArmy { army_id: 1 }.is_cancellable());
        assert!(InputMode::MovingFleet { fleet_id: 1 }.is_cancellable());
        assert!(InputMode::DeclaringWar.is_cancellable());
    }
}
