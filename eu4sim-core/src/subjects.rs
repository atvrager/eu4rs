//! Subject type definitions and registry.
//!
//! EU4 has 26+ subject types (vassal, march, appanage, daimyo_vassal, eyalet, etc.)
//! loaded from `common/subject_types/`. This module provides:
//!
//! - [`SubjectTypeId`]: Type-safe identifier (like `BuildingId`)
//! - [`SubjectTypeDef`]: Static definition loaded from game files
//! - [`SubjectTypeRegistry`]: Collection of all subject types with name lookup
//!
//! ## Inheritance
//!
//! Subject types can inherit from others via `copy_from` and declare equivalence
//! via `count`. For example, `appanage` has `count = vassal`, meaning it "counts as"
//! a vassal for most game triggers.

use crate::fixed::Fixed;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type-safe subject type identifier.
///
/// Sequential IDs (0..N) assigned at load time. EU4 has ~30 subject types,
/// so `u8` is sufficient with room for mods like Anbennar.
#[derive(
    Hash, Eq, PartialEq, Clone, Copy, Debug, Default, Serialize, Deserialize, PartialOrd, Ord,
)]
pub struct SubjectTypeId(pub u8);

impl SubjectTypeId {
    /// Invalid/unknown subject type marker.
    pub const UNKNOWN: SubjectTypeId = SubjectTypeId(u8::MAX);
}

/// Static subject type definition loaded from `common/subject_types/`.
///
/// These are immutable after loading and shared across all diplomatic relationships.
#[derive(Debug, Clone)]
pub struct SubjectTypeDef {
    /// Unique identifier assigned at load time.
    pub id: SubjectTypeId,
    /// Name from game files: "vassal", "appanage", "daimyo_vassal", etc.
    pub name: String,

    // === Inheritance ===
    /// Parent type this copies properties from (`copy_from = vassal`).
    pub copy_from: Option<SubjectTypeId>,
    /// Type this "counts as" for game triggers (`count = vassal`).
    /// Appanage counts as vassal, client_march counts as march, etc.
    pub counts_as: Option<SubjectTypeId>,

    // === War behavior ===
    /// Subject auto-joins overlord's wars (default true for most types).
    pub joins_overlords_wars: bool,
    /// Whether overlord can be called to defend subject (tributaries: optional).
    pub overlord_protects_external: bool,
    /// Subject can declare independence war.
    pub can_fight_independence_war: bool,

    // === Diplomacy ===
    /// Uses one of overlord's diplomatic relation slots.
    pub takes_diplo_slot: bool,
    /// Can be diplomatically integrated/annexed.
    pub can_be_integrated: bool,
    /// Shares overlord's ruler (personal unions).
    pub has_overlords_ruler: bool,
    /// Subject can leave relationship voluntarily (tributaries).
    pub is_voluntary: bool,

    // === Liberty desire ===
    /// Base liberty desire modifier (-15 for march, +10 for daimyo, etc.).
    pub base_liberty_desire: i8,
    /// Liberty desire per development ratio (subject_dev / overlord_dev).
    pub liberty_desire_development_ratio: Fixed,

    // === Income ===
    /// Fraction of income paid to overlord (1.0 for vassals, 0.0 for marches).
    pub pays_overlord: Fixed,
    /// Fraction of subject's forcelimit added to overlord's.
    pub forcelimit_to_overlord: Fixed,
}

impl Default for SubjectTypeDef {
    fn default() -> Self {
        // Matches EU4's "default" subject type template
        Self {
            id: SubjectTypeId::UNKNOWN,
            name: String::new(),
            copy_from: None,
            counts_as: None,
            joins_overlords_wars: true,
            overlord_protects_external: false,
            can_fight_independence_war: true,
            takes_diplo_slot: true,
            can_be_integrated: false,
            has_overlords_ruler: false,
            is_voluntary: false,
            base_liberty_desire: 0,
            liberty_desire_development_ratio: Fixed::ZERO,
            pays_overlord: Fixed::ZERO,
            forcelimit_to_overlord: Fixed::ZERO,
        }
    }
}

