//! EU4 GUI system.
//!
//! Parses EU4's .gui and .gfx layout files to render authentic UI
//! using the game's actual sprites and positions.

// ========================================
// Module Declarations
// ========================================

// Phase 1: Legacy layout system
#[allow(dead_code)] // Country select panel WIP
pub mod country_select;
#[allow(dead_code)]
pub mod layout;
pub mod nine_slice;
pub mod parser;
pub mod sprite_cache;
#[allow(dead_code)]
pub mod types;

// Phase 2: Generic UI Binder system
pub mod binder;
pub mod core;
pub mod interner;
pub mod primitives;

// Phase 5: Input system & focus management
pub mod input;

// Phase 6: Screen & panel management
pub mod ui_root;

// Phase 6.1: Frontend integration
pub mod frontend;

// Phase 3: Macro system tests
#[cfg(test)]
mod macro_test;

// Phase 3.5: Macro-based UI panels
pub mod speed_controls;
pub mod topbar;

// Phase 4: Interactive UI panels with button support
pub mod main_menu;

// Phase 8.2: Country selection left panel
pub mod bookmarks;
pub mod country_select_left;

// Phase 8.3: Country selection top panel
pub mod country_select_top;

// Phase 8.4: Lobby controls (play button)
pub mod lobby_controls;

// Refactored modules (moved from mod.rs to reduce file size)
mod layout_types;
mod legacy_loaders;
mod renderer;

// ========================================
// Public Re-exports
// ========================================

// Country select panel
#[allow(unused_imports)] // SelectedCountryState used in tests
pub use country_select::{CountrySelectLayout, CountrySelectPanel, SelectedCountryState};
pub use country_select_left::CountrySelectLeftPanel;
#[allow(unused_imports)] // WIP - not yet integrated
pub use country_select_top::CountrySelectTopPanel;
#[allow(unused_imports)] // WIP - not yet integrated
pub use lobby_controls::LobbyControlsPanel;

// Layout utilities
#[allow(unused_imports)] // Public API
pub use layout::{
    compute_masked_flag_rect, get_window_anchor, position_from_anchor, rect_to_clip_space,
};

// Layout types
#[allow(unused_imports)] // Public API
pub use layout_types::{
    FrontendPanelLayout, SpeedControlsIcon, SpeedControlsLayout, SpeedControlsText, TopBarIcon,
    TopBarLayout, TopBarText,
};

// Parser
#[allow(unused_imports)] // Public API
pub use parser::{parse_gfx_file, parse_gui_file};

// Rendering
pub use renderer::GuiRenderer;

// Sprite cache
#[allow(unused_imports)] // Public API
pub use sprite_cache::{SpriteBorder, SpriteCache};

// Core GUI types
#[allow(unused_imports)] // Public API
pub use core::{MapMode, UiAction};

// Legacy types
#[allow(unused_imports)] // Public API
pub use types::{
    CountryResources, GfxDatabase, GuiAction, GuiElement, GuiState, HitBox, Orientation,
};
