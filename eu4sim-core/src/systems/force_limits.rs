//! Force limit calculation system.
//!
//! Calculates land and naval force limits based on:
//! - Base values (6 land, 12 naval for all nations)
//! - Province development (0.1 per dev point)
//! - Trade good bonuses (grain +0.5 land, naval_supplies +0.5 naval)
//! - Building bonuses (from building definitions)
//! - Local autonomy (reduces all province contributions)
//! - Country modifiers (multiplicative on the total)

use crate::buildings::BuildingDef;
use crate::fixed::Fixed;
use crate::modifiers::{BuildingId, TradegoodId};
use crate::state::{ProvinceId, ProvinceState, Tag};
use std::collections::HashMap;

/// Base land force limit for all nations (EU4: 6)
pub const BASE_LAND_FORCE_LIMIT: i64 = 6;

/// Base naval force limit for all nations (EU4: 12)
pub const BASE_NAVAL_FORCE_LIMIT: i64 = 12;

/// Force limit contribution per point of development (EU4: 0.1)
pub const FL_PER_DEV: f32 = 0.1;

/// Land force limit bonus from grain trade good (EU4: +0.5)
pub const GRAIN_LAND_FL_BONUS: f32 = 0.5;

/// Naval force limit bonus from naval_supplies trade good (EU4: +0.5)
pub const NAVAL_SUPPLIES_NAVAL_FL_BONUS: f32 = 0.5;

/// Trade good IDs for force limit bonuses.
/// These need to match the IDs assigned during game data loading.
pub struct ForceLimitTradeGoods {
    /// Trade good ID for grain
    pub grain: TradegoodId,
    /// Trade good ID for naval_supplies
    pub naval_supplies: TradegoodId,
}

impl Default for ForceLimitTradeGoods {
    fn default() -> Self {
        // Default IDs based on typical EU4 load order
        // TODO: These should be loaded from game data
        Self {
            grain: TradegoodId(1),
            naval_supplies: TradegoodId(21),
        }
    }
}

/// Calculate land and naval force limits for a country.
///
/// Returns (land_force_limit, naval_force_limit) as Fixed values.
pub fn calculate_force_limits(
    tag: &Tag,
    provinces: &HashMap<ProvinceId, ProvinceState>,
    modifiers: &crate::modifiers::GameModifiers,
    building_defs: &HashMap<BuildingId, BuildingDef>,
    trade_goods: &ForceLimitTradeGoods,
) -> (Fixed, Fixed) {
    let mut land_province_contrib = Fixed::ZERO;
    let mut naval_province_contrib = Fixed::ZERO;

    let fl_per_dev = Fixed::from_f32(FL_PER_DEV);
    let grain_bonus = Fixed::from_f32(GRAIN_LAND_FL_BONUS);
    let naval_supplies_bonus = Fixed::from_f32(NAVAL_SUPPLIES_NAVAL_FL_BONUS);

    for (&prov_id, province) in provinces {
        // Only count provinces owned by this country
        if province.owner.as_ref() != Some(tag) {
            continue;
        }

        // Get autonomy (clamped to [0, 1])
        let raw_autonomy = modifiers
            .province_autonomy
            .get(&prov_id)
            .copied()
            .unwrap_or(Fixed::ZERO);

        // Apply coring floor (uncored provinces have minimum 75% autonomy)
        let floor = crate::systems::coring::effective_autonomy(province, tag);
        let autonomy = raw_autonomy.max(floor).clamp(Fixed::ZERO, Fixed::ONE);
        let autonomy_mult = Fixed::ONE - autonomy;

        // Development contribution
        let total_dev = province.base_tax + province.base_production + province.base_manpower;
        let dev_contrib = total_dev.mul(fl_per_dev);

        // Land FL: dev + grain bonus + building bonuses
        let mut prov_land_fl = dev_contrib;
        if province.trade_goods_id == Some(trade_goods.grain) {
            prov_land_fl += grain_bonus;
        }

        // Naval FL: dev + naval_supplies bonus + building bonuses
        let mut prov_naval_fl = dev_contrib;
        if province.trade_goods_id == Some(trade_goods.naval_supplies) {
            prov_naval_fl += naval_supplies_bonus;
        }

        // Building contributions (from building definitions)
        for building_id in province.buildings.iter() {
            if let Some(def) = building_defs.get(&building_id) {
                if let Some(land_fl) = def.land_forcelimit {
                    prov_land_fl += Fixed::from_int(land_fl as i64);
                }
                if let Some(naval_fl) = def.naval_forcelimit {
                    prov_naval_fl += Fixed::from_int(naval_fl as i64);
                }
            }
        }

        // Apply autonomy reduction to province contributions
        land_province_contrib += prov_land_fl.mul(autonomy_mult);
        naval_province_contrib += prov_naval_fl.mul(autonomy_mult);
    }

    // Base + province contributions
    let base_land = Fixed::from_int(BASE_LAND_FORCE_LIMIT) + land_province_contrib;
    let base_naval = Fixed::from_int(BASE_NAVAL_FORCE_LIMIT) + naval_province_contrib;

    // Apply country-wide force limit modifiers (multiplicative)
    let land_mod = modifiers
        .country_land_forcelimit
        .get(tag)
        .copied()
        .unwrap_or(Fixed::ZERO);
    let naval_mod = modifiers
        .country_naval_forcelimit
        .get(tag)
        .copied()
        .unwrap_or(Fixed::ZERO);

    // Final calculation: base * (1 + modifier)
    // Note: The building contributions are already in country_*_forcelimit from
    // recompute_country_modifiers_from_buildings, but those are flat values not percentages.
    // For now, we treat the modifier as a flat addition (matching how buildings work).
    // TODO: Distinguish between flat and percentage modifiers properly
    let land_fl = base_land + land_mod;
    let naval_fl = base_naval + naval_mod;

    (land_fl, naval_fl)
}