impl SubjectTypeDef {
    /// Check if this type "counts as" another for game mechanics.
    ///
    /// Recursively checks the `counts_as` chain. For example:
    /// - `appanage.counts_as(vassal) -> true` (count = vassal)
    /// - `vassal.counts_as(vassal) -> true` (identity)
    /// - `tributary.counts_as(vassal) -> false`
    pub fn counts_as_type(&self, other: SubjectTypeId, registry: &SubjectTypeRegistry) -> bool {
        if self.id == other {
            return true;
        }
        match self.counts_as {
            Some(parent_id) if parent_id == other => true,
            Some(parent_id) => registry
                .get(parent_id)
                .is_some_and(|parent| parent.counts_as_type(other, registry)),
            None => false,
        }
    }

    /// Whether this subject type joins overlord's offensive wars.
    ///
    /// Same as `joins_overlords_wars` - offensive and defensive typically match.
    pub fn joins_offensive_wars(&self) -> bool {
        self.joins_overlords_wars
    }

    /// Whether this subject type joins overlord's defensive wars.
    ///
    /// Most subjects join defensive wars; tributaries are the main exception.
    pub fn joins_defensive_wars(&self) -> bool {
        // Tributaries don't auto-join wars (overlord can optionally defend them)
        self.joins_overlords_wars
    }
}

/// Raw subject type data for building the registry.
///
/// This is a plain struct rather than a trait to avoid orphan rule issues
/// when the parser is in a different crate.
#[derive(Debug, Clone, Default)]
pub struct RawSubjectType {
    pub name: String,
    pub copy_from: Option<String>,
    pub count: Option<String>,
    pub joins_overlords_wars: Option<bool>,
    pub overlord_protects_external: Option<bool>,
    pub can_fight_independence_war: Option<bool>,
    pub takes_diplo_slot: Option<bool>,
    pub can_be_integrated: Option<bool>,
    pub has_overlords_ruler: Option<bool>,
    pub is_voluntary: Option<bool>,
    pub base_liberty_desire: Option<f32>,
    pub liberty_desire_development_ratio: Option<f32>,
    pub pays_overlord: Option<f32>,
    pub forcelimit_to_overlord: Option<f32>,
}

/// Registry of all subject types, populated from game files.
///
/// Provides O(1) lookup by ID and name. Well-known type IDs are cached
/// for fast path checks without string comparison.
#[derive(Debug, Clone, Default)]
pub struct SubjectTypeRegistry {
    /// All subject type definitions, indexed by ID.
    types: Vec<SubjectTypeDef>,
    /// Name -> ID lookup for parsing.
    by_name: HashMap<String, SubjectTypeId>,

    // === Well-known IDs for fast checks ===
    /// Vassal type ID (most common subject).
    pub vassal_id: SubjectTypeId,
    /// March type ID (military buffer, no taxes).
    pub march_id: SubjectTypeId,
    /// Personal union type ID (shared ruler).
    pub personal_union_id: SubjectTypeId,
    /// Colony type ID (colonial nations).
    pub colony_id: SubjectTypeId,
    /// Tributary type ID (loose subject, doesn't join wars).
    pub tributary_id: SubjectTypeId,
}

