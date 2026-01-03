//\! Unit tests for estates.rs estate system.
use super::*;
use crate::estates::{CountryEstateState, EstateState, EstateTypeDef, EstateTypeId};
use crate::testing::WorldStateBuilder;

fn create_test_estate_def() -> EstateTypeDef {
    EstateTypeDef {
        id: EstateTypeId::NOBLES,
        name: "estate_nobles".to_string(),
        base_loyalty_equilibrium: Fixed::from_int(50),
        base_influence_per_land: Fixed::ONE,
        low_loyalty_modifiers: vec![],
        medium_loyalty_modifiers: vec![],
        high_loyalty_modifiers: vec![],
        disaster_influence_threshold: Fixed::from_int(100),
    }
}

#[test]
fn test_loyalty_decays_toward_equilibrium_from_above() {
    let estate_def = create_test_estate_def();
    let mut estate_state = EstateState {
        loyalty: Fixed::from_int(80), // Above equilibrium (50)
        influence: Fixed::ZERO,
        privileges: vec![],
        land_share: Fixed::ZERO,
        disaster_progress: 0,
    };

    update_estate_loyalty(&mut estate_state, &estate_def);

    // Should decay by 2 points toward 50
    assert_eq!(estate_state.loyalty, Fixed::from_int(78));
}

#[test]
fn test_loyalty_decays_toward_equilibrium_from_below() {
    let estate_def = create_test_estate_def();
    let mut estate_state = EstateState {
        loyalty: Fixed::from_int(20), // Below equilibrium (50)
        influence: Fixed::ZERO,
        privileges: vec![],
        land_share: Fixed::ZERO,
        disaster_progress: 0,
    };

    update_estate_loyalty(&mut estate_state, &estate_def);

    // Should increase by 2 points toward 50
    assert_eq!(estate_state.loyalty, Fixed::from_int(22));
}

#[test]
fn test_loyalty_stops_at_equilibrium() {
    let estate_def = create_test_estate_def();
    let mut estate_state = EstateState {
        loyalty: Fixed::from_int(51), // 1 point above equilibrium
        influence: Fixed::ZERO,
        privileges: vec![],
        land_share: Fixed::ZERO,
        disaster_progress: 0,
    };

    update_estate_loyalty(&mut estate_state, &estate_def);

    // Should decay to exactly 50, not below
    assert_eq!(estate_state.loyalty, Fixed::from_int(50));
}

#[test]
fn test_loyalty_clamps_to_100() {
    let estate_def = create_test_estate_def();
    let mut estate_state = EstateState {
        loyalty: Fixed::from_int(101), // Invalid state, but should clamp
        influence: Fixed::ZERO,
        privileges: vec![],
        land_share: Fixed::ZERO,
        disaster_progress: 0,
    };

    update_estate_loyalty(&mut estate_state, &estate_def);

    assert!(estate_state.loyalty <= Fixed::from_int(100));
}

#[test]
fn test_influence_calculated_from_land_share() {
    let estate_def = create_test_estate_def();
    let mut estate_state = EstateState {
        loyalty: Fixed::from_int(50),
        influence: Fixed::ZERO,
        privileges: vec![],
        land_share: Fixed::from_int(25), // 25% land
        disaster_progress: 0,
    };

    update_estate_influence(&mut estate_state, &estate_def);

    // 25% land * 1.0 influence per land = 25 influence
    assert_eq!(estate_state.influence, Fixed::from_int(25));
}

#[test]
fn test_influence_clamps_to_100() {
    let estate_def = create_test_estate_def();
    let mut estate_state = EstateState {
        loyalty: Fixed::from_int(50),
        influence: Fixed::ZERO,
        privileges: vec![],
        land_share: Fixed::from_int(150), // Invalid, but should clamp
        disaster_progress: 0,
    };

    update_estate_influence(&mut estate_state, &estate_def);

    assert_eq!(estate_state.influence, Fixed::from_int(100));
}