/// Simplified calculation for verification purposes.
///
/// This version takes raw data extracted from save files rather than the full
/// WorldState, making it usable from eu4sim-verify.
pub fn calculate_land_force_limit_simple(
    owned_province_ids: &[u32],
    provinces: &HashMap<u32, ProvinceVerifyInput>,
) -> f64 {
    let mut province_contrib = 0.0;

    for &prov_id in owned_province_ids {
        if let Some(prov) = provinces.get(&prov_id) {
            // Autonomy reduction
            let autonomy_mult = 1.0 - (prov.local_autonomy / 100.0).clamp(0.0, 1.0);

            // Development contribution
            let total_dev = prov.base_tax + prov.base_production + prov.base_manpower;
            let mut prov_fl = total_dev * FL_PER_DEV as f64;

            // Grain bonus
            if prov.trade_good.as_deref() == Some("grain") {
                prov_fl += GRAIN_LAND_FL_BONUS as f64;
            }

            // Building bonuses
            for building in &prov.buildings {
                prov_fl += land_building_bonus(building);
            }

            province_contrib += prov_fl * autonomy_mult;
        }
    }

    BASE_LAND_FORCE_LIMIT as f64 + province_contrib
}

/// Simplified naval force limit calculation for verification.
pub fn calculate_naval_force_limit_simple(
    owned_province_ids: &[u32],
    provinces: &HashMap<u32, ProvinceVerifyInput>,
) -> f64 {
    let mut province_contrib = 0.0;

    for &prov_id in owned_province_ids {
        if let Some(prov) = provinces.get(&prov_id) {
            // Autonomy reduction
            let autonomy_mult = 1.0 - (prov.local_autonomy / 100.0).clamp(0.0, 1.0);

            // Development contribution
            let total_dev = prov.base_tax + prov.base_production + prov.base_manpower;
            let mut prov_fl = total_dev * FL_PER_DEV as f64;

            // Naval supplies bonus
            if prov.trade_good.as_deref() == Some("naval_supplies") {
                prov_fl += NAVAL_SUPPLIES_NAVAL_FL_BONUS as f64;
            }

            // Building bonuses
            for building in &prov.buildings {
                prov_fl += naval_building_bonus(building);
            }

            province_contrib += prov_fl * autonomy_mult;
        }
    }

    BASE_NAVAL_FORCE_LIMIT as f64 + province_contrib
}

/// Input data for province verification (matches what we extract from saves).
#[derive(Debug, Clone)]
pub struct ProvinceVerifyInput {
    pub base_tax: f64,
    pub base_production: f64,
    pub base_manpower: f64,
    pub local_autonomy: f64,
    pub trade_good: Option<String>,
    pub buildings: Vec<String>,
}

/// Land force limit bonus from a building.
fn land_building_bonus(building: &str) -> f64 {
    match building {
        "regimental_camp" => 1.0,
        "conscription_center" => 3.0,
        _ => 0.0,
    }
}

