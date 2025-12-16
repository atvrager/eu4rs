use eu4data_derive::{SchemaType, TolerantDeserialize};
use serde::Serialize;

/// Represents a timed modifier in EU4 that decays over time.
///
/// Timed modifiers have a base value and decay by a certain amount each year.
#[derive(Debug, Clone, Serialize, TolerantDeserialize, SchemaType)]
pub struct TimedModifier {
    /// The base value of the modifier
    pub value: Option<f32>,

    /// The amount the modifier decays each year
    pub yearly_decay: Option<f32>,
}