impl SubjectTypeRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build registry from raw parsed subject types.
    ///
    /// This resolves `copy_from` inheritance and converts string references
    /// to type IDs.
    pub fn from_raw<I>(raw_types: I) -> Self
    where
        I: IntoIterator<Item = RawSubjectType>,
    {
        let raw_vec: Vec<_> = raw_types.into_iter().collect();

        // First pass: create all types without resolving inheritance
        // We need all names registered before we can resolve copy_from/count
        let mut registry = Self::new();
        let mut pending: Vec<(usize, RawSubjectType)> = Vec::new();

        // Sort by name for deterministic ID assignment
        let mut sorted: Vec<RawSubjectType> = raw_vec;
        sorted.sort_by(|a, b| a.name.cmp(&b.name));

        for raw in sorted {
            // Create minimal entry with just the name
            let def = SubjectTypeDef {
                name: raw.name.clone(),
                ..Default::default()
            };
            let _id = registry.add(def);
            pending.push((registry.types.len() - 1, raw));
        }

        // Second pass: resolve inheritance and populate fields
        for (idx, raw) in pending {
            let copy_from_id = raw
                .copy_from
                .as_deref()
                .and_then(|name| registry.id_by_name(name));
            let counts_as_id = raw
                .count
                .as_deref()
                .and_then(|name| registry.id_by_name(name));

            // Start with defaults from copy_from or global defaults
            let base = copy_from_id
                .and_then(|id| registry.get(id))
                .cloned()
                .unwrap_or_default();

            // Apply this type's overrides
            let def = &mut registry.types[idx];
            def.copy_from = copy_from_id;
            def.counts_as = counts_as_id;

            // Inherit from base, then override with explicit values
            def.joins_overlords_wars = raw
                .joins_overlords_wars
                .unwrap_or(base.joins_overlords_wars);
            def.overlord_protects_external = raw
                .overlord_protects_external
                .unwrap_or(base.overlord_protects_external);
            def.can_fight_independence_war = raw
                .can_fight_independence_war
                .unwrap_or(base.can_fight_independence_war);
            def.takes_diplo_slot = raw.takes_diplo_slot.unwrap_or(base.takes_diplo_slot);
            def.can_be_integrated = raw.can_be_integrated.unwrap_or(base.can_be_integrated);
            def.has_overlords_ruler = raw.has_overlords_ruler.unwrap_or(base.has_overlords_ruler);
            def.is_voluntary = raw.is_voluntary.unwrap_or(base.is_voluntary);
            def.base_liberty_desire =
                raw.base_liberty_desire
                    .unwrap_or(base.base_liberty_desire as f32) as i8;
            def.liberty_desire_development_ratio = raw
                .liberty_desire_development_ratio
                .map(Fixed::from_f32)
                .unwrap_or(base.liberty_desire_development_ratio);
            def.pays_overlord = raw
                .pays_overlord
                .map(Fixed::from_f32)
                .unwrap_or(base.pays_overlord);
            def.forcelimit_to_overlord = raw
                .forcelimit_to_overlord
                .map(Fixed::from_f32)
                .unwrap_or(base.forcelimit_to_overlord);
        }

        registry
    }

    /// Add a subject type definition to the registry.
    ///
    /// Returns the assigned ID.
    pub fn add(&mut self, mut def: SubjectTypeDef) -> SubjectTypeId {
        let id = SubjectTypeId(self.types.len() as u8);
        def.id = id;

        // Track well-known types
        match def.name.as_str() {
            "vassal" => self.vassal_id = id,
            "march" => self.march_id = id,
            "personal_union" => self.personal_union_id = id,
            "colony" => self.colony_id = id,
            "tributary_state" => self.tributary_id = id,
            _ => {}
        }

        self.by_name.insert(def.name.clone(), id);
        self.types.push(def);
        id
    }

    /// Get a subject type by ID.
    pub fn get(&self, id: SubjectTypeId) -> Option<&SubjectTypeDef> {
        self.types.get(id.0 as usize)
    }

    /// Get a subject type by name.
    pub fn get_by_name(&self, name: &str) -> Option<&SubjectTypeDef> {
        self.by_name.get(name).and_then(|id| self.get(*id))
    }

    /// Look up a type ID by name.
    pub fn id_by_name(&self, name: &str) -> Option<SubjectTypeId> {
        self.by_name.get(name).copied()
    }

    /// Number of registered subject types.
    pub fn len(&self) -> usize {
        self.types.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    /// Iterate over all subject types.
    pub fn iter(&self) -> impl Iterator<Item = &SubjectTypeDef> {
        self.types.iter()
    }

    /// Check if a subject type is tributary-like (doesn't join overlord wars).
    pub fn is_tributary(&self, id: SubjectTypeId) -> bool {
        self.get(id).is_some_and(|def| !def.joins_overlords_wars)
    }

    /// Find the first tributary-type subject type ID.
    ///
    /// Returns None if no tributary types are registered.
    /// Used when creating new tributary relationships.
    pub fn find_tributary_type(&self) -> Option<SubjectTypeId> {
        for (idx, def) in self.types.iter().enumerate() {
            if !def.joins_overlords_wars {
                return Some(SubjectTypeId(idx as u8));
            }
        }
        // Fallback: look for a type named "tributary"
        self.id_by_name("tributary_state")
            .or_else(|| self.id_by_name("tributary"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_registry() -> SubjectTypeRegistry {
        let mut registry = SubjectTypeRegistry::new();

        // Add vassal
        registry.add(SubjectTypeDef {
            name: "vassal".into(),
            joins_overlords_wars: true,
            takes_diplo_slot: true,
            can_be_integrated: true,
            liberty_desire_development_ratio: Fixed::from_f32(0.25),
            pays_overlord: Fixed::ONE,
            forcelimit_to_overlord: Fixed::from_f32(0.1),
            ..Default::default()
        });

        // Add march (inherits from vassal)
        let vassal_id = registry.vassal_id;
        registry.add(SubjectTypeDef {
            name: "march".into(),
            copy_from: Some(vassal_id),
            counts_as: Some(vassal_id),
            joins_overlords_wars: true,
            can_be_integrated: false,
            base_liberty_desire: -15,
            pays_overlord: Fixed::ZERO,
            forcelimit_to_overlord: Fixed::from_f32(0.2),
            ..Default::default()
        });

        // Add tributary (doesn't join wars)
        registry.add(SubjectTypeDef {
            name: "tributary_state".into(),
            joins_overlords_wars: false,
            is_voluntary: true,
            can_fight_independence_war: true,
            ..Default::default()
        });

        // Add appanage (counts as vassal)
        registry.add(SubjectTypeDef {
            name: "appanage".into(),
            counts_as: Some(vassal_id),
            joins_overlords_wars: true,
            base_liberty_desire: 35,
            ..Default::default()
        });

        registry
    }

    #[test]
    fn test_well_known_ids() {
        let registry = make_test_registry();

        assert_eq!(registry.vassal_id, SubjectTypeId(0));
        assert_eq!(registry.march_id, SubjectTypeId(1));
        assert_eq!(registry.tributary_id, SubjectTypeId(2));
    }

    #[test]
    fn test_lookup_by_name() {
        let registry = make_test_registry();

        assert!(registry.get_by_name("vassal").is_some());
        assert!(registry.get_by_name("march").is_some());
        assert!(registry.get_by_name("appanage").is_some());
        assert!(registry.get_by_name("nonexistent").is_none());
    }

    #[test]
    fn test_counts_as() {
        let registry = make_test_registry();

        let march = registry.get_by_name("march").unwrap();
        let appanage = registry.get_by_name("appanage").unwrap();
        let tributary = registry.get_by_name("tributary_state").unwrap();

        // March counts as vassal
        assert!(march.counts_as_type(registry.vassal_id, &registry));
        // Appanage counts as vassal
        assert!(appanage.counts_as_type(registry.vassal_id, &registry));
        // Tributary does NOT count as vassal
        assert!(!tributary.counts_as_type(registry.vassal_id, &registry));
    }

    #[test]
    fn test_is_tributary() {
        let registry = make_test_registry();

        assert!(!registry.is_tributary(registry.vassal_id));
        assert!(!registry.is_tributary(registry.march_id));
        assert!(registry.is_tributary(registry.tributary_id));
    }

    #[test]
    fn test_joins_wars() {
        let registry = make_test_registry();

        let vassal = registry.get(registry.vassal_id).unwrap();
        let tributary = registry.get(registry.tributary_id).unwrap();

        assert!(vassal.joins_offensive_wars());
        assert!(vassal.joins_defensive_wars());
        assert!(!tributary.joins_offensive_wars());
        assert!(!tributary.joins_defensive_wars());
    }
}
