//! Policy system - synergies between idea groups.
//!
//! Policies combine two idea groups to provide additional modifiers.
//! Countries can enable policies if they have both required idea groups fully unlocked.

use crate::ideas::ModifierEntry;
use crate::modifiers::GameModifiers;
use crate::state::{HashMap, Tag};
use serde::{Deserialize, Serialize};

/// Type-safe policy identifier.
#[derive(Hash, Eq, PartialEq, Clone, Copy, Debug, Serialize, Deserialize, PartialOrd, Ord)]
pub struct PolicyId(pub u16);

/// Policy category (monarch power type).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyCategory {
    Administrative,
    Diplomatic,
    Military,
}

/// Static policy definition loaded from game files.
#[derive(Debug, Clone)]
pub struct PolicyDef {
    pub id: PolicyId,
    pub name: String,
    pub category: PolicyCategory,

    /// First required idea group (e.g., "economic_ideas")
    pub idea_group_1: String,
    /// Second required idea group (e.g., "quality_ideas")
    pub idea_group_2: String,

    /// Modifiers granted by this policy
    pub modifiers: Vec<ModifierEntry>,
}

/// Registry of all policies.
#[derive(Debug, Clone, Default)]
pub struct PolicyRegistry {
    policies: HashMap<PolicyId, PolicyDef>,
    by_name: HashMap<String, PolicyId>,
}

impl PolicyRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, policy: PolicyDef) {
        self.by_name.insert(policy.name.clone(), policy.id);
        self.policies.insert(policy.id, policy);
    }

    pub fn get(&self, id: PolicyId) -> Option<&PolicyDef> {
        self.policies.get(&id)
    }

    pub fn get_by_name(&self, name: &str) -> Option<&PolicyDef> {
        self.by_name.get(name).and_then(|id| self.policies.get(id))
    }

    pub fn len(&self) -> usize {
        self.policies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.policies.is_empty()
    }
}

/// Apply modifiers from all enabled policies for a country.
pub fn apply_policy_modifiers(
    tag: &Tag,
    enabled_policies: &[PolicyId],
    policy_registry: &PolicyRegistry,
    modifiers: &mut GameModifiers,
) {
    for policy_id in enabled_policies {
        if let Some(policy) = policy_registry.get(*policy_id) {
            for modifier in &policy.modifiers {
                crate::systems::ideas::apply_modifier(
                    modifiers,
                    tag,
                    modifier,
                    &crate::systems::ideas::ModifierStubTracker::new(),
                );
            }
        }
    }
}

/// Calculate number of policy slots based on completed idea groups.
///
/// In EU4: +1 policy slot per completed idea group.
pub fn calculate_policy_slots(idea_state: &crate::ideas::CountryIdeaState) -> u8 {
    let completed = idea_state
        .groups
        .iter()
        .filter(|(_, &unlocked_count)| unlocked_count >= 7)
        .count();

    completed as u8
}

/// Error returned when policy operations fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    /// Policy not found in registry.
    PolicyNotFound,
    /// Policy already enabled.
    AlreadyEnabled,
    /// Policy not currently enabled.
    NotEnabled,
    /// No available policy slots.
    NoAvailableSlots,
    /// Missing required idea group.
    MissingIdeaGroup(String),
    /// Idea group not fully unlocked (need 7 ideas).
    IdeaGroupNotComplete(String),
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PolicyNotFound => write!(f, "Policy not found"),
            Self::AlreadyEnabled => write!(f, "Policy already enabled"),
            Self::NotEnabled => write!(f, "Policy not enabled"),
            Self::NoAvailableSlots => write!(f, "No available policy slots"),
            Self::MissingIdeaGroup(group) => write!(f, "Missing idea group: {}", group),
            Self::IdeaGroupNotComplete(group) => write!(f, "Idea group not complete: {}", group),
        }
    }
}

impl std::error::Error for PolicyError {}

