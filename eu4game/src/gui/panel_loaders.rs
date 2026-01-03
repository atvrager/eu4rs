//! Legacy layout loading functions.
//!
//! This module contains helper functions that parse EU4's .gui files
//! and extract layout metadata for speed controls, topbar, and country select.
//! These are gradually being replaced by the generic UI binder system.

use super::country_select::{
    CountrySelectButton, CountrySelectIcon, CountrySelectLayout, CountrySelectText,
};
use super::interner;
use super::layout_types::{
    SpeedControlsIcon, SpeedControlsLayout, SpeedControlsText, TopBarIcon, TopBarLayout,
};
use super::parser::parse_gui_file;
use super::types::{GuiElement, Orientation};
use std::path::Path;

pub(super) fn load_speed_controls_split(
    game_path: &Path,
    interner: &interner::StringInterner,
) -> (SpeedControlsLayout, Option<GuiElement>) {
    let gui_path = game_path.join("interface/speed_controls.gui");

    if !gui_path.exists() {
        log::warn!("speed_controls.gui not found, using defaults");
        return (SpeedControlsLayout::default(), None);
    }

    match parse_gui_file(&gui_path, interner) {
        Ok(db) => {
            // Find the speed_controls window
            let symbol = interner.intern("speed_controls");
            if let Some(root) = db.get(&symbol)
                && let GuiElement::Window {
                    position,
                    orientation,
                    children,
                    ..
                } = root
            {
                let layout = extract_speed_controls_layout(position, orientation, children);
                return (layout, Some(root.clone()));
            }
            log::warn!("speed_controls window not found in GUI file");
            (SpeedControlsLayout::default(), None)
        }
        Err(e) => {
            log::warn!("Failed to parse speed_controls.gui: {}", e);
            (SpeedControlsLayout::default(), None)
        }
    }
}

/// Extract speed controls layout data from parsed GUI elements (rendering metadata only).
fn extract_speed_controls_layout(
    window_pos: &(i32, i32),
    orientation: &Orientation,
    children: &[GuiElement],
) -> SpeedControlsLayout {
    let mut controls = SpeedControlsLayout {
        window_pos: *window_pos,
        orientation: *orientation,
        ..Default::default()
    };

    for child in children {
        match child {
            GuiElement::Icon {
                name,
                sprite_type,
                position,
                orientation,
                ..
            } => {
                if name == "date_bg" || name == "icon_date_bg" {
                    controls.bg_sprite = sprite_type.clone();
                    controls.bg_pos = *position;
                    controls.bg_orientation = *orientation;
                    log::debug!(
                        "Parsed icon_date_bg: pos={:?}, orientation={:?}, sprite={}",
                        position,
                        orientation,
                        sprite_type
                    );
                } else {
                    // Collect additional icons (e.g., icon_score)
                    controls.icons.push(SpeedControlsIcon {
                        name: name.clone(),
                        sprite: sprite_type.clone(),
                        position: *position,
                        orientation: *orientation,
                    });
                    log::debug!(
                        "Parsed icon {}: pos={:?}, orientation={:?}, sprite={}",
                        name,
                        position,
                        orientation,
                        sprite_type
                    );
                }
            }
            GuiElement::TextBox {
                name,
                position,
                orientation,
                max_width,
                max_height,
                font,
                border_size,
                ..
            } => {
                // EU4 uses "DateText" for the date display
                if name == "date" || name == "DateText" {
                    controls.date_pos = *position;
                    controls.date_orientation = *orientation;
                    controls.date_max_width = *max_width;
                    controls.date_max_height = *max_height;
                    controls.date_font = font.clone();
                    controls.date_border_size = *border_size;
                    log::debug!(
                        "Parsed DateText: pos={:?}, orientation={:?}, maxWidth={}, maxHeight={}, font={}, borderSize={:?}",
                        position,
                        orientation,
                        max_width,
                        max_height,
                        font,
                        border_size
                    );
                } else {
                    // Collect additional text labels (e.g., text_score, text_score_rank)
                    controls.texts.push(SpeedControlsText {
                        name: name.clone(),
                        position: *position,
                        font: font.clone(),
                        max_width: *max_width,
                        max_height: *max_height,
                        orientation: *orientation,
                        border_size: *border_size,
                    });
                    log::debug!(
                        "Parsed text {}: pos={:?}, orientation={:?}, font={}",
                        name,
                        position,
                        orientation,
                        font
                    );
                }
            }
            GuiElement::Button {
                name,
                position,
                sprite_type,
                orientation,
                ..
            } => {
                if name == "speed_indicator" {
                    controls.speed_sprite = sprite_type.clone();
                    controls.speed_pos = *position;
                    controls.speed_orientation = *orientation;
                    log::debug!(
                        "Parsed speed_indicator: pos={:?}, orientation={:?}, sprite={}",
                        position,
                        orientation,
                        sprite_type
                    );
                } else {
                    controls.buttons.push((
                        name.clone(),
                        *position,
                        *orientation,
                        sprite_type.clone(),
                    ));
                    log::debug!(
                        "Parsed button {}: pos={:?}, orientation={:?}",
                        name,
                        position,
                        orientation
                    );
                }
            }
            _ => {}
        }
    }

    controls
}

