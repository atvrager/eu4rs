//! EU4 GUI system.
//!
//! Parses EU4's .gui and .gfx layout files to render authentic UI
//! using the game's actual sprites and positions.

#[allow(dead_code)]
pub mod layout;
pub mod parser;
pub mod sprite_cache;
#[allow(dead_code)]
pub mod types;

pub use layout::{get_window_anchor, position_from_anchor, rect_to_clip_space};
pub use parser::{parse_gfx_file, parse_gui_file};
pub use sprite_cache::SpriteCache;
pub use types::{GfxDatabase, GuiAction, GuiElement, GuiState, HitBox, Orientation};

use crate::bmfont::BitmapFontCache;
use crate::render::SpriteRenderer;
use std::path::Path;

/// Loaded speed controls layout.
pub struct SpeedControls {
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
}

impl Default for SpeedControls {
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
#[allow(dead_code)] // Will be used for topbar text rendering
#[derive(Debug, Clone)]
pub struct TopBarText {
    pub name: String,
    pub position: (i32, i32),
    pub font: String,
    pub max_width: u32,
    pub orientation: Orientation,
}

/// Main topbar layout data.
pub struct TopBar {
    /// Window position.
    pub window_pos: (i32, i32),
    /// Window orientation.
    pub orientation: Orientation,
    /// Background icons (rendered first).
    pub backgrounds: Vec<TopBarIcon>,
    /// Resource icons (gold, manpower, etc).
    pub icons: Vec<TopBarIcon>,
    /// Text labels for resources.
    pub texts: Vec<TopBarText>,
}

impl Default for TopBar {
    fn default() -> Self {
        Self {
            window_pos: (0, -1),
            orientation: Orientation::UpperLeft,
            backgrounds: vec![],
            icons: vec![],
            texts: vec![],
        }
    }
}

/// GUI renderer that uses EU4's authentic layout and sprites.
pub struct GuiRenderer {
    /// Sprite database from .gfx files.
    gfx_db: GfxDatabase,
    /// Sprite texture cache.
    sprite_cache: SpriteCache,
    /// Bitmap font cache.
    font_cache: BitmapFontCache,
    /// Speed controls layout.
    speed_controls: SpeedControls,
    /// Main topbar layout.
    topbar: TopBar,
    /// Cached bind groups for frequently used sprites.
    bg_bind_group: Option<wgpu::BindGroup>,
    speed_bind_group: Option<wgpu::BindGroup>,
    /// Font texture bind group.
    font_bind_group: Option<wgpu::BindGroup>,
    /// Cached topbar icon bind groups: (sprite_name, bind_group, width, height).
    topbar_icons: Vec<(String, wgpu::BindGroup, u32, u32)>,
    /// Cached button bind groups: (button_name, bind_group, width, height).
    button_bind_groups: Vec<(String, wgpu::BindGroup, u32, u32)>,
    /// Hit boxes for interactive elements (screen pixel coords).
    hit_boxes: Vec<(String, HitBox)>,
    /// Background sprite dimensions.
    bg_size: (u32, u32),
    /// Speed indicator dimensions (per frame).
    speed_size: (u32, u32),
}

impl GuiRenderer {
    /// Create a new GUI renderer.
    pub fn new(game_path: &Path) -> Self {
        let mut gfx_db = GfxDatabase::default();

        // Load relevant .gfx files
        let gfx_files = ["interface/speed_controls.gfx", "interface/topbar.gfx"];

        for gfx_file in &gfx_files {
            let path = game_path.join(gfx_file);
            if path.exists() {
                match parse_gfx_file(&path) {
                    Ok(db) => {
                        log::info!("Loaded {} sprites from {}", db.sprites.len(), gfx_file);
                        gfx_db.merge(db);
                    }
                    Err(e) => {
                        log::warn!("Failed to parse {}: {}", gfx_file, e);
                    }
                }
            }
        }

        // Load speed_controls.gui layout
        let speed_controls = load_speed_controls(game_path);

        // Load topbar.gui layout
        let topbar = load_topbar(game_path);

        Self {
            gfx_db,
            sprite_cache: SpriteCache::new(game_path.to_path_buf()),
            font_cache: BitmapFontCache::new(game_path),
            speed_controls,
            topbar,
            bg_bind_group: None,
            speed_bind_group: None,
            font_bind_group: None,
            topbar_icons: Vec::new(),
            button_bind_groups: Vec::new(),
            hit_boxes: Vec::new(),
            bg_size: (1, 1),    // Updated from texture in ensure_textures()
            speed_size: (1, 1), // Updated from texture in ensure_textures()
        }
    }

