use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ProvinceHistory {
    pub trade_goods: Option<String>,
    pub owner: Option<String>,
    pub base_tax: Option<f32>,
    pub base_production: Option<f32>,
    pub base_manpower: Option<f32>,
}
