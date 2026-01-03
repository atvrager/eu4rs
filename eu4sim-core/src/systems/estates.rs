//! Estate system monthly tick logic.
//!
//! Handles loyalty decay, influence calculation, and disaster detection.

use crate::estates::{EstateRegistry, EstateState, EstateTypeId, PrivilegeId};
use crate::fixed::Fixed;
use crate::state::{CountryState, WorldState};

/// Run monthly estate updates for all countries.
///
/// This should be called once per month (when `date.day == 1`).
pub fn run_estate_tick(state: &mut WorldState) {
    let registry = &state.estates;
    for (_tag, country) in state.countries.iter_mut() {
        update_country_estates(country, registry);
    }
}

/// Update all estates for a single country.
fn update_country_estates(country: &mut CountryState, registry: &EstateRegistry) {
    for &estate_id in &country.estates.available_estates {
        if let Some(estate_state) = country.estates.estates.get_mut(&estate_id) {
            if let Some(estate_def) = registry.get_estate(estate_id) {
                update_estate_loyalty(estate_state, estate_def);
                update_estate_influence(estate_state, estate_def);
                check_estate_disaster(estate_state, estate_def);
            }
        }
    }
}

/// Update loyalty for a single estate (decays toward equilibrium).
///
/// Loyalty decays by 2 points per month toward equilibrium.
/// Equilibrium = base (50) + privilege bonuses + modifier bonuses.
fn update_estate_loyalty(
    estate_state: &mut EstateState,
    estate_def: &crate::estates::EstateTypeDef,
) {
    // Calculate equilibrium (base + modifiers)
    let equilibrium = estate_def.base_loyalty_equilibrium;
    // TODO Phase 5: Add privilege loyalty bonuses
    // TODO Phase 5: Add modifier loyalty bonuses

    // Decay 2 points per month toward equilibrium
    let decay_rate = Fixed::from_int(2);

    if estate_state.loyalty > equilibrium {
        // Decay downward
        estate_state.loyalty = (estate_state.loyalty - decay_rate).max(equilibrium);
    } else if estate_state.loyalty < equilibrium {
        // Decay upward
        estate_state.loyalty = (estate_state.loyalty + decay_rate).min(equilibrium);
    }

    // Clamp to 0-100
    estate_state.loyalty = estate_state
        .loyalty
        .clamp(Fixed::ZERO, Fixed::from_int(100));
}

/// Update influence for a single estate.
///
/// Influence = land_share * influence_per_land + privilege bonuses + modifier bonuses.
fn update_estate_influence(
    estate_state: &mut EstateState,
    estate_def: &crate::estates::EstateTypeDef,
) {
    // Base influence from land share
    let base_influence = estate_state.land_share * estate_def.base_influence_per_land;

    // TODO Phase 5: Add privilege influence bonuses
    // TODO Phase 5: Add modifier influence bonuses

    estate_state.influence = base_influence;

    // Clamp to 0-100
    estate_state.influence = estate_state
        .influence
        .clamp(Fixed::ZERO, Fixed::from_int(100));
}

/// Check for estate disaster conditions.
///
/// Disaster triggers when influence >= threshold (100) AND loyalty < 30.
/// Increments disaster_progress each month conditions are met.
/// At 12 months, disaster would fire (stubbed for now).
fn check_estate_disaster(
    estate_state: &mut EstateState,
    estate_def: &crate::estates::EstateTypeDef,
) {
    let high_influence = estate_state.influence >= estate_def.disaster_influence_threshold;
    let low_loyalty = estate_state.loyalty < Fixed::from_int(30);

    if high_influence && low_loyalty {
        // Increment disaster progress
        estate_state.disaster_progress = estate_state.disaster_progress.saturating_add(1);

        // Log warning when disaster is imminent
        if estate_state.disaster_progress >= 12 {
            log::warn!(
                "Estate disaster conditions met for 12 months (influence: {}, loyalty: {})",
                estate_state.influence,
                estate_state.loyalty
            );
            // TODO: Trigger actual disaster event (requires event system)
        }
    } else {
        // Reset progress when conditions no longer met
        estate_state.disaster_progress = 0;
    }
}