    /// Ensure textures are loaded and bind groups created.
    fn ensure_textures(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sprite_renderer: &SpriteRenderer,
    ) {
        // Load background texture
        if self.bg_bind_group.is_none()
            && let Some(sprite) = self.gfx_db.get(&self.speed_controls.bg_sprite)
            && let Some((view, w, h)) = self.sprite_cache.get(&sprite.texture_file, device, queue)
        {
            log::debug!(
                "Loaded bg texture: {} -> {}x{} (window_pos={:?}, orientation={:?})",
                sprite.texture_file,
                w,
                h,
                self.speed_controls.window_pos,
                self.speed_controls.orientation
            );
            self.bg_size = (w, h);
            self.bg_bind_group = Some(sprite_renderer.create_bind_group(device, view));
        }

        // Load speed indicator texture
        if self.speed_bind_group.is_none()
            && let Some(sprite) = self.gfx_db.get(&self.speed_controls.speed_sprite)
            && let Some((view, w, h)) = self.sprite_cache.get(&sprite.texture_file, device, queue)
        {
            // Speed indicator is a horizontal strip - frame height = total / frames
            let num_frames = sprite.num_frames.max(1);
            log::debug!(
                "Loaded speed indicator: {} -> {}x{}, {} frames, frame_size={}x{}",
                sprite.texture_file,
                w,
                h,
                num_frames,
                w / num_frames,
                h
            );
            self.speed_size = (w / num_frames, h);
            self.speed_bind_group = Some(sprite_renderer.create_bind_group(device, view));
        }

        // Load button textures
        if self.button_bind_groups.is_empty() {
            for (name, _, _, sprite_name) in &self.speed_controls.buttons {
                if let Some(sprite) = self.gfx_db.get(sprite_name)
                    && let Some((view, w, h)) =
                        self.sprite_cache.get(&sprite.texture_file, device, queue)
                {
                    let bind_group = sprite_renderer.create_bind_group(device, view);
                    log::debug!(
                        "Loaded button texture {}: {} -> {}x{}",
                        name,
                        sprite.texture_file,
                        w,
                        h
                    );
                    self.button_bind_groups
                        .push((name.clone(), bind_group, w, h));
                }
            }
        }
    }

    /// Ensure the font texture is loaded.
    fn ensure_font(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sprite_renderer: &SpriteRenderer,
    ) {
        if self.font_bind_group.is_none() {
            let font_name = &self.speed_controls.date_font;
            if let Some(loaded) = self.font_cache.get(font_name, device, queue) {
                self.font_bind_group =
                    Some(sprite_renderer.create_bind_group(device, &loaded.view));
            }
        }
    }

    /// Ensure topbar icon textures are loaded.
    fn ensure_topbar_textures(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sprite_renderer: &SpriteRenderer,
    ) {
        // Only load once
        if !self.topbar_icons.is_empty() {
            return;
        }

        // Collect all sprites we need
        let all_icons: Vec<_> = self
            .topbar
            .backgrounds
            .iter()
            .chain(self.topbar.icons.iter())
            .collect();

        for icon in all_icons {
            if let Some(sprite) = self.gfx_db.get(&icon.sprite)
                && let Some((view, w, h)) =
                    self.sprite_cache.get(&sprite.texture_file, device, queue)
            {
                let bind_group = sprite_renderer.create_bind_group(device, view);
                self.topbar_icons
                    .push((icon.sprite.clone(), bind_group, w, h));
                log::debug!("Loaded topbar texture: {} -> {}x{}", icon.sprite, w, h);
            }
        }
    }

