//! Parser for EU4 .gfx and .gui files.
//!
//! Uses eu4txt for tokenization and parsing.

use super::types::{
    CorneredTileSprite, GfxDatabase, GfxSprite, GuiElement, Orientation, TextFormat, WindowDatabase,
};
use crate::gui::interner::StringInterner;
use eu4txt::{DefaultEU4Txt, EU4Txt, EU4TxtAstItem, EU4TxtParseNode};
use std::path::Path;

/// Parse a .gfx file and extract sprite definitions.
pub fn parse_gfx_file(path: &Path) -> Result<GfxDatabase, String> {
    let tokens = DefaultEU4Txt::open_txt(path.to_str().unwrap_or(""))
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    let ast = DefaultEU4Txt::parse(tokens)
        .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;

    let mut db = GfxDatabase::default();

    // Walk the AST looking for spriteTypes
    extract_sprites_from_node(&ast, &mut db);

    Ok(db)
}

/// Parse a .gui file and extract GUI elements into a WindowDatabase.
pub fn parse_gui_file(path: &Path, interner: &StringInterner) -> Result<WindowDatabase, String> {
    let tokens = DefaultEU4Txt::open_txt(path.to_str().unwrap_or(""))
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    let ast = DefaultEU4Txt::parse(tokens)
        .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;

    let mut db = WindowDatabase::new();
    extract_gui_elements_from_node(&ast, &mut db, interner);

    Ok(db)
}

/// Raw element counts from a GUI file (for gap detection).
#[derive(Debug, Default, Clone)]
#[allow(dead_code)]
pub struct RawGuiCounts {
    pub windows: usize,
    pub icons: usize,
    pub textboxes: usize,
    pub buttons: usize,
    pub checkboxes: usize,
    pub editboxes: usize,
    /// Element type names we saw but don't parse yet
    pub unknown_types: Vec<String>,
}

impl RawGuiCounts {
    #[allow(dead_code)]
    pub fn total(&self) -> usize {
        self.windows + self.icons + self.textboxes + self.buttons + self.checkboxes + self.editboxes
    }
}

/// Count all GUI elements in a file without filtering.
/// This helps detect parsing gaps by comparing raw counts to parsed output.
#[allow(dead_code)]
pub fn count_raw_gui_elements(path: &Path) -> Result<RawGuiCounts, String> {
    let tokens = DefaultEU4Txt::open_txt(path.to_str().unwrap_or(""))
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    let ast = DefaultEU4Txt::parse(tokens)
        .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;

    let mut counts = RawGuiCounts::default();
    count_elements_recursive(&ast, &mut counts);

    Ok(counts)
}

/// Recursively count all GUI element types in the AST.
#[allow(dead_code)]
fn count_elements_recursive(node: &EU4TxtParseNode, counts: &mut RawGuiCounts) {
    if let EU4TxtAstItem::Assignment = &node.entry
        && let Some(key) = get_assignment_key(node)
    {
        match key.as_str() {
            "windowType" => counts.windows += 1,
            "iconType" => counts.icons += 1,
            "instantTextBoxType" => counts.textboxes += 1,
            "guiButtonType" => counts.buttons += 1,
            "checkboxType" => counts.checkboxes += 1,
            "editBoxType" => counts.editboxes += 1,
            // Track element types we don't parse yet
            "listboxType"
            | "scrollbarType"
            | "OverlappingElementsBoxType"
            | "positionType"
            | "smoothListboxType"
            | "textBoxType"
            | "extendedScrollbarType"
            | "gridBoxType" => {
                if !counts.unknown_types.contains(&key) {
                    counts.unknown_types.push(key);
                }
            }
            _ => {}
        }
    }

    for child in &node.children {
        count_elements_recursive(child, counts);
    }
}

/// Extract sprites from a parsed AST node.
fn extract_sprites_from_node(node: &EU4TxtParseNode, db: &mut GfxDatabase) {
    match &node.entry {
        EU4TxtAstItem::AssignmentList => {
            // Look for spriteTypes block
            for child in &node.children {
                if let EU4TxtAstItem::Assignment = &child.entry
                    && let Some(key) = get_assignment_key(child)
                    && key == "spriteTypes"
                {
                    // Found spriteTypes block - process children
                    if let Some(list) = get_assignment_value(child) {
                        extract_sprite_types(list, db);
                    }
                }
            }
        }
        _ => {
            for child in &node.children {
                extract_sprites_from_node(child, db);
            }
        }
    }
}