#[test]
fn test_disaster_progress_increments() {
    let estate_def = create_test_estate_def();
    let mut estate_state = EstateState {
        loyalty: Fixed::from_int(20),    // Low loyalty
        influence: Fixed::from_int(100), // High influence
        privileges: vec![],
        land_share: Fixed::ZERO,
        disaster_progress: 0,
    };

    check_estate_disaster(&mut estate_state, &estate_def);

    assert_eq!(estate_state.disaster_progress, 1);
}

#[test]
fn test_disaster_progress_resets() {
    let estate_def = create_test_estate_def();
    let mut estate_state = EstateState {
        loyalty: Fixed::from_int(50),    // Normal loyalty
        influence: Fixed::from_int(100), // High influence
        privileges: vec![],
        land_share: Fixed::ZERO,
        disaster_progress: 5, // Had progress before
    };

    check_estate_disaster(&mut estate_state, &estate_def);

    // Should reset when conditions no longer met
    assert_eq!(estate_state.disaster_progress, 0);
}

#[test]
fn test_disaster_requires_both_conditions() {
    let estate_def = create_test_estate_def();

    // High influence but normal loyalty - no disaster
    let mut estate_state = EstateState {
        loyalty: Fixed::from_int(50),
        influence: Fixed::from_int(100),
        privileges: vec![],
        land_share: Fixed::ZERO,
        disaster_progress: 0,
    };
    check_estate_disaster(&mut estate_state, &estate_def);
    assert_eq!(estate_state.disaster_progress, 0);

    // Low loyalty but normal influence - no disaster
    let mut estate_state = EstateState {
        loyalty: Fixed::from_int(20),
        influence: Fixed::from_int(50),
        privileges: vec![],
        land_share: Fixed::ZERO,
        disaster_progress: 0,
    };
    check_estate_disaster(&mut estate_state, &estate_def);
    assert_eq!(estate_state.disaster_progress, 0);
}

#[test]
fn test_run_estate_tick_updates_all_estates() {
    use crate::estates::EstateRegistry;
    use crate::government::GovernmentTypeId;

    // Build test state with proper estate registry
    let mut state = WorldStateBuilder::new().with_country("TST").build();
    state.estates = EstateRegistry::new();

    // Initialize estates for test country
    let test_country = state.countries.get_mut("TST").unwrap();
    test_country.estates =
        CountryEstateState::new_for_country(GovernmentTypeId::MONARCHY, "catholic", &state.estates);

    // Set loyalty above equilibrium for nobles
    if let Some(nobles) = test_country.estates.estates.get_mut(&EstateTypeId::NOBLES) {
        nobles.loyalty = Fixed::from_int(80);
    }

    let initial_loyalty = state
        .countries
        .get("TST")
        .unwrap()
        .estates
        .estates
        .get(&EstateTypeId::NOBLES)
        .unwrap()
        .loyalty;

    run_estate_tick(&mut state);

    let updated_loyalty = state
        .countries
        .get("TST")
        .unwrap()
        .estates
        .estates
        .get(&EstateTypeId::NOBLES)
        .unwrap()
        .loyalty;

    // Loyalty should have decayed
    assert!(updated_loyalty < initial_loyalty);
}