    /// Render the GUI overlay.
    #[allow(clippy::too_many_arguments)]
    pub fn render<'a>(
        &'a mut self,
        render_pass: &mut wgpu::RenderPass<'a>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sprite_renderer: &'a SpriteRenderer,
        state: &GuiState,
        screen_size: (u32, u32),
    ) {
        self.ensure_textures(device, queue, sprite_renderer);
        self.ensure_font(device, queue, sprite_renderer);
        self.ensure_topbar_textures(device, queue, sprite_renderer);
        self.hit_boxes.clear();

        // Collect topbar draw commands first (to avoid borrowing self during draw)
        let topbar_draws: Vec<(usize, f32, f32, f32, f32)> = {
            let topbar_anchor =
                get_window_anchor(self.topbar.window_pos, self.topbar.orientation, screen_size);

            let mut draws = Vec::new();

            // Backgrounds first
            for bg in &self.topbar.backgrounds {
                if let Some(idx) = self
                    .topbar_icons
                    .iter()
                    .position(|(name, _, _, _)| name == &bg.sprite)
                {
                    let (_, _, w, h) = &self.topbar_icons[idx];
                    let screen_pos =
                        position_from_anchor(topbar_anchor, bg.position, bg.orientation, (*w, *h));
                    let (clip_x, clip_y, clip_w, clip_h) =
                        rect_to_clip_space(screen_pos, (*w, *h), screen_size);
                    draws.push((idx, clip_x, clip_y, clip_w, clip_h));
                }
            }

            // Icons
            for icon in &self.topbar.icons {
                if let Some(idx) = self
                    .topbar_icons
                    .iter()
                    .position(|(name, _, _, _)| name == &icon.sprite)
                {
                    let (_, _, w, h) = &self.topbar_icons[idx];
                    let screen_pos = position_from_anchor(
                        topbar_anchor,
                        icon.position,
                        icon.orientation,
                        (*w, *h),
                    );
                    let (clip_x, clip_y, clip_w, clip_h) =
                        rect_to_clip_space(screen_pos, (*w, *h), screen_size);
                    draws.push((idx, clip_x, clip_y, clip_w, clip_h));
                }
            }

            draws
        };

        // Execute topbar draws
        for (idx, clip_x, clip_y, clip_w, clip_h) in topbar_draws {
            let bind_group = &self.topbar_icons[idx].1;
            sprite_renderer.draw(
                render_pass,
                bind_group,
                queue,
                clip_x,
                clip_y,
                clip_w,
                clip_h,
            );
        }

        // Get window anchor point - window is just an anchor, not a rectangle
        let window_anchor = get_window_anchor(
            self.speed_controls.window_pos,
            self.speed_controls.orientation,
            screen_size,
        );

        // Draw background at its own position relative to window anchor
        if let Some(ref bind_group) = self.bg_bind_group {
            let bg_screen_pos = position_from_anchor(
                window_anchor,
                self.speed_controls.bg_pos,
                self.speed_controls.bg_orientation,
                self.bg_size,
            );

            let (clip_x, clip_y, clip_w, clip_h) =
                rect_to_clip_space(bg_screen_pos, self.bg_size, screen_size);

            sprite_renderer.draw(
                render_pass,
                bind_group,
                queue,
                clip_x,
                clip_y,
                clip_w,
                clip_h,
            );
        }

        // Draw button backgrounds/chrome BEFORE speed indicator and text
        // (button_pause is a background element that goes behind the date)
        let button_draws: Vec<(usize, f32, f32, f32, f32)> = {
            let mut draws = Vec::new();
            for (name, pos, orientation, _) in &self.speed_controls.buttons {
                // Find the bind group index for this button
                if let Some(idx) = self
                    .button_bind_groups
                    .iter()
                    .position(|(n, _, _, _)| n == name)
                {
                    let (_, _, w, h) = &self.button_bind_groups[idx];
                    let screen_pos =
                        position_from_anchor(window_anchor, *pos, *orientation, (*w, *h));
                    let (clip_x, clip_y, clip_w, clip_h) =
                        rect_to_clip_space(screen_pos, (*w, *h), screen_size);
                    draws.push((idx, clip_x, clip_y, clip_w, clip_h));
                }
            }
            draws
        };

        for (idx, clip_x, clip_y, clip_w, clip_h) in button_draws {
            let bind_group = &self.button_bind_groups[idx].1;
            sprite_renderer.draw(
                render_pass,
                bind_group,
                queue,
                clip_x,
                clip_y,
                clip_w,
                clip_h,
            );
        }

        // Draw speed indicator
        if let Some(ref bind_group) = self.speed_bind_group {
            // Select frame based on state
            // EU4 speed_indicator.dds: frames 0-4 = speeds 1-5, frame 5 = paused
            let frame = if state.paused {
                5
            } else {
                (state.speed.saturating_sub(1)).min(4)
            };

            // Speed indicator position relative to window anchor
            let speed_screen_pos = position_from_anchor(
                window_anchor,
                self.speed_controls.speed_pos,
                self.speed_controls.speed_orientation,
                self.speed_size,
            );

            // Get UVs for this frame
            if let Some(sprite) = self.gfx_db.get(&self.speed_controls.speed_sprite) {
                let (u_min, v_min, u_max, v_max) = sprite.frame_uv(frame);

                let (clip_x, clip_y, clip_w, clip_h) =
                    rect_to_clip_space(speed_screen_pos, self.speed_size, screen_size);

                sprite_renderer.draw_uv(
                    render_pass,
                    bind_group,
                    queue,
                    clip_x,
                    clip_y,
                    clip_w,
                    clip_h,
                    u_min,
                    v_min,
                    u_max,
                    v_max,
                );
            }
        }

        // Draw date text using bitmap font (on top of buttons)
        // Text position relative to window anchor
        let text_box_size = (
            self.speed_controls.date_max_width,
            self.speed_controls.date_max_height,
        );
        let date_screen_pos = position_from_anchor(
            window_anchor,
            self.speed_controls.date_pos,
            self.speed_controls.date_orientation,
            text_box_size,
        );

        // Render text using bitmap font
        if let Some(ref font_bind_group) = self.font_bind_group {
            let font_name = &self.speed_controls.date_font;
            if let Some(loaded) = self.font_cache.get(font_name, device, queue) {
                let font = &loaded.font;

                // Measure text width for centering
                let text_width = font.measure_width(&state.date);

                // Apply border/padding
                // In EU4, borderSize.y is top offset, format=centre is horizontal only
                let border = self.speed_controls.date_border_size;

                // Center horizontally within text box
                let start_x = date_screen_pos.0 + (text_box_size.0 as f32 - text_width) / 2.0;
                // Vertical: use borderSize.y as top offset (not centering)
                let start_y = date_screen_pos.1 + border.1 as f32;

                let mut cursor_x = start_x;

                for c in state.date.chars() {
                    if let Some(glyph) = font.get_glyph(c) {
                        if glyph.width > 0 && glyph.height > 0 {
                            let glyph_x = cursor_x + glyph.xoffset as f32;
                            let glyph_y = start_y + glyph.yoffset as f32;

                            let (u_min, v_min, u_max, v_max) = font.glyph_uv(glyph);

                            let (clip_x, clip_y, clip_w, clip_h) = rect_to_clip_space(
                                (glyph_x, glyph_y),
                                (glyph.width, glyph.height),
                                screen_size,
                            );

                            sprite_renderer.draw_uv(
                                render_pass,
                                font_bind_group,
                                queue,
                                clip_x,
                                clip_y,
                                clip_w,
                                clip_h,
                                u_min,
                                v_min,
                                u_max,
                                v_max,
                            );
                        }
                        cursor_x += glyph.xadvance as f32;
                    }
                }
            }
        }

        // Register hit boxes for speed controls from parsed button positions
        for (name, pos, orientation, sprite_name) in &self.speed_controls.buttons {
            // Get button size from sprite dimensions if available
            let button_size = self
                .gfx_db
                .get(sprite_name)
                .and_then(|sprite| {
                    self.sprite_cache
                        .get_dimensions(&sprite.texture_file)
                        .map(|(w, h)| (w as f32, h as f32))
                })
                .unwrap_or((32.0, 32.0)); // Fallback if sprite not found

            let button_screen_pos = position_from_anchor(
                window_anchor,
                *pos,
                *orientation,
                (button_size.0 as u32, button_size.1 as u32),
            );

            let hit_box = HitBox {
                x: button_screen_pos.0,
                y: button_screen_pos.1,
                width: button_size.0,
                height: button_size.1,
            };

            // Map button names to action names
            let action_name = match name.as_str() {
                "button_speedup" => "speed_up",
                "button_speeddown" => "speed_down",
                "button_pause" => "pause",
                _ => name.as_str(),
            };

            self.hit_boxes.push((action_name.to_string(), hit_box));
        }
    }

