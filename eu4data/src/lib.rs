use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize)]
pub struct Tradegood {
    #[serde(default)]
    pub color: Vec<f32>,
    #[serde(default)]
    pub modifier: HashMap<String, f32>,
    #[serde(default)]
    pub province: Option<HashMap<String, f32>>,
    #[serde(default)] 
    pub chance: Option<HashMap<String, serde_json::Value>>, 
}
