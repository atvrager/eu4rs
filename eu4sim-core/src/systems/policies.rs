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

        let mut registry = PolicyRegistry::new();
        let mut modifiers = GameModifiers::default();

        let policy = PolicyDef {
            id: PolicyId(0),
            name: "test_policy".to_string(),
            category: PolicyCategory::Administrative,
            idea_group_1: "economic_ideas".to_string(),
            idea_group_2: "quality_ideas".to_string(),
            modifiers: vec![
                ModifierEntry::new("discipline", Fixed::from_f32(0.05)),
            ],
        };

        registry.register(policy);

        let tag = "TST".to_string();
        let enabled = vec![PolicyId(0)];

        apply_policy_modifiers(&tag, &enabled, &registry, &mut modifiers);

        assert_eq!(
            modifiers.country_discipline.get(&tag),
            Some(&Fixed::from_f32(0.05))
        );
    }
}