    /// Handle a click at screen coordinates.
    /// Returns an action if a GUI element was clicked.
    pub fn handle_click(&self, x: f32, y: f32, current_state: &GuiState) -> Option<GuiAction> {
        for (name, hit_box) in &self.hit_boxes {
            if hit_box.contains(x, y) {
                return match name.as_str() {
                    "speed_up" => {
                        let new_speed = (current_state.speed + 1).min(5);
                        Some(GuiAction::SetSpeed(new_speed))
                    }
                    "speed_down" => {
                        let new_speed = current_state.speed.saturating_sub(1).max(1);
                        Some(GuiAction::SetSpeed(new_speed))
                    }
                    "pause" => Some(GuiAction::TogglePause),
                    _ => None,
                };
            }
        }
        None
    }
}

/// Load speed controls layout from game files.
fn load_speed_controls(game_path: &Path) -> SpeedControls {
    let gui_path = game_path.join("interface/speed_controls.gui");

    if !gui_path.exists() {
        log::warn!("speed_controls.gui not found, using defaults");
        return SpeedControls::default();
    }

    match parse_gui_file(&gui_path) {
        Ok(elements) => {
            // Find the speed_controls window
            for element in &elements {
                if let GuiElement::Window {
                    name,
                    position,
                    orientation,
                    children,
                    ..
                } = element
                    && name == "speed_controls"
                {
                    return extract_speed_controls(position, orientation, children);
                }
            }
            log::warn!("speed_controls window not found in GUI file");
            SpeedControls::default()
        }
        Err(e) => {
            log::warn!("Failed to parse speed_controls.gui: {}", e);
            SpeedControls::default()
        }
    }
}

