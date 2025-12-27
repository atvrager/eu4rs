//! Estate system for EU4 simulation.
//!
//! Estates represent powerful groups within a country (Nobility, Clergy, Burghers)
//! that provide benefits through privileges in exchange for influence and loyalty management.
//!
//! ## Core Mechanics
//!
//! - **Loyalty** (0-100): Decays toward equilibrium (base 50 + modifiers), 2 points/month
//! - **Influence** (0-100): Based on land share + privilege bonuses
//! - **Privileges**: Grants from player providing modifiers in exchange for loyalty/influence
//! - **Crown Land**: Percentage of land not assigned to estates
//!
//! ## Availability
//!
//! Estates are gated by government type:
//! - **Pirate Republics**: No estates
//! - **Native Councils**: No estates until reformed
//! - **Theocracies**: May lack Nobility
//! - **Most governments**: Core 3 (Nobles, Clergy, Burghers)

use crate::fixed::Fixed;
use crate::government::GovernmentTypeId;
use crate::ideas::ModifierEntry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type-safe estate type identifier.
#[derive(
    Hash, Eq, PartialEq, Clone, Copy, Debug, Default, Serialize, Deserialize, PartialOrd, Ord,
)]
pub struct EstateTypeId(pub u8);

impl EstateTypeId {
    pub const UNKNOWN: EstateTypeId = EstateTypeId(u8::MAX);

    // Core 3 estates (available to most countries)
    pub const NOBLES: EstateTypeId = EstateTypeId(0);
    pub const CLERGY: EstateTypeId = EstateTypeId(1);
    pub const BURGHERS: EstateTypeId = EstateTypeId(2);

    // Special estates (regional/conditional)
    pub const DHIMMI: EstateTypeId = EstateTypeId(3);
    pub const COSSACKS: EstateTypeId = EstateTypeId(4);
    pub const TRIBES: EstateTypeId = EstateTypeId(5);
    pub const JAINS: EstateTypeId = EstateTypeId(6);
    pub const MARATHAS: EstateTypeId = EstateTypeId(7);
    pub const RAJPUTS: EstateTypeId = EstateTypeId(8);
    pub const BRAHMINS: EstateTypeId = EstateTypeId(9);
    pub const EUNUCHS: EstateTypeId = EstateTypeId(10);
    pub const JANISSARIES: EstateTypeId = EstateTypeId(11);
    pub const QIZILBASH: EstateTypeId = EstateTypeId(12);
    pub const GHULAMS: EstateTypeId = EstateTypeId(13);
    pub const NOMADIC_TRIBES: EstateTypeId = EstateTypeId(14);
}

/// Type-safe privilege identifier.
#[derive(
    Hash, Eq, PartialEq, Clone, Copy, Debug, Default, Serialize, Deserialize, PartialOrd, Ord,
)]
pub struct PrivilegeId(pub u16);

impl PrivilegeId {
    pub const UNKNOWN: PrivilegeId = PrivilegeId(u16::MAX);
}

/// Static estate type definition from game files.
#[derive(Debug, Clone)]
pub struct EstateTypeDef {
    pub id: EstateTypeId,
    pub name: String,
    pub base_loyalty_equilibrium: Fixed,
    pub base_influence_per_land: Fixed,
    /// Low loyalty modifiers (loyalty < 30)
    pub low_loyalty_modifiers: Vec<ModifierEntry>,
    /// Medium loyalty modifiers (30-60)
    pub medium_loyalty_modifiers: Vec<ModifierEntry>,
    /// High loyalty modifiers (> 60)
    pub high_loyalty_modifiers: Vec<ModifierEntry>,
    /// Influence threshold for disaster (typically 100)
    pub disaster_influence_threshold: Fixed,
}

/// Static privilege definition from game files.
#[derive(Debug, Clone)]
pub struct PrivilegeDef {
    pub id: PrivilegeId,
    pub name: String,
    pub estate_type: EstateTypeId,
    /// Loyalty bonus granted by this privilege
    pub loyalty_bonus: Fixed,
    /// Influence bonus granted by this privilege
    pub influence_bonus: Fixed,
    /// Max absolutism penalty (negative)
    pub max_absolutism_penalty: i8,
    /// Country modifiers granted by this privilege
    pub modifiers: Vec<ModifierEntry>,
    /// Cooldown in months before can revoke/regrant
    pub cooldown_months: u16,
    /// Only one exclusive privilege can be active at a time per estate
    pub is_exclusive: bool,
    /// Land share granted (0-100)
    pub land_share: Fixed,
}

