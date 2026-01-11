//! Legacy layout type definitions for GUI rendering.
//!
//! These types contain rendering metadata extracted from EU4's .gui files.
//! They are gradually being replaced by macro-based widgets.

use super::types::{Orientation, TextFormat};

/// Window layout metadata for frontend panels (Phase 8.5.2).
///
/// Stores just the window position and orientation, since widget metadata
/// is stored in the widget fields themselves (via GuiWindow binding).
#[derive(Debug, Clone, Default)]
pub struct FrontendPanelLayout {
    pub window_pos: (i32, i32),
    pub orientation: Orientation,
}

/// Icon element from speed controls layout.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields read by speed_controls logic but flagged by aggressive lints
pub struct SpeedControlsIcon {
    pub name: String,
    pub sprite: String,
    pub position: (i32, i32),
    pub orientation: Orientation,
}

/// Text element from speed controls layout.
#[allow(dead_code)] // Will be used for score/rank text rendering
#[derive(Debug, Clone)]
pub struct SpeedControlsText {
    pub name: String,
    pub position: (i32, i32),
    pub font: String,
    pub max_width: u32,
    pub max_height: u32,
    pub orientation: Orientation,
    pub border_size: (i32, i32),
}

/// Loaded speed controls layout (rendering metadata only).
///
/// Phase 3.5: Renamed from SpeedControls. Dynamic text widgets moved to
/// macro-based speed_controls::SpeedControls struct.
pub struct SpeedControlsLayout {
    /// Background panel sprite.
    pub bg_sprite: String,
    /// Background position (relative to window).
    pub bg_pos: (i32, i32),
    /// Background orientation.
    pub bg_orientation: Orientation,
    /// Speed indicator sprite (10 frames).
    pub speed_sprite: String,
    /// Speed indicator position (relative to window).
    pub speed_pos: (i32, i32),
    /// Speed indicator orientation.
    pub speed_orientation: Orientation,
    /// Date text position.
    pub date_pos: (i32, i32),
    /// Date text orientation (for positioning within parent).
    pub date_orientation: Orientation,
    /// Date text max width.
    pub date_max_width: u32,
    /// Date text max height.
    pub date_max_height: u32,
    /// Date text font name.
    pub date_font: String,
    /// Date text border/padding size (x, y).
    pub date_border_size: (i32, i32),
    /// Position of the whole window.
    pub window_pos: (i32, i32),
    /// Window orientation.
    pub orientation: Orientation,
    /// Speed buttons: (name, position, orientation, sprite).
    pub buttons: Vec<(String, (i32, i32), Orientation, String)>,
    /// Additional icons (score icon, etc).
    pub icons: Vec<SpeedControlsIcon>,
    /// Additional text labels (score, rank, etc).
    pub texts: Vec<SpeedControlsText>,
}

impl Default for SpeedControlsLayout {
    fn default() -> Self {
        // Fallback values if parsing fails - these should rarely be used
        Self {
            bg_sprite: "GFX_date_bg".to_string(),
            bg_pos: (0, 0),
            bg_orientation: Orientation::UpperLeft,
            speed_sprite: "GFX_speed_indicator".to_string(),
            speed_pos: (0, 0),
            speed_orientation: Orientation::UpperLeft,
            date_pos: (0, 0),
            date_orientation: Orientation::UpperLeft,
            date_max_width: 100,
            date_max_height: 32,
            date_font: "vic_18".to_string(),
            date_border_size: (0, 0),
            window_pos: (0, 0),
            orientation: Orientation::UpperLeft,
            buttons: vec![],
            icons: vec![],
            texts: vec![],
        }
    }
}

/// Icon element from topbar layout.
#[derive(Debug, Clone)]
pub struct TopBarIcon {
    #[allow(dead_code)] // Used for debugging and future hit box registration
    pub name: String,
    pub sprite: String,
    pub position: (i32, i32),
    pub orientation: Orientation,
}

/// Text element from topbar layout.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Legacy - will be removed in Phase 3.5.4
pub struct TopBarText {
    pub name: String,
    pub position: (i32, i32),
    #[allow(dead_code)] // Will be used for font selection
    pub font: String,
    pub max_width: u32,
    pub max_height: u32,
    pub orientation: Orientation,
    pub format: TextFormat,
    pub border_size: (i32, i32),
}

/// Legacy topbar layout data (Phase 3.5: Will be removed after migration).
///
/// Contains rendering metadata like icon positions and backgrounds.
/// The actual text widgets are now managed by the macro-based `topbar::TopBar`.
pub struct TopBarLayout {
    /// Window position.
    pub window_pos: (i32, i32),
    /// Window orientation.
    pub orientation: Orientation,
    /// Background icons (rendered first).
    pub backgrounds: Vec<TopBarIcon>,
    /// Resource icons (gold, manpower, etc).
    pub icons: Vec<TopBarIcon>,
    /// Player shield position (for flag display).
    pub player_shield: Option<TopBarIcon>,
}

impl Default for TopBarLayout {
    fn default() -> Self {
        Self {
            window_pos: (0, -1),
            orientation: Orientation::UpperLeft,
            backgrounds: vec![],
            icons: vec![],
            player_shield: None,
        }
    }
}