#[test]
fn test_grant_privilege_success() {
    use crate::estates::{EstateRegistry, PrivilegeDef, PrivilegeId};
    use crate::government::GovernmentTypeId;

    let mut state = WorldStateBuilder::new().with_country("TST").build();
    let mut registry = EstateRegistry::new();

    // Add a test privilege
    let privilege_id = PrivilegeId(1);
    registry.add_privilege_for_test(PrivilegeDef {
        id: privilege_id,
        name: "test_privilege".to_string(),
        estate_type: EstateTypeId::NOBLES,
        loyalty_bonus: Fixed::from_int(10),
        influence_bonus: Fixed::ZERO,
        max_absolutism_penalty: -5,
        modifiers: vec![],
        cooldown_months: 0,
        is_exclusive: false,
        land_share: Fixed::from_int(5),
    });

    state.estates = registry;

    // Initialize estates
    let country = state.countries.get_mut("TST").unwrap();
    country.estates =
        CountryEstateState::new_for_country(GovernmentTypeId::MONARCHY, "catholic", &state.estates);

    let initial_loyalty = country
        .estates
        .estates
        .get(&EstateTypeId::NOBLES)
        .unwrap()
        .loyalty;
    let initial_crown_land = country.estates.crown_land;

    // Grant privilege
    let result = grant_privilege(country, EstateTypeId::NOBLES, privilege_id, &state.estates);
    assert!(result.is_ok());

    // Check loyalty increased
    let new_loyalty = country
        .estates
        .estates
        .get(&EstateTypeId::NOBLES)
        .unwrap()
        .loyalty;
    assert_eq!(new_loyalty, initial_loyalty + Fixed::from_int(10));

    // Check land share increased
    let land_share = country
        .estates
        .estates
        .get(&EstateTypeId::NOBLES)
        .unwrap()
        .land_share;
    assert_eq!(land_share, Fixed::from_int(5));

    // Check crown land decreased
    assert_eq!(
        country.estates.crown_land,
        initial_crown_land - Fixed::from_int(5)
    );

    // Check privilege is recorded
    assert!(country
        .estates
        .estates
        .get(&EstateTypeId::NOBLES)
        .unwrap()
        .privileges
        .contains(&privilege_id));
}

#[test]
fn test_grant_privilege_already_granted() {
    use crate::estates::{EstateRegistry, PrivilegeDef, PrivilegeId};
    use crate::government::GovernmentTypeId;

    let mut state = WorldStateBuilder::new().with_country("TST").build();
    let mut registry = EstateRegistry::new();

    let privilege_id = PrivilegeId(1);
    registry.add_privilege_for_test(PrivilegeDef {
        id: privilege_id,
        name: "test_privilege".to_string(),
        estate_type: EstateTypeId::NOBLES,
        loyalty_bonus: Fixed::from_int(10),
        influence_bonus: Fixed::ZERO,
        max_absolutism_penalty: 0,
        modifiers: vec![],
        cooldown_months: 0,
        is_exclusive: false,
        land_share: Fixed::ZERO,
    });

    state.estates = registry;

    let country = state.countries.get_mut("TST").unwrap();
    country.estates =
        CountryEstateState::new_for_country(GovernmentTypeId::MONARCHY, "catholic", &state.estates);

    // Grant once
    grant_privilege(country, EstateTypeId::NOBLES, privilege_id, &state.estates).unwrap();

    // Try to grant again
    let result = grant_privilege(country, EstateTypeId::NOBLES, privilege_id, &state.estates);
    assert_eq!(result, Err(PrivilegeError::AlreadyGranted));
}

#[test]
fn test_grant_privilege_wrong_estate() {
    use crate::estates::{EstateRegistry, PrivilegeDef, PrivilegeId};
    use crate::government::GovernmentTypeId;

    let mut state = WorldStateBuilder::new().with_country("TST").build();
    let mut registry = EstateRegistry::new();

    let privilege_id = PrivilegeId(1);
    registry.add_privilege_for_test(PrivilegeDef {
        id: privilege_id,
        name: "test_privilege".to_string(),
        estate_type: EstateTypeId::NOBLES,
        loyalty_bonus: Fixed::from_int(10),
        influence_bonus: Fixed::ZERO,
        max_absolutism_penalty: 0,
        modifiers: vec![],
        cooldown_months: 0,
        is_exclusive: false,
        land_share: Fixed::ZERO,
    });

    state.estates = registry;

    let country = state.countries.get_mut("TST").unwrap();
    country.estates =
        CountryEstateState::new_for_country(GovernmentTypeId::MONARCHY, "catholic", &state.estates);

    // Try to grant nobles privilege to clergy
    let result = grant_privilege(country, EstateTypeId::CLERGY, privilege_id, &state.estates);
    assert_eq!(result, Err(PrivilegeError::WrongEstate));
}

