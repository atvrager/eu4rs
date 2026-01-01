//! Core types for EU4 GUI system.
//!
//! These types match EU4's .gfx and .gui file structures.

use crate::gui::interner::Symbol;
use std::collections::HashMap;

/// A collection of named GUI windows, potentially including templates.
pub type WindowDatabase = HashMap<Symbol, GuiElement>;

/// Sprite definition from .gfx files.
#[derive(Debug, Clone)]
pub struct GfxSprite {
    /// Sprite name (e.g., "GFX_speed_indicator").
    pub name: String,
    /// Path to texture file (e.g., "gfx/interface/speed_indicator.dds").
    pub texture_file: String,
    /// Number of frames for sprite strips (1 = single image).
    pub num_frames: u32,
    /// Whether frames are arranged horizontally (true) or vertically.
    pub horizontal_frames: bool,
}

/// Cornered tile sprite (9-slice) for scalable UI elements like panels.
/// The texture is divided into 9 regions using borderSize:
/// - 4 corners (fixed size)
/// - 4 edges (stretched in one direction)
/// - 1 center (stretched in both directions)
#[derive(Debug, Clone)]
pub struct CorneredTileSprite {
    /// Sprite name (e.g., "GFX_country_selection_panel_bg").
    pub name: String,
    /// Path to texture file.
    pub texture_file: String,
    /// Target size when rendered (x, y).
    pub size: (u32, u32),
    /// Border size defining the 9-slice regions (x, y).
    pub border_size: (u32, u32),
}

impl GfxSprite {
    /// Calculate UV coordinates for a specific frame.
    /// Returns (u_min, v_min, u_max, v_max).
    pub fn frame_uv(&self, frame: u32) -> (f32, f32, f32, f32) {
        if self.num_frames <= 1 {
            return (0.0, 0.0, 1.0, 1.0);
        }

        let frame = frame.min(self.num_frames - 1);
        let frame_size = 1.0 / self.num_frames as f32;

        if self.horizontal_frames {
            let u_min = frame as f32 * frame_size;
            let u_max = (frame + 1) as f32 * frame_size;
            (u_min, 0.0, u_max, 1.0)
        } else {
            let v_min = frame as f32 * frame_size;
            let v_max = (frame + 1) as f32 * frame_size;
            (0.0, v_min, 1.0, v_max)
        }
    }
}

/// Database of all loaded sprites.
#[derive(Debug, Default)]
pub struct GfxDatabase {
    /// Regular sprites indexed by name (e.g., "GFX_speed_indicator").
    pub sprites: HashMap<String, GfxSprite>,
    /// Cornered tile (9-slice) sprites indexed by name.
    pub cornered_tiles: HashMap<String, CorneredTileSprite>,
}

impl GfxDatabase {
    /// Get a regular sprite by name.
    pub fn get(&self, name: &str) -> Option<&GfxSprite> {
        self.sprites.get(name)
    }

    /// Get a cornered tile sprite by name.
    pub fn get_cornered_tile(&self, name: &str) -> Option<&CorneredTileSprite> {
        self.cornered_tiles.get(name)
    }

    /// Merge another database into this one.
    pub fn merge(&mut self, other: GfxDatabase) {
        self.sprites.extend(other.sprites);
        self.cornered_tiles.extend(other.cornered_tiles);
    }
}

/// Orientation/anchor for GUI elements.
/// Determines which corner of the screen (or parent) the position is relative to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Orientation {
    #[default]
    UpperLeft,
    UpperRight,
    LowerLeft,
    LowerRight,
    Center,
    CenterUp,
    CenterDown,
}

impl Orientation {
    /// Parse from EU4 orientation string.
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "UPPER_LEFT" => Orientation::UpperLeft,
            "UPPER_RIGHT" => Orientation::UpperRight,
            "LOWER_LEFT" => Orientation::LowerLeft,
            "LOWER_RIGHT" => Orientation::LowerRight,
            "CENTER" => Orientation::Center,
            "CENTER_UP" => Orientation::CenterUp,
            "CENTER_DOWN" => Orientation::CenterDown,
            _ => Orientation::UpperLeft,
        }
    }
}

/// Text format/alignment.
#[derive(Debug, Clone, Copy, Default)]
pub enum TextFormat {
    #[default]
    Left,
    Center,
    Right,
}

impl TextFormat {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "center" | "centre" => TextFormat::Center,
            "right" => TextFormat::Right,
            _ => TextFormat::Left,
        }
    }
}