/// Extract spriteType blocks from a spriteTypes list.
fn extract_sprite_types(node: &EU4TxtParseNode, db: &mut GfxDatabase) {
    for child in &node.children {
        if let EU4TxtAstItem::Assignment = &child.entry
            && let Some(key) = get_assignment_key(child)
        {
            match key.as_str() {
                "spriteType" | "textSpriteType" => {
                    if let Some(sprite) = parse_sprite_type(get_assignment_value(child)) {
                        db.sprites.insert(sprite.name.clone(), sprite);
                    }
                }
                "corneredTileSpriteType" => {
                    if let Some(tile) = parse_cornered_tile_sprite(get_assignment_value(child)) {
                        log::debug!(
                            "Parsed cornered tile sprite: {} ({}x{}, border {}x{})",
                            tile.name,
                            tile.size.0,
                            tile.size.1,
                            tile.border_size.0,
                            tile.border_size.1
                        );
                        db.cornered_tiles.insert(tile.name.clone(), tile);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Parse a single spriteType block.
fn parse_sprite_type(node: Option<&EU4TxtParseNode>) -> Option<GfxSprite> {
    let node = node?;

    let mut name = None;
    let mut texture_file = None;
    let mut num_frames = 1u32;

    for child in &node.children {
        if let EU4TxtAstItem::Assignment = &child.entry
            && let Some(key) = get_assignment_key(child)
        {
            match key.as_str() {
                "name" => {
                    name = get_string_value(get_assignment_value(child));
                }
                "texturefile" => {
                    texture_file = get_string_value(get_assignment_value(child));
                }
                "noOfFrames" => {
                    if let Some(n) = get_int_value(get_assignment_value(child)) {
                        num_frames = n as u32;
                    }
                }
                _ => {}
            }
        }
    }

    Some(GfxSprite {
        name: name?,
        texture_file: texture_file?,
        num_frames,
        horizontal_frames: true, // EU4 uses horizontal strips by default
    })
}

/// Parse a corneredTileSpriteType block (9-slice sprite).
fn parse_cornered_tile_sprite(node: Option<&EU4TxtParseNode>) -> Option<CorneredTileSprite> {
    let node = node?;

    let mut name = None;
    let mut texture_file = None;
    let mut size = (0u32, 0u32);
    let mut border_size = (0u32, 0u32);

    for child in &node.children {
        if let EU4TxtAstItem::Assignment = &child.entry
            && let Some(key) = get_assignment_key(child)
        {
            match key.as_str() {
                "name" => {
                    name = get_string_value(get_assignment_value(child));
                }
                "texturefile" | "textureFile" => {
                    texture_file = get_string_value(get_assignment_value(child));
                }
                "size" => {
                    let (x, y) = parse_position(get_assignment_value(child));
                    size = (x.max(0) as u32, y.max(0) as u32);
                }
                "borderSize" => {
                    let (x, y) = parse_position(get_assignment_value(child));
                    border_size = (x.max(0) as u32, y.max(0) as u32);
                }
                _ => {}
            }
        }
    }

    // Texture file is required for cornered tiles
    let texture_file = texture_file?;

    Some(CorneredTileSprite {
        name: name?,
        texture_file,
        size,
        border_size,
    })
}

/// Extract GUI elements from a parsed AST.
fn extract_gui_elements_from_node(
    node: &EU4TxtParseNode,
    db: &mut WindowDatabase,
    interner: &StringInterner,
) {
    match &node.entry {
        EU4TxtAstItem::AssignmentList => {
            for child in &node.children {
                if let EU4TxtAstItem::Assignment = &child.entry
                    && let Some(key) = get_assignment_key(child)
                {
                    match key.as_str() {
                        "guiTypes" => {
                            if let Some(list) = get_assignment_value(child) {
                                extract_gui_types(list, db, interner);
                            }
                        }
                        "windowType" => {
                            if let Some(window) = parse_window_type(get_assignment_value(child)) {
                                let symbol = interner.intern(window.name());
                                db.insert(symbol, window);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        _ => {
            for child in &node.children {
                extract_gui_elements_from_node(child, db, interner);
            }
        }
    }
}

/// Extract elements from a guiTypes block.
fn extract_gui_types(node: &EU4TxtParseNode, db: &mut WindowDatabase, interner: &StringInterner) {
    for child in &node.children {
        if let EU4TxtAstItem::Assignment = &child.entry
            && let Some(key) = get_assignment_key(child)
        {
            match key.as_str() {
                "windowType" => {
                    if let Some(window) = parse_window_type(get_assignment_value(child)) {
                        let symbol = interner.intern(window.name());
                        db.insert(symbol, window);
                    }
                }
                "iconType" | "instantTextBoxType" | "guiButtonType" => {
                    // These are usually handled as children of windows,
                    // but if they appear at the top level, we might want to track them?
                    // For now, we only populate the WindowDatabase with actual windows/templates.
                }
                _ => {}
            }
        }
    }
}

/// Parse a windowType block.
fn parse_window_type(node: Option<&EU4TxtParseNode>) -> Option<GuiElement> {
    let node = node?;

    let mut name = String::new();
    let mut position = (0i32, 0i32);
    let mut size = (100u32, 100u32);
    let mut orientation = Orientation::UpperLeft;
    let mut children = Vec::new();

    for child in &node.children {
        if let EU4TxtAstItem::Assignment = &child.entry
            && let Some(key) = get_assignment_key(child)
        {
            match key.as_str() {
                "name" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        name = s;
                    }
                }
                "position" => {
                    position = parse_position(get_assignment_value(child));
                }
                "size" => {
                    size = parse_size(get_assignment_value(child));
                }
                "Orientation" | "orientation" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        orientation = Orientation::from_str(&s);
                    }
                }
                "iconType" => {
                    if let Some(icon) = parse_icon_type(get_assignment_value(child)) {
                        children.push(icon);
                    }
                }
                "instantTextBoxType" => {
                    if let Some(text) = parse_textbox_type(get_assignment_value(child)) {
                        children.push(text);
                    }
                }
                "guiButtonType" => {
                    if let Some(button) = parse_button_type(get_assignment_value(child)) {
                        children.push(button);
                    }
                }
                "checkboxType" => {
                    if let Some(checkbox) = parse_checkbox_type(get_assignment_value(child)) {
                        children.push(checkbox);
                    }
                }
                "editBoxType" => {
                    if let Some(editbox) = parse_editbox_type(get_assignment_value(child)) {
                        children.push(editbox);
                    }
                }
                "listboxType" => {
                    if let Some(listbox) = parse_listbox_type(get_assignment_value(child)) {
                        children.push(listbox);
                    }
                }
                "scrollbarType" => {
                    if let Some(scrollbar) = parse_scrollbar_type(get_assignment_value(child)) {
                        children.push(scrollbar);
                    }
                }
                "windowType" => {
                    if let Some(window) = parse_window_type(get_assignment_value(child)) {
                        children.push(window);
                    }
                }
                _ => {}
            }
        }
    }

    Some(GuiElement::Window {
        name,
        position,
        size,
        orientation,
        children,
    })
}

/// Parse an iconType block.
fn parse_icon_type(node: Option<&EU4TxtParseNode>) -> Option<GuiElement> {
    let node = node?;

    let mut name = String::new();
    let mut position = (0i32, 0i32);
    let mut sprite_type = String::new();
    let mut frame = 0u32;
    let mut orientation = Orientation::UpperLeft;
    let mut scale = 1.0f32;

    for child in &node.children {
        if let EU4TxtAstItem::Assignment = &child.entry
            && let Some(key) = get_assignment_key(child)
        {
            match key.as_str() {
                "name" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        name = s;
                    }
                }
                "position" => {
                    position = parse_position(get_assignment_value(child));
                }
                "spriteType" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        sprite_type = s;
                    }
                }
                "frame" => {
                    if let Some(n) = get_int_value(get_assignment_value(child)) {
                        frame = n as u32;
                    }
                }
                "Orientation" | "orientation" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        orientation = Orientation::from_str(&s);
                    }
                }
                "scale" => {
                    if let Some(n) = get_float_value(get_assignment_value(child)) {
                        scale = n;
                    }
                }
                _ => {}
            }
        }
    }

    Some(GuiElement::Icon {
        name,
        position,
        sprite_type,
        frame,
        orientation,
        scale,
    })
}

/// Parse an instantTextBoxType block.
fn parse_textbox_type(node: Option<&EU4TxtParseNode>) -> Option<GuiElement> {
    let node = node?;

    let mut name = String::new();
    let mut position = (0i32, 0i32);
    let mut font = String::from("vic_18");
    let mut max_width = 200u32;
    let mut max_height = 32u32;
    let mut format = TextFormat::Left;
    let mut orientation = Orientation::UpperLeft;
    let mut text = String::new();
    let mut border_size = (0i32, 0i32);

    for child in &node.children {
        if let EU4TxtAstItem::Assignment = &child.entry
            && let Some(key) = get_assignment_key(child)
        {
            match key.as_str() {
                "name" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        name = s;
                    }
                }
                "position" => {
                    position = parse_position(get_assignment_value(child));
                }
                "font" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        font = s;
                    }
                }
                "maxWidth" => {
                    if let Some(n) = get_int_value(get_assignment_value(child)) {
                        max_width = n as u32;
                    }
                }
                "maxHeight" => {
                    if let Some(n) = get_int_value(get_assignment_value(child)) {
                        max_height = n as u32;
                    }
                }
                "format" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        format = TextFormat::from_str(&s);
                    }
                }
                "Orientation" | "orientation" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        orientation = Orientation::from_str(&s);
                    }
                }
                "text" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        text = s;
                    }
                }
                "borderSize" => {
                    border_size = parse_position(get_assignment_value(child));
                }
                _ => {}
            }
        }
    }

    Some(GuiElement::TextBox {
        name,
        position,
        font,
        max_width,
        max_height,
        format,
        orientation,
        text,
        border_size,
    })
}

