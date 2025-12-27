//! Government types and reforms.
//!
//! Tracks country government types (Monarchy, Republic, Theocracy, Tribal)
//! and their reforms. Used to gate estate availability and other mechanics.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Type-safe government type identifier.
#[derive(
    Hash, Eq, PartialEq, Clone, Copy, Debug, Default, Serialize, Deserialize, PartialOrd, Ord,
)]
pub struct GovernmentTypeId(pub u16);

impl GovernmentTypeId {
    pub const UNKNOWN: GovernmentTypeId = GovernmentTypeId(u16::MAX);

    // Well-known government types (hardcoded for Phase 0)
    pub const MONARCHY: GovernmentTypeId = GovernmentTypeId(0);
    pub const REPUBLIC: GovernmentTypeId = GovernmentTypeId(1);
    pub const THEOCRACY: GovernmentTypeId = GovernmentTypeId(2);
    pub const TRIBAL: GovernmentTypeId = GovernmentTypeId(3);
    pub const PIRATE_REPUBLIC: GovernmentTypeId = GovernmentTypeId(4);
    pub const NATIVE_COUNCIL: GovernmentTypeId = GovernmentTypeId(5);
}

/// Type-safe government reform identifier.
#[derive(
    Hash, Eq, PartialEq, Clone, Copy, Debug, Default, Serialize, Deserialize, PartialOrd, Ord,
)]
pub struct ReformId(pub u16);

impl ReformId {
    pub const UNKNOWN: ReformId = ReformId(u16::MAX);
}

/// Government category (high-level grouping).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GovernmentCategory {
    Monarchy,
    Republic,
    Theocracy,
    Tribal,
}

/// Static government type definition.
#[derive(Debug, Clone)]
pub struct GovernmentTypeDef {
    pub id: GovernmentTypeId,
    pub name: String,
    pub category: GovernmentCategory,
}

/// Static government reform definition.
#[derive(Debug, Clone)]
pub struct ReformDef {
    pub id: ReformId,
    pub name: String,
    pub category: GovernmentCategory,
}

/// Registry of all government types and reforms.
#[derive(Debug, Clone, Default)]
pub struct GovernmentRegistry {
    types: Vec<GovernmentTypeDef>,
    reforms: Vec<ReformDef>,
}

impl GovernmentRegistry {
    pub fn new() -> Self {
        let mut registry = Self::default();

        // Add hardcoded government types for Phase 0
        registry.types.push(GovernmentTypeDef {
            id: GovernmentTypeId::MONARCHY,
            name: "monarchy".to_string(),
            category: GovernmentCategory::Monarchy,
        });
        registry.types.push(GovernmentTypeDef {
            id: GovernmentTypeId::REPUBLIC,
            name: "republic".to_string(),
            category: GovernmentCategory::Republic,
        });
        registry.types.push(GovernmentTypeDef {
            id: GovernmentTypeId::THEOCRACY,
            name: "theocracy".to_string(),
            category: GovernmentCategory::Theocracy,
        });
        registry.types.push(GovernmentTypeDef {
            id: GovernmentTypeId::TRIBAL,
            name: "tribal".to_string(),
            category: GovernmentCategory::Tribal,
        });
        registry.types.push(GovernmentTypeDef {
            id: GovernmentTypeId::PIRATE_REPUBLIC,
            name: "pirate_republic".to_string(),
            category: GovernmentCategory::Republic,
        });
        registry.types.push(GovernmentTypeDef {
            id: GovernmentTypeId::NATIVE_COUNCIL,
            name: "native_council".to_string(),
            category: GovernmentCategory::Tribal,
        });

        registry
    }

    pub fn get_type(&self, id: GovernmentTypeId) -> Option<&GovernmentTypeDef> {
        self.types.get(id.0 as usize)
    }

    pub fn get_reform(&self, id: ReformId) -> Option<&ReformDef> {
        self.reforms.get(id.0 as usize)
    }
}

/// Government state for a single country.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountryGovernmentState {
    pub government_type: GovernmentTypeId,
    pub government_reforms: HashSet<ReformId>,
}

impl Default for CountryGovernmentState {
    fn default() -> Self {
        Self {
            government_type: GovernmentTypeId::MONARCHY, // Default to monarchy
            government_reforms: HashSet::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_government_registry_initialization() {
        let registry = GovernmentRegistry::new();

        // Verify well-known types exist
        assert!(registry.get_type(GovernmentTypeId::MONARCHY).is_some());
        assert!(registry.get_type(GovernmentTypeId::REPUBLIC).is_some());
        assert!(registry.get_type(GovernmentTypeId::THEOCRACY).is_some());
        assert!(registry
            .get_type(GovernmentTypeId::PIRATE_REPUBLIC)
            .is_some());
    }

    #[test]
    fn test_government_type_categories() {
        let registry = GovernmentRegistry::new();

        let monarchy = registry.get_type(GovernmentTypeId::MONARCHY).unwrap();
        assert_eq!(monarchy.category, GovernmentCategory::Monarchy);

        let theocracy = registry.get_type(GovernmentTypeId::THEOCRACY).unwrap();
        assert_eq!(theocracy.category, GovernmentCategory::Theocracy);
    }

    #[test]
    fn test_country_government_state_default() {
        let state = CountryGovernmentState::default();

        // Should default to monarchy with no reforms
        assert_eq!(state.government_type, GovernmentTypeId::MONARCHY);
        assert!(state.government_reforms.is_empty());
    }

    #[test]
    fn test_country_state_includes_government() {
        use crate::state::CountryState;

        let state = CountryState::default();

        // Verify government fields are present and default to monarchy
        assert_eq!(state.government_type, GovernmentTypeId::MONARCHY);
        assert!(state.government_reforms.is_empty());
    }
}