/// Naval force limit bonus from a building.
fn naval_building_bonus(building: &str) -> f64 {
    match building {
        "shipyard" => 2.0,
        "grand_shipyard" => 4.0,
        "drydock" => 6.0,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_province(
        base_tax: f64,
        base_prod: f64,
        base_mp: f64,
        autonomy: f64,
        trade_good: Option<&str>,
        buildings: Vec<&str>,
    ) -> ProvinceVerifyInput {
        ProvinceVerifyInput {
            base_tax,
            base_production: base_prod,
            base_manpower: base_mp,
            local_autonomy: autonomy,
            trade_good: trade_good.map(String::from),
            buildings: buildings.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn test_land_fl_base_only() {
        // No provinces = just base
        let provinces = HashMap::new();
        let fl = calculate_land_force_limit_simple(&[], &provinces);
        assert_eq!(fl, BASE_LAND_FORCE_LIMIT as f64);
    }

    #[test]
    fn test_land_fl_with_dev() {
        // 10 tax + 10 prod + 10 mp = 30 dev -> 3.0 FL
        let mut provinces = HashMap::new();
        provinces.insert(1, make_province(10.0, 10.0, 10.0, 0.0, None, vec![]));

        let fl = calculate_land_force_limit_simple(&[1], &provinces);
        assert!((fl - 9.0).abs() < 0.001); // 6 base + 3.0 from dev
    }

    #[test]
    fn test_land_fl_with_autonomy() {
        // 30 dev at 50% autonomy -> 1.5 FL contribution
        let mut provinces = HashMap::new();
        provinces.insert(1, make_province(10.0, 10.0, 10.0, 50.0, None, vec![]));

        let fl = calculate_land_force_limit_simple(&[1], &provinces);
        assert!((fl - 7.5).abs() < 0.001); // 6 base + 1.5 from dev
    }

    #[test]
    fn test_land_fl_with_grain() {
        // 30 dev + grain at 0% autonomy
        let mut provinces = HashMap::new();
        provinces.insert(
            1,
            make_province(10.0, 10.0, 10.0, 0.0, Some("grain"), vec![]),
        );

        let fl = calculate_land_force_limit_simple(&[1], &provinces);
        assert!((fl - 9.5).abs() < 0.001); // 6 base + 3.0 dev + 0.5 grain
    }

    #[test]
    fn test_land_fl_with_buildings() {
        // 30 dev + regimental_camp + conscription_center
        let mut provinces = HashMap::new();
        provinces.insert(
            1,
            make_province(
                10.0,
                10.0,
                10.0,
                0.0,
                None,
                vec!["regimental_camp", "conscription_center"],
            ),
        );

        let fl = calculate_land_force_limit_simple(&[1], &provinces);
        assert!((fl - 13.0).abs() < 0.001); // 6 base + 3.0 dev + 1 camp + 3 center
    }

    #[test]
    fn test_naval_fl_base_only() {
        let provinces = HashMap::new();
        let fl = calculate_naval_force_limit_simple(&[], &provinces);
        assert_eq!(fl, BASE_NAVAL_FORCE_LIMIT as f64);
    }

    #[test]
    fn test_naval_fl_with_naval_supplies() {
        let mut provinces = HashMap::new();
        provinces.insert(
            1,
            make_province(10.0, 10.0, 10.0, 0.0, Some("naval_supplies"), vec![]),
        );

        let fl = calculate_naval_force_limit_simple(&[1], &provinces);
        assert!((fl - 15.5).abs() < 0.001); // 12 base + 3.0 dev + 0.5 naval supplies
    }

    #[test]
    fn test_naval_fl_with_buildings() {
        let mut provinces = HashMap::new();
        provinces.insert(
            1,
            make_province(10.0, 10.0, 10.0, 0.0, None, vec!["shipyard", "drydock"]),
        );

        let fl = calculate_naval_force_limit_simple(&[1], &provinces);
        assert!((fl - 23.0).abs() < 0.001); // 12 base + 3.0 dev + 2 shipyard + 6 drydock
    }

    #[test]
    fn test_buildings_affected_by_autonomy() {
        // Buildings are also reduced by autonomy
        let mut provinces = HashMap::new();
        provinces.insert(
            1,
            make_province(10.0, 10.0, 10.0, 50.0, None, vec!["regimental_camp"]),
        );

        let fl = calculate_land_force_limit_simple(&[1], &provinces);
        // 6 base + (3.0 dev + 1.0 camp) * 0.5 = 6 + 2 = 8
        assert!((fl - 8.0).abs() < 0.001);
    }
}
