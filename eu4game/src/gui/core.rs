#![allow(dead_code)]

//! Core traits and types for the Generic UI Binder system.
//!
//! This module defines the foundational abstractions that enable
//! runtime binding of Rust code to GUI layout files while maintaining
//! type safety and CI compatibility.

use crate::gui::types::Rect;

/// Result of handling a UI event.
///
/// Widgets return this to indicate whether they consumed an event
/// or whether it should continue propagating to other widgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
    /// Event was handled by this widget; stop propagation.
    Consumed,
    /// Event was not handled; continue to next widget in hit-test order.
    Ignored,
}

/// Generational index for safe widget references.
///
/// The generation counter prevents use-after-free bugs when widgets
/// are removed and their IDs are reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WidgetId {
    pub index: u32,
    pub generation: u32,
}

impl WidgetId {
    pub fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }
}

/// Context passed to widgets during rendering and input handling.
///
/// Contains global UI state that widgets may need to query or
/// reference during their update cycles.
pub struct UiContext<'a> {
    /// Current mouse cursor position in screen coordinates.
    pub mouse_pos: (f32, f32),
    /// Current time since UI system initialization (seconds).
    pub time: f32,
    /// Time elapsed since last frame (seconds).
    pub delta_time: f32,
    /// Localization resolver for $KEY$ tokens.
    pub localizer: &'a dyn Localizer,
    /// Currently focused widget (receives keyboard input).
    pub focused_widget: Option<WidgetId>,
}

/// Base trait for all interactive UI widgets.
///
/// All GUI primitives (buttons, text, icons, containers) implement this
/// trait to participate in the render/input pipeline.
pub trait GuiWidget {
    /// Render this widget and its children to the screen.
    fn render(&self, ctx: &UiContext, renderer: &mut dyn GuiRenderer);

    /// Handle an input event.
    ///
    /// Returns `Consumed` to stop event propagation to widgets below,
    /// or `Ignored` to allow the event to continue.
    fn handle_input(&mut self, event: &UiEvent, ctx: &UiContext) -> EventResult;

    /// Get the bounding rectangle for hit testing.
    ///
    /// Used to determine whether the mouse cursor is over this widget.
    fn bounds(&self) -> Rect;

    /// Check if a point is within this widget's interactive area.
    ///
    /// Default implementation uses `bounds()`, but widgets can override
    /// for non-rectangular hit areas (e.g., circular buttons).
    fn hit_test(&self, x: f32, y: f32) -> bool {
        self.bounds().contains(x, y)
    }
}

/// Trait for widgets that can be bound from GUI layout files.
///
/// This enables the runtime binding pattern: given a parsed `GuiNode`,
/// try to construct a typed widget handle. If the node is missing or
/// incompatible, return `None` and let the binder fall back to a placeholder.
pub trait Bindable: Sized {
    /// Attempt to create this widget from a parsed GUI node.
    fn from_node(node: &GuiNode) -> Option<Self>;

    /// Create a no-op placeholder for CI/missing assets.
    ///
    /// All methods on the placeholder do nothing and return safe defaults.
    /// This ensures code compiles and runs even without GUI assets.
    fn placeholder() -> Self;
}

/// Trait for resolving localization tokens in UI strings.
///
/// EU4 uses `$KEY$` syntax for localized text. Implementations of this
/// trait load the appropriate language files and perform token substitution.
pub trait Localizer: Send + Sync {
    /// Resolve a potentially-localized string.
    ///
    /// Returns the input unchanged if no `$KEY$` tokens are found,
    /// or if running in CI mode without localization data.
    fn resolve<'a>(&'a self, text: &'a str) -> std::borrow::Cow<'a, str>;

    /// Current language code (e.g., "l_english").
    fn language(&self) -> &str;
}

/// Placeholder trait for the renderer (will be properly defined elsewhere).
///
/// The actual renderer implementation lives in `mod.rs` and handles
/// drawing sprites, text, and 9-slice backgrounds.
pub trait GuiRenderer {
    // Methods will be added as needed by widget implementations
}

/// UI event types mapped from winit events.
///
/// These are a simplified, UI-specific representation of window system
/// events, focused on what GUI widgets need to handle.
#[derive(Debug, Clone)]
pub enum UiEvent {
    MouseMove {
        x: f32,
        y: f32,
    },
    MouseButton {
        button: MouseButton,
        state: ButtonState,
        x: f32,
        y: f32,
    },
    MouseWheel {
        delta_y: f32,
        x: f32,
        y: f32,
    },
    KeyPress {
        key: KeyCode,
        modifiers: Modifiers,
    },
    TextInput {
        character: char,
    },
    FocusGained,
    FocusLost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Pressed,
    Released,
}

/// Keyboard key codes (simplified subset for UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Tab,
    Enter,
    Escape,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    // Extend as needed
}

/// Keyboard modifiers.
#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// Placeholder for GuiNode (defined in binder.rs as type alias).
///
/// We need this as a forward declaration for the Bindable trait.
/// The actual implementation is GuiElement from the types module.
pub use crate::gui::binder::GuiNode;

/// Which part of the date to adjust.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatePart {
    Year,
    Month,
    Day,
}

/// Actions that can result from UI interactions.
///
/// Map mode selection options.
///
/// Each mode changes how the map is rendered (political borders, terrain, religion, etc.).
/// Currently only Political mode is fully implemented in the rendering engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapMode {
    /// Terrain heightmap view
    Terrain,
    /// Political borders (default)
    Political,
    /// Trade nodes and routes
    Trade,
    /// Religious map
    Religion,
    /// HRE borders
    Empire,
    /// Diplomatic relations
    Diplomacy,
    /// Economic development
    Economy,
    /// Geographic regions
    Region,
    /// Culture groups
    Culture,
    /// Multiplayer player nations
    Players,
}

/// Buttons and other interactive widgets return these actions to indicate
/// what should happen (screen transitions, game state changes, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiAction {
    /// Transition to single player screen (country selection).
    ShowSinglePlayer,
    /// Transition to multiplayer screen.
    ShowMultiplayer,
    /// Show tutorial screen.
    ShowTutorial,
    /// Show credits screen.
    ShowCredits,
    /// Show settings screen.
    ShowSettings,
    /// Exit the application.
    Exit,
    /// Start the game with selected country.
    StartGame,
    /// Return to previous screen.
    Back,
    /// Adjust date by delta (positive = forward, negative = backward).
    DateAdjust(DatePart, i32),
    /// Select a bookmark by index.
    SelectBookmark(usize),
    /// Select a save game by index.
    SelectSaveGame(usize),
    /// Change map rendering mode.
    SetMapMode(MapMode),
    /// No action (button not yet wired up).
    None,
}

/// No-op localizer for CI environments.
///
/// Returns all text unchanged, allowing the UI system to function
/// without access to EU4's localization files.
pub struct NoOpLocalizer;

impl Localizer for NoOpLocalizer {
    fn resolve<'a>(&'a self, text: &'a str) -> std::borrow::Cow<'a, str> {
        std::borrow::Cow::Borrowed(text)
    }

    fn language(&self) -> &str {
        "l_english"
    }
}
