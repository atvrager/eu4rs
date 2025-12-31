#![allow(dead_code)]
//! The Binder system - runtime binding of Rust code to GUI layouts.
//!
//! This module implements the core binding logic that connects typed Rust
//! structs to parsed GUI element trees, enabling CI-compatible runtime
//! layout resolution with graceful degradation for missing assets.

use crate::gui::interner::{StringInterner, Symbol};
use crate::gui::types::{GuiElement, WindowDatabase};

/// Type alias for clarity: GuiNode is the parsed element from the GUI file.
pub type GuiNode = GuiElement;

/// The Binder walks a parsed GUI tree to locate named widgets.
///
/// It uses string interning for efficient name comparison during
/// tree traversal, and supports both required and optional widget binding.
pub struct Binder<'a> {
    root: &'a GuiNode,
    interner: &'a StringInterner,
}

impl<'a> Binder<'a> {
    /// Create a new binder for the given GUI tree.
    pub fn new(root: &'a GuiNode, interner: &'a StringInterner) -> Self {
        Self { root, interner }
    }

    /// Bind a widget by name.
    ///
    /// Logs a warning and returns a placeholder if the widget is not found.
    /// This ensures code continues to work in CI environments without assets.
    pub fn bind<T: Bindable>(&self, name: &str) -> T {
        match self.bind_optional(name) {
            Some(widget) => widget,
            None => {
                log::warn!(
                    "UI Binding Failed: '{}' not found in '{}'",
                    name,
                    self.root.name()
                );
                T::placeholder()
            }
        }
    }

    /// Bind a widget by name, returning None if not found (no warning).
    ///
    /// Use this for truly optional widgets where absence is expected
    /// and doesn't indicate a problem.
    pub fn bind_optional<T: Bindable>(&self, name: &str) -> Option<T> {
        let target_symbol = self.interner.intern(name);
        self.find_node_iterative(target_symbol)
            .and_then(|node| T::from_node(node))
    }

    /// Iterative tree traversal to find a node by symbol.
    ///
    /// Uses an explicit stack to avoid stack overflow on deeply nested
    /// GUI hierarchies (EU4 panels can nest 10+ levels deep).
    fn find_node_iterative(&self, target: Symbol) -> Option<&'a GuiNode> {
        let mut stack = vec![self.root];

        while let Some(node) = stack.pop() {
            // Check if this node matches
            let node_symbol = self.interner.intern(node.name());
            if node_symbol == target {
                return Some(node);
            }

            // Push children in reverse order for left-to-right traversal
            for child in node.children().iter().rev() {
                stack.push(child);
            }
        }

        None
    }

    /// Instantiate a template from the database.
    ///
    /// This is used for dynamic UI elements like list items where
    /// a template defined in the GUI file is cloned multiple times.
    pub fn instantiate_template<T: Bindable>(&self, name: &str, db: &WindowDatabase) -> T {
        let symbol = self.interner.intern(name);
        match db.get(&symbol) {
            Some(node) => {
                // T::from_node should typically clone the tree
                T::from_node(node).unwrap_or_else(|| {
                    log::warn!(
                        "Template '{}' found but incompatible with expected type",
                        name
                    );
                    T::placeholder()
                })
            }
            None => {
                log::warn!("Template '{}' not found in database", name);
                T::placeholder()
            }
        }
    }
}

/// Trait for widgets that can be bound from GUI layout files.
///
/// Implementations attempt to extract type-specific data from a GuiNode
/// and return None if the node type is incompatible.
pub trait Bindable: Sized {
    /// Attempt to create this widget from a parsed GUI node.
    ///
    /// Returns None if the node type doesn't match this widget type.
    fn from_node(node: &GuiNode) -> Option<Self>;

    /// Create a no-op placeholder for CI/missing assets.
    ///
    /// All methods on the placeholder do nothing and return safe defaults.
    fn placeholder() -> Self;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::types::{GuiElement, Orientation};

    #[test]
    fn test_find_top_level_element() {
        let interner = StringInterner::new();

        let tree = GuiElement::Window {
            name: "root".to_string(),
            position: (0, 0),
            size: (100, 100),
            orientation: Orientation::UpperLeft,
            children: vec![GuiElement::TextBox {
                name: "label".to_string(),
                position: (10, 10),
                font: "default".to_string(),
                max_width: 80,
                max_height: 20,
                format: crate::gui::types::TextFormat::Left,
                orientation: Orientation::UpperLeft,
                text: "Hello".to_string(),
                border_size: (0, 0),
            }],
        };

        let binder = Binder::new(&tree, &interner);
        let label_symbol = interner.intern("label");
        let found = binder.find_node_iterative(label_symbol);

        assert!(found.is_some(), "Should find 'label' element");
        assert_eq!(found.unwrap().name(), "label");
    }

    #[test]
    fn test_find_nested_element() {
        let interner = StringInterner::new();

        let tree = GuiElement::Window {
            name: "root".to_string(),
            position: (0, 0),
            size: (100, 100),
            orientation: Orientation::UpperLeft,
            children: vec![GuiElement::Window {
                name: "panel".to_string(),
                position: (10, 10),
                size: (80, 80),
                orientation: Orientation::UpperLeft,
                children: vec![GuiElement::Button {
                    name: "nested_button".to_string(),
                    position: (5, 5),
                    sprite_type: "GFX_button".to_string(),
                    orientation: Orientation::UpperLeft,
                    shortcut: None,
                }],
            }],
        };

        let binder = Binder::new(&tree, &interner);
        let button_symbol = interner.intern("nested_button");
        let found = binder.find_node_iterative(button_symbol);

        assert!(found.is_some(), "Should find deeply nested button");
        assert_eq!(found.unwrap().name(), "nested_button");
    }

    #[test]
    fn test_missing_element_returns_none() {
        let interner = StringInterner::new();

        let tree = GuiElement::Window {
            name: "root".to_string(),
            position: (0, 0),
            size: (100, 100),
            orientation: Orientation::UpperLeft,
            children: vec![],
        };

        let binder = Binder::new(&tree, &interner);
        let missing_symbol = interner.intern("nonexistent");
        let found = binder.find_node_iterative(missing_symbol);

        assert!(found.is_none(), "Should not find nonexistent element");
    }
}
