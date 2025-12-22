//! Keyboard and mouse input automation for EU4 game control.
//!
//! Uses `enigo` for cross-platform input simulation.

use anyhow::Result;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;

/// Controller for sending keyboard/mouse input to the game.
pub struct InputController {
    enigo: Enigo,
}

impl InputController {
    /// Create a new input controller.
    pub fn new() -> Result<Self> {
        let enigo = Enigo::new(&Settings::default())
            .map_err(|e| anyhow::anyhow!("Failed to initialize input controller: {:?}", e))?;
        Ok(Self { enigo })
    }

    /// Press spacebar to toggle pause state.
    pub fn toggle_pause(&mut self) -> Result<()> {
        self.enigo
            .key(Key::Space, Direction::Click)
            .map_err(|e| anyhow::anyhow!("Failed to send Space key: {:?}", e))?;
        // Brief delay for game to process
        thread::sleep(Duration::from_millis(100));
        Ok(())
    }

    /// Press a specific key (future: mapmode switching, hotkeys).
    #[allow(dead_code)]
    pub fn press_key(&mut self, key: Key) -> Result<()> {
        self.enigo
            .key(key, Direction::Click)
            .map_err(|e| anyhow::anyhow!("Failed to send key: {:?}", e))?;
        thread::sleep(Duration::from_millis(50));
        Ok(())
    }

    /// Type a character (useful for console commands in future).
    #[allow(dead_code)]
    pub fn type_char(&mut self, c: char) -> Result<()> {
        self.enigo
            .key(Key::Unicode(c), Direction::Click)
            .map_err(|e| anyhow::anyhow!("Failed to type char '{}': {:?}", c, e))?;
        Ok(())
    }

    /// Type a string (useful for console commands in future).
    #[allow(dead_code)]
    pub fn type_text(&mut self, text: &str) -> Result<()> {
        self.enigo
            .text(text)
            .map_err(|e| anyhow::anyhow!("Failed to type text: {:?}", e))?;
        Ok(())
    }
}