/// Check if country can enable a policy (has both required idea groups fully unlocked).
pub fn can_enable_policy(
    policy: &PolicyDef,
    idea_state: &crate::ideas::CountryIdeaState,
    enabled_policies: &[PolicyId],
    policy_slots: u8,
    idea_registry: &crate::ideas::IdeaGroupRegistry,
) -> Result<(), PolicyError> {
    // Check if already enabled
    if enabled_policies.contains(&policy.id) {
        return Err(PolicyError::AlreadyEnabled);
    }

    // Check if slot available
    if enabled_policies.len() >= policy_slots as usize {
        return Err(PolicyError::NoAvailableSlots);
    }

    // Look up idea group IDs from string names
    let group_1_id = idea_registry
        .get_by_name(&policy.idea_group_1)
        .ok_or_else(|| PolicyError::MissingIdeaGroup(policy.idea_group_1.clone()))?
        .id;
    let group_2_id = idea_registry
        .get_by_name(&policy.idea_group_2)
        .ok_or_else(|| PolicyError::MissingIdeaGroup(policy.idea_group_2.clone()))?
        .id;

    // Check if both idea groups are unlocked
    let group_1_count = idea_state.groups.get(&group_1_id).copied().unwrap_or(0);
    let group_2_count = idea_state.groups.get(&group_2_id).copied().unwrap_or(0);

    if group_1_count == 0 {
        return Err(PolicyError::MissingIdeaGroup(policy.idea_group_1.clone()));
    }
    if group_2_count == 0 {
        return Err(PolicyError::MissingIdeaGroup(policy.idea_group_2.clone()));
    }

    // Check if both are fully unlocked (7 ideas each)
    if group_1_count < 7 {
        return Err(PolicyError::IdeaGroupNotComplete(
            policy.idea_group_1.clone(),
        ));
    }
    if group_2_count < 7 {
        return Err(PolicyError::IdeaGroupNotComplete(
            policy.idea_group_2.clone(),
        ));
    }

    Ok(())
}

/// Enable a policy for a country.
pub fn enable_policy(
    policy_id: PolicyId,
    country: &mut crate::state::CountryState,
    policy_registry: &PolicyRegistry,
    idea_registry: &crate::ideas::IdeaGroupRegistry,
) -> Result<(), PolicyError> {
    let policy = policy_registry
        .get(policy_id)
        .ok_or(PolicyError::PolicyNotFound)?;

    can_enable_policy(
        policy,
        &country.ideas,
        &country.enabled_policies,
        country.policy_slots,
        idea_registry,
    )?;

    country.enabled_policies.push(policy_id);
    Ok(())
}

