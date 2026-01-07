//! Building construction and management system.
//!
//! Handles construction eligibility, slot calculation, and progress ticking.

use crate::buildings::{BuildingConstruction, BuildingDef};
use crate::fixed::Fixed;
use crate::fixed_generic::Mod32;
use crate::modifiers::{BuildingId, GameModifiers, TradegoodId};
use crate::state::{CountryState, HashMap, ProvinceId, ProvinceState, Tag, Terrain, WorldState};
use tracing::instrument;

/// Error returned when a building action cannot be performed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingError {
    /// Building already exists in this province.
    AlreadyBuilt,
    /// Province already has an upgraded version of this building.
    HasUpgradedVersion,
    /// Country lacks required administrative technology.
    InsufficientAdmTech { required: u8, have: u8 },
    /// Country lacks required diplomatic technology.
    InsufficientDipTech { required: u8, have: u8 },
    /// Country lacks required military technology.
    InsufficientMilTech { required: u8, have: u8 },
    /// Building requires a port but province has none.
    RequiresPort,
    /// Manufactory not eligible for this trade good.
    TradeGoodNotEligible,
    /// Province already has a manufactory.
    AlreadyHasManufactory,
    /// Province has no available building slots.
    NoAvailableSlots,
    /// Province already has a building under construction.
    AlreadyConstructing,
    /// Not enough gold in treasury.
    InsufficientGold { required: Fixed, have: Fixed },
    /// Province not owned by this country.
    NotOwned,
    /// Province not found.
    ProvinceNotFound,
    /// Building definition not found.
    BuildingNotFound,
    /// Country not found.
    CountryNotFound,
    /// No construction in progress.
    NoConstructionInProgress,
    /// Cannot demolish this building.
    CannotDemolish,
    /// Building not present.
    BuildingNotPresent,
}

impl std::fmt::Display for BuildingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyBuilt => write!(f, "Building already exists"),
            Self::HasUpgradedVersion => write!(f, "Province has upgraded version"),
            Self::InsufficientAdmTech { required, have } => {
                write!(f, "Requires ADM tech {} (have {})", required, have)
            }
            Self::InsufficientDipTech { required, have } => {
                write!(f, "Requires DIP tech {} (have {})", required, have)
            }
            Self::InsufficientMilTech { required, have } => {
                write!(f, "Requires MIL tech {} (have {})", required, have)
            }
            Self::RequiresPort => write!(f, "Requires port"),
            Self::TradeGoodNotEligible => write!(f, "Trade good not eligible for manufactory"),
            Self::AlreadyHasManufactory => write!(f, "Province already has a manufactory"),
            Self::NoAvailableSlots => write!(f, "No available building slots"),
            Self::AlreadyConstructing => write!(f, "Already constructing"),
            Self::InsufficientGold { required, have } => {
                write!(f, "Requires {} gold (have {})", required, have)
            }
            Self::NotOwned => write!(f, "Province not owned"),
            Self::ProvinceNotFound => write!(f, "Province not found"),
            Self::BuildingNotFound => write!(f, "Building not found"),
            Self::CountryNotFound => write!(f, "Country not found"),
            Self::NoConstructionInProgress => write!(f, "No construction in progress"),
            Self::CannotDemolish => write!(f, "Cannot demolish this building"),
            Self::BuildingNotPresent => write!(f, "Building not present"),
        }
    }
}

impl std::error::Error for BuildingError {}

/// Calculate maximum building slots from development and terrain.
///
/// Formula: base 2 + floor(dev/10) + terrain modifier, capped at 12.
pub fn max_building_slots(province: &ProvinceState, terrain: Option<Terrain>) -> u8 {
    let dev = (province.base_tax + province.base_production + province.base_manpower).to_int();

    // Base 2 + floor(dev/10)
    let base = 2 + (dev / 10);

    // Terrain modifier
    let terrain_mod: i32 = match terrain {
        Some(Terrain::Mountains) => -1,
        Some(Terrain::Farmlands) => 1,
        // Future: Some(Terrain::Grasslands) => 1, (if added)
        _ => 0,
    };

    ((base + terrain_mod).max(1) as u8).min(12)
}

