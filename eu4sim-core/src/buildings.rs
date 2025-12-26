//! Building definitions and bitmask storage.
//!
//! Buildings are province improvements that provide economic and military bonuses.
//! Each province can have at most one of each building type, stored efficiently
//! as a bitmask via [`BuildingSet`].

use crate::fixed::Fixed;
use crate::modifiers::{BuildingId, TradegoodId};
use crate::state::Date;
use serde::{Deserialize, Serialize};

/// Bitmask storage for buildings in a province.
///
/// Zero-allocation, O(1) operations. Supports up to 128 building types.
/// EU4 has ~70 buildings, so u128 provides comfortable headroom.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildingSet(pub u128);

impl BuildingSet {
    /// Check if a building is present.
    #[inline]
    pub fn contains(&self, id: BuildingId) -> bool {
        self.0 & id.as_mask() != 0
    }

    /// Add a building to the set.
    #[inline]
    pub fn insert(&mut self, id: BuildingId) {
        self.0 |= id.as_mask();
    }

    /// Remove a building from the set.
    #[inline]
    pub fn remove(&mut self, id: BuildingId) {
        self.0 &= !id.as_mask();
    }

    /// Count of buildings in the set.
    #[inline]
    pub fn count(&self) -> u32 {
        self.0.count_ones()
    }

    /// Check if the set is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Iterate over building IDs in the set.
    pub fn iter(&self) -> impl Iterator<Item = BuildingId> + '_ {
        (0..128u8)
            .filter(move |&i| self.0 & (1u128 << i) != 0)
            .map(BuildingId)
    }
}

/// Static building definition loaded from game files.
///
/// These are immutable after loading and shared across all provinces.
#[derive(Debug, Clone)]
pub struct BuildingDef {
    pub id: BuildingId,
    pub name: String,

    // Construction
    /// Gold cost to build.
    pub cost: Fixed,
    /// Months to construct (default 12).
    pub time: u8,

    // Tech requirements (Gemini: Critical!)
    pub adm_tech: Option<u8>,
    pub dip_tech: Option<u8>,
    pub mil_tech: Option<u8>,

    // Requirements
    /// Requires a coastal province with port.
    pub requires_port: bool,
    /// If Some, this is a manufactory eligible only for these trade goods.
    pub manufactory_goods: Option<Vec<TradegoodId>>,

    // Upgrade chain (Gemini: Building upgrades)
    /// Building this one replaces (e.g., Cathedral replaces Temple).
    pub replaces_building: Option<BuildingId>,

    // Effects - Province-level modifiers
    pub local_tax_modifier: Option<Fixed>,
    pub local_production_efficiency: Option<Fixed>,
    pub local_trade_power: Option<Fixed>,
    pub local_manpower_modifier: Option<Fixed>,
    pub local_sailors_modifier: Option<Fixed>,
    pub local_defensiveness: Option<Fixed>,
    pub local_ship_repair: Option<Fixed>,
    pub local_ship_cost: Option<Fixed>,

    // Effects - Country-level modifiers (aggregated from all provinces)
    pub land_forcelimit: Option<u8>,
    pub naval_forcelimit: Option<u8>,
    pub ship_recruit_speed: Option<Fixed>,

    // Effects - Special
    pub fort_level: Option<u8>,
    /// Manufactories add +1 trade goods produced.
    pub trade_goods_size: Option<Fixed>,
}

/// Progress of an in-construction building.
///
/// Only one building can be under construction per province at a time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BuildingConstruction {
    pub building_id: BuildingId,
    pub start_date: Date,
    /// Months of construction completed.
    pub progress: u8,
    /// Total months required.
    pub required: u8,
    /// Gold paid (for refund on manual cancel).
    pub cost_paid: Fixed,
}

/// Source of building slots in a province.
///
/// Currently only base development provides slots.
/// Infrastructure expansion is stubbed for future implementation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BuildingSlotSource {
    #[default]
    BaseDevelopment,
    // Future: InfrastructureExpansion,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_building_set_empty() {
        let set = BuildingSet::default();
        assert!(set.is_empty());
        assert_eq!(set.count(), 0);
        assert!(!set.contains(BuildingId(0)));
    }

    #[test]
    fn test_building_set_insert_remove() {
        let mut set = BuildingSet::default();
        let temple = BuildingId(0);
        let workshop = BuildingId(1);

        set.insert(temple);
        assert!(set.contains(temple));
        assert!(!set.contains(workshop));
        assert_eq!(set.count(), 1);

        set.insert(workshop);
        assert!(set.contains(temple));
        assert!(set.contains(workshop));
        assert_eq!(set.count(), 2);

        set.remove(temple);
        assert!(!set.contains(temple));
        assert!(set.contains(workshop));
        assert_eq!(set.count(), 1);
    }

    #[test]
    fn test_building_set_iter() {
        let mut set = BuildingSet::default();
        set.insert(BuildingId(0));
        set.insert(BuildingId(5));
        set.insert(BuildingId(10));

        let ids: Vec<_> = set.iter().collect();
        assert_eq!(ids, vec![BuildingId(0), BuildingId(5), BuildingId(10)]);
    }

    #[test]
    fn test_building_set_high_ids() {
        let mut set = BuildingSet::default();
        set.insert(BuildingId(127)); // Max supported
        assert!(set.contains(BuildingId(127)));
        assert_eq!(set.count(), 1);
    }
}