/// Disable a policy for a country.
pub fn disable_policy(
    policy_id: PolicyId,
    country: &mut crate::state::CountryState,
) -> Result<(), PolicyError> {
    let pos = country
        .enabled_policies
        .iter()
        .position(|&id| id == policy_id)
        .ok_or(PolicyError::NotEnabled)?;

    country.enabled_policies.remove(pos);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_registry() {
        let mut registry = PolicyRegistry::new();

        let policy = PolicyDef {
            id: PolicyId(0),
            name: "test_policy".to_string(),
            category: PolicyCategory::Administrative,
            idea_group_1: "economic_ideas".to_string(),
            idea_group_2: "quality_ideas".to_string(),
            modifiers: vec![],
        };

        registry.register(policy);

        assert_eq!(registry.len(), 1);
        assert!(registry.get(PolicyId(0)).is_some());
        assert!(registry.get_by_name("test_policy").is_some());
    }

    #[test]
    fn test_apply_policy_modifiers() {
        use crate::fixed::Fixed;
        use crate::fixed_generic::Mod32;

        let mut registry = PolicyRegistry::new();
        let mut modifiers = GameModifiers::default();

        let policy = PolicyDef {
            id: PolicyId(0),
            name: "test_policy".to_string(),
            category: PolicyCategory::Administrative,
            idea_group_1: "economic_ideas".to_string(),
            idea_group_2: "quality_ideas".to_string(),
            modifiers: vec![ModifierEntry::new("discipline", Fixed::from_f32(0.05))],
        };

        registry.register(policy);

        let tag = "TST".to_string();
        let enabled = vec![PolicyId(0)];

        apply_policy_modifiers(&tag, &enabled, &registry, &mut modifiers);

        assert_eq!(
            modifiers.country_discipline.get(&tag),
            Some(&Mod32::from_f32(0.05))
        );
    }

    #[test]
    fn test_calculate_policy_slots() {
        use crate::ideas::{CountryIdeaState, IdeaGroupId};

        let mut idea_state = CountryIdeaState::default();

        // No completed groups = 0 slots
        assert_eq!(calculate_policy_slots(&idea_state), 0);

        // 1 completed group (7 ideas) = 1 slot
        idea_state.groups.insert(IdeaGroupId(1), 7);
        assert_eq!(calculate_policy_slots(&idea_state), 1);

        // 2 completed groups = 2 slots
        idea_state.groups.insert(IdeaGroupId(2), 7);
        assert_eq!(calculate_policy_slots(&idea_state), 2);

        // Incomplete group doesn't count
        idea_state.groups.insert(IdeaGroupId(3), 5);
        assert_eq!(calculate_policy_slots(&idea_state), 2);
    }

    // Helper to create test fixtures
    fn create_test_registry() -> crate::ideas::IdeaGroupRegistry {
        use crate::ideas::{IdeaCategory, IdeaDef, IdeaGroupDef, IdeaGroupId, IdeaGroupRegistry};

        let mut registry = IdeaGroupRegistry::default();

        // Register economic_ideas
        let economic = IdeaGroupDef {
            id: IdeaGroupId(0), // Will be reassigned by add()
            name: "economic_ideas".to_string(),
            category: Some(IdeaCategory::Adm),
            is_national: false,
            required_tag: None,
            is_free: false,
            ideas: (0..7)
                .map(|i| IdeaDef {
                    name: format!("economic_{}", i),
                    position: i,
                    modifiers: vec![],
                })
                .collect(),
            start_modifiers: vec![],
            bonus_modifiers: vec![],
            ai_will_do_factor: crate::fixed::Fixed::ONE,
        };
        registry.add(economic);

        // Register quality_ideas
        let quality = IdeaGroupDef {
            id: IdeaGroupId(0), // Will be reassigned by add()
            name: "quality_ideas".to_string(),
            category: Some(IdeaCategory::Mil),
            is_national: false,
            required_tag: None,
            is_free: false,
            ideas: (0..7)
                .map(|i| IdeaDef {
                    name: format!("quality_{}", i),
                    position: i,
                    modifiers: vec![],
                })
                .collect(),
            start_modifiers: vec![],
            bonus_modifiers: vec![],
            ai_will_do_factor: crate::fixed::Fixed::ONE,
        };
        registry.add(quality);

        registry
    }

    #[test]
    fn test_can_enable_policy_success() {
        use crate::ideas::{CountryIdeaState, IdeaGroupId};

        let registry = create_test_registry();
        let mut idea_state = CountryIdeaState::default();
        idea_state.groups.insert(IdeaGroupId(0), 7); // economic_ideas
        idea_state.groups.insert(IdeaGroupId(1), 7); // quality_ideas

        let policy = PolicyDef {
            id: PolicyId(0),
            name: "test_policy".to_string(),
            category: PolicyCategory::Administrative,
            idea_group_1: "economic_ideas".to_string(),
            idea_group_2: "quality_ideas".to_string(),
            modifiers: vec![],
        };

        let enabled = vec![];
        let slots = 2;

        assert!(can_enable_policy(&policy, &idea_state, &enabled, slots, &registry).is_ok());
    }

    #[test]
    fn test_can_enable_policy_missing_idea_group() {
        use crate::ideas::{CountryIdeaState, IdeaGroupId};

        let registry = create_test_registry();
        let mut idea_state = CountryIdeaState::default();
        idea_state.groups.insert(IdeaGroupId(0), 7); // economic_ideas
                                                     // quality_ideas not unlocked

        let policy = PolicyDef {
            id: PolicyId(0),
            name: "test_policy".to_string(),
            category: PolicyCategory::Administrative,
            idea_group_1: "economic_ideas".to_string(),
            idea_group_2: "quality_ideas".to_string(),
            modifiers: vec![],
        };

        let enabled = vec![];
        let slots = 2;

        assert_eq!(
            can_enable_policy(&policy, &idea_state, &enabled, slots, &registry),
            Err(PolicyError::MissingIdeaGroup("quality_ideas".to_string()))
        );
    }

    #[test]
    fn test_can_enable_policy_incomplete_idea_group() {
        use crate::ideas::{CountryIdeaState, IdeaGroupId};

        let registry = create_test_registry();
        let mut idea_state = CountryIdeaState::default();
        idea_state.groups.insert(IdeaGroupId(0), 7); // economic_ideas
        idea_state.groups.insert(IdeaGroupId(1), 5); // quality_ideas - only 5/7

        let policy = PolicyDef {
            id: PolicyId(0),
            name: "test_policy".to_string(),
            category: PolicyCategory::Administrative,
            idea_group_1: "economic_ideas".to_string(),
            idea_group_2: "quality_ideas".to_string(),
            modifiers: vec![],
        };

        let enabled = vec![];
        let slots = 2;

        assert_eq!(
            can_enable_policy(&policy, &idea_state, &enabled, slots, &registry),
            Err(PolicyError::IdeaGroupNotComplete(
                "quality_ideas".to_string()
            ))
        );
    }

    #[test]
    fn test_can_enable_policy_no_slots() {
        use crate::ideas::{CountryIdeaState, IdeaGroupId};

        let registry = create_test_registry();
        let mut idea_state = CountryIdeaState::default();
        idea_state.groups.insert(IdeaGroupId(0), 7); // economic_ideas
        idea_state.groups.insert(IdeaGroupId(1), 7); // quality_ideas

        let policy = PolicyDef {
            id: PolicyId(0),
            name: "test_policy".to_string(),
            category: PolicyCategory::Administrative,
            idea_group_1: "economic_ideas".to_string(),
            idea_group_2: "quality_ideas".to_string(),
            modifiers: vec![],
        };

        let enabled = vec![PolicyId(1), PolicyId(2)]; // 2 policies enabled
        let slots = 2; // Only 2 slots

        assert_eq!(
            can_enable_policy(&policy, &idea_state, &enabled, slots, &registry),
            Err(PolicyError::NoAvailableSlots)
        );
    }

    #[test]
    fn test_can_enable_policy_already_enabled() {
        use crate::ideas::{CountryIdeaState, IdeaGroupId};

        let registry = create_test_registry();
        let mut idea_state = CountryIdeaState::default();
        idea_state.groups.insert(IdeaGroupId(0), 7); // economic_ideas
        idea_state.groups.insert(IdeaGroupId(1), 7); // quality_ideas

        let policy = PolicyDef {
            id: PolicyId(0),
            name: "test_policy".to_string(),
            category: PolicyCategory::Administrative,
            idea_group_1: "economic_ideas".to_string(),
            idea_group_2: "quality_ideas".to_string(),
            modifiers: vec![],
        };

        let enabled = vec![PolicyId(0)]; // Already enabled
        let slots = 2;

        assert_eq!(
            can_enable_policy(&policy, &idea_state, &enabled, slots, &registry),
            Err(PolicyError::AlreadyEnabled)
        );
    }

    #[test]
    fn test_enable_disable_policy() {
        use crate::ideas::IdeaGroupId;

        let idea_registry = create_test_registry();
        let mut policy_registry = PolicyRegistry::new();
        let policy = PolicyDef {
            id: PolicyId(0),
            name: "test_policy".to_string(),
            category: PolicyCategory::Administrative,
            idea_group_1: "economic_ideas".to_string(),
            idea_group_2: "quality_ideas".to_string(),
            modifiers: vec![],
        };
        policy_registry.register(policy);

        let mut country = crate::state::CountryState::default();
        country.ideas.groups.insert(IdeaGroupId(0), 7); // economic_ideas
        country.ideas.groups.insert(IdeaGroupId(1), 7); // quality_ideas
        country.policy_slots = 2;

        // Enable policy
        assert!(enable_policy(PolicyId(0), &mut country, &policy_registry, &idea_registry).is_ok());
        assert_eq!(country.enabled_policies.len(), 1);
        assert!(country.enabled_policies.contains(&PolicyId(0)));

        // Disable policy
        assert!(disable_policy(PolicyId(0), &mut country).is_ok());
        assert_eq!(country.enabled_policies.len(), 0);
    }
}
