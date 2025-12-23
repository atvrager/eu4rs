//! Keyboard and mouse input automation for EU4 game control.
//!
//! Uses `enigo` for cross-platform input simulation.

use anyhow::Result;
use enigo::{Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use std::thread;
use std::time::Duration;

use crate::regions::Region;

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

    /// Click at absolute screen coordinates.
    pub fn click_at(&mut self, x: i32, y: i32) -> Result<()> {
        self.enigo
            .move_mouse(x, y, Coordinate::Abs)
            .map_err(|e| anyhow::anyhow!("Failed to move mouse to ({}, {}): {:?}", x, y, e))?;
        thread::sleep(Duration::from_millis(50)); // Let cursor settle

        self.enigo
            .button(Button::Left, Direction::Click)
            .map_err(|e| anyhow::anyhow!("Failed to click at ({}, {}): {:?}", x, y, e))?;
        thread::sleep(Duration::from_millis(100)); // Wait for UI response
        Ok(())
    }

    /// Click at center of a Region.
    pub fn click_region(&mut self, region: &Region) -> Result<()> {
        let x = region.x as i32 + (region.width as i32 / 2);
        let y = region.y as i32 + (region.height as i32 / 2);
        log::debug!("Clicking region '{}' at ({}, {})", region.name, x, y);
        self.click_at(x, y)
    }
}
