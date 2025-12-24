//! # EU4 Game Data Library
//!
//! Strongly-typed Rust structures for Europa Universalis IV game data.
//!
//! This crate provides data models and loading logic for EU4's text-based
//! configuration files. It uses [`eu4txt`] for parsing and provides
//! convenient APIs for accessing game data.
//!
//! ## Modules
//!
//! | Module | Description |
//! |--------|-------------|
//! | [`countries`] | Country definitions and tags |
//! | [`map`] | Province definitions, adjacencies, terrain |
//! | [`tradenodes`] | Trade node graph and routing |
//! | [`tradegoods`] | Trade goods and modifiers |
//! | [`history`] | Province and country history files |
//! | [`localisation`] | Localization string lookup |
//! | [`defines`] | Game defines (constants) |
//!
//! ## Example
//!
//! ```ignore
//! use eu4data::{path::GameFiles, map::Map};
//!
//! let game = GameFiles::detect()?;
//! let map = Map::load(&game)?;
//! println!("Loaded {} provinces", map.provinces.len());
//! ```
//!
//! ## Code Generation
//!
//! The [`generated`] module contains auto-generated structs from EU4 data
//! schemas. See `cargo xtask coverage --generate` for regeneration.

pub mod adjacency;
pub mod cache;
pub mod climate;
pub mod countries;
pub mod coverage;
pub mod cultures;
pub mod defines;
pub mod discovery;
pub mod generated;
pub mod history;
pub mod localisation;
pub mod manifest;
pub mod map;
pub mod path;
pub mod religions;
pub mod terrain;
pub mod tradegoods;
pub mod tradenodes;
pub mod types;
pub use types::*;

// Re-export common types for backward compatibility