/// Parse a guiButtonType block.
fn parse_button_type(node: Option<&EU4TxtParseNode>) -> Option<GuiElement> {
    let node = node?;

    let mut name = String::new();
    let mut position = (0i32, 0i32);
    let mut sprite_type = String::new();
    let mut orientation = Orientation::UpperLeft;
    let mut shortcut = None;

    for child in &node.children {
        if let EU4TxtAstItem::Assignment = &child.entry
            && let Some(key) = get_assignment_key(child)
        {
            match key.as_str() {
                "name" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        name = s;
                    }
                }
                "position" => {
                    position = parse_position(get_assignment_value(child));
                }
                "spriteType" | "quadTextureSprite" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        sprite_type = s;
                    }
                }
                "Orientation" | "orientation" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        orientation = Orientation::from_str(&s);
                    }
                }
                "shortcut" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        shortcut = Some(s);
                    }
                }
                _ => {}
            }
        }
    }

    Some(GuiElement::Button {
        name,
        position,
        sprite_type,
        orientation,
        shortcut,
    })
}

/// Parse a checkboxType block.
fn parse_checkbox_type(node: Option<&EU4TxtParseNode>) -> Option<GuiElement> {
    let node = node?;

    let mut name = String::new();
    let mut position = (0i32, 0i32);
    let mut sprite_type = String::new();
    let mut orientation = Orientation::UpperLeft;

    for child in &node.children {
        if let EU4TxtAstItem::Assignment = &child.entry
            && let Some(key) = get_assignment_key(child)
        {
            match key.as_str() {
                "name" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        name = s;
                    }
                }
                "position" => {
                    position = parse_position(get_assignment_value(child));
                }
                "spriteType" | "quadTextureSprite" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        sprite_type = s;
                    }
                }
                "Orientation" | "orientation" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        orientation = Orientation::from_str(&s);
                    }
                }
                _ => {}
            }
        }
    }

    Some(GuiElement::Checkbox {
        name,
        position,
        sprite_type,
        orientation,
    })
}

