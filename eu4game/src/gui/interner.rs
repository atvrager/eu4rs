#![allow(dead_code)]
//! String interning for efficient widget name lookups.
//!
//! GUI files contain many repeated string names ("icon", "button", "text").
//! Interning these strings allows O(1) comparison via integer IDs instead
//! of repeated string comparisons during tree traversal.

use std::collections::HashMap;
use std::sync::RwLock;

/// Interned string identifier.
///
/// Two `Symbol`s are equal if and only if they reference the same
/// interned string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Symbol(u32);

impl Symbol {
    const fn new(id: u32) -> Self {
        Self(id)
    }
}

/// String interning pool for efficient name comparison.
///
/// Widget names from GUI files are interned once and compared by
/// integer ID thereafter, avoiding repeated string allocations and
/// comparisons during tree traversal.
pub struct StringInterner {
    /// Map from string to symbol ID.
    strings: RwLock<HashMap<String, Symbol>>,
    /// Map from symbol ID back to string (for debugging).
    reverse: RwLock<Vec<String>>,
}

impl StringInterner {
    /// Create a new interner with common GUI widget names pre-interned.
    pub fn new() -> Self {
        let interner = Self {
            strings: RwLock::new(HashMap::new()),
            reverse: RwLock::new(Vec::new()),
        };

        // Pre-intern common widget names to avoid lock contention
        for name in COMMON_WIDGET_NAMES {
            interner.intern(name);
        }

        interner
    }

    /// Intern a string, returning its symbol.
    ///
    /// If the string has been interned before, returns the existing symbol.
    /// Otherwise, allocates a new symbol and stores the string.
    pub fn intern(&self, s: &str) -> Symbol {
        // Fast path: check if already interned (read lock)
        {
            let strings = self.strings.read().unwrap();
            if let Some(&symbol) = strings.get(s) {
                return symbol;
            }
        }

        // Slow path: intern new string (write lock)
        let mut strings = self.strings.write().unwrap();
        let mut reverse = self.reverse.write().unwrap();

        // Double-check in case another thread interned it
        if let Some(&symbol) = strings.get(s) {
            return symbol;
        }

        let id = reverse.len() as u32;
        let symbol = Symbol::new(id);
        reverse.push(s.to_string());
        strings.insert(s.to_string(), symbol);
        symbol
    }

    /// Resolve a symbol back to its string (for debugging).
    ///
    /// Panics if the symbol is invalid (shouldn't happen in practice).
    pub fn resolve(&self, symbol: Symbol) -> String {
        let reverse = self.reverse.read().unwrap();
        reverse[symbol.0 as usize].clone()
    }

    /// Get a symbol without interning (returns None if not already interned).
    ///
    /// Useful for lookups where you don't want to pollute the intern table.
    pub fn get(&self, s: &str) -> Option<Symbol> {
        let strings = self.strings.read().unwrap();
        strings.get(s).copied()
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

/// Common widget names to pre-intern for performance.
const COMMON_WIDGET_NAMES: &[&str] = &[
    "icon",
    "text",
    "button",
    "background",
    "window",
    "panel",
    "container",
    "label",
    "image",
    "sprite",
    "frame",
    "border",
    "title",
    "header",
    "footer",
    "listbox",
    "scrollbar",
    "checkbox",
    "editbox",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intern_same_string() {
        let interner = StringInterner::new();
        let s1 = interner.intern("hello");
        let s2 = interner.intern("hello");
        assert_eq!(s1, s2, "Same string should produce same symbol");
    }

    #[test]
    fn test_intern_different_strings() {
        let interner = StringInterner::new();
        let s1 = interner.intern("hello");
        let s2 = interner.intern("world");
        assert_ne!(s1, s2, "Different strings should produce different symbols");
    }

    #[test]
    fn test_resolve() {
        let interner = StringInterner::new();
        let symbol = interner.intern("test_string");
        assert_eq!(interner.resolve(symbol), "test_string");
    }

    #[test]
    fn test_pre_interned_common_names() {
        let interner = StringInterner::new();
        // Common names should already be interned
        let icon1 = interner.get("icon");
        let icon2 = interner.intern("icon");
        assert_eq!(icon1, Some(icon2), "Common names should be pre-interned");
    }
}