#[test]
fn test_revoke_privilege_success() {
    use crate::estates::{EstateRegistry, PrivilegeDef, PrivilegeId};
    use crate::government::GovernmentTypeId;

    let mut state = WorldStateBuilder::new().with_country("TST").build();
    let mut registry = EstateRegistry::new();

    let privilege_id = PrivilegeId(1);
    registry.add_privilege_for_test(PrivilegeDef {
        id: privilege_id,
        name: "test_privilege".to_string(),
        estate_type: EstateTypeId::NOBLES,
        loyalty_bonus: Fixed::from_int(10),
        influence_bonus: Fixed::ZERO,
        max_absolutism_penalty: 0,
        modifiers: vec![],
        cooldown_months: 0,
        is_exclusive: false,
        land_share: Fixed::from_int(5),
    });

    state.estates = registry;

    let country = state.countries.get_mut("TST").unwrap();
    country.estates =
        CountryEstateState::new_for_country(GovernmentTypeId::MONARCHY, "catholic", &state.estates);

    // Grant privilege first
    grant_privilege(country, EstateTypeId::NOBLES, privilege_id, &state.estates).unwrap();

    let loyalty_after_grant = country
        .estates
        .estates
        .get(&EstateTypeId::NOBLES)
        .unwrap()
        .loyalty;
    let crown_land_after_grant = country.estates.crown_land;

    // Revoke privilege
    let result = revoke_privilege(country, EstateTypeId::NOBLES, privilege_id, &state.estates);
    assert!(result.is_ok());

    // Check loyalty decreased
    let new_loyalty = country
        .estates
        .estates
        .get(&EstateTypeId::NOBLES)
        .unwrap()
        .loyalty;
    assert_eq!(new_loyalty, loyalty_after_grant - Fixed::from_int(10));

    // Check land share decreased
    let land_share = country
        .estates
        .estates
        .get(&EstateTypeId::NOBLES)
        .unwrap()
        .land_share;
    assert_eq!(land_share, Fixed::ZERO);

    // Check crown land increased
    assert_eq!(
        country.estates.crown_land,
        crown_land_after_grant + Fixed::from_int(5)
    );

    // Check privilege is removed
    assert!(!country
        .estates
        .estates
        .get(&EstateTypeId::NOBLES)
        .unwrap()
        .privileges
        .contains(&privilege_id));
}

#[test]
fn test_revoke_privilege_not_granted() {
    use crate::estates::{EstateRegistry, PrivilegeDef, PrivilegeId};
    use crate::government::GovernmentTypeId;

    let mut state = WorldStateBuilder::new().with_country("TST").build();
    let mut registry = EstateRegistry::new();

    let privilege_id = PrivilegeId(1);
    registry.add_privilege_for_test(PrivilegeDef {
        id: privilege_id,
        name: "test_privilege".to_string(),
        estate_type: EstateTypeId::NOBLES,
        loyalty_bonus: Fixed::from_int(10),
        influence_bonus: Fixed::ZERO,
        max_absolutism_penalty: 0,
        modifiers: vec![],
        cooldown_months: 0,
        is_exclusive: false,
        land_share: Fixed::ZERO,
    });

    state.estates = registry;

    let country = state.countries.get_mut("TST").unwrap();
    country.estates =
        CountryEstateState::new_for_country(GovernmentTypeId::MONARCHY, "catholic", &state.estates);

    // Try to revoke without granting
    let result = revoke_privilege(country, EstateTypeId::NOBLES, privilege_id, &state.estates);
    assert_eq!(result, Err(PrivilegeError::NotGranted));
}

