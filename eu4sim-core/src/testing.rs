use crate::state::{CountryState, Date, ProvinceId, ProvinceState, WorldState};
use std::collections::HashMap;

pub struct WorldStateBuilder {
    state: WorldState,
}

impl WorldStateBuilder {
    pub fn new() -> Self {
        Self {
            state: WorldState {
                date: Date::new(1444, 11, 11),
                rng_seed: 0,
                provinces: HashMap::new(),
                countries: HashMap::new(),
                diplomacy: Default::default(),
                global: Default::default(),
            },
        }
    }

    pub fn date(mut self, year: i32, month: u8, day: u8) -> Self {
        self.state.date = Date::new(year, month, day);
        self
    }

    pub fn with_country(mut self, tag: &str) -> Self {
        self.state.countries.insert(
            tag.to_string(),
            CountryState {
                treasury: 100.0, // Default generous treasury for testing
                manpower: 50000.0,
                stability: 0,
                prestige: 0.0,
            },
        );
        self
    }

    pub fn with_province(mut self, id: ProvinceId, owner_tag: Option<&str>) -> Self {
        self.state.provinces.insert(
            id,
            ProvinceState {
                owner: owner_tag.map(|s| s.to_string()),
                religion: None,
                culture: None,
                tax: 1.0,
                production: 1.0,
                manpower: 1.0,
            },
        );
        self
    }

    pub fn build(self) -> WorldState {
        self.state
    }
}

impl Default for WorldStateBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_methods() {
        let state = WorldStateBuilder::default()
            .with_province(1, Some("SWE"))
            .with_province(2, None)
            .build();

        assert!(state.provinces.contains_key(&1));
        assert!(state.provinces.contains_key(&2));
        assert_eq!(
            state.provinces.get(&1).unwrap().owner.as_deref(),
            Some("SWE")
        );
        assert!(state.provinces.get(&2).unwrap().owner.is_none());
    }
}