/// Error type for privilege operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivilegeError {
    /// Estate not found in country
    EstateNotAvailable,
    /// Privilege not found in registry
    PrivilegeNotFound,
    /// Privilege already granted
    AlreadyGranted,
    /// Privilege not currently active
    NotGranted,
    /// Privilege belongs to a different estate
    WrongEstate,
}

/// Grant a privilege to an estate.
///
/// This increases loyalty and influence, grants country modifiers,
/// and may reduce crown land and max absolutism.
pub fn grant_privilege(
    country: &mut CountryState,
    estate_id: EstateTypeId,
    privilege_id: PrivilegeId,
    registry: &EstateRegistry,
) -> Result<(), PrivilegeError> {
    // Check that estate is available
    if !country.estates.available_estates.contains(&estate_id) {
        return Err(PrivilegeError::EstateNotAvailable);
    }

    // Get privilege definition
    let privilege_def = registry
        .get_privilege(privilege_id)
        .ok_or(PrivilegeError::PrivilegeNotFound)?;

    // Check that privilege belongs to this estate
    if privilege_def.estate_type != estate_id {
        return Err(PrivilegeError::WrongEstate);
    }

    // Get estate state
    let estate_state = country
        .estates
        .estates
        .get_mut(&estate_id)
        .ok_or(PrivilegeError::EstateNotAvailable)?;

    // Check if already granted
    if estate_state.privileges.contains(&privilege_id) {
        return Err(PrivilegeError::AlreadyGranted);
    }

    // Grant the privilege
    estate_state.privileges.push(privilege_id);

    // Apply immediate effects
    estate_state.loyalty = (estate_state.loyalty + privilege_def.loyalty_bonus)
        .clamp(Fixed::ZERO, Fixed::from_int(100));
    estate_state.land_share = (estate_state.land_share + privilege_def.land_share)
        .clamp(Fixed::ZERO, Fixed::from_int(100));

    // Update crown land
    country.estates.crown_land =
        (country.estates.crown_land - privilege_def.land_share).max(Fixed::ZERO);

    // TODO Phase 5: Apply privilege modifiers to country
    // TODO Phase 6: Apply max_absolutism_penalty

    log::debug!(
        "Granted privilege {} to estate {:?}",
        privilege_def.name,
        estate_id
    );

    Ok(())
}

/// Revoke a privilege from an estate.
///
/// This decreases loyalty and removes bonuses.
/// Subject to cooldown timer (not implemented in Phase 4).
pub fn revoke_privilege(
    country: &mut CountryState,
    estate_id: EstateTypeId,
    privilege_id: PrivilegeId,
    registry: &EstateRegistry,
) -> Result<(), PrivilegeError> {
    // Check that estate is available
    if !country.estates.available_estates.contains(&estate_id) {
        return Err(PrivilegeError::EstateNotAvailable);
    }

    // Get privilege definition
    let privilege_def = registry
        .get_privilege(privilege_id)
        .ok_or(PrivilegeError::PrivilegeNotFound)?;

    // Check that privilege belongs to this estate
    if privilege_def.estate_type != estate_id {
        return Err(PrivilegeError::WrongEstate);
    }

    // Get estate state
    let estate_state = country
        .estates
        .estates
        .get_mut(&estate_id)
        .ok_or(PrivilegeError::EstateNotAvailable)?;

    // Check if privilege is granted
    if !estate_state.privileges.contains(&privilege_id) {
        return Err(PrivilegeError::NotGranted);
    }

    // Remove the privilege
    estate_state.privileges.retain(|&id| id != privilege_id);

    // Apply immediate effects (reverse of grant)
    estate_state.loyalty = (estate_state.loyalty - privilege_def.loyalty_bonus)
        .clamp(Fixed::ZERO, Fixed::from_int(100));
    estate_state.land_share = (estate_state.land_share - privilege_def.land_share).max(Fixed::ZERO);

    // Update crown land
    country.estates.crown_land =
        (country.estates.crown_land + privilege_def.land_share).min(Fixed::from_int(100));

    // TODO Phase 5: Remove privilege modifiers from country
    // TODO Phase 6: Remove max_absolutism_penalty

    log::debug!(
        "Revoked privilege {} from estate {:?}",
        privilege_def.name,
        estate_id
    );

    Ok(())
}

