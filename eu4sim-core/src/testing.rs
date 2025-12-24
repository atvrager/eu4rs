use crate::fixed::Fixed;
use crate::modifiers::{GameModifiers, TradegoodId};
use crate::state::{CountryState, Date, HashMap, ProvinceId, ProvinceState, Terrain, WorldState};
use crate::trade::{ProvinceTradeState, TradeTopology};

pub struct WorldStateBuilder {
    state: WorldState,
}

impl WorldStateBuilder {
    #[allow(clippy::should_implement_trait)]
    pub fn new() -> Self {
        Self {
            state: WorldState {
                date: Date::new(1444, 11, 11),
                rng_seed: 0,
                rng_state: 0,
                provinces: HashMap::default(),
                countries: HashMap::default(),
                base_goods_prices: HashMap::default(),
                modifiers: GameModifiers::default(),
                diplomacy: Default::default(),
                global: Default::default(),
                armies: HashMap::default(),
                next_army_id: 1,
                fleets: HashMap::default(),
                next_fleet_id: 1,
                colonies: HashMap::default(),
                // Combat system
                generals: HashMap::default(),
                next_general_id: 1,
                admirals: HashMap::default(),
                next_admiral_id: 1,
                battles: HashMap::default(),
                next_battle_id: 1,
                naval_battles: HashMap::default(),
                next_naval_battle_id: 1,
                sieges: HashMap::default(),
                next_siege_id: 1,
                // Trade system
                trade_nodes: HashMap::default(),
                province_trade_node: HashMap::default(),
                trade_topology: TradeTopology::default(),
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
                treasury: Fixed::from_int(100), // Default generous treasury
                manpower: Fixed::from_int(50000),
                ..Default::default()
            },
        );
        self
    }

    pub fn with_province_state(mut self, id: ProvinceId, province: ProvinceState) -> Self {
        self.state.provinces.insert(id, province);
        self
    }

    pub fn with_province(mut self, id: ProvinceId, owner_tag: Option<&str>) -> Self {
        let mut cores = std::collections::HashSet::new();
        if let Some(tag) = owner_tag {
            cores.insert(tag.to_string());
        }
        self.state.provinces.insert(
            id,
            ProvinceState {
                owner: owner_tag.map(|s| s.to_string()),
                controller: owner_tag.map(|s| s.to_string()),
                religion: None,
                culture: None,
                trade_goods_id: None,
                base_production: Fixed::ONE,
                base_tax: Fixed::ONE,
                base_manpower: Fixed::ONE,
                fort_level: 0,
                is_capital: false,
                is_mothballed: false,
                is_sea: false,
                is_wasteland: false,
                terrain: None,
                institution_presence: HashMap::default(),
                trade: ProvinceTradeState::default(),
                cores,
                coring_progress: None,
            },
        );
        self
    }

    pub fn with_terrain(mut self, id: ProvinceId, terrain: Terrain) -> Self {
        if let Some(p) = self.state.provinces.get_mut(&id) {
            p.terrain = Some(terrain);
        }
        self
    }

    /// Add a province with trade goods and production value.
    pub fn with_province_full(
        mut self,
        id: ProvinceId,
        owner_tag: Option<&str>,
        trade_goods_id: Option<TradegoodId>,
        base_production: Fixed,
    ) -> Self {
        let mut cores = std::collections::HashSet::new();
        if let Some(tag) = owner_tag {
            cores.insert(tag.to_string());
        }
        self.state.provinces.insert(
            id,
            ProvinceState {
                owner: owner_tag.map(|s| s.to_string()),
                controller: owner_tag.map(|s| s.to_string()),
                religion: None,
                culture: None,
                trade_goods_id,
                base_production,
                base_tax: Fixed::ONE,
                base_manpower: Fixed::ONE,
                fort_level: 0,
                is_capital: false,
                is_mothballed: false,
                is_sea: false,
                is_wasteland: false,
                terrain: None,
                institution_presence: HashMap::default(),
                trade: ProvinceTradeState::default(),
                cores,
                coring_progress: None,
            },
        );
        self
    }

    /// Add a base goods price.
    pub fn with_goods_price(mut self, goods_id: TradegoodId, price: Fixed) -> Self {
        self.state.base_goods_prices.insert(goods_id, price);
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

/// Create a test army with correct cached regiment counts.
/// Use this instead of manual Army struct construction in tests.
pub fn make_test_army(
    id: crate::state::ArmyId,
    owner: &str,
    location: ProvinceId,
    regiments: Vec<crate::state::Regiment>,
) -> crate::state::Army {
    use crate::state::RegimentType;
    let (inf, cav, art) = regiments
        .iter()
        .fold((0, 0, 0), |(i, c, a), r| match r.type_ {
            RegimentType::Infantry => (i + 1, c, a),
            RegimentType::Cavalry => (i, c + 1, a),
            RegimentType::Artillery => (i, c, a + 1),
        });
    crate::state::Army {
        id,
        name: format!("{} Army {}", owner, id),
        owner: owner.to_string(),
        location,
        previous_location: None,
        regiments,
        movement: None,
        embarked_on: None,
        general: None,
        in_battle: None,
        infantry_count: inf,
        cavalry_count: cav,
        artillery_count: art,
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

    #[test]
    fn test_with_province_full() {
        let grain = TradegoodId(0);
        let state = WorldStateBuilder::default()
            .with_province_full(1, Some("SWE"), Some(grain), Fixed::from_int(5))
            .with_goods_price(grain, Fixed::from_f32(2.5))
            .build();

        assert_eq!(state.provinces[&1].base_production, Fixed::from_int(5));
        assert_eq!(state.provinces[&1].trade_goods_id, Some(grain));
        assert_eq!(state.base_goods_prices[&grain], Fixed::from_f32(2.5));
    }
}