/// Load topbar layout from game files (Phase 3.5: returns layout + root element).
///
/// Returns a tuple of (TopBarLayout, Option<GuiElement>) where:
/// - TopBarLayout contains rendering metadata (icons, backgrounds, positions)
/// - GuiElement is the root window for macro-based text widget binding
pub(super) fn load_topbar_split(
    game_path: &Path,
    interner: &interner::StringInterner,
) -> (TopBarLayout, Option<GuiElement>) {
    let gui_path = game_path.join("interface/topbar.gui");

    if !gui_path.exists() {
        log::warn!("topbar.gui not found, using defaults");
        return (TopBarLayout::default(), None);
    }

    match parse_gui_file(&gui_path, interner) {
        Ok(db) => {
            // Find the topbar window
            let symbol = interner.intern("topbar");
            if let Some(root) = db.get(&symbol)
                && let GuiElement::Window {
                    position,
                    orientation,
                    children,
                    ..
                } = root
            {
                let layout = extract_topbar_layout(position, orientation, children);
                return (layout, Some(root.clone()));
            }
            log::warn!("topbar window not found in GUI file");
            (TopBarLayout::default(), None)
        }
        Err(e) => {
            log::warn!("Failed to parse topbar.gui: {}", e);
            (TopBarLayout::default(), None)
        }
    }
}

/// Extract topbar layout data from parsed GUI elements (Phase 3.5).
///
/// This extracts only rendering metadata (icon positions, backgrounds).
/// Text widgets are handled by the macro-based topbar::TopBar.
fn extract_topbar_layout(
    window_pos: &(i32, i32),
    orientation: &Orientation,
    children: &[GuiElement],
) -> TopBarLayout {
    let mut layout = TopBarLayout {
        window_pos: *window_pos,
        orientation: *orientation,
        ..Default::default()
    };

    // Background icon names - rendered first
    let bg_names = [
        "topbar_upper_left_bg",
        "topbar_upper_left_bg2",
        "topbar_upper_left_bg4",
        "brown_bg",
        "topbar_1",
        "topbar_2",
        "topbar_3",
    ];

    // Resource icon names we want to render
    let icon_names = [
        // Core resources
        "icon_gold",
        "icon_manpower",
        "icon_sailors",
        "icon_stability",
        "icon_prestige",
        "icon_corruption",
        // Monarch power
        "icon_ADM",
        "icon_DIP",
        "icon_MIL",
        // Envoys
        "icon_merchant",
        "icon_settler",
        "icon_diplomat",
        "icon_missionary",
    ];

    for child in children {
        match child {
            GuiElement::Icon {
                name,
                sprite_type,
                position,
                orientation,
                ..
            } => {
                let icon = TopBarIcon {
                    name: name.clone(),
                    sprite: sprite_type.clone(),
                    position: *position,
                    orientation: *orientation,
                };

                if name == "player_shield" {
                    log::debug!(
                        "Parsed player_shield: pos={:?}, sprite={}",
                        position,
                        sprite_type
                    );
                    layout.player_shield = Some(icon);
                } else if bg_names.contains(&name.as_str()) {
                    log::debug!(
                        "Parsed topbar bg {}: pos={:?}, sprite={}",
                        name,
                        position,
                        sprite_type
                    );
                    layout.backgrounds.push(icon);
                } else if icon_names.contains(&name.as_str()) {
                    log::debug!(
                        "Parsed topbar icon {}: pos={:?}, sprite={}",
                        name,
                        position,
                        sprite_type
                    );
                    layout.icons.push(icon);
                }
            }
            GuiElement::Button {
                name,
                sprite_type,
                position,
                orientation,
                ..
            } => {
                // player_shield is a guiButtonType in topbar.gui
                if name == "player_shield" {
                    log::debug!(
                        "Parsed player_shield (button): pos={:?}, sprite={}",
                        position,
                        sprite_type
                    );
                    layout.player_shield = Some(TopBarIcon {
                        name: name.clone(),
                        sprite: sprite_type.clone(),
                        position: *position,
                        orientation: *orientation,
                    });
                } else if icon_names.contains(&name.as_str()) {
                    // Some icons are buttons (like mana icons)
                    log::debug!(
                        "Parsed topbar button-icon {}: pos={:?}, sprite={}",
                        name,
                        position,
                        sprite_type
                    );
                    layout.icons.push(TopBarIcon {
                        name: name.clone(),
                        sprite: sprite_type.clone(),
                        position: *position,
                        orientation: *orientation,
                    });
                }
            }
            // Phase 3.5: Text widgets now handled by macro-based topbar::TopBar
            _ => {}
        }
    }

    log::info!(
        "Loaded topbar layout: {} backgrounds, {} icons, player_shield={}",
        layout.backgrounds.len(),
        layout.icons.len(),
        layout.player_shield.is_some()
    );

    layout
}

