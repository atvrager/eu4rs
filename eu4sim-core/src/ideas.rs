//! National ideas system for EU4 simulation.
//!
//! EU4 has two types of ideas:
//! - **Idea Groups**: Generic groups (Aristocratic, Quantity, etc.) any country can pick
//! - **National Ideas**: Country-specific ideas (FRA_ideas, TUR_ideas) auto-granted at start
//!
//! Each idea group contains 7 ideas plus completion bonuses.
//!
//! ## Modifier Stub Tracking
//!
//! Ideas reference 400+ modifier types. Most are not yet implemented in the sim.
//! The [`ModifierStubTracker`] accumulates unimplemented modifiers as they're
//! encountered, providing a roadmap for future mechanics.

use crate::fixed::Fixed;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type-safe idea group identifier.
///
/// Sequential IDs (0..N) for efficient lookup. EU4 has ~50 generic idea groups
/// plus ~400 national idea sets, so `u16` is sufficient.
#[derive(
    Hash, Eq, PartialEq, Clone, Copy, Debug, Default, Serialize, Deserialize, PartialOrd, Ord,
)]
pub struct IdeaGroupId(pub u16);

impl IdeaGroupId {
    /// Invalid/unknown idea group marker.
    pub const UNKNOWN: IdeaGroupId = IdeaGroupId(u16::MAX);
}

/// Idea group category (determines which mana type is spent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum IdeaCategory {
    #[default]
    Adm,
    Dip,
    Mil,
}

impl IdeaCategory {
    /// Parse from EU4 string ("ADM", "DIP", "MIL").
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "ADM" => Some(Self::Adm),
            "DIP" => Some(Self::Dip),
            "MIL" => Some(Self::Mil),
            _ => None,
        }
    }
}

/// A modifier entry with key and value.
///
/// Stores the raw modifier key (e.g., "cavalry_power", "global_manpower_modifier")
/// and its value as Fixed for determinism.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifierEntry {
    /// Modifier key from EU4: "cavalry_power", "global_tax_modifier", etc.
    pub key: String,
    /// Modifier value as Fixed for determinism.
    pub value: Fixed,
}

impl ModifierEntry {
    /// Create a new modifier entry.
    pub fn new(key: impl Into<String>, value: Fixed) -> Self {
        Self {
            key: key.into(),
            value,
        }
    }

    /// Create from f32 value (common case from parsing).
    pub fn from_f32(key: impl Into<String>, value: f32) -> Self {
        Self {
            key: key.into(),
            value: Fixed::from_f32(value),
        }
    }
}

/// A single idea within an idea group.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IdeaDef {
    /// Unique name within the group: "noble_knights", "serfdom", etc.
    pub name: String,
    /// Position in group (0-6 for regular ideas).
    pub position: u8,
    /// Modifiers granted by this idea.
    pub modifiers: Vec<ModifierEntry>,
}

/// Static definition of an idea group.
///
/// Loaded from `common/ideas/*.txt` at startup. Immutable after loading.
#[derive(Debug, Clone, Default)]
pub struct IdeaGroupDef {
    /// Unique identifier assigned at load time.
    pub id: IdeaGroupId,
    /// Name: "aristocracy_ideas", "quantity_ideas", "FRA_ideas", etc.
    pub name: String,
    /// Category for generic groups (None for national ideas).
    pub category: Option<IdeaCategory>,
    /// Is this a country-specific national idea group?
    pub is_national: bool,
    /// Tag requirement for national ideas (e.g., "FRA" for FRA_ideas).
    pub required_tag: Option<String>,
    /// "free = yes" means ideas are auto-granted (national ideas).
    pub is_free: bool,
    /// Start bonuses (granted immediately when group is picked).
    /// For national ideas, these apply at game start.
    pub start_modifiers: Vec<ModifierEntry>,
    /// Completion bonus (granted when all 7 ideas are unlocked).
    pub bonus_modifiers: Vec<ModifierEntry>,
    /// The 7 individual ideas.
    pub ideas: Vec<IdeaDef>,
    /// AI willingness factor (for future AI priority).
    pub ai_will_do_factor: Fixed,
}