/// Error type for crown land operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrownLandError {
    /// Insufficient estate land to seize
    InsufficientEstateLand,
    /// Insufficient crown land to sell
    InsufficientCrownLand,
    /// Estate not found in country
    EstateNotAvailable,
    /// Invalid percentage (must be 1-100)
    InvalidPercentage,
}

/// Seize land from estates to increase crown land.
///
/// This costs loyalty with all estates and increases crown land percentage.
/// All estates lose land proportionally.
pub fn seize_land(country: &mut CountryState, percentage: u8) -> Result<(), CrownLandError> {
    // Validate percentage
    if percentage == 0 || percentage > 100 {
        return Err(CrownLandError::InvalidPercentage);
    }

    let amount = Fixed::from_int(percentage as i64);

    // Check if estates have enough land to seize
    let total_estate_land: Fixed = country
        .estates
        .estates
        .values()
        .fold(Fixed::ZERO, |acc, e| acc + e.land_share);

    if amount > total_estate_land {
        return Err(CrownLandError::InsufficientEstateLand);
    }

    // Seize the land (increase crown land)
    country.estates.crown_land = (country.estates.crown_land + amount).min(Fixed::from_int(100));

    // Reduce land from all estates proportionally
    let num_estates = country.estates.estates.len() as i64;
    if num_estates > 0 {
        let reduction_per_estate = amount / Fixed::from_int(num_estates);

        for estate_state in country.estates.estates.values_mut() {
            estate_state.land_share =
                (estate_state.land_share - reduction_per_estate).max(Fixed::ZERO);
            // Seizing land reduces loyalty
            estate_state.loyalty = (estate_state.loyalty - Fixed::from_int(10)).max(Fixed::ZERO);
        }
    }

    log::debug!("Seized {}% crown land", percentage);

    Ok(())
}

/// Sell crown land to an estate.
///
/// This increases loyalty and land share with the estate and decreases crown land.
pub fn sale_land(
    country: &mut CountryState,
    estate_id: EstateTypeId,
    percentage: u8,
) -> Result<(), CrownLandError> {
    // Validate percentage
    if percentage == 0 || percentage > 100 {
        return Err(CrownLandError::InvalidPercentage);
    }

    let amount = Fixed::from_int(percentage as i64);

    // Check that we have enough crown land to sell
    if amount > country.estates.crown_land {
        return Err(CrownLandError::InsufficientCrownLand);
    }

    // Check that estate is available
    if !country.estates.available_estates.contains(&estate_id) {
        return Err(CrownLandError::EstateNotAvailable);
    }

    // Get estate state
    let estate_state = country
        .estates
        .estates
        .get_mut(&estate_id)
        .ok_or(CrownLandError::EstateNotAvailable)?;

    // Sell the land
    country.estates.crown_land = (country.estates.crown_land - amount).max(Fixed::ZERO);
    estate_state.land_share = (estate_state.land_share + amount).min(Fixed::from_int(100));

    // Selling land increases loyalty
    estate_state.loyalty = (estate_state.loyalty + Fixed::from_int(5)).min(Fixed::from_int(100));

    log::debug!("Sold {}% crown land to estate {:?}", percentage, estate_id);

    Ok(())
}

#[cfg(test)]
#[path = "estates_tests.rs"]
mod tests;