#[test]
fn test_grant_revoke_crown_land_accounting() {
    use crate::estates::{EstateRegistry, PrivilegeDef, PrivilegeId};
    use crate::government::GovernmentTypeId;

    let mut state = WorldStateBuilder::new().with_country("TST").build();
    let mut registry = EstateRegistry::new();

    let privilege_id = PrivilegeId(1);
    registry.add_privilege_for_test(PrivilegeDef {
        id: privilege_id,
        name: "test_privilege".to_string(),
        estate_type: EstateTypeId::NOBLES,
        loyalty_bonus: Fixed::from_int(10),
        influence_bonus: Fixed::ZERO,
        max_absolutism_penalty: 0,
        modifiers: vec![],
        cooldown_months: 0,
        is_exclusive: false,
        land_share: Fixed::from_int(20),
    });

    state.estates = registry;

    let country = state.countries.get_mut("TST").unwrap();
    country.estates =
        CountryEstateState::new_for_country(GovernmentTypeId::MONARCHY, "catholic", &state.estates);

    let initial_crown_land = country.estates.crown_land;

    // Grant and revoke multiple times
    grant_privilege(country, EstateTypeId::NOBLES, privilege_id, &state.estates).unwrap();
    revoke_privilege(country, EstateTypeId::NOBLES, privilege_id, &state.estates).unwrap();
    grant_privilege(country, EstateTypeId::NOBLES, privilege_id, &state.estates).unwrap();
    revoke_privilege(country, EstateTypeId::NOBLES, privilege_id, &state.estates).unwrap();

    // Crown land should return to initial value
    assert_eq!(country.estates.crown_land, initial_crown_land);
}

#[test]
fn test_seize_land_success() {
    use crate::government::GovernmentTypeId;

    let mut state = WorldStateBuilder::new().with_country("TST").build();
    let registry = EstateRegistry::new();
    state.estates = registry;

    let country = state.countries.get_mut("TST").unwrap();
    country.estates =
        CountryEstateState::new_for_country(GovernmentTypeId::MONARCHY, "catholic", &state.estates);

    // Give estates some land
    if let Some(nobles) = country.estates.estates.get_mut(&EstateTypeId::NOBLES) {
        nobles.land_share = Fixed::from_int(30);
    }
    if let Some(clergy) = country.estates.estates.get_mut(&EstateTypeId::CLERGY) {
        clergy.land_share = Fixed::from_int(30);
    }
    if let Some(burghers) = country.estates.estates.get_mut(&EstateTypeId::BURGHERS) {
        burghers.land_share = Fixed::from_int(30);
    }

    let initial_crown = country.estates.crown_land;

    // Seize 15% land
    seize_land(country, 15).unwrap();

    // Crown land should increase by 15
    assert_eq!(
        country.estates.crown_land,
        initial_crown + Fixed::from_int(15)
    );

    // All estates should lose loyalty
    if let Some(nobles) = country.estates.estates.get(&EstateTypeId::NOBLES) {
        assert!(nobles.loyalty < Fixed::from_int(50));
    }
}

#[test]
fn test_seize_land_insufficient_estate_land() {
    use crate::government::GovernmentTypeId;

    let mut state = WorldStateBuilder::new().with_country("TST").build();
    let registry = EstateRegistry::new();
    state.estates = registry;

    let country = state.countries.get_mut("TST").unwrap();
    country.estates =
        CountryEstateState::new_for_country(GovernmentTypeId::MONARCHY, "catholic", &state.estates);

    // Estates start with no land
    // Try to seize 10% when estates have 0
    let result = seize_land(country, 10);

    assert_eq!(result, Err(CrownLandError::InsufficientEstateLand));
}

#[test]
fn test_seize_land_invalid_percentage() {
    use crate::government::GovernmentTypeId;

    let mut state = WorldStateBuilder::new().with_country("TST").build();
    let registry = EstateRegistry::new();
    state.estates = registry;

    let country = state.countries.get_mut("TST").unwrap();
    country.estates =
        CountryEstateState::new_for_country(GovernmentTypeId::MONARCHY, "catholic", &state.estates);

    // Test 0%
    assert_eq!(
        seize_land(country, 0),
        Err(CrownLandError::InvalidPercentage)
    );

    // Test >100%
    assert_eq!(
        seize_land(country, 101),
        Err(CrownLandError::InvalidPercentage)
    );
}