/// Per-estate runtime state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstateState {
    /// Current loyalty (0-100)
    pub loyalty: Fixed,
    /// Current influence (0-100)
    pub influence: Fixed,
    /// Active privileges for this estate
    pub privileges: Vec<PrivilegeId>,
    /// Land share assigned to this estate (0-100)
    pub land_share: Fixed,
    /// Months of disaster conditions (high influence + low loyalty)
    pub disaster_progress: u8,
}

impl EstateState {
    /// Create a new estate with default starting values.
    pub fn new() -> Self {
        Self {
            loyalty: Fixed::from_int(50), // Start at equilibrium
            influence: Fixed::ZERO,
            privileges: Vec::new(),
            land_share: Fixed::ZERO,
            disaster_progress: 0,
        }
    }
}

impl Default for EstateState {
    fn default() -> Self {
        Self::new()
    }
}

/// All estate state for a country.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CountryEstateState {
    /// Active estates (only estates available to this country)
    pub estates: HashMap<EstateTypeId, EstateState>,
    /// Cached list of available estates (recomputed on government change)
    pub available_estates: Vec<EstateTypeId>,
    /// Crown land percentage (not assigned to estates)
    pub crown_land: Fixed,
    /// Active agenda (estate, agenda_id) for Diet mechanics (stub)
    pub active_agenda: Option<(EstateTypeId, u16)>,
}

impl CountryEstateState {
    /// Create estate state for a country based on government type.
    pub fn new_for_country(
        gov_type: GovernmentTypeId,
        _religion: &str,
        _registry: &EstateRegistry,
    ) -> Self {
        let available = get_available_estates(gov_type);
        let mut estates = HashMap::new();

        for &estate_id in &available {
            estates.insert(estate_id, EstateState::new());
        }

        Self {
            estates,
            available_estates: available,
            crown_land: Fixed::from_int(30), // 30% starting crown land
            active_agenda: None,
        }
    }

    /// Recompute available estates when government changes.
    pub fn recompute_available_estates(
        &mut self,
        new_gov_type: GovernmentTypeId,
        _religion: &str,
        _registry: &EstateRegistry,
    ) {
        let new_available = get_available_estates(new_gov_type);

        // Remove estates that are no longer available
        self.estates.retain(|id, _| new_available.contains(id));

        // Add new estates that became available
        for &estate_id in &new_available {
            self.estates.entry(estate_id).or_default();
        }

        self.available_estates = new_available;
    }
}

/// Hardcoded estate availability mapping (Phase 0 - no trigger parsing).
///
/// Returns the list of estates available for a given government type.
/// Based on EU4 game logic but simplified for MVP.
pub fn get_available_estates(gov_type: GovernmentTypeId) -> Vec<EstateTypeId> {
    match gov_type {
        // No estates for pirates and natives
        GovernmentTypeId::PIRATE_REPUBLIC | GovernmentTypeId::NATIVE_COUNCIL => vec![],

        // Theocracies lack nobility
        GovernmentTypeId::THEOCRACY => vec![EstateTypeId::CLERGY, EstateTypeId::BURGHERS],

        // Most governments have the core 3
        _ => vec![
            EstateTypeId::NOBLES,
            EstateTypeId::CLERGY,
            EstateTypeId::BURGHERS,
        ],
    }
}

/// Registry of all estate types and privileges.
#[derive(Debug, Clone, Default)]
pub struct EstateRegistry {
    estate_types: Vec<EstateTypeDef>,
    privileges: Vec<PrivilegeDef>,
    estate_by_name: HashMap<String, EstateTypeId>,
    privilege_by_name: HashMap<String, PrivilegeId>,
}

