use serde::Deserialize;

/// Represents the historical data of a province (e.g., in `history/provinces`).
#[derive(Debug, Deserialize)]
pub struct ProvinceHistory {
    /// The trade good produced in the province.
    pub trade_goods: Option<String>,
    /// The tag of the country that owns the province.
    pub owner: Option<String>,
    /// The base tax value of the province.
    pub base_tax: Option<f32>,
    /// The base production value of the province.
    pub base_production: Option<f32>,
    /// The base manpower value of the province.
    pub base_manpower: Option<f32>,
}
