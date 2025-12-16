//! Public type definitions for EU4 game data.
//!
//! This module provides a clean public API for accessing EU4 data types.
//! Types are either:
//! - Auto-generated from schema (re-exported from `generated::types`)
//! - Manually implemented for complex cases
//!
//! To override a generated type, replace the shim file with a manual implementation.

// Re-exports for convenience (optional, but good for flat API eu4data::types::Technology)
pub use crate::generated::types::*;