impl IdeaGroupDef {
    /// Get total modifiers count across all ideas.
    pub fn total_modifier_count(&self) -> usize {
        self.start_modifiers.len()
            + self.bonus_modifiers.len()
            + self.ideas.iter().map(|i| i.modifiers.len()).sum::<usize>()
    }
}

/// Tracks which ideas a country has unlocked.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CountryIdeaState {
    /// Picked idea groups (up to 8 in vanilla EU4).
    /// Map from IdeaGroupId -> number of ideas unlocked (0-7).
    pub groups: HashMap<IdeaGroupId, u8>,
    /// National ideas group (if applicable).
    pub national_ideas: Option<IdeaGroupId>,
    /// Number of national ideas unlocked (0-7).
    pub national_ideas_progress: u8,
}

impl CountryIdeaState {
    /// Total number of ideas unlocked across all groups.
    pub fn total_ideas_unlocked(&self) -> u32 {
        let group_ideas: u32 = self.groups.values().map(|&n| n as u32).sum();
        let national_ideas = self.national_ideas_progress as u32;
        group_ideas + national_ideas
    }

    /// Check if a specific idea group has all 7 ideas unlocked.
    pub fn is_group_complete(&self, group_id: IdeaGroupId) -> bool {
        self.groups.get(&group_id).copied().unwrap_or(0) >= 7
    }

    /// Check if national ideas are complete.
    pub fn are_national_ideas_complete(&self) -> bool {
        self.national_ideas.is_some() && self.national_ideas_progress >= 7
    }
}

/// Raw idea data for building the registry.
///
/// Intermediate representation from parser before conversion to IdeaGroupDef.
#[derive(Debug, Clone, Default)]
pub struct RawIdeaGroup {
    pub name: String,
    pub category: Option<String>,
    pub is_free: bool,
    pub required_tag: Option<String>,
    pub start_modifiers: Vec<(String, f32)>,
    pub bonus_modifiers: Vec<(String, f32)>,
    pub ideas: Vec<RawIdea>,
    pub ai_will_do_factor: f32,
}

/// Raw idea from parser.
#[derive(Debug, Clone, Default)]
pub struct RawIdea {
    pub name: String,
    pub position: u8,
    pub modifiers: Vec<(String, f32)>,
}

/// Registry of all idea groups, populated from game files.
///
/// Provides O(1) lookup by ID and name. Well-known type IDs are cached
/// for fast path checks without string comparison.
#[derive(Debug, Clone, Default)]
pub struct IdeaGroupRegistry {
    /// All idea group definitions, indexed by ID.
    groups: Vec<IdeaGroupDef>,
    /// Name -> ID lookup for parsing.
    by_name: HashMap<String, IdeaGroupId>,

    // === Well-known IDs for fast checks ===
    /// Aristocracy idea group.
    pub aristocracy_id: IdeaGroupId,
    /// Quantity ideas.
    pub quantity_id: IdeaGroupId,
    /// Quality ideas.
    pub quality_id: IdeaGroupId,
    /// Economic ideas.
    pub economic_id: IdeaGroupId,
    /// Trade ideas.
    pub trade_id: IdeaGroupId,
    /// Default ideas (for countries without national ideas).
    pub default_id: IdeaGroupId,
}

