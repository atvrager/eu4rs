use eu4data_derive::{SchemaType, TolerantDeserialize};
use serde::Serialize;

/// Represents technology definitions in EU4, including institution requirements.
///
/// Technologies in EU4 define the institution requirements that affect technology costs.
/// Each institution field represents the penalty reduction when that institution is present.
#[derive(Debug, Clone, Serialize, TolerantDeserialize, SchemaType)]
pub struct Technology {
    /// Feudalism institution requirement
    pub feudalism: Option<f32>,

    /// Renaissance institution requirement
    pub renaissance: Option<f32>,

    /// New World I institution requirement
    pub new_world_i: Option<f32>,

    /// Printing Press institution requirement
    pub printing_press: Option<f32>,

    /// Global Trade institution requirement
    pub global_trade: Option<f32>,

    /// Manufactories institution requirement
    pub manufactories: Option<f32>,

    /// Enlightenment institution requirement
    pub enlightenment: Option<f32>,

    /// Industrialization institution requirement
    pub industrialization: Option<f32>,
}