/// Check if a province has any manufactory.
pub fn has_manufactory(
    province: &ProvinceState,
    building_defs: &HashMap<BuildingId, BuildingDef>,
) -> bool {
    for building_id in province.buildings.iter() {
        if let Some(def) = building_defs.get(&building_id) {
            if def.manufactory_goods.is_some() {
                return true;
            }
        }
    }
    false
}

/// Check if a building can be built in a province.
pub fn can_build(
    province: &ProvinceState,
    building: &BuildingDef,
    country: &CountryState,
    building_defs: &HashMap<BuildingId, BuildingDef>,
    upgraded_by: &HashMap<BuildingId, BuildingId>,
) -> Result<(), BuildingError> {
    // 1. Already has this building
    if province.buildings.contains(building.id) {
        return Err(BuildingError::AlreadyBuilt);
    }

    // 2. Has a better version (one-way upgrades)
    if let Some(&upgrader) = upgraded_by.get(&building.id) {
        if province.buildings.contains(upgrader) {
            return Err(BuildingError::HasUpgradedVersion);
        }
    }

    // 3. Tech requirements
    if let Some(req) = building.adm_tech {
        if country.adm_tech < req {
            return Err(BuildingError::InsufficientAdmTech {
                required: req,
                have: country.adm_tech,
            });
        }
    }
    if let Some(req) = building.dip_tech {
        if country.dip_tech < req {
            return Err(BuildingError::InsufficientDipTech {
                required: req,
                have: country.dip_tech,
            });
        }
    }
    if let Some(req) = building.mil_tech {
        if country.mil_tech < req {
            return Err(BuildingError::InsufficientMilTech {
                required: req,
                have: country.mil_tech,
            });
        }
    }

    // 4. Port requirement
    if building.requires_port && !province.has_port {
        return Err(BuildingError::RequiresPort);
    }

    // 5. Manufactory eligibility (trade good)
    if let Some(ref goods) = building.manufactory_goods {
        let province_goods = province.trade_goods_id.unwrap_or(TradegoodId(0));
        if !goods.contains(&province_goods) {
            return Err(BuildingError::TradeGoodNotEligible);
        }
        // Only one manufactory per province
        if has_manufactory(province, building_defs) {
            return Err(BuildingError::AlreadyHasManufactory);
        }
    }

    // 6. Slot limit
    let used_slots = province.buildings.count() as u8;
    let max_slots = max_building_slots(province, province.terrain);
    if used_slots >= max_slots {
        return Err(BuildingError::NoAvailableSlots);
    }

    // 7. Active construction
    if province.building_construction.is_some() {
        return Err(BuildingError::AlreadyConstructing);
    }

    // 8. Cost
    if country.treasury < building.cost {
        return Err(BuildingError::InsufficientGold {
            required: building.cost,
            have: country.treasury,
        });
    }

    Ok(())
}

/// Start construction of a building in a province.
///
/// Deducts gold from treasury and begins construction.
pub fn start_construction(
    state: &mut WorldState,
    province_id: ProvinceId,
    building_name: &str,
    country_tag: &str,
) -> Result<(), BuildingError> {
    // Look up building ID
    let building_id = state
        .building_name_to_id
        .get(building_name)
        .copied()
        .ok_or(BuildingError::BuildingNotFound)?;

    let building_def = state
        .building_defs
        .get(&building_id)
        .ok_or(BuildingError::BuildingNotFound)?;

    let province = state
        .provinces
        .get(&province_id)
        .ok_or(BuildingError::ProvinceNotFound)?;

    // Verify ownership
    if province.owner.as_deref() != Some(country_tag) {
        return Err(BuildingError::NotOwned);
    }

    let country = state
        .countries
        .get(country_tag)
        .ok_or(BuildingError::CountryNotFound)?;

    // Validate eligibility
    can_build(
        province,
        building_def,
        country,
        &state.building_defs,
        &state.building_upgraded_by,
    )?;

    // Capture values we need before mutable borrows
    let cost = building_def.cost;
    let time = building_def.time;
    let current_date = state.date;

    // Deduct gold
    let country = state
        .countries
        .get_mut(country_tag)
        .ok_or(BuildingError::CountryNotFound)?;
    country.treasury -= cost;

    // Start construction
    let province = state
        .provinces
        .get_mut(&province_id)
        .ok_or(BuildingError::ProvinceNotFound)?;

    province.building_construction = Some(BuildingConstruction {
        building_id,
        start_date: current_date,
        progress: 0,
        required: time,
        cost_paid: cost,
    });

    log::info!(
        "{} started building {} in province {} (cost: {}, time: {} months)",
        country_tag,
        building_name,
        province_id,
        cost,
        time
    );

    Ok(())
}