/// Parse an editBoxType block.
fn parse_editbox_type(node: Option<&EU4TxtParseNode>) -> Option<GuiElement> {
    let node = node?;

    let mut name = String::new();
    let mut position = (0i32, 0i32);
    let mut size = (0u32, 0u32);
    let mut font = String::from("default");
    let mut orientation = Orientation::UpperLeft;
    let mut max_characters = 256; // Default reasonable limit

    for child in &node.children {
        if let EU4TxtAstItem::Assignment = &child.entry
            && let Some(key) = get_assignment_key(child)
        {
            match key.as_str() {
                "name" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        name = s;
                    }
                }
                "position" => {
                    position = parse_position(get_assignment_value(child));
                }
                "size" => {
                    size = parse_size(get_assignment_value(child));
                }
                "font" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        font = s;
                    }
                }
                "Orientation" | "orientation" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        orientation = Orientation::from_str(&s);
                    }
                }
                "max_characters" | "maxCharacters" => {
                    if let Some(n) = get_int_value(get_assignment_value(child)) {
                        max_characters = n.max(0) as u32;
                    }
                }
                _ => {}
            }
        }
    }

    Some(GuiElement::EditBox {
        name,
        position,
        size,
        font,
        orientation,
        max_characters,
    })
}

/// Parse a listboxType block (Phase 7).
fn parse_listbox_type(node: Option<&EU4TxtParseNode>) -> Option<GuiElement> {
    let node = node?;

    let mut name = String::new();
    let mut position = (0i32, 0i32);
    let mut size = (100u32, 100u32);
    let mut orientation = Orientation::UpperLeft;
    let mut spacing = 0i32;
    let mut scrollbar_type = None;
    let mut background = None;

    for child in &node.children {
        if let EU4TxtAstItem::Assignment = &child.entry
            && let Some(key) = get_assignment_key(child)
        {
            match key.as_str() {
                "name" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        name = s;
                    }
                }
                "position" => {
                    position = parse_position(get_assignment_value(child));
                }
                "size" => {
                    size = parse_size(get_assignment_value(child));
                }
                "Orientation" | "orientation" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        orientation = Orientation::from_str(&s);
                    }
                }
                "spacing" => {
                    if let Some(n) = get_int_value(get_assignment_value(child)) {
                        spacing = n;
                    }
                }
                "scrollbartype" | "scrollbarType" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        scrollbar_type = Some(s);
                    }
                }
                "background" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        background = Some(s);
                    }
                }
                _ => {}
            }
        }
    }

    Some(GuiElement::Listbox {
        name,
        position,
        size,
        orientation,
        spacing,
        scrollbar_type,
        background,
    })
}