impl EstateRegistry {
    /// Create a new registry with hardcoded estate types (Phase 0).
    pub fn new() -> Self {
        let mut registry = Self::default();

        // Add core 3 estates
        registry.add_estate(EstateTypeDef {
            id: EstateTypeId::NOBLES,
            name: "estate_nobles".to_string(),
            base_loyalty_equilibrium: Fixed::from_int(50),
            base_influence_per_land: Fixed::ONE,
            low_loyalty_modifiers: vec![],
            medium_loyalty_modifiers: vec![],
            high_loyalty_modifiers: vec![],
            disaster_influence_threshold: Fixed::from_int(100),
        });

        registry.add_estate(EstateTypeDef {
            id: EstateTypeId::CLERGY,
            name: "estate_church".to_string(),
            base_loyalty_equilibrium: Fixed::from_int(50),
            base_influence_per_land: Fixed::ONE,
            low_loyalty_modifiers: vec![],
            medium_loyalty_modifiers: vec![],
            high_loyalty_modifiers: vec![],
            disaster_influence_threshold: Fixed::from_int(100),
        });

        registry.add_estate(EstateTypeDef {
            id: EstateTypeId::BURGHERS,
            name: "estate_burghers".to_string(),
            base_loyalty_equilibrium: Fixed::from_int(50),
            base_influence_per_land: Fixed::ONE,
            low_loyalty_modifiers: vec![],
            medium_loyalty_modifiers: vec![],
            high_loyalty_modifiers: vec![],
            disaster_influence_threshold: Fixed::from_int(100),
        });

        // Add special estates (will load from files in Phase 2)
        registry.add_estate(EstateTypeDef {
            id: EstateTypeId::DHIMMI,
            name: "estate_dhimmi".to_string(),
            base_loyalty_equilibrium: Fixed::from_int(50),
            base_influence_per_land: Fixed::ONE,
            low_loyalty_modifiers: vec![],
            medium_loyalty_modifiers: vec![],
            high_loyalty_modifiers: vec![],
            disaster_influence_threshold: Fixed::from_int(100),
        });

        registry
    }

    fn add_estate(&mut self, estate: EstateTypeDef) {
        self.estate_by_name.insert(estate.name.clone(), estate.id);
        let index = estate.id.0 as usize;

        // Ensure vector is large enough
        if index >= self.estate_types.len() {
            self.estate_types.resize(
                index + 1,
                EstateTypeDef {
                    id: EstateTypeId::UNKNOWN,
                    name: String::new(),
                    base_loyalty_equilibrium: Fixed::ZERO,
                    base_influence_per_land: Fixed::ZERO,
                    low_loyalty_modifiers: vec![],
                    medium_loyalty_modifiers: vec![],
                    high_loyalty_modifiers: vec![],
                    disaster_influence_threshold: Fixed::from_int(100),
                },
            );
        }

        self.estate_types[index] = estate;
    }

    /// Get estate type definition by ID.
    pub fn get_estate(&self, id: EstateTypeId) -> Option<&EstateTypeDef> {
        self.estate_types.get(id.0 as usize)
    }

    /// Get estate type ID by name.
    pub fn estate_id_by_name(&self, name: &str) -> Option<EstateTypeId> {
        self.estate_by_name.get(name).copied()
    }

    /// Get privilege definition by ID.
    pub fn get_privilege(&self, id: PrivilegeId) -> Option<&PrivilegeDef> {
        self.privileges.get(id.0 as usize)
    }

    /// Get privilege ID by name.
    pub fn privilege_id_by_name(&self, name: &str) -> Option<PrivilegeId> {
        self.privilege_by_name.get(name).copied()
    }

    /// Number of estate types in registry.
    pub fn estate_count(&self) -> usize {
        self.estate_types
            .iter()
            .filter(|e| e.id != EstateTypeId::UNKNOWN)
            .count()
    }

    /// Number of privileges in registry.
    pub fn privilege_count(&self) -> usize {
        self.privileges.len()
    }