/// Cancel construction manually (100% refund).
pub fn cancel_construction_manual(
    state: &mut WorldState,
    province_id: ProvinceId,
    country_tag: &str,
) -> Result<Fixed, BuildingError> {
    let province = state
        .provinces
        .get(&province_id)
        .ok_or(BuildingError::ProvinceNotFound)?;

    // Verify ownership
    if province.owner.as_deref() != Some(country_tag) {
        return Err(BuildingError::NotOwned);
    }

    let construction = province
        .building_construction
        .as_ref()
        .ok_or(BuildingError::NoConstructionInProgress)?;

    let refund = construction.cost_paid;

    // Apply refund
    let country = state
        .countries
        .get_mut(country_tag)
        .ok_or(BuildingError::CountryNotFound)?;
    country.treasury += refund;

    // Cancel construction
    let province = state
        .provinces
        .get_mut(&province_id)
        .ok_or(BuildingError::ProvinceNotFound)?;
    province.building_construction = None;

    log::info!(
        "{} cancelled construction in province {} (refund: {})",
        country_tag,
        province_id,
        refund
    );

    Ok(refund)
}

/// Cancel construction on conquest (no refund).
pub fn cancel_construction_conquest(province: &mut ProvinceState) {
    if province.building_construction.take().is_some() {
        log::debug!("Construction cancelled due to conquest");
    }
}

/// Transfer construction on diplomatic annexation.
///
/// Inherits construction if new owner has required tech, otherwise cancels.
pub fn transfer_construction_diplomatic(
    province: &mut ProvinceState,
    new_owner: &CountryState,
    building_defs: &HashMap<BuildingId, BuildingDef>,
) {
    if let Some(ref construction) = province.building_construction {
        if let Some(def) = building_defs.get(&construction.building_id) {
            let valid = def.adm_tech.is_none_or(|t| new_owner.adm_tech >= t)
                && def.dip_tech.is_none_or(|t| new_owner.dip_tech >= t)
                && def.mil_tech.is_none_or(|t| new_owner.mil_tech >= t);

            if !valid {
                log::info!(
                    "Construction of {} cancelled on annexation (new owner lacks tech)",
                    def.name
                );
                province.building_construction = None;
            } else {
                log::debug!("Construction of {} inherited by new owner", def.name);
            }
        }
    }
}

/// Demolish a building (no refund).
pub fn demolish_building(
    state: &mut WorldState,
    province_id: ProvinceId,
    building_name: &str,
    country_tag: &str,
) -> Result<(), BuildingError> {
    let building_id = state
        .building_name_to_id
        .get(building_name)
        .copied()
        .ok_or(BuildingError::BuildingNotFound)?;

    let province = state
        .provinces
        .get(&province_id)
        .ok_or(BuildingError::ProvinceNotFound)?;

    // Verify ownership
    if province.owner.as_deref() != Some(country_tag) {
        return Err(BuildingError::NotOwned);
    }

    // Check building exists
    if !province.buildings.contains(building_id) {
        return Err(BuildingError::BuildingNotPresent);
    }

    // Remove building
    let province = state
        .provinces
        .get_mut(&province_id)
        .ok_or(BuildingError::ProvinceNotFound)?;
    province.buildings.remove(building_id);

    log::info!(
        "{} demolished {} in province {}",
        country_tag,
        building_name,
        province_id
    );

    Ok(())
}

/// Called when province trade good changes - invalidates incompatible manufactories.
pub fn validate_manufactory_on_goods_change(
    province: &mut ProvinceState,
    new_goods: TradegoodId,
    building_defs: &HashMap<BuildingId, BuildingDef>,
) {
    let mut to_remove = None;

    for building_id in province.buildings.iter() {
        if let Some(def) = building_defs.get(&building_id) {
            if let Some(ref valid_goods) = def.manufactory_goods {
                if !valid_goods.contains(&new_goods) {
                    to_remove = Some((building_id, def.name.clone()));
                    break; // Only one manufactory possible
                }
            }
        }
    }

    if let Some((building_id, name)) = to_remove {
        province.buildings.remove(building_id);
        log::info!("Manufactory {} destroyed: trade good changed", name);
    }
}

