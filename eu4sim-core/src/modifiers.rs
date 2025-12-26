//! Modifier system for dynamic game state mutations.
//!
//! Events, decisions, and other game mechanics modify these values.
//! All values use [`Fixed`] for deterministic simulation.

use crate::fixed::Fixed;
use crate::state::{ProvinceId, Tag};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type-safe trade good identifier.
///
/// Prevents mixing up trade good IDs with province IDs or other numeric types.
#[derive(Hash, Eq, PartialEq, Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct TradegoodId(pub u16);

/// Type-safe building identifier.
///
/// Sequential IDs (0..N) enable efficient bitmask storage via [`crate::buildings::BuildingSet`].
/// EU4 has ~70 buildings, so `u8` is sufficient.
#[derive(
    Hash, Eq, PartialEq, Clone, Copy, Debug, Default, Serialize, Deserialize, PartialOrd, Ord,
)]
pub struct BuildingId(pub u8);

impl BuildingId {
    /// Convert to bitmask position for [`crate::buildings::BuildingSet`].
    #[inline]
    pub fn as_mask(self) -> u128 {
        1u128 << self.0
    }
}

/// Dynamic game state modifiable by events.
///
/// Keys are typed IDs for safety; values are [`Fixed`] for determinism.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GameModifiers {
    /// Price modifiers for trade goods (from events like "Price Change: Cotton").
    /// Added to base price: effective = base + modifier.
    pub goods_price_mods: HashMap<TradegoodId, Fixed>,

    /// Province-level production efficiency bonuses.
    /// Applied as: (1 + efficiency) multiplier.
    pub province_production_efficiency: HashMap<ProvinceId, Fixed>,

    /// Province-level autonomy values.
    /// Applied as: (1 - autonomy) multiplier.
    pub province_autonomy: HashMap<ProvinceId, Fixed>,

    /// Country-level tax efficiency (national tax modifier).
    /// Applied as: (1 + modifier) multiplier.
    pub country_tax_modifier: HashMap<Tag, Fixed>,

    /// Province-level tax modifier.
    /// Applied to base tax.
    pub province_tax_modifier: HashMap<ProvinceId, Fixed>,

    /// Country-level land maintenance modifier.
    /// Applied as: (1 + modifier) multiplier for army cost.
    pub land_maintenance_modifier: HashMap<Tag, Fixed>,

    /// Country-level fort maintenance modifier.
    /// Applied as: (1 + modifier) multiplier for fort cost.
    pub fort_maintenance_modifier: HashMap<Tag, Fixed>,
}

impl GameModifiers {
    /// Get effective goods price: base + modifier.
    ///
    /// Returns the base price if no modifier exists.
    #[inline]
    pub fn effective_price(&self, id: TradegoodId, base: Fixed) -> Fixed {
        let modifier = self
            .goods_price_mods
            .get(&id)
            .copied()
            .unwrap_or(Fixed::ZERO);
        base + modifier
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tradegood_id_equality() {
        let grain = TradegoodId(0);
        let iron = TradegoodId(1);
        let grain2 = TradegoodId(0);

        assert_eq!(grain, grain2);
        assert_ne!(grain, iron);
    }

    #[test]
    fn test_effective_price_no_modifier() {
        let mods = GameModifiers::default();
        let base = Fixed::from_f32(2.5);
        let grain = TradegoodId(0);

        assert_eq!(mods.effective_price(grain, base), base);
    }

    #[test]
    fn test_effective_price_with_modifier() {
        let mut mods = GameModifiers::default();
        let grain = TradegoodId(0);
        let base = Fixed::from_f32(2.5);
        let bonus = Fixed::from_f32(0.5); // +0.5 price bonus

        mods.goods_price_mods.insert(grain, bonus);

        let expected = Fixed::from_f32(3.0);
        assert_eq!(mods.effective_price(grain, base), expected);
    }

    #[test]
    fn test_game_modifiers_default() {
        let mods = GameModifiers::default();
        assert!(mods.goods_price_mods.is_empty());
        assert!(mods.province_production_efficiency.is_empty());
        assert!(mods.province_autonomy.is_empty());
    }

    #[test]
    fn test_building_id_equality() {
        let temple = BuildingId(0);
        let workshop = BuildingId(1);
        let temple2 = BuildingId(0);

        assert_eq!(temple, temple2);
        assert_ne!(temple, workshop);
    }

    #[test]
    fn test_building_id_as_mask() {
        assert_eq!(BuildingId(0).as_mask(), 1);
        assert_eq!(BuildingId(1).as_mask(), 2);
        assert_eq!(BuildingId(7).as_mask(), 128);
        assert_eq!(BuildingId(63).as_mask(), 1u128 << 63);
    }
}