/// A parsed GUI element.
#[derive(Debug, Clone)]
pub enum GuiElement {
    /// Window container with children.
    Window {
        name: String,
        position: (i32, i32),
        size: (u32, u32),
        orientation: Orientation,
        children: Vec<GuiElement>,
    },
    /// Static icon/sprite.
    Icon {
        name: String,
        position: (i32, i32),
        sprite_type: String,
        frame: u32,
        orientation: Orientation,
        /// Scale factor (1.0 = normal size).
        scale: f32,
    },
    /// Text display.
    TextBox {
        name: String,
        position: (i32, i32),
        font: String,
        max_width: u32,
        max_height: u32,
        format: TextFormat,
        orientation: Orientation,
        text: String,
        /// Border/padding size (x, y).
        border_size: (i32, i32),
    },
    /// Interactive button.
    Button {
        name: String,
        position: (i32, i32),
        sprite_type: String,
        orientation: Orientation,
        /// Optional shortcut key.
        shortcut: Option<String>,
    },
    /// Toggle checkbox.
    Checkbox {
        name: String,
        position: (i32, i32),
        sprite_type: String,
        orientation: Orientation,
    },
    /// Text input field.
    EditBox {
        name: String,
        position: (i32, i32),
        size: (u32, u32),
        font: String,
        orientation: Orientation,
        /// Maximum text length.
        max_characters: u32,
    },
    /// Scrollable list (Phase 7).
    Listbox {
        name: String,
        position: (i32, i32),
        size: (u32, u32),
        orientation: Orientation,
        /// Spacing between list items.
        spacing: i32,
        /// Name of the scrollbar to use.
        scrollbar_type: Option<String>,
        /// Background sprite (optional).
        background: Option<String>,
    },
    /// Scrollbar widget (Phase 7).
    Scrollbar {
        name: String,
        position: (i32, i32),
        size: (u32, u32),
        orientation: Orientation,
        /// Maximum scroll range.
        max_value: i32,
        /// Sprite for track background.
        track_sprite: Option<String>,
        /// Sprite for slider handle.
        slider_sprite: Option<String>,
    },
}

impl GuiElement {
    /// Get the element's name.
    pub fn name(&self) -> &str {
        match self {
            GuiElement::Window { name, .. } => name,
            GuiElement::Icon { name, .. } => name,
            GuiElement::TextBox { name, .. } => name,
            GuiElement::Button { name, .. } => name,
            GuiElement::Checkbox { name, .. } => name,
            GuiElement::EditBox { name, .. } => name,
            GuiElement::Listbox { name, .. } => name,
            GuiElement::Scrollbar { name, .. } => name,
        }
    }

    /// Get the element's position.
    pub fn position(&self) -> (i32, i32) {
        match self {
            GuiElement::Window { position, .. } => *position,
            GuiElement::Icon { position, .. } => *position,
            GuiElement::TextBox { position, .. } => *position,
            GuiElement::Button { position, .. } => *position,
            GuiElement::Checkbox { position, .. } => *position,
            GuiElement::EditBox { position, .. } => *position,
            GuiElement::Listbox { position, .. } => *position,
            GuiElement::Scrollbar { position, .. } => *position,
        }
    }

    /// Get the element's orientation.
    pub fn orientation(&self) -> Orientation {
        match self {
            GuiElement::Window { orientation, .. } => *orientation,
            GuiElement::Icon { orientation, .. } => *orientation,
            GuiElement::TextBox { orientation, .. } => *orientation,
            GuiElement::Button { orientation, .. } => *orientation,
            GuiElement::Checkbox { orientation, .. } => *orientation,
            GuiElement::EditBox { orientation, .. } => *orientation,
            GuiElement::Listbox { orientation, .. } => *orientation,
            GuiElement::Scrollbar { orientation, .. } => *orientation,
        }
    }

    /// Get the element's children (only Windows have children).
    pub fn children(&self) -> &[GuiElement] {
        match self {
            GuiElement::Window { children, .. } => children,
            _ => &[],
        }
    }
}

/// Current state for GUI rendering.
#[derive(Debug, Clone)]
pub struct GuiState {
    /// Current game date as string (e.g., "11 November 1444").
    pub date: String,
    /// Current simulation speed (1-5).
    pub speed: u32,
    /// Whether the simulation is paused.
    pub paused: bool,
    /// Country resources for topbar display.
    pub country: Option<CountryResources>,
}

/// Country resource values displayed in the topbar.
#[derive(Debug, Clone, Default)]
pub struct CountryResources {
    /// Current treasury (gold).
    pub treasury: f32,
    /// Monthly income.
    pub income: f32,
    /// Current manpower.
    pub manpower: i32,
    /// Maximum manpower.
    pub max_manpower: i32,
    /// Current sailors.
    pub sailors: i32,
    /// Maximum sailors.
    pub max_sailors: i32,
    /// Stability (-3 to +3).
    pub stability: i32,
    /// Prestige (-100 to +100).
    pub prestige: f32,
    /// Corruption (0-100).
    pub corruption: f32,
    /// Administrative monarch points.
    pub adm_power: i32,
    /// Diplomatic monarch points.
    pub dip_power: i32,
    /// Military monarch points.
    pub mil_power: i32,
    /// Available merchants.
    pub merchants: i32,
    /// Maximum merchants.
    pub max_merchants: i32,
    /// Available colonists.
    pub colonists: i32,
    /// Maximum colonists.
    pub max_colonists: i32,
    /// Available diplomats.
    pub diplomats: i32,
    /// Maximum diplomats.
    pub max_diplomats: i32,
    /// Available missionaries.
    pub missionaries: i32,
    /// Maximum missionaries.
    pub max_missionaries: i32,
}

impl Default for GuiState {
    fn default() -> Self {
        Self {
            date: "11 November 1444".to_string(),
            speed: 3,
            paused: true,
            country: None,
        }
    }
}

/// Bounding box for hit testing.
#[derive(Debug, Clone, Copy)]
pub struct HitBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl HitBox {
    /// Check if a point is inside this box.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

/// Rectangle for bounding boxes and hit testing.
/// Alias for HitBox with more semantic naming for the UI binder system.
pub type Rect = HitBox;

/// GUI interaction events.
#[derive(Debug, Clone)]
pub enum GuiAction {
    /// Speed button clicked (new speed 1-5).
    SetSpeed(u32),
    /// Pause/unpause toggled.
    TogglePause,
}
