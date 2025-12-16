use eu4data_derive::{SchemaType, TolerantDeserialize};
use serde::Serialize;
use std::collections::HashMap;

/// Represents an advisor type in EU4.
///
/// Advisors provide country-wide bonuses and are a core mechanic in the base game.
/// Each advisor type has modifiers (bonuses) and a spawn chance factor.
#[derive(Debug, Clone, Serialize, TolerantDeserialize, SchemaType)]
pub struct AdvisorType {
    /// The bonuses this advisor provides (e.g., "trade_efficiency": 0.1)
    pub modifier: Option<HashMap<String, f32>>,

    /// Spawn chance/availability modifier
    pub factor: Option<f32>,
}