/// Parse a scrollbarType block (Phase 7).
fn parse_scrollbar_type(node: Option<&EU4TxtParseNode>) -> Option<GuiElement> {
    let node = node?;

    let mut name = String::new();
    let mut position = (0i32, 0i32);
    let mut size = (20u32, 100u32); // Default scrollbar dimensions
    let mut orientation = Orientation::UpperLeft;
    let mut max_value = 100i32;
    let mut track_sprite = None;
    let mut slider_sprite = None;

    for child in &node.children {
        if let EU4TxtAstItem::Assignment = &child.entry
            && let Some(key) = get_assignment_key(child)
        {
            match key.as_str() {
                "name" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        name = s;
                    }
                }
                "position" => {
                    position = parse_position(get_assignment_value(child));
                }
                "size" => {
                    size = parse_size(get_assignment_value(child));
                }
                "Orientation" | "orientation" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        orientation = Orientation::from_str(&s);
                    }
                }
                "maxValue" | "maxvalue" => {
                    if let Some(n) = get_int_value(get_assignment_value(child)) {
                        max_value = n;
                    }
                }
                "track" | "trackSprite" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        track_sprite = Some(s);
                    }
                }
                "slider" | "sliderSprite" => {
                    if let Some(s) = get_string_value(get_assignment_value(child)) {
                        slider_sprite = Some(s);
                    }
                }
                _ => {}
            }
        }
    }

    Some(GuiElement::Scrollbar {
        name,
        position,
        size,
        orientation,
        max_value,
        track_sprite,
        slider_sprite,
    })
}

/// Parse a position = { x = N y = N } block.
fn parse_position(node: Option<&EU4TxtParseNode>) -> (i32, i32) {
    let Some(node) = node else {
        return (0, 0);
    };

    let mut x = 0i32;
    let mut y = 0i32;

    for child in &node.children {
        if let EU4TxtAstItem::Assignment = &child.entry
            && let Some(key) = get_assignment_key(child)
        {
            match key.as_str() {
                "x" => {
                    if let Some(n) = get_int_value(get_assignment_value(child)) {
                        x = n;
                    }
                }
                "y" => {
                    if let Some(n) = get_int_value(get_assignment_value(child)) {
                        y = n;
                    }
                }
                _ => {}
            }
        }
    }

    (x, y)
}

/// Parse a size = { x = N y = N } block.
fn parse_size(node: Option<&EU4TxtParseNode>) -> (u32, u32) {
    let pos = parse_position(node);
    (pos.0.max(0) as u32, pos.1.max(0) as u32)
}

// Helper functions to extract values from AST nodes