#[test]
fn test_sale_land_success() {
    use crate::government::GovernmentTypeId;

    let mut state = WorldStateBuilder::new().with_country("TST").build();
    let registry = EstateRegistry::new();
    state.estates = registry;

    let country = state.countries.get_mut("TST").unwrap();
    country.estates =
        CountryEstateState::new_for_country(GovernmentTypeId::MONARCHY, "catholic", &state.estates);

    let initial_crown = country.estates.crown_land;
    let initial_nobles_loyalty = country
        .estates
        .estates
        .get(&EstateTypeId::NOBLES)
        .map(|e| e.loyalty)
        .unwrap_or(Fixed::ZERO);

    // Sell 10% land to nobles
    sale_land(country, EstateTypeId::NOBLES, 10).unwrap();

    // Crown land should decrease by 10
    assert_eq!(
        country.estates.crown_land,
        initial_crown - Fixed::from_int(10)
    );

    // Nobles should have +10 land share
    if let Some(nobles) = country.estates.estates.get(&EstateTypeId::NOBLES) {
        assert_eq!(nobles.land_share, Fixed::from_int(10));
        // Loyalty should increase
        assert_eq!(nobles.loyalty, initial_nobles_loyalty + Fixed::from_int(5));
    }
}

#[test]
fn test_sale_land_insufficient_crown_land() {
    use crate::government::GovernmentTypeId;

    let mut state = WorldStateBuilder::new().with_country("TST").build();
    let registry = EstateRegistry::new();
    state.estates = registry;

    let country = state.countries.get_mut("TST").unwrap();
    country.estates =
        CountryEstateState::new_for_country(GovernmentTypeId::MONARCHY, "catholic", &state.estates);

    // Set crown land to 5%
    country.estates.crown_land = Fixed::from_int(5);

    // Try to sell 10% when we only have 5%
    let result = sale_land(country, EstateTypeId::NOBLES, 10);

    assert_eq!(result, Err(CrownLandError::InsufficientCrownLand));
}

#[test]
fn test_sale_land_invalid_percentage() {
    use crate::government::GovernmentTypeId;

    let mut state = WorldStateBuilder::new().with_country("TST").build();
    let registry = EstateRegistry::new();
    state.estates = registry;

    let country = state.countries.get_mut("TST").unwrap();
    country.estates =
        CountryEstateState::new_for_country(GovernmentTypeId::MONARCHY, "catholic", &state.estates);

    // Test 0%
    assert_eq!(
        sale_land(country, EstateTypeId::NOBLES, 0),
        Err(CrownLandError::InvalidPercentage)
    );

    // Test >100%
    assert_eq!(
        sale_land(country, EstateTypeId::NOBLES, 101),
        Err(CrownLandError::InvalidPercentage)
    );
}

#[test]
fn test_seize_sale_crown_land_accounting() {
    use crate::government::GovernmentTypeId;

    let mut state = WorldStateBuilder::new().with_country("TST").build();
    let registry = EstateRegistry::new();
    state.estates = registry;

    let country = state.countries.get_mut("TST").unwrap();
    country.estates =
        CountryEstateState::new_for_country(GovernmentTypeId::MONARCHY, "catholic", &state.estates);

    // Give estates initial land
    if let Some(nobles) = country.estates.estates.get_mut(&EstateTypeId::NOBLES) {
        nobles.land_share = Fixed::from_int(20);
    }
    if let Some(clergy) = country.estates.estates.get_mut(&EstateTypeId::CLERGY) {
        clergy.land_share = Fixed::from_int(20);
    }
    if let Some(burghers) = country.estates.estates.get_mut(&EstateTypeId::BURGHERS) {
        burghers.land_share = Fixed::from_int(20);
    }

    country.estates.crown_land = Fixed::from_int(40);

    // Seize 15%, then sell 15% back
    seize_land(country, 15).unwrap();
    assert_eq!(country.estates.crown_land, Fixed::from_int(55));

    sale_land(country, EstateTypeId::NOBLES, 15).unwrap();
    assert_eq!(country.estates.crown_land, Fixed::from_int(40));
}