/// Tick construction progress for all provinces.
///
/// Called each month. Completes buildings when progress reaches required time.
#[instrument(skip_all, name = "buildings")]
pub fn tick_building_construction(state: &mut WorldState) {
    let province_ids: Vec<_> = state.provinces.keys().copied().collect();

    // Collect completed buildings first to avoid borrow issues
    let mut completed: Vec<(ProvinceId, BuildingId)> = Vec::new();
    let mut had_completion = false;

    for province_id in province_ids {
        let Some(province) = state.provinces.get_mut(&province_id) else {
            continue;
        };

        let Some(construction) = province.building_construction.as_mut() else {
            continue;
        };

        construction.progress += 1;

        if construction.progress >= construction.required {
            completed.push((province_id, construction.building_id));
            had_completion = true;
        }
    }

    // Now complete the buildings
    for (province_id, building_id) in completed {
        // Handle upgrade chain - remove replaced building
        if let Some(def) = state.building_defs.get(&building_id) {
            if let Some(replaces) = def.replaces_building {
                if let Some(province) = state.provinces.get_mut(&province_id) {
                    province.buildings.remove(replaces);
                    log::debug!(
                        "Replaced {} with {} in province {}",
                        replaces.0,
                        building_id.0,
                        province_id
                    );
                }
            }
        }

        if let Some(province) = state.provinces.get_mut(&province_id) {
            province.buildings.insert(building_id);
            province.building_construction = None;
        }

        if let Some(def) = state.building_defs.get(&building_id) {
            log::info!(
                "Building {} completed in province {}",
                def.name,
                province_id
            );
        }

        // Recompute modifiers for this province
        if let Some(province) = state.provinces.get(&province_id) {
            let province_clone = province.clone();
            recompute_province_modifiers(
                province_id,
                &province_clone,
                &state.building_defs,
                &mut state.modifiers,
            );
        }
    }

    // Recompute country-level modifiers from all buildings if any completed
    if had_completion {
        recompute_country_modifiers_from_buildings(state);
    }
}

/// Recompute modifiers for a single province based on its buildings.
pub fn recompute_province_modifiers(
    province_id: ProvinceId,
    province: &ProvinceState,
    building_defs: &HashMap<BuildingId, BuildingDef>,
    modifiers: &mut GameModifiers,
) {
    // Accumulate all province-level modifiers from buildings
    let mut tax_mod = Fixed::ZERO;
    let mut prod_eff = Fixed::ZERO;
    let mut trade_power = Fixed::ZERO;
    let mut manpower_mod = Fixed::ZERO;
    let mut sailors_mod = Fixed::ZERO;
    let mut defensiveness = Fixed::ZERO;
    let mut ship_repair = Fixed::ZERO;
    let mut ship_cost = Fixed::ZERO;
    let mut trade_goods = Fixed::ZERO;

    for building_id in province.buildings.iter() {
        if let Some(def) = building_defs.get(&building_id) {
            if let Some(v) = def.local_tax_modifier {
                tax_mod += v;
            }
            if let Some(v) = def.local_production_efficiency {
                prod_eff += v;
            }
            if let Some(v) = def.local_trade_power {
                trade_power += v;
            }
            if let Some(v) = def.local_manpower_modifier {
                manpower_mod += v;
            }
            if let Some(v) = def.local_sailors_modifier {
                sailors_mod += v;
            }
            if let Some(v) = def.local_defensiveness {
                defensiveness += v;
            }
            if let Some(v) = def.local_ship_repair {
                ship_repair += v;
            }
            if let Some(v) = def.local_ship_cost {
                ship_cost += v;
            }
            if let Some(v) = def.trade_goods_size {
                trade_goods += v;
            }
        }
    }

    // Update or remove province modifiers
    // Pattern: insert if non-zero, remove if zero
    // Convert Fixed -> Mod32 for GameModifiers
    if tax_mod != Fixed::ZERO {
        modifiers
            .province_tax_modifier
            .insert(province_id, Mod32::from_fixed(tax_mod));
    } else {
        modifiers.province_tax_modifier.remove(&province_id);
    }

    if prod_eff != Fixed::ZERO {
        modifiers
            .province_production_efficiency
            .insert(province_id, Mod32::from_fixed(prod_eff));
    } else {
        modifiers
            .province_production_efficiency
            .remove(&province_id);
    }

    if trade_power != Fixed::ZERO {
        modifiers
            .province_trade_power
            .insert(province_id, Mod32::from_fixed(trade_power));
    } else {
        modifiers.province_trade_power.remove(&province_id);
    }

    if manpower_mod != Fixed::ZERO {
        modifiers
            .province_manpower_modifier
            .insert(province_id, Mod32::from_fixed(manpower_mod));
    } else {
        modifiers.province_manpower_modifier.remove(&province_id);
    }

    if sailors_mod != Fixed::ZERO {
        modifiers
            .province_sailors_modifier
            .insert(province_id, Mod32::from_fixed(sailors_mod));
    } else {
        modifiers.province_sailors_modifier.remove(&province_id);
    }

    if defensiveness != Fixed::ZERO {
        modifiers
            .province_defensiveness
            .insert(province_id, Mod32::from_fixed(defensiveness));
    } else {
        modifiers.province_defensiveness.remove(&province_id);
    }

    if ship_repair != Fixed::ZERO {
        modifiers
            .province_ship_repair
            .insert(province_id, Mod32::from_fixed(ship_repair));
    } else {
        modifiers.province_ship_repair.remove(&province_id);
    }

    if ship_cost != Fixed::ZERO {
        modifiers
            .province_ship_cost
            .insert(province_id, Mod32::from_fixed(ship_cost));
    } else {
        modifiers.province_ship_cost.remove(&province_id);
    }

    if trade_goods != Fixed::ZERO {
        modifiers
            .province_trade_goods_size
            .insert(province_id, Mod32::from_fixed(trade_goods));
    } else {
        modifiers.province_trade_goods_size.remove(&province_id);
    }
}

