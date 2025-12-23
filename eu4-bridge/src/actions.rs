//! Translate AI Commands to UI actions.
//!
//! Maps simulation commands to mouse clicks on game UI elements.

use crate::input::InputController;
use crate::regions::{PROV_MANP_BTN, PROV_PROD_BTN, PROV_TAX_BTN};
use anyhow::Result;
use eu4sim_core::Command;
use eu4sim_core::input::DevType;

/// Executes AI commands by interacting with the game UI.
pub struct ActionExecutor<'a> {
    input: &'a mut InputController,
}

impl<'a> ActionExecutor<'a> {
    /// Create a new action executor.
    pub fn new(input: &'a mut InputController) -> Self {
        Self { input }
    }

    /// Execute a command by clicking the appropriate UI element.
    ///
    /// Returns Ok(true) if the command was executed, Ok(false) if it was
    /// a no-op (like Pass), or Err if execution failed.
    pub fn execute(&mut self, cmd: &Command) -> Result<bool> {
        match cmd {
            Command::Pass => {
                log::debug!("Executing: Pass (no action)");
                Ok(false)
            }
            Command::DevelopProvince { province, dev_type } => {
                log::info!("Executing: Develop province {} ({:?})", province, dev_type);
                match dev_type {
                    DevType::Tax => {
                        self.input.click_region(&PROV_TAX_BTN)?;
                    }
                    DevType::Production => {
                        self.input.click_region(&PROV_PROD_BTN)?;
                    }
                    DevType::Manpower => {
                        self.input.click_region(&PROV_MANP_BTN)?;
                    }
                }
                Ok(true)
            }
            _ => {
                log::warn!("Command not implemented for UI execution: {:?}", cmd);
                Ok(false)
            }
        }
    }

    /// Execute a batch of commands.
    ///
    /// Returns the count of successfully executed commands.
    pub fn execute_all(&mut self, cmds: &[Command]) -> usize {
        let mut executed = 0;
        for cmd in cmds {
            match self.execute(cmd) {
                Ok(true) => executed += 1,
                Ok(false) => {}
                Err(e) => log::error!("Failed to execute {:?}: {}", cmd, e),
            }
        }
        executed
    }
}
