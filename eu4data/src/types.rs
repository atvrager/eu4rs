//! Public type definitions for EU4 game data.
//!
//! This module provides a clean public API for accessing EU4 data types.
//! Types are either:
//! - Auto-generated from schema (re-exported from `generated::types`)
//! - Manually implemented for complex cases
//!
//! To override a generated type, replace the shim file with a manual implementation.

pub mod advisortypes;
pub mod technologies;
pub mod timed_modifiers;
pub mod tradegoods;
pub mod tradenodes;

// Re-exports for convenience (optional, but good for flat API eu4data::types::Technology)
pub use advisortypes::AdvisorType;
pub use technologies::Technology;
pub use timed_modifiers::TimedModifier;
pub use tradegoods::Tradegood;
pub use tradenodes::TradeNode;