/// Load country selection panel layout from frontend.gui (Phase 3.5: returns layout + root element).
///
/// Returns a tuple of (CountrySelectLayout, Option<GuiElement>) where:
/// - CountrySelectLayout contains rendering metadata (window size, icon vectors, text vectors, etc.)
/// - GuiElement is the root window for macro-based widget binding (CountrySelectRightPanel)
pub(super) fn load_country_select_split(
    game_path: &Path,
    interner: &interner::StringInterner,
) -> (CountrySelectLayout, Option<GuiElement>) {
    let gui_path = game_path.join("interface/frontend.gui");

    if !gui_path.exists() {
        log::warn!("frontend.gui not found, using defaults");
        return (CountrySelectLayout::default(), None);
    }

    match parse_gui_file(&gui_path, interner) {
        Ok(db) => {
            // The structure is: country_selection_panel > ... > singleplayer
            // We search all top-level windows in the database
            for element in db.values() {
                if let Some((layout, root)) = find_singleplayer_window_in_node_split(element) {
                    return (layout, Some(root));
                }
            }
            log::warn!("singleplayer window not found in frontend.gui");
            (CountrySelectLayout::default(), None)
        }
        Err(e) => {
            log::warn!("Failed to parse frontend.gui: {}", e);
            (CountrySelectLayout::default(), None)
        }
    }
}

/// Panel data: GuiElement root and layout metadata.
type PanelData = (GuiElement, super::layout_types::FrontendPanelLayout);

/// All frontend panels loaded from frontend.gui.
#[derive(Default)]
pub struct FrontendPanels {
    /// Main menu panel (mainmenu window)
    pub main_menu: Option<PanelData>,
    /// Country selection left panel (left window)
    pub left: Option<PanelData>,
    /// Country selection top panel (top window)
    pub top: Option<PanelData>,
    /// Country selection right panel / lobby controls (right window)
    pub right: Option<PanelData>,
}

/// Load frontend panels from frontend.gui for Phase 8.5 integration.
///
/// Returns FrontendPanels containing all panel data.
/// Returns None for any window not found (CI-safe).
pub(super) fn load_frontend_panels(
    game_path: &Path,
    interner: &interner::StringInterner,
) -> FrontendPanels {
    let gui_path = game_path.join("interface/frontend.gui");

    if !gui_path.exists() {
        log::warn!("frontend.gui not found for panel loading");
        return FrontendPanels::default();
    }

    match parse_gui_file(&gui_path, interner) {
        Ok(db) => {
            // Search for panels - mainmenu is at top level, others are nested
            let mut panels = FrontendPanels::default();

            for element in db.values() {
                find_panels_recursive(element, &mut panels);
                // Early exit if we found all panels
                if panels.main_menu.is_some()
                    && panels.left.is_some()
                    && panels.top.is_some()
                    && panels.right.is_some()
                {
                    break;
                }
            }

            if panels.main_menu.is_none() {
                log::warn!("'mainmenu_panel_bottom' window not found in frontend.gui");
            }
            if panels.left.is_none() {
                log::warn!("'left' window not found in frontend.gui");
            }
            if panels.top.is_none() {
                log::warn!("'top' window not found in frontend.gui");
            }
            if panels.right.is_none() {
                log::warn!("'right' window not found in frontend.gui");
            }

            panels
        }
        Err(e) => {
            log::warn!("Failed to parse frontend.gui for panels: {}", e);
            FrontendPanels::default()
        }
    }
}