    /// Add a privilege for testing purposes (cfg(test) only).
    #[cfg(test)]
    pub fn add_privilege_for_test(&mut self, privilege: PrivilegeDef) {
        self.privilege_by_name
            .insert(privilege.name.clone(), privilege.id);
        let index = privilege.id.0 as usize;

        // Ensure vector is large enough
        if index >= self.privileges.len() {
            self.privileges.resize(
                index + 1,
                PrivilegeDef {
                    id: PrivilegeId(0),
                    name: String::new(),
                    estate_type: EstateTypeId::UNKNOWN,
                    loyalty_bonus: Fixed::ZERO,
                    influence_bonus: Fixed::ZERO,
                    max_absolutism_penalty: 0,
                    modifiers: vec![],
                    cooldown_months: 0,
                    is_exclusive: false,
                    land_share: Fixed::ZERO,
                },
            );
        }

        self.privileges[index] = privilege;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estate_registry_initialization() {
        let registry = EstateRegistry::new();

        // Verify core 3 estates exist
        assert!(registry.get_estate(EstateTypeId::NOBLES).is_some());
        assert!(registry.get_estate(EstateTypeId::CLERGY).is_some());
        assert!(registry.get_estate(EstateTypeId::BURGHERS).is_some());
        assert!(registry.get_estate(EstateTypeId::DHIMMI).is_some());

        assert_eq!(registry.estate_count(), 4);
    }

    #[test]
    fn test_estate_lookup_by_name() {
        let registry = EstateRegistry::new();

        let nobles_id = registry.estate_id_by_name("estate_nobles");
        assert_eq!(nobles_id, Some(EstateTypeId::NOBLES));

        let clergy_id = registry.estate_id_by_name("estate_church");
        assert_eq!(clergy_id, Some(EstateTypeId::CLERGY));

        let unknown = registry.estate_id_by_name("nonexistent");
        assert_eq!(unknown, None);
    }

    #[test]
    fn test_estate_state_default() {
        let state = EstateState::new();

        assert_eq!(state.loyalty, Fixed::from_int(50));
        assert_eq!(state.influence, Fixed::ZERO);
        assert_eq!(state.land_share, Fixed::ZERO);
        assert!(state.privileges.is_empty());
        assert_eq!(state.disaster_progress, 0);
    }

    #[test]
    fn test_get_available_estates_monarchy() {
        let estates = get_available_estates(GovernmentTypeId::MONARCHY);

        assert_eq!(estates.len(), 3);
        assert!(estates.contains(&EstateTypeId::NOBLES));
        assert!(estates.contains(&EstateTypeId::CLERGY));
        assert!(estates.contains(&EstateTypeId::BURGHERS));
    }

    #[test]
    fn test_get_available_estates_theocracy() {
        let estates = get_available_estates(GovernmentTypeId::THEOCRACY);

        assert_eq!(estates.len(), 2);
        assert!(!estates.contains(&EstateTypeId::NOBLES));
        assert!(estates.contains(&EstateTypeId::CLERGY));
        assert!(estates.contains(&EstateTypeId::BURGHERS));
    }

    #[test]
    fn test_get_available_estates_pirate() {
        let estates = get_available_estates(GovernmentTypeId::PIRATE_REPUBLIC);
        assert!(estates.is_empty());
    }

    #[test]
    fn test_get_available_estates_native() {
        let estates = get_available_estates(GovernmentTypeId::NATIVE_COUNCIL);
        assert!(estates.is_empty());
    }

    #[test]
    fn test_country_estate_state_initialization() {
        let registry = EstateRegistry::new();
        let state =
            CountryEstateState::new_for_country(GovernmentTypeId::MONARCHY, "catholic", &registry);

        assert_eq!(state.available_estates.len(), 3);
        assert_eq!(state.estates.len(), 3);
        assert_eq!(state.crown_land, Fixed::from_int(30));
        assert!(state.active_agenda.is_none());

        // Verify all 3 estates are initialized
        assert!(state.estates.contains_key(&EstateTypeId::NOBLES));
        assert!(state.estates.contains_key(&EstateTypeId::CLERGY));
        assert!(state.estates.contains_key(&EstateTypeId::BURGHERS));
    }

    #[test]
    fn test_recompute_available_estates() {
        let registry = EstateRegistry::new();
        let mut state =
            CountryEstateState::new_for_country(GovernmentTypeId::MONARCHY, "catholic", &registry);

        // Initially has 3 estates
        assert_eq!(state.estates.len(), 3);

        // Change to theocracy (loses nobles)
        state.recompute_available_estates(GovernmentTypeId::THEOCRACY, "catholic", &registry);

        assert_eq!(state.estates.len(), 2);
        assert!(!state.estates.contains_key(&EstateTypeId::NOBLES));
        assert!(state.estates.contains_key(&EstateTypeId::CLERGY));
        assert!(state.estates.contains_key(&EstateTypeId::BURGHERS));
    }

    #[test]
    fn test_recompute_adds_new_estates() {
        let registry = EstateRegistry::new();
        let mut state = CountryEstateState::new_for_country(
            GovernmentTypeId::PIRATE_REPUBLIC,
            "catholic",
            &registry,
        );

        // Pirates start with no estates
        assert!(state.estates.is_empty());

        // Reform to monarchy (gains estates)
        state.recompute_available_estates(GovernmentTypeId::MONARCHY, "catholic", &registry);

        assert_eq!(state.estates.len(), 3);
        assert!(state.estates.contains_key(&EstateTypeId::NOBLES));
        assert!(state.estates.contains_key(&EstateTypeId::CLERGY));
        assert!(state.estates.contains_key(&EstateTypeId::BURGHERS));
    }
}