/// Extract speed controls data from parsed GUI elements.
fn extract_speed_controls(
    window_pos: &(i32, i32),
    orientation: &Orientation,
    children: &[GuiElement],
) -> SpeedControls {
    let mut controls = SpeedControls {
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

/// Load topbar layout from game files.
fn load_topbar(game_path: &Path) -> TopBar {
    let gui_path = game_path.join("interface/topbar.gui");

    if !gui_path.exists() {
        log::warn!("topbar.gui not found, using defaults");
        return TopBar::default();
    }

    match parse_gui_file(&gui_path) {
        Ok(elements) => {
            // Find the topbar window
            for element in &elements {
                if let GuiElement::Window {
                    name,
                    position,
                    orientation,
                    children,
                    ..
                } = element
                    && name == "topbar"
                {
                    return extract_topbar(position, orientation, children);
                }
            }
            log::warn!("topbar window not found in GUI file");
            TopBar::default()
        }
        Err(e) => {
            log::warn!("Failed to parse topbar.gui: {}", e);
            TopBar::default()
        }
    }
}

/// Extract topbar data from parsed GUI elements.
fn extract_topbar(
    window_pos: &(i32, i32),
    orientation: &Orientation,
    children: &[GuiElement],
) -> TopBar {
    let mut topbar = TopBar {
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
    ];

    // Resource icon names we want to render
    let icon_names = [
        "icon_gold",
        "icon_manpower",
        "icon_sailors",
        "icon_stability",
        "icon_prestige",
        "icon_ADM",
        "icon_DIP",
        "icon_MIL",
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

                if bg_names.contains(&name.as_str()) {
                    log::debug!(
                        "Parsed topbar bg {}: pos={:?}, sprite={}",
                        name,
                        position,
                        sprite_type
                    );
                    topbar.backgrounds.push(icon);
                } else if icon_names.contains(&name.as_str()) {
                    log::debug!(
                        "Parsed topbar icon {}: pos={:?}, sprite={}",
                        name,
                        position,
                        sprite_type
                    );
                    topbar.icons.push(icon);
                }
            }
            GuiElement::Button {
                name,
                sprite_type,
                position,
                orientation,
                ..
            } => {
                // Some icons are buttons (like mana icons)
                if icon_names.contains(&name.as_str()) {
                    log::debug!(
                        "Parsed topbar button-icon {}: pos={:?}, sprite={}",
                        name,
                        position,
                        sprite_type
                    );
                    topbar.icons.push(TopBarIcon {
                        name: name.clone(),
                        sprite: sprite_type.clone(),
                        position: *position,
                        orientation: *orientation,
                    });
                }
            }
            GuiElement::TextBox {
                name,
                position,
                font,
                max_width,
                orientation,
                ..
            } => {
                // Text labels for resources
                if name.starts_with("text_") {
                    log::debug!(
                        "Parsed topbar text {}: pos={:?}, font={}",
                        name,
                        position,
                        font
                    );
                    topbar.texts.push(TopBarText {
                        name: name.clone(),
                        position: *position,
                        font: font.clone(),
                        max_width: *max_width,
                        orientation: *orientation,
                    });
                }
            }
            _ => {}
        }
    }

    log::info!(
        "Loaded topbar: {} backgrounds, {} icons, {} texts",
        topbar.backgrounds.len(),
        topbar.icons.len(),
        topbar.texts.len()
    );

    topbar
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::SpriteRenderer;
    use crate::testing::{HeadlessGpu, assert_snapshot};
    use image::RgbaImage;

    fn get_test_context() -> Option<(HeadlessGpu, std::path::PathBuf)> {
        // Try to get GPU
        let gpu = pollster::block_on(HeadlessGpu::new())?;

        // Try to get game path
        let game_path = eu4data::path::detect_game_path()?;

        Some((gpu, game_path))
    }

    /// Render GUI to an image for snapshot testing.
    ///
    /// This function encapsulates all rendering logic to avoid closure lifetime issues.
    fn render_gui_to_image(
        gpu: &HeadlessGpu,
        game_path: &std::path::Path,
        gui_state: &GuiState,
        screen_size: (u32, u32),
    ) -> RgbaImage {
        let format = gpu.format;
        let sprite_renderer = SpriteRenderer::new(&gpu.device, format);
        let mut gui_renderer = GuiRenderer::new(game_path);

        // Create offscreen texture
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Test Texture"),
            size: wgpu::Extent3d {
                width: screen_size.0,
                height: screen_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create readback buffer with proper alignment
        // wgpu requires COPY_BYTES_PER_ROW_ALIGNMENT (256 bytes)
        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = bytes_per_pixel * screen_size.0;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
        let buffer_size = (padded_bytes_per_row * screen_size.1) as wgpu::BufferAddress;
        let output_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Readback Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Render
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Test Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Test Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            sprite_renderer.begin_frame();
            gui_renderer.render(
                &mut render_pass,
                &gpu.device,
                &gpu.queue,
                &sprite_renderer,
                gui_state,
                screen_size,
            );
        }

        // Copy to buffer
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &output_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(screen_size.1),
                },
            },
            wgpu::Extent3d {
                width: screen_size.0,
                height: screen_size.1,
                depth_or_array_layers: 1,
            },
        );

        gpu.queue.submit(Some(encoder.finish()));

        // Read back
        let buffer_slice = output_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| tx.send(v).unwrap());
        gpu.device.poll(wgpu::Maintain::Wait);
        rx.recv().unwrap().unwrap();

        let data = buffer_slice.get_mapped_range();

        // Strip row padding if present
        let image = if padded_bytes_per_row != unpadded_bytes_per_row {
            let mut pixels = Vec::with_capacity((unpadded_bytes_per_row * screen_size.1) as usize);
            for row in 0..screen_size.1 {
                let row_start = (row * padded_bytes_per_row) as usize;
                let row_end = row_start + unpadded_bytes_per_row as usize;
                pixels.extend_from_slice(&data[row_start..row_end]);
            }
            RgbaImage::from_raw(screen_size.0, screen_size.1, pixels).unwrap()
        } else {
            RgbaImage::from_raw(screen_size.0, screen_size.1, data.to_vec()).unwrap()
        };

        drop(data);
        output_buffer.unmap();

        image
    }

    #[test]
    fn test_speed_controls_snapshot() {
        let Some((gpu, game_path)) = get_test_context() else {
            println!("Skipping test_speed_controls_snapshot: prerequisites not available");
            return;
        };

        let screen_size = (400, 200);
        let gui_state = GuiState {
            date: "11 November 1444".to_string(),
            speed: 3,
            paused: false,
        };

        let image = render_gui_to_image(&gpu, &game_path, &gui_state, screen_size);
        assert_snapshot(&image, "speed_controls");
    }

    #[test]
    fn test_topbar_snapshot() {
        let Some((gpu, game_path)) = get_test_context() else {
            println!("Skipping test_topbar_snapshot: prerequisites not available");
            return;
        };

        // Use a wider screen to show full topbar
        let screen_size = (1024, 100);
        let gui_state = GuiState {
            date: "11 November 1444".to_string(),
            speed: 1,
            paused: true,
        };

        let image = render_gui_to_image(&gpu, &game_path, &gui_state, screen_size);
        assert_snapshot(&image, "topbar");
    }

    #[test]
    fn test_gui_layout_coverage() {
        let Some((_, game_path)) = get_test_context() else {
            println!("Skipping test_gui_layout_coverage: prerequisites not available");
            return;
        };

        let gui_renderer = GuiRenderer::new(&game_path);

        // Check speed controls coverage
        let sc = &gui_renderer.speed_controls;
        assert!(
            !sc.bg_sprite.is_empty(),
            "Background sprite should be loaded"
        );
        assert!(!sc.speed_sprite.is_empty(), "Speed sprite should be loaded");
        assert!(!sc.date_font.is_empty(), "Date font should be specified");
        assert!(!sc.buttons.is_empty(), "Buttons should be parsed");

        println!("Speed controls layout coverage:");
        println!("  Background: {} at {:?}", sc.bg_sprite, sc.bg_pos);
        println!(
            "  Speed indicator: {} at {:?}",
            sc.speed_sprite, sc.speed_pos
        );
        println!("  Date text at {:?}, font: {}", sc.date_pos, sc.date_font);
        println!("  Buttons: {}", sc.buttons.len());
        for (name, pos, _, sprite) in &sc.buttons {
            println!("    - {} at {:?} ({})", name, pos, sprite);
        }

        // Check topbar coverage
        let tb = &gui_renderer.topbar;
        println!("\nTopbar layout coverage:");
        println!(
            "  Window pos: {:?}, orientation: {:?}",
            tb.window_pos, tb.orientation
        );
        println!("  Backgrounds: {}", tb.backgrounds.len());
        for bg in &tb.backgrounds {
            println!("    - {} at {:?} ({})", bg.name, bg.position, bg.sprite);
        }
        println!("  Icons: {}", tb.icons.len());
        for icon in &tb.icons {
            println!(
                "    - {} at {:?} ({})",
                icon.name, icon.position, icon.sprite
            );
        }
        println!("  Texts: {}", tb.texts.len());
        for text in &tb.texts {
            println!(
                "    - {} at {:?} (font: {})",
                text.name, text.position, text.font
            );
        }

        // Assert minimum expected elements
        assert!(
            !tb.backgrounds.is_empty(),
            "Should have at least 1 background"
        );
        assert!(tb.icons.len() >= 5, "Should have at least 5 icons");
    }
}