fn get_assignment_key(node: &EU4TxtParseNode) -> Option<String> {
    let key_node = node.children.first()?;
    match &key_node.entry {
        EU4TxtAstItem::Identifier(s) | EU4TxtAstItem::StringValue(s) => Some(s.clone()),
        _ => None,
    }
}

fn get_assignment_value(node: &EU4TxtParseNode) -> Option<&EU4TxtParseNode> {
    node.children.get(1)
}

fn get_string_value(node: Option<&EU4TxtParseNode>) -> Option<String> {
    let node = node?;
    match &node.entry {
        EU4TxtAstItem::StringValue(s) | EU4TxtAstItem::Identifier(s) => Some(s.clone()),
        _ => None,
    }
}

fn get_int_value(node: Option<&EU4TxtParseNode>) -> Option<i32> {
    let node = node?;
    match &node.entry {
        EU4TxtAstItem::IntValue(n) => Some(*n),
        EU4TxtAstItem::FloatValue(f) => Some(*f as i32),
        _ => None,
    }
}

fn get_float_value(node: Option<&EU4TxtParseNode>) -> Option<f32> {
    let node = node?;
    match &node.entry {
        EU4TxtAstItem::FloatValue(f) => Some(*f),
        EU4TxtAstItem::IntValue(n) => Some(*n as f32),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_simple_sprite() {
        let content = r#"
spriteTypes = {
    spriteType = {
        name = "GFX_speed_indicator"
        texturefile = "gfx/interface/speed_indicator.dds"
        noOfFrames = 10
    }
}
"#;

        let mut file = tempfile::NamedTempFile::new().unwrap();
        write!(file, "{}", content).unwrap();
        let path = file.path();

        let db = parse_gfx_file(path).unwrap();
        assert_eq!(db.sprites.len(), 1);

        let sprite = db.get("GFX_speed_indicator").unwrap();
        assert_eq!(sprite.texture_file, "gfx/interface/speed_indicator.dds");
        assert_eq!(sprite.num_frames, 10);
    }

    #[test]
    fn test_parse_simple_gui() {
        let content = r#"
guiTypes = {
    windowType = {
        name = "test_window"
        position = { x = 100 y = 50 }
        size = { x = 200 y = 100 }
        Orientation = "UPPER_RIGHT"

        iconType = {
            name = "test_icon"
            position = { x = 10 y = 20 }
            spriteType = "GFX_test"
            frame = 5
        }
    }
}
"#;

        let mut file = tempfile::NamedTempFile::new().unwrap();
        write!(file, "{}", content).unwrap();
        let path = file.path();

        let interner = StringInterner::new();
        let db = parse_gui_file(path, &interner).unwrap();
        assert_eq!(db.len(), 1);

        let symbol = interner.get("test_window").unwrap();
        let window = db.get(&symbol).unwrap();

        if let GuiElement::Window {
            name,
            position,
            size,
            orientation,
            children,
        } = window
        {
            assert_eq!(name, "test_window");
            assert_eq!(*position, (100, 50));
            assert_eq!(*size, (200, 100));
            assert_eq!(*orientation, Orientation::UpperRight);
            assert_eq!(children.len(), 1);

            if let GuiElement::Icon {
                name,
                position,
                sprite_type,
                frame,
                ..
            } = &children[0]
            {
                assert_eq!(name, "test_icon");
                assert_eq!(*position, (10, 20));
                assert_eq!(sprite_type, "GFX_test");
                assert_eq!(*frame, 5);
            } else {
                panic!("Expected Icon");
            }
        } else {
            panic!("Expected Window");
        }
    }

    #[test]
    fn test_sprite_frame_uv() {
        let sprite = GfxSprite {
            name: "test".to_string(),
            texture_file: "test.dds".to_string(),
            num_frames: 5,
            horizontal_frames: true,
        };

        let (u_min, v_min, u_max, v_max) = sprite.frame_uv(0);
        assert!((u_min - 0.0).abs() < 0.001);
        assert!((u_max - 0.2).abs() < 0.001);
        assert!((v_min - 0.0).abs() < 0.001);
        assert!((v_max - 1.0).abs() < 0.001);

        let (u_min, _, u_max, _) = sprite.frame_uv(2);
        assert!((u_min - 0.4).abs() < 0.001);
        assert!((u_max - 0.6).abs() < 0.001);
    }

    #[test]
    fn test_parse_cornered_tile_sprite() {
        let content = r#"
spriteTypes = {
    corneredTileSpriteType = {
        name = "GFX_country_selection_panel_bg"
        size = { x = 320 y = 704 }
        texturefile = "gfx//interface//tiles_dialog.tga"
        borderSize = { x = 32 y = 32 }
    }
}
"#;

        let mut file = tempfile::NamedTempFile::new().unwrap();
        write!(file, "{}", content).unwrap();
        let path = file.path();

        let db = parse_gfx_file(path).unwrap();
        assert_eq!(db.cornered_tiles.len(), 1);

        let tile = db
            .get_cornered_tile("GFX_country_selection_panel_bg")
            .unwrap();
        assert_eq!(tile.texture_file, "gfx//interface//tiles_dialog.tga");
        assert_eq!(tile.size, (320, 704));
        assert_eq!(tile.border_size, (32, 32));
    }

    #[test]
    fn test_parse_mixed_mode_gui() {
        // Mixed mode: Top-level windowType alongside guiTypes block (standard EU4 pattern)
        let content = r#"
windowType = {
    name = "main_panel"
    position = { x = 0 y = 0 }
    size = { x = 100 y = 100 }
}

guiTypes = {
    windowType = {
        name = "list_item_template"
        position = { x = 0 y = 0 }
        size = { x = 50 y = 20 }
        
        iconType = {
            name = "icon"
            spriteType = "GFX_icon"
        }
    }
}
"#;

        let mut file = tempfile::NamedTempFile::new().unwrap();
        write!(file, "{}", content).unwrap();
        let path = file.path();

        let interner = StringInterner::new();
        let db = parse_gui_file(path, &interner).unwrap();

        assert_eq!(db.len(), 2);
        assert!(db.contains_key(&interner.intern("main_panel")));
        assert!(db.contains_key(&interner.intern("list_item_template")));

        let template = db.get(&interner.intern("list_item_template")).unwrap();
        assert_eq!(template.children().len(), 1);
    }

    #[test]
    fn test_parse_entry_template() {
        // Entry template: windowType used as a listbox row template (Phase 7.2)
        let content = r#"
guiTypes = {
    windowType = {
        name = "savegameentry"
        position = { x = 0 y = 0 }
        size = { x = 320 y = 41 }

        checkboxType = {
            name = "save_game"
            position = { x = 14 y = 0 }
        }

        guiButtonType = {
            name = "save_game_shield"
            position = { x = 17 y = 5 }
            quadTextureSprite = "GFX_shield_small"
        }

        instantTextBoxType = {
            name = "save_game_title"
            position = { x = 52 y = 5 }
            font = "vic_18"
            maxWidth = 132
            maxHeight = 20
        }

        iconType = {
            name = "ironman_icon"
            spriteType = "GFX_ironman_icon"
            position = { x = 200 y = 7 }
            scale = 0.8
        }
    }
}
"#;

        let mut file = tempfile::NamedTempFile::new().unwrap();
        write!(file, "{}", content).unwrap();
        let path = file.path();

        let interner = StringInterner::new();
        let db = parse_gui_file(path, &interner).unwrap();
        assert_eq!(db.len(), 1);

        let symbol = interner.get("savegameentry").unwrap();
        let entry = db.get(&symbol).unwrap();

        if let GuiElement::Window {
            name,
            size,
            children,
            ..
        } = entry
        {
            assert_eq!(name, "savegameentry");
            assert_eq!(*size, (320, 41)); // Row height defined by entry template
            assert_eq!(children.len(), 4); // checkbox, button, text, icon

            // Verify all widget types are parsed
            let has_checkbox = children
                .iter()
                .any(|c| matches!(c, GuiElement::Checkbox { .. }));
            let has_button = children
                .iter()
                .any(|c| matches!(c, GuiElement::Button { .. }));
            let has_text = children
                .iter()
                .any(|c| matches!(c, GuiElement::TextBox { .. }));
            let has_icon = children
                .iter()
                .any(|c| matches!(c, GuiElement::Icon { .. }));

            assert!(has_checkbox, "Entry should have checkbox widget");
            assert!(has_button, "Entry should have button widget");
            assert!(has_text, "Entry should have text widget");
            assert!(has_icon, "Entry should have icon widget");
        } else {
            panic!("Expected Window");
        }
    }
}