/// Recompute country-level modifiers from all buildings.
///
/// Aggregates modifiers like `land_forcelimit` and `naval_forcelimit` from all provinces.
/// Should be called when buildings change or provinces change ownership.
pub fn recompute_country_modifiers_from_buildings(state: &mut WorldState) {
    // Clear existing building-sourced country modifiers
    // (We'll rebuild them from scratch)

    // Aggregate per country
    let mut land_fl: HashMap<Tag, i32> = HashMap::new();
    let mut naval_fl: HashMap<Tag, i32> = HashMap::new();
    let mut ship_speed: HashMap<Tag, Fixed> = HashMap::new();

    for province in state.provinces.values() {
        let Some(owner) = &province.owner else {
            continue;
        };

        for building_id in province.buildings.iter() {
            if let Some(def) = state.building_defs.get(&building_id) {
                if let Some(v) = def.land_forcelimit {
                    *land_fl.entry(owner.clone()).or_insert(0) += v as i32;
                }
                if let Some(v) = def.naval_forcelimit {
                    *naval_fl.entry(owner.clone()).or_insert(0) += v as i32;
                }
                if let Some(v) = def.ship_recruit_speed {
                    *ship_speed.entry(owner.clone()).or_insert(Fixed::ZERO) += v;
                }
            }
        }
    }

    // Update country modifiers (convert to Mod32 for GameModifiers)
    for (tag, value) in land_fl {
        if value != 0 {
            state
                .modifiers
                .country_land_forcelimit
                .insert(tag, Mod32::from_int(value));
        }
    }

    for (tag, value) in naval_fl {
        if value != 0 {
            state
                .modifiers
                .country_naval_forcelimit
                .insert(tag, Mod32::from_int(value));
        }
    }

    for (tag, value) in ship_speed {
        if value != Fixed::ZERO {
            state
                .modifiers
                .country_global_ship_recruit_speed
                .insert(tag, Mod32::from_fixed(value));
        }
    }
}

/// Recompute fort level from buildings (cached property).
pub fn recompute_fort_level(
    province: &ProvinceState,
    building_defs: &HashMap<BuildingId, BuildingDef>,
    is_capital: bool,
) -> u8 {
    let capital_bonus = if is_capital { 1 } else { 0 };

    let building_level = province
        .buildings
        .iter()
        .filter_map(|id| building_defs.get(&id))
        .filter_map(|def| def.fort_level)
        .max()
        .unwrap_or(0);

    capital_bonus + building_level
}