impl IdeaGroupRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build registry from raw parsed idea groups.
    pub fn from_raw<I>(raw_groups: I) -> Self
    where
        I: IntoIterator<Item = RawIdeaGroup>,
    {
        let mut registry = Self::new();

        // Sort by name for deterministic ID assignment
        let mut sorted: Vec<RawIdeaGroup> = raw_groups.into_iter().collect();
        sorted.sort_by(|a, b| a.name.cmp(&b.name));

        for raw in sorted {
            let def = IdeaGroupDef {
                id: IdeaGroupId::UNKNOWN, // Will be set by add()
                name: raw.name.clone(),
                category: raw.category.as_deref().and_then(IdeaCategory::parse),
                is_national: raw.is_free || raw.required_tag.is_some(),
                required_tag: raw.required_tag,
                is_free: raw.is_free,
                start_modifiers: raw
                    .start_modifiers
                    .into_iter()
                    .map(|(k, v)| ModifierEntry::from_f32(k, v))
                    .collect(),
                bonus_modifiers: raw
                    .bonus_modifiers
                    .into_iter()
                    .map(|(k, v)| ModifierEntry::from_f32(k, v))
                    .collect(),
                ideas: raw
                    .ideas
                    .into_iter()
                    .map(|idea| IdeaDef {
                        name: idea.name,
                        position: idea.position,
                        modifiers: idea
                            .modifiers
                            .into_iter()
                            .map(|(k, v)| ModifierEntry::from_f32(k, v))
                            .collect(),
                    })
                    .collect(),
                ai_will_do_factor: Fixed::from_f32(raw.ai_will_do_factor),
            };
            registry.add(def);
        }

        registry
    }

    /// Add an idea group definition to the registry.
    ///
    /// Returns the assigned ID.
    pub fn add(&mut self, mut def: IdeaGroupDef) -> IdeaGroupId {
        let id = IdeaGroupId(self.groups.len() as u16);
        def.id = id;

        // Track well-known groups
        match def.name.as_str() {
            "aristocracy_ideas" => self.aristocracy_id = id,
            "quantity_ideas" => self.quantity_id = id,
            "quality_ideas" => self.quality_id = id,
            "economic_ideas" => self.economic_id = id,
            "trade_ideas" => self.trade_id = id,
            "default_ideas" => self.default_id = id,
            _ => {}
        }

        self.by_name.insert(def.name.clone(), id);
        self.groups.push(def);
        id
    }

    /// Get an idea group by ID.
    pub fn get(&self, id: IdeaGroupId) -> Option<&IdeaGroupDef> {
        self.groups.get(id.0 as usize)
    }

    /// Get an idea group by name.
    pub fn get_by_name(&self, name: &str) -> Option<&IdeaGroupDef> {
        self.by_name.get(name).and_then(|id| self.get(*id))
    }

    /// Look up a group ID by name.
    pub fn id_by_name(&self, name: &str) -> Option<IdeaGroupId> {
        self.by_name.get(name).copied()
    }

    /// Number of registered idea groups.
    pub fn len(&self) -> usize {
        self.groups.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    /// Iterate over all idea groups.
    pub fn iter(&self) -> impl Iterator<Item = &IdeaGroupDef> {
        self.groups.iter()
    }

    /// Get all generic (pickable) idea groups.
    pub fn generic_groups(&self) -> impl Iterator<Item = &IdeaGroupDef> {
        self.groups.iter().filter(|g| !g.is_national)
    }

    /// Get all national idea groups.
    pub fn national_groups(&self) -> impl Iterator<Item = &IdeaGroupDef> {
        self.groups.iter().filter(|g| g.is_national)
    }

    /// Get national ideas for a specific country tag.
    pub fn national_ideas_for(&self, tag: &str) -> Option<&IdeaGroupDef> {
        // Try direct match first (FRA_ideas for FRA)
        let ideas_name = format!("{}_ideas", tag);
        if let Some(group) = self.get_by_name(&ideas_name) {
            return Some(group);
        }

        // Fall back to required_tag match
        self.groups
            .iter()
            .find(|g| g.is_national && g.required_tag.as_deref() == Some(tag))
    }

    /// Count total modifiers across all idea groups.
    pub fn total_modifier_count(&self) -> usize {
        self.groups.iter().map(|g| g.total_modifier_count()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_registry() -> IdeaGroupRegistry {
        let mut registry = IdeaGroupRegistry::new();

        // Add aristocracy ideas (generic group)
        registry.add(IdeaGroupDef {
            name: "aristocracy_ideas".into(),
            category: Some(IdeaCategory::Mil),
            is_national: false,
            bonus_modifiers: vec![ModifierEntry::from_f32("cavalry_power", 0.10)],
            ideas: vec![
                IdeaDef {
                    name: "noble_knights".into(),
                    position: 0,
                    modifiers: vec![ModifierEntry::from_f32("cavalry_power", 0.10)],
                },
                IdeaDef {
                    name: "local_nobility".into(),
                    position: 1,
                    modifiers: vec![
                        ModifierEntry::from_f32("legitimacy", 1.0),
                        ModifierEntry::from_f32("monthly_heir_claim_increase", 0.05),
                    ],
                },
            ],
            ..Default::default()
        });

        // Add French national ideas
        registry.add(IdeaGroupDef {
            name: "FRA_ideas".into(),
            is_national: true,
            required_tag: Some("FRA".into()),
            is_free: true,
            start_modifiers: vec![ModifierEntry::from_f32("diplomatic_upkeep", 1.0)],
            bonus_modifiers: vec![ModifierEntry::from_f32("diplomatic_reputation", 2.0)],
            ideas: vec![IdeaDef {
                name: "french_language_in_all_courts".into(),
                position: 0,
                modifiers: vec![ModifierEntry::from_f32("diplomatic_reputation", 2.0)],
            }],
            ..Default::default()
        });

        registry
    }

    #[test]
    fn test_idea_group_id() {
        assert_eq!(IdeaGroupId(0), IdeaGroupId(0));
        assert_ne!(IdeaGroupId(0), IdeaGroupId(1));
        assert_eq!(IdeaGroupId::UNKNOWN.0, u16::MAX);
    }

    #[test]
    fn test_idea_category_from_str() {
        assert_eq!(IdeaCategory::parse("ADM"), Some(IdeaCategory::Adm));
        assert_eq!(IdeaCategory::parse("dip"), Some(IdeaCategory::Dip));
        assert_eq!(IdeaCategory::parse("MIL"), Some(IdeaCategory::Mil));
        assert_eq!(IdeaCategory::parse("invalid"), None);
    }

    #[test]
    fn test_registry_lookup() {
        let registry = make_test_registry();

        assert!(registry.get_by_name("aristocracy_ideas").is_some());
        assert!(registry.get_by_name("FRA_ideas").is_some());
        assert!(registry.get_by_name("nonexistent").is_none());
    }

    #[test]
    fn test_national_ideas_for() {
        let registry = make_test_registry();

        let fra = registry.national_ideas_for("FRA");
        assert!(fra.is_some());
        assert_eq!(fra.unwrap().name, "FRA_ideas");

        // No national ideas for random tag
        assert!(registry.national_ideas_for("XXX").is_none());
    }

    #[test]
    fn test_generic_vs_national() {
        let registry = make_test_registry();

        let generic: Vec<_> = registry.generic_groups().collect();
        let national: Vec<_> = registry.national_groups().collect();

        assert_eq!(generic.len(), 1); // aristocracy
        assert_eq!(national.len(), 1); // FRA_ideas
        assert_eq!(generic[0].name, "aristocracy_ideas");
        assert_eq!(national[0].name, "FRA_ideas");
    }

    #[test]
    fn test_country_idea_state() {
        let mut state = CountryIdeaState::default();
        let group_id = IdeaGroupId(0);

        // Start with no ideas
        assert_eq!(state.total_ideas_unlocked(), 0);
        assert!(!state.is_group_complete(group_id));

        // Unlock some ideas
        state.groups.insert(group_id, 3);
        assert_eq!(state.total_ideas_unlocked(), 3);
        assert!(!state.is_group_complete(group_id));

        // Complete the group
        state.groups.insert(group_id, 7);
        assert!(state.is_group_complete(group_id));

        // Add national ideas
        state.national_ideas = Some(IdeaGroupId(1));
        state.national_ideas_progress = 7;
        assert!(state.are_national_ideas_complete());
        assert_eq!(state.total_ideas_unlocked(), 14);
    }

    #[test]
    fn test_modifier_entry() {
        let entry = ModifierEntry::from_f32("cavalry_power", 0.10);
        assert_eq!(entry.key, "cavalry_power");
        assert_eq!(entry.value, Fixed::from_f32(0.10));
    }

    #[test]
    fn test_total_modifier_count() {
        let registry = make_test_registry();

        // aristocracy: 0 start + 1 bonus + 3 ideas = 4
        // FRA: 1 start + 1 bonus + 1 idea = 3
        // Total = 7
        assert_eq!(registry.total_modifier_count(), 7);
    }
}
