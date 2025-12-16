use eu4data_derive::{SchemaType, TolerantDeserialize};
use serde::Serialize;

/// Represents a trade node in EU4's trade network.
///
/// Trade nodes are fundamental to the economic system, defining how trade
/// flows between regions and where merchants can be placed.
#[derive(Debug, Clone, Serialize, TolerantDeserialize, SchemaType)]
pub struct TradeNode {
    /// Control points/coordinates (likely for UI positioning)
    pub control: Option<Vec<f32>>,

    /// Name of the trade node
    pub name: Option<String>,

    /// Trade flow paths to other nodes
    pub path: Option<Vec<String>>,
}