/// Get list of buildings that can be built in a province.
pub fn available_buildings(
    province: &ProvinceState,
    country: &CountryState,
    building_defs: &HashMap<BuildingId, BuildingDef>,
    upgraded_by: &HashMap<BuildingId, BuildingId>,
) -> Vec<BuildingId> {
    building_defs
        .keys()
        .copied()
        .filter(|&id| {
            if let Some(def) = building_defs.get(&id) {
                can_build(province, def, country, building_defs, upgraded_by).is_ok()
            } else {
                false
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buildings::BuildingSet;

    fn make_province() -> ProvinceState {
        ProvinceState {
            owner: Some("TST".to_string()),
            controller: Some("TST".to_string()),
            religion: None,
            culture: None,
            trade_goods_id: Some(TradegoodId(1)), // Grain
            base_production: Mod32::from_int(5),
            base_tax: Mod32::from_int(5),
            base_manpower: Mod32::from_int(5),
            fort_level: 0,
            is_capital: false,
            is_mothballed: false,
            is_sea: false,
            is_wasteland: false,
            is_in_hre: false,
            terrain: Some(Terrain::Plains),
            institution_presence: HashMap::default(),
            trade: Default::default(),
            cores: Default::default(),
            coring_progress: None,
            buildings: BuildingSet::default(),
            building_construction: None,
            has_port: true,
            devastation: Mod32::ZERO,
        }
    }

    fn make_country() -> CountryState {
        CountryState {
            treasury: Fixed::from_int(500),
            adm_tech: 10,
            dip_tech: 10,
            mil_tech: 10,
            ..Default::default()
        }
    }

    fn make_building_def(id: u8, name: &str) -> BuildingDef {
        BuildingDef {
            id: BuildingId(id),
            name: name.to_string(),
            cost: Fixed::from_int(100),
            time: 12,
            adm_tech: None,
            dip_tech: None,
            mil_tech: None,
            requires_port: false,
            manufactory_goods: None,
            replaces_building: None,
            local_tax_modifier: None,
            local_production_efficiency: None,
            local_trade_power: None,
            local_manpower_modifier: None,
            local_sailors_modifier: None,
            local_defensiveness: None,
            local_ship_repair: None,
            local_ship_cost: None,
            land_forcelimit: None,
            naval_forcelimit: None,
            ship_recruit_speed: None,
            fort_level: None,
            trade_goods_size: None,
        }
    }

    #[test]
    fn test_max_building_slots_base() {
        let mut province = make_province();
        // dev = 15, slots = 2 + 1 = 3
        assert_eq!(max_building_slots(&province, Some(Terrain::Plains)), 3);

        // Higher dev
        province.base_tax = Mod32::from_int(10);
        province.base_production = Mod32::from_int(10);
        province.base_manpower = Mod32::from_int(10);
        // dev = 30, slots = 2 + 3 = 5
        assert_eq!(max_building_slots(&province, Some(Terrain::Plains)), 5);
    }

    #[test]
    fn test_max_building_slots_terrain() {
        let province = make_province();

        // Mountains: -1 slot
        assert_eq!(max_building_slots(&province, Some(Terrain::Mountains)), 2);

        // Farmlands: +1 slot
        assert_eq!(max_building_slots(&province, Some(Terrain::Farmlands)), 4);
    }

    #[test]
    fn test_can_build_success() {
        let province = make_province();
        let country = make_country();
        let building = make_building_def(0, "temple");
        let defs: HashMap<_, _> = [(BuildingId(0), building.clone())].into_iter().collect();
        let upgraded_by = HashMap::new();

        assert!(can_build(&province, &building, &country, &defs, &upgraded_by).is_ok());
    }

    #[test]
    fn test_can_build_already_built() {
        let mut province = make_province();
        province.buildings.insert(BuildingId(0));

        let country = make_country();
        let building = make_building_def(0, "temple");
        let defs: HashMap<_, _> = [(BuildingId(0), building.clone())].into_iter().collect();
        let upgraded_by = HashMap::new();

        assert_eq!(
            can_build(&province, &building, &country, &defs, &upgraded_by),
            Err(BuildingError::AlreadyBuilt)
        );
    }

    #[test]
    fn test_can_build_insufficient_tech() {
        let province = make_province();
        let mut country = make_country();
        country.adm_tech = 5;

        let mut building = make_building_def(0, "temple");
        building.adm_tech = Some(10);

        let defs: HashMap<_, _> = [(BuildingId(0), building.clone())].into_iter().collect();
        let upgraded_by = HashMap::new();

        assert_eq!(
            can_build(&province, &building, &country, &defs, &upgraded_by),
            Err(BuildingError::InsufficientAdmTech {
                required: 10,
                have: 5
            })
        );
    }

    #[test]
    fn test_can_build_requires_port() {
        let mut province = make_province();
        province.has_port = false;

        let country = make_country();
        let mut building = make_building_def(0, "shipyard");
        building.requires_port = true;

        let defs: HashMap<_, _> = [(BuildingId(0), building.clone())].into_iter().collect();
        let upgraded_by = HashMap::new();

        assert_eq!(
            can_build(&province, &building, &country, &defs, &upgraded_by),
            Err(BuildingError::RequiresPort)
        );
    }

    #[test]
    fn test_can_build_manufactory_wrong_goods() {
        let province = make_province(); // Has TradegoodId(1)

        let country = make_country();
        let mut building = make_building_def(0, "weapons");
        building.manufactory_goods = Some(vec![TradegoodId(2), TradegoodId(3)]); // Not 1

        let defs: HashMap<_, _> = [(BuildingId(0), building.clone())].into_iter().collect();
        let upgraded_by = HashMap::new();

        assert_eq!(
            can_build(&province, &building, &country, &defs, &upgraded_by),
            Err(BuildingError::TradeGoodNotEligible)
        );
    }

    #[test]
    fn test_can_build_insufficient_gold() {
        let province = make_province();
        let mut country = make_country();
        country.treasury = Fixed::from_int(50);

        let building = make_building_def(0, "temple");
        let defs: HashMap<_, _> = [(BuildingId(0), building.clone())].into_iter().collect();
        let upgraded_by = HashMap::new();

        assert_eq!(
            can_build(&province, &building, &country, &defs, &upgraded_by),
            Err(BuildingError::InsufficientGold {
                required: Fixed::from_int(100),
                have: Fixed::from_int(50)
            })
        );
    }

    #[test]
    fn test_has_upgraded_version() {
        let mut province = make_province();
        province.buildings.insert(BuildingId(1)); // Cathedral

        let country = make_country();
        let temple = make_building_def(0, "temple");
        let cathedral = make_building_def(1, "cathedral");

        let defs: HashMap<_, _> = [(BuildingId(0), temple.clone()), (BuildingId(1), cathedral)]
            .into_iter()
            .collect();

        // Temple is upgraded by Cathedral
        let upgraded_by: HashMap<_, _> = [(BuildingId(0), BuildingId(1))].into_iter().collect();

        assert_eq!(
            can_build(&province, &temple, &country, &defs, &upgraded_by),
            Err(BuildingError::HasUpgradedVersion)
        );
    }

    #[test]
    fn test_recompute_fort_level() {
        let mut province = make_province();
        let mut defs = HashMap::new();

        // Fort building with level 2
        let mut fort = make_building_def(0, "fort");
        fort.fort_level = Some(2);
        defs.insert(BuildingId(0), fort);

        // Another fort with level 4
        let mut castle = make_building_def(1, "castle");
        castle.fort_level = Some(4);
        defs.insert(BuildingId(1), castle);

        // No buildings
        assert_eq!(recompute_fort_level(&province, &defs, false), 0);
        assert_eq!(recompute_fort_level(&province, &defs, true), 1); // Capital bonus

        // Add fort
        province.buildings.insert(BuildingId(0));
        assert_eq!(recompute_fort_level(&province, &defs, false), 2);

        // Add castle (max should be 4)
        province.buildings.insert(BuildingId(1));
        assert_eq!(recompute_fort_level(&province, &defs, false), 4);
        assert_eq!(recompute_fort_level(&province, &defs, true), 5); // + capital
    }

    #[test]
    fn test_recompute_province_modifiers() {
        let province_id = ProvinceId::from(100u32);
        let mut province = make_province();
        let mut modifiers = GameModifiers::default();
        let mut defs = HashMap::new();

        // Create building with multiple modifiers
        let mut temple = make_building_def(0, "temple");
        temple.local_tax_modifier = Some(Fixed::from_f32(0.4));
        temple.local_production_efficiency = Some(Fixed::from_f32(0.1));
        defs.insert(BuildingId(0), temple);

        let mut workshop = make_building_def(1, "workshop");
        workshop.local_production_efficiency = Some(Fixed::from_f32(0.5));
        workshop.local_trade_power = Some(Fixed::from_f32(0.5));
        defs.insert(BuildingId(1), workshop);

        // No buildings yet
        recompute_province_modifiers(province_id, &province, &defs, &mut modifiers);
        assert!(!modifiers.province_tax_modifier.contains_key(&province_id));

        // Add temple
        province.buildings.insert(BuildingId(0));
        recompute_province_modifiers(province_id, &province, &defs, &mut modifiers);
        assert_eq!(
            modifiers.province_tax_modifier.get(&province_id),
            Some(&Mod32::from_f32(0.4))
        );
        assert_eq!(
            modifiers.province_production_efficiency.get(&province_id),
            Some(&Mod32::from_f32(0.1))
        );

        // Add workshop (modifiers should stack)
        province.buildings.insert(BuildingId(1));
        recompute_province_modifiers(province_id, &province, &defs, &mut modifiers);
        assert_eq!(
            modifiers.province_production_efficiency.get(&province_id),
            Some(&Mod32::from_f32(0.6)) // 0.1 + 0.5
        );
        assert_eq!(
            modifiers.province_trade_power.get(&province_id),
            Some(&Mod32::from_f32(0.5))
        );
    }

    #[test]
    fn test_recompute_province_modifiers_sailors_and_naval() {
        let province_id = ProvinceId::from(200u32);
        let mut province = make_province();
        let mut modifiers = GameModifiers::default();
        let mut defs = HashMap::new();

        let mut dock = make_building_def(0, "dock");
        dock.local_sailors_modifier = Some(Fixed::from_f32(0.5));
        dock.local_ship_cost = Some(Fixed::from_f32(-0.1));
        dock.local_ship_repair = Some(Fixed::from_f32(0.25));
        defs.insert(BuildingId(0), dock);

        province.buildings.insert(BuildingId(0));
        recompute_province_modifiers(province_id, &province, &defs, &mut modifiers);

        assert_eq!(
            modifiers.province_sailors_modifier.get(&province_id),
            Some(&Mod32::from_f32(0.5))
        );
        assert_eq!(
            modifiers.province_ship_cost.get(&province_id),
            Some(&Mod32::from_f32(-0.1))
        );
        assert_eq!(
            modifiers.province_ship_repair.get(&province_id),
            Some(&Mod32::from_f32(0.25))
        );
    }

    #[test]
    fn test_recompute_province_modifiers_fort_and_defensiveness() {
        let province_id = ProvinceId::from(300u32);
        let mut province = make_province();
        let mut modifiers = GameModifiers::default();
        let mut defs = HashMap::new();

        let mut ramparts = make_building_def(0, "ramparts");
        ramparts.local_defensiveness = Some(Fixed::from_f32(0.25));
        defs.insert(BuildingId(0), ramparts);

        province.buildings.insert(BuildingId(0));
        recompute_province_modifiers(province_id, &province, &defs, &mut modifiers);

        assert_eq!(
            modifiers.province_defensiveness.get(&province_id),
            Some(&Mod32::from_f32(0.25))
        );
    }

    #[test]
    fn test_recompute_province_modifiers_trade_goods_size() {
        let province_id = ProvinceId::from(400u32);
        let mut province = make_province();
        let mut modifiers = GameModifiers::default();
        let mut defs = HashMap::new();

        let mut manufactory = make_building_def(0, "textile_manufactory");
        manufactory.trade_goods_size = Some(Fixed::ONE);
        defs.insert(BuildingId(0), manufactory);

        province.buildings.insert(BuildingId(0));
        recompute_province_modifiers(province_id, &province, &defs, &mut modifiers);

        assert_eq!(
            modifiers.province_trade_goods_size.get(&province_id),
            Some(&Mod32::ONE)
        );
    }
}
