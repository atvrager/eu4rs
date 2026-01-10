//! EU4 game engine library.
//!
//! This crate provides the core game engine functionality including:
//! - GUI parsing and layout (`gui` module)
//! - Graphics rendering (internal)
//! - Game state management (internal)

pub mod gui {
    pub mod interner;
    pub mod layout;
    pub mod parser;
    pub mod types;
}

pub mod generated {
    pub mod gui;
}