/// Recursively search for frontend panel windows in the GUI element tree.
fn find_panels_recursive(element: &GuiElement, panels: &mut FrontendPanels) {
    use super::layout_types::FrontendPanelLayout;

    if let GuiElement::Window {
        name,
        position,
        orientation,
        children,
        ..
    } = element
    {
        // Create layout metadata for this window
        let layout = FrontendPanelLayout {
            window_pos: *position,
            orientation: *orientation,
        };

        // Check if this is one of the panels we're looking for
        // - "mainmenu_panel_bottom" contains single player, multiplayer, exit buttons
        //   (has CENTER_DOWN orientation for proper bottom positioning)
        // - "left" contains bookmarks, save games, date widget, back button
        // - "top" contains map mode buttons, year label
        // - "right" contains play button, random country, nation designer buttons
        if name == "mainmenu_panel_bottom" && panels.main_menu.is_none() {
            panels.main_menu = Some((element.clone(), layout));
        } else if name == "left" && panels.left.is_none() {
            panels.left = Some((element.clone(), layout));
        } else if name == "top" && panels.top.is_none() {
            panels.top = Some((element.clone(), layout));
        } else if name == "right" && panels.right.is_none() {
            panels.right = Some((element.clone(), layout));
        }

        // Recurse into children
        for child in children {
            find_panels_recursive(child, panels);
        }
    }
}

/// Recursively search for the singleplayer window and return both layout and root (Phase 3.5).
fn find_singleplayer_window_in_node_split(
    element: &GuiElement,
) -> Option<(CountrySelectLayout, GuiElement)> {
    if let GuiElement::Window {
        name,
        position,
        size,
        orientation,
        children,
    } = element
    {
        if name == "singleplayer" {
            let layout = extract_country_select(position, size, orientation, children);
            return Some((layout, element.clone()));
        }
        // Recurse into child windows
        for child in children {
            if let Some(result) = find_singleplayer_window_in_node_split(child) {
                return Some(result);
            }
        }
    }
    None
}

/// Extract country select data from the singleplayer window.
fn extract_country_select(
    window_pos: &(i32, i32),
    window_size: &(u32, u32),
    orientation: &Orientation,
    children: &[GuiElement],
) -> CountrySelectLayout {
    let mut layout = CountrySelectLayout {
        window_pos: *window_pos,
        window_size: *window_size,
        window_orientation: *orientation,
        loaded: true,
        ..Default::default()
    };

    for child in children {
        match child {
            GuiElement::Icon {
                name,
                sprite_type,
                position,
                orientation,
                frame,
                scale,
            } => {
                log::debug!(
                    "Parsed country select icon {}: pos={:?}, sprite={}, scale={}",
                    name,
                    position,
                    sprite_type,
                    scale
                );
                layout.icons.push(CountrySelectIcon {
                    name: name.clone(),
                    sprite: sprite_type.clone(),
                    position: *position,
                    orientation: *orientation,
                    frame: *frame,
                    scale: *scale,
                });
            }
            GuiElement::Button {
                name,
                sprite_type,
                position,
                orientation,
                ..
            } => {
                log::debug!(
                    "Parsed country select button {}: pos={:?}, sprite={}",
                    name,
                    position,
                    sprite_type
                );
                layout.buttons.push(CountrySelectButton {
                    name: name.clone(),
                    sprite: sprite_type.clone(),
                    position: *position,
                    orientation: *orientation,
                });
            }
            GuiElement::TextBox {
                name,
                position,
                font,
                max_width,
                max_height,
                orientation,
                format,
                border_size,
                ..
            } => {
                log::debug!(
                    "Parsed country select text {}: pos={:?}, font={}, format={:?}",
                    name,
                    position,
                    font,
                    format
                );
                layout.texts.push(CountrySelectText {
                    name: name.clone(),
                    position: *position,
                    font: font.clone(),
                    max_width: *max_width,
                    max_height: *max_height,
                    format: *format,
                    orientation: *orientation,
                    border_size: *border_size,
                });
            }
            GuiElement::Window { .. } => {
                // Skip nested windows (like listboxes) for now
            }
            GuiElement::Checkbox { .. } => {
                // Skip checkboxes for now (not used in country select)
            }
            GuiElement::EditBox { .. } => {
                // Skip editboxes for now (not used in country select)
            }
            GuiElement::Listbox { .. } => {
                // Skip listboxes for now (Phase 7 - not yet implemented in country select)
            }
            GuiElement::Scrollbar { .. } => {
                // Skip scrollbars for now (Phase 7 - not yet implemented in country select)
            }
        }
    }

    log::info!(
        "Loaded country select: {} icons, {} texts, {} buttons",
        layout.icons.len(),
        layout.texts.len(),
        layout.buttons.len(),
    );

    layout
}

#[cfg(test)]
#[path = "panel_loaders_tests.rs"]
mod tests;
