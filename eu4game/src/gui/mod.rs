//! EU4 GUI system.
//!
//! Parses EU4's .gui and .gfx layout files to render authentic UI
//! using the game's actual sprites and positions.

#[allow(dead_code)] // Country select panel WIP
pub mod country_select;
#[allow(dead_code)]
pub mod layout;
pub mod parser;
pub mod sprite_cache;
#[allow(dead_code)]
pub mod types;

#[allow(unused_imports)] // SelectedCountryState used in tests
pub use country_select::{CountrySelectLayout, SelectedCountryState};
pub use layout::{
    compute_masked_flag_rect, get_window_anchor, position_from_anchor, rect_to_clip_space,
};
pub use parser::{parse_gfx_file, parse_gui_file};
pub use sprite_cache::SpriteCache;
pub use types::{
    CountryResources, GfxDatabase, GuiAction, GuiElement, GuiState, HitBox, Orientation,
};

use crate::bmfont::BitmapFontCache;
use crate::render::SpriteRenderer;
use country_select::{CountrySelectButton, CountrySelectIcon, CountrySelectText};
use std::path::Path;

/// Format a number with smart suffixes.
/// - Under 100K: show full number with commas (e.g., "25,000")
/// - 100K to 1M: show with K suffix (e.g., "150K")
/// - 1M+: show with M suffix (e.g., "1.5M")
fn format_k(value: i32) -> String {
    if value >= 1_000_000 {
        let millions = value as f32 / 1_000_000.0;
        if millions >= 10.0 {
            format!("{:.0}M", millions)
        } else {
            format!("{:.1}M", millions)
        }
    } else if value >= 100_000 {
        format!("{}K", value / 1000)
    } else if value >= 1000 {
        // Format with commas for values 1K-100K
        let s = format!("{}", value);
        let chars: Vec<char> = s.chars().collect();
        let mut result = String::new();
        for (i, c) in chars.iter().enumerate() {
            if i > 0 && (chars.len() - i).is_multiple_of(3) {
                result.push(',');
            }
            result.push(*c);
        }
        result
    } else {
        format!("{}", value)
    }
}

/// Icon element from speed controls layout.
#[derive(Debug, Clone)]
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
    /// Additional icons (score icon, etc).
    pub icons: Vec<SpeedControlsIcon>,
    /// Additional text labels (score, rank, etc).
    pub texts: Vec<SpeedControlsText>,
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
pub struct TopBarText {
    pub name: String,
    pub position: (i32, i32),
    #[allow(dead_code)] // Will be used for font selection
    pub font: String,
    pub max_width: u32,
    pub max_height: u32,
    pub orientation: Orientation,
    pub format: types::TextFormat,
    pub border_size: (i32, i32),
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
    /// Player shield position (for flag display).
    pub player_shield: Option<TopBarIcon>,
}

impl Default for TopBar {
    fn default() -> Self {
        Self {
            window_pos: (0, -1),
            orientation: Orientation::UpperLeft,
            backgrounds: vec![],
            icons: vec![],
            texts: vec![],
            player_shield: None,
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
    /// Country selection panel layout (WIP - used in tests).
    #[allow(dead_code)]
    country_select: CountrySelectLayout,
    /// Cached bind groups for frequently used sprites.
    bg_bind_group: Option<wgpu::BindGroup>,
    speed_bind_group: Option<wgpu::BindGroup>,
    /// Font texture bind group.
    font_bind_group: Option<wgpu::BindGroup>,
    /// Cached topbar icon bind groups: (sprite_name, bind_group, width, height).
    topbar_icons: Vec<(String, wgpu::BindGroup, u32, u32)>,
    /// Cached button bind groups: (button_name, bind_group, width, height).
    button_bind_groups: Vec<(String, wgpu::BindGroup, u32, u32)>,
    /// Cached speed controls icon bind groups: (sprite_name, bind_group, width, height).
    speed_icon_bind_groups: Vec<(String, wgpu::BindGroup, u32, u32)>,
    /// Cached country select icon bind groups: (sprite_name, bind_group, width, height, WIP - used in tests).
    #[allow(dead_code)]
    country_select_icons: Vec<(String, wgpu::BindGroup, u32, u32)>,
    /// Cached panel background bind group: (bind_group, tex_width, tex_height).
    #[allow(dead_code)]
    panel_bg_bind_group: Option<(wgpu::BindGroup, u32, u32)>,
    /// Cached shield frame bind group for country select.
    shield_frame_bind_group: Option<(wgpu::BindGroup, u32, u32)>,
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
        let gfx_files = [
            "interface/speed_controls.gfx",
            "interface/topbar.gfx",
            // Country select panel sprites
            "interface/general_stuff.gfx", // shield_thin, tech icons, ideas icon
            "interface/countrydiplomacyview.gfx", // government_rank_strip
            "interface/countrygovernmentview.gfx", // tech_group_strip
            "interface/countryview.gfx",   // icon_religion
            "interface/endgamedialog.gfx", // province_icon
            "interface/provinceview.gfx",  // development_icon, fort_defense_icon
            "interface/ideas.gfx",         // GFX_idea_empty, national idea sprites
            "interface/frontend.gfx",      // GFX_country_selection_panel_bg (9-slice)
        ];

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

        // Load country select panel layout from frontend.gui
        let country_select = load_country_select(game_path);

        Self {
            gfx_db,
            sprite_cache: SpriteCache::new(game_path.to_path_buf()),
            font_cache: BitmapFontCache::new(game_path),
            speed_controls,
            topbar,
            country_select,
            bg_bind_group: None,
            speed_bind_group: None,
            font_bind_group: None,
            topbar_icons: Vec::new(),
            button_bind_groups: Vec::new(),
            speed_icon_bind_groups: Vec::new(),
            country_select_icons: Vec::new(),
            panel_bg_bind_group: None,
            shield_frame_bind_group: None,
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

        // Load additional icon textures (e.g., score icon)
        if self.speed_icon_bind_groups.is_empty() {
            for icon in &self.speed_controls.icons {
                if let Some(sprite) = self.gfx_db.get(&icon.sprite)
                    && let Some((view, w, h)) =
                        self.sprite_cache.get(&sprite.texture_file, device, queue)
                {
                    let bind_group = sprite_renderer.create_bind_group(device, view);
                    log::debug!(
                        "Loaded speed controls icon {}: {} -> {}x{}",
                        icon.name,
                        sprite.texture_file,
                        w,
                        h
                    );
                    self.speed_icon_bind_groups
                        .push((icon.sprite.clone(), bind_group, w, h));
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

    /// Ensure country select icon textures are loaded (WIP - used in tests).
    #[allow(dead_code)]
    fn ensure_country_select_textures(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sprite_renderer: &SpriteRenderer,
    ) {
        // Only load once
        if !self.country_select_icons.is_empty() {
            return;
        }

        // Collect all unique sprite names
        let mut sprites_to_load: Vec<&str> = Vec::new();

        for icon in &self.country_select.icons {
            if !sprites_to_load.contains(&icon.sprite.as_str()) {
                sprites_to_load.push(&icon.sprite);
            }
        }

        for button in &self.country_select.buttons {
            if !sprites_to_load.contains(&button.sprite.as_str()) {
                sprites_to_load.push(&button.sprite);
            }
        }

        for sprite_name in sprites_to_load {
            if let Some(sprite) = self.gfx_db.get(sprite_name) {
                if let Some((view, w, h)) =
                    self.sprite_cache.get(&sprite.texture_file, device, queue)
                {
                    let bind_group = sprite_renderer.create_bind_group(device, view);
                    self.country_select_icons
                        .push((sprite_name.to_string(), bind_group, w, h));
                    log::debug!(
                        "Loaded country select texture: {} -> {}x{} ({} frames)",
                        sprite_name,
                        w,
                        h,
                        sprite.num_frames
                    );
                } else {
                    log::warn!(
                        "Country select: texture not found for {} -> {}",
                        sprite_name,
                        sprite.texture_file
                    );
                }
            } else {
                log::warn!("Country select: sprite not in gfx_db: {}", sprite_name);
            }
        }

        // Load panel background (9-slice sprite)
        if self.panel_bg_bind_group.is_none()
            && let Some(panel_bg) = self
                .gfx_db
                .get_cornered_tile("GFX_country_selection_panel_bg")
            && let Some((view, w, h)) = self.sprite_cache.get(&panel_bg.texture_file, device, queue)
        {
            let bind_group = sprite_renderer.create_bind_group(device, view);
            self.panel_bg_bind_group = Some((bind_group, w, h));
            log::debug!(
                "Loaded panel background: {} -> {}x{}",
                panel_bg.texture_file,
                w,
                h
            );
        }

        // Load shield frame texture for country select
        if self.shield_frame_bind_group.is_none()
            && let Some((view, w, h)) =
                self.sprite_cache
                    .get("gfx/interface/shield_frame.dds", device, queue)
        {
            let bind_group = sprite_renderer.create_bind_group(device, view);
            self.shield_frame_bind_group = Some((bind_group, w, h));
            log::debug!("Loaded shield frame texture: {}x{}", w, h);
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

        // Draw topbar texts if country data is available
        if let Some(ref country) = state.country
            && let Some(ref font_bind_group) = self.font_bind_group
        {
            let topbar_anchor =
                get_window_anchor(self.topbar.window_pos, self.topbar.orientation, screen_size);

            // Get font for text rendering (reuse existing font from speed controls)
            let font_name = &self.speed_controls.date_font; // vic_18
            if let Some(loaded) = self.font_cache.get(font_name, device, queue) {
                let font = &loaded.font;

                for text in &self.topbar.texts {
                    // Map text name to value
                    let value = match text.name.as_str() {
                        "text_gold" => format!("{:.0}", country.treasury),
                        "text_manpower" => format_k(country.manpower),
                        "text_sailors" => format_k(country.sailors),
                        "text_stability" => format!("{:+}", country.stability),
                        "text_prestige" => format!("{:.0}", country.prestige),
                        "text_corruption" => format!("{:.1}", country.corruption),
                        "text_ADM" => format!("{}", country.adm_power),
                        "text_DIP" => format!("{}", country.dip_power),
                        "text_MIL" => format!("{}", country.mil_power),
                        "text_merchants" => {
                            format!("{}/{}", country.merchants, country.max_merchants)
                        }
                        "text_settlers" => {
                            format!("{}/{}", country.colonists, country.max_colonists)
                        }
                        "text_diplomats" => {
                            format!("{}/{}", country.diplomats, country.max_diplomats)
                        }
                        "text_missionaries" => {
                            format!("{}/{}", country.missionaries, country.max_missionaries)
                        }
                        _ => continue, // Skip unknown text fields
                    };

                    let text_screen_pos = position_from_anchor(
                        topbar_anchor,
                        text.position,
                        text.orientation,
                        (text.max_width, text.max_height),
                    );

                    // Measure text width for alignment
                    let text_width = font.measure_width(&value);

                    // Calculate starting X based on format (alignment)
                    let start_x = match text.format {
                        types::TextFormat::Left => text_screen_pos.0 + text.border_size.0 as f32,
                        types::TextFormat::Center => {
                            text_screen_pos.0 + (text.max_width as f32 - text_width) / 2.0
                        }
                        types::TextFormat::Right => {
                            text_screen_pos.0 + text.max_width as f32
                                - text_width
                                - text.border_size.0 as f32
                        }
                    };

                    let mut cursor_x = start_x;
                    let cursor_y = text_screen_pos.1 + text.border_size.1 as f32;

                    for c in value.chars() {
                        if let Some(glyph) = font.get_glyph(c) {
                            if glyph.width > 0 && glyph.height > 0 {
                                let glyph_x = cursor_x + glyph.xoffset as f32;
                                let glyph_y = cursor_y + glyph.yoffset as f32;
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

        // Draw additional icons (score icon, etc.)
        for icon in &self.speed_controls.icons {
            if let Some(idx) = self
                .speed_icon_bind_groups
                .iter()
                .position(|(sprite, _, _, _)| sprite == &icon.sprite)
            {
                let (_, _, w, h) = &self.speed_icon_bind_groups[idx];
                let screen_pos =
                    position_from_anchor(window_anchor, icon.position, icon.orientation, (*w, *h));
                let (clip_x, clip_y, clip_w, clip_h) =
                    rect_to_clip_space(screen_pos, (*w, *h), screen_size);
                let bind_group = &self.speed_icon_bind_groups[idx].1;
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

    /// Render only the speed controls component (for isolated testing).
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub fn render_speed_controls_only<'a>(
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

        // Place speed controls in center of screen for testing
        let window_anchor = (screen_size.0 as f32 / 2.0, screen_size.1 as f32 / 2.0);

        // Draw background
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

        // Draw buttons
        for (name, pos, orientation, _) in &self.speed_controls.buttons {
            if let Some(idx) = self
                .button_bind_groups
                .iter()
                .position(|(n, _, _, _)| n == name)
            {
                let (_, _, w, h) = &self.button_bind_groups[idx];
                let screen_pos = position_from_anchor(window_anchor, *pos, *orientation, (*w, *h));
                let (clip_x, clip_y, clip_w, clip_h) =
                    rect_to_clip_space(screen_pos, (*w, *h), screen_size);
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
        }

        // Draw additional icons (score icon, etc.)
        for icon in &self.speed_controls.icons {
            if let Some(idx) = self
                .speed_icon_bind_groups
                .iter()
                .position(|(sprite, _, _, _)| sprite == &icon.sprite)
            {
                let (_, _, w, h) = &self.speed_icon_bind_groups[idx];
                let screen_pos =
                    position_from_anchor(window_anchor, icon.position, icon.orientation, (*w, *h));
                let (clip_x, clip_y, clip_w, clip_h) =
                    rect_to_clip_space(screen_pos, (*w, *h), screen_size);
                let bind_group = &self.speed_icon_bind_groups[idx].1;
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
        }

        // Draw speed indicator
        if let Some(ref bind_group) = self.speed_bind_group {
            let frame = if state.paused {
                5
            } else {
                (state.speed.saturating_sub(1)).min(4)
            };

            let speed_screen_pos = position_from_anchor(
                window_anchor,
                self.speed_controls.speed_pos,
                self.speed_controls.speed_orientation,
                self.speed_size,
            );

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

        // Draw date text
        if let Some(ref font_bind_group) = self.font_bind_group {
            let font_name = &self.speed_controls.date_font;
            if let Some(loaded) = self.font_cache.get(font_name, device, queue) {
                let font = &loaded.font;
                let text_box_size = (
                    self.speed_controls.date_max_width,
                    self.speed_controls.date_max_height,
                );
                let text_screen_pos = position_from_anchor(
                    window_anchor,
                    self.speed_controls.date_pos,
                    self.speed_controls.date_orientation,
                    text_box_size,
                );

                // Measure text width for centering
                let text_width = font.measure_width(&state.date);
                let border = self.speed_controls.date_border_size;
                let start_x = text_screen_pos.0 + (text_box_size.0 as f32 - text_width) / 2.0;
                let start_y = text_screen_pos.1 + border.1 as f32;
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
    }

    /// Render only the topbar component (for isolated testing).
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub fn render_topbar_only<'a>(
        &'a mut self,
        render_pass: &mut wgpu::RenderPass<'a>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sprite_renderer: &'a SpriteRenderer,
        state: &GuiState,
        screen_size: (u32, u32),
    ) {
        self.ensure_topbar_textures(device, queue, sprite_renderer);
        self.ensure_font(device, queue, sprite_renderer);

        // Use standard topbar anchor (UPPER_LEFT at position 0,0)
        let topbar_anchor =
            get_window_anchor(self.topbar.window_pos, self.topbar.orientation, screen_size);

        // Draw backgrounds
        for icon in &self.topbar.backgrounds {
            if let Some(idx) = self
                .topbar_icons
                .iter()
                .position(|(name, _, _, _)| name == &icon.sprite)
            {
                let (_, _, w, h) = &self.topbar_icons[idx];
                let screen_pos =
                    position_from_anchor(topbar_anchor, icon.position, icon.orientation, (*w, *h));
                let (clip_x, clip_y, clip_w, clip_h) =
                    rect_to_clip_space(screen_pos, (*w, *h), screen_size);
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
        }

        // Draw icons
        for icon in &self.topbar.icons {
            if let Some(idx) = self
                .topbar_icons
                .iter()
                .position(|(name, _, _, _)| name == &icon.sprite)
            {
                let (_, _, w, h) = &self.topbar_icons[idx];
                let screen_pos =
                    position_from_anchor(topbar_anchor, icon.position, icon.orientation, (*w, *h));
                let (clip_x, clip_y, clip_w, clip_h) =
                    rect_to_clip_space(screen_pos, (*w, *h), screen_size);
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
        }

        // Draw texts if country data is available
        if let Some(ref country) = state.country
            && let Some(ref font_bind_group) = self.font_bind_group
        {
            let font_name = &self.speed_controls.date_font; // vic_18
            if let Some(loaded) = self.font_cache.get(font_name, device, queue) {
                let font = &loaded.font;

                for text in &self.topbar.texts {
                    let value = match text.name.as_str() {
                        "text_gold" => format!("{:.0}", country.treasury),
                        "text_manpower" => format_k(country.manpower),
                        "text_sailors" => format_k(country.sailors),
                        "text_stability" => format!("{:+}", country.stability),
                        "text_prestige" => format!("{:.0}", country.prestige),
                        "text_corruption" => format!("{:.1}", country.corruption),
                        "text_ADM" => format!("{}", country.adm_power),
                        "text_DIP" => format!("{}", country.dip_power),
                        "text_MIL" => format!("{}", country.mil_power),
                        "text_merchants" => {
                            format!("{}/{}", country.merchants, country.max_merchants)
                        }
                        "text_settlers" => {
                            format!("{}/{}", country.colonists, country.max_colonists)
                        }
                        "text_diplomats" => {
                            format!("{}/{}", country.diplomats, country.max_diplomats)
                        }
                        "text_missionaries" => {
                            format!("{}/{}", country.missionaries, country.max_missionaries)
                        }
                        _ => continue,
                    };

                    let text_screen_pos = position_from_anchor(
                        topbar_anchor,
                        text.position,
                        text.orientation,
                        (text.max_width, text.max_height),
                    );

                    // Measure text width for alignment
                    let text_width = font.measure_width(&value);

                    // Calculate starting X based on format (alignment)
                    let start_x = match text.format {
                        types::TextFormat::Left => text_screen_pos.0 + text.border_size.0 as f32,
                        types::TextFormat::Center => {
                            text_screen_pos.0 + (text.max_width as f32 - text_width) / 2.0
                        }
                        types::TextFormat::Right => {
                            text_screen_pos.0 + text.max_width as f32
                                - text_width
                                - text.border_size.0 as f32
                        }
                    };

                    let mut cursor_x = start_x;
                    let cursor_y = text_screen_pos.1 + text.border_size.1 as f32;

                    for c in value.chars() {
                        if let Some(glyph) = font.get_glyph(c) {
                            if glyph.width > 0 && glyph.height > 0 {
                                let glyph_x = cursor_x + glyph.xoffset as f32;
                                let glyph_y = cursor_y + glyph.yoffset as f32;
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
        }
    }

    /// Render only the country select panel (for isolated testing).
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub fn render_country_select_only<'a>(
        &'a mut self,
        render_pass: &mut wgpu::RenderPass<'a>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sprite_renderer: &'a SpriteRenderer,
        country_state: &SelectedCountryState,
        screen_size: (u32, u32),
    ) {
        self.ensure_country_select_textures(device, queue, sprite_renderer);
        self.ensure_font(device, queue, sprite_renderer);

        // Window content area
        let window_size = (
            self.country_select.window_size.0 as f32,
            self.country_select.window_size.1 as f32,
        );

        // Get border size from panel definition
        let border_size = self
            .gfx_db
            .get_cornered_tile("GFX_country_selection_panel_bg")
            .map(|p| (p.border_size.0 as f32, p.border_size.1 as f32))
            .unwrap_or((32.0, 32.0));

        // Calculate panel size to fit content: window + border padding
        // The y_offset (40) positions content within the panel
        let y_offset = self.country_select.window_pos.1 as f32;

        // Content extends beyond declared window_size (e.g., diplomacy label at y=402)
        // Calculate actual content height from element positions
        let max_content_y = self
            .country_select
            .icons
            .iter()
            .map(|i| i.position.1)
            .chain(self.country_select.texts.iter().map(|t| t.position.1))
            .max()
            .unwrap_or(340) as f32
            + 30.0; // Add padding for element height

        let panel_size = (
            window_size.0 + border_size.0 * 2.0, // content + left/right borders
            y_offset + max_content_y + border_size.1, // y_offset + content + bottom border
        );

        // For isolated testing, center the panel on screen
        let panel_top_left = (
            (screen_size.0 as f32 - panel_size.0) / 2.0,
            (screen_size.1 as f32 - panel_size.1) / 2.0,
        );

        // Content offset: border padding left, y_offset from top
        let content_offset = (border_size.0, y_offset);

        // Draw 9-slice panel background first (behind everything else)
        if let Some(panel_bg) = self
            .gfx_db
            .get_cornered_tile("GFX_country_selection_panel_bg")
            && let Some((ref bind_group, tex_w, tex_h)) = self.panel_bg_bind_group
        {
            // Convert to clip space - use computed panel_size, not the gfx file size
            let (clip_x, clip_y, clip_w, clip_h) = rect_to_clip_space(
                panel_top_left,
                (panel_size.0 as u32, panel_size.1 as u32),
                screen_size,
            );

            sprite_renderer.draw_nine_slice(
                render_pass,
                bind_group,
                queue,
                clip_x,
                clip_y,
                clip_w,
                clip_h,
                panel_bg.border_size.0,
                panel_bg.border_size.1,
                tex_w,
                tex_h,
                screen_size,
            );
        }

        // Content anchor is panel top-left plus centering offset
        let window_anchor = (
            panel_top_left.0 + content_offset.0,
            panel_top_left.1 + content_offset.1,
        );

        // Draw icons (with proper frame selection)
        for icon in &self.country_select.icons {
            if let Some(idx) = self
                .country_select_icons
                .iter()
                .position(|(name, _, _, _)| name == &icon.sprite)
            {
                let (sprite_name, bind_group, tex_w, tex_h) = &self.country_select_icons[idx];

                // Get sprite info for frame count
                let sprite = self.gfx_db.get(sprite_name);
                let num_frames = sprite.map(|s| s.num_frames).unwrap_or(1);

                // Determine frame based on icon name and country state
                let frame = match icon.name.as_str() {
                    "government_rank" => {
                        // 0=Duchy, 1=Kingdom, 2=Empire (internal: 1, 2, 3)
                        (country_state.government_rank.saturating_sub(1) as u32).min(num_frames - 1)
                    }
                    "religion_icon" | "secondary_religion_icon" => {
                        country_state.religion_frame.min(num_frames - 1)
                    }
                    "techgroup_icon" => country_state.tech_group_frame.min(num_frames - 1),
                    _ => icon.frame.min(num_frames - 1),
                };

                // Calculate per-frame dimensions
                let (frame_w, frame_h) = if num_frames > 1 {
                    // Horizontal strip
                    (*tex_w / num_frames, *tex_h)
                } else {
                    (*tex_w, *tex_h)
                };

                // Apply scale factor
                let scaled_w = (frame_w as f32 * icon.scale) as u32;
                let scaled_h = (frame_h as f32 * icon.scale) as u32;

                let screen_pos = position_from_anchor(
                    window_anchor,
                    icon.position,
                    icon.orientation,
                    (scaled_w, scaled_h),
                );
                let (clip_x, clip_y, clip_w, clip_h) =
                    rect_to_clip_space(screen_pos, (scaled_w, scaled_h), screen_size);

                // Calculate UVs for frame
                let (u_min, v_min, u_max, v_max) = if let Some(s) = sprite {
                    s.frame_uv(frame)
                } else {
                    (0.0, 0.0, 1.0, 1.0)
                };

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

        // Draw text labels
        if let Some(ref font_bind_group) = self.font_bind_group {
            // Try vic_18 font first, fall back to vic_22
            let font_name = "vic_18";
            if let Some(loaded) = self.font_cache.get(font_name, device, queue) {
                let font = &loaded.font;

                for text_elem in &self.country_select.texts {
                    let value = match text_elem.name.as_str() {
                        "selected_nation_label" => country_state.name.clone(),
                        "selected_nation_status_label" => country_state.government_type.clone(),
                        "selected_fog" => country_state.fog_status.clone(),
                        "selected_ruler" => country_state.ruler_name.clone(),
                        "ruler_adm_value" => format!("{}", country_state.ruler_adm),
                        "ruler_dip_value" => format!("{}", country_state.ruler_dip),
                        "ruler_mil_value" => format!("{}", country_state.ruler_mil),
                        "admtech_value" => format!("{}", country_state.adm_tech),
                        "diptech_value" => format!("{}", country_state.dip_tech),
                        "miltech_value" => format!("{}", country_state.mil_tech),
                        "national_ideagroup_name" => country_state.ideas_name.clone(),
                        "ideas_value" => format!("{}", country_state.ideas_unlocked),
                        "provinces_value" => format!("{}", country_state.province_count),
                        "economy_value" => format!("{}", country_state.total_development),
                        "fort_value" => format!("{}", country_state.fort_level),
                        "diplomacy_banner_label" => country_state.diplomacy_header.clone(),
                        _ => continue,
                    };

                    // Skip empty strings
                    if value.is_empty() {
                        continue;
                    }

                    let text_screen_pos = position_from_anchor(
                        window_anchor,
                        text_elem.position,
                        text_elem.orientation,
                        (text_elem.max_width, text_elem.max_height),
                    );

                    let text_width = font.measure_width(&value);

                    let start_x = match text_elem.format {
                        types::TextFormat::Left => {
                            text_screen_pos.0 + text_elem.border_size.0 as f32
                        }
                        types::TextFormat::Center => {
                            text_screen_pos.0 + (text_elem.max_width as f32 - text_width) / 2.0
                        }
                        types::TextFormat::Right => {
                            text_screen_pos.0 + text_elem.max_width as f32
                                - text_width
                                - text_elem.border_size.0 as f32
                        }
                    };

                    let mut cursor_x = start_x;
                    let cursor_y = text_screen_pos.1 + text_elem.border_size.1 as f32;

                    for c in value.chars() {
                        if let Some(glyph) = font.get_glyph(c) {
                            if glyph.width > 0 && glyph.height > 0 {
                                let glyph_x = cursor_x + glyph.xoffset as f32;
                                let glyph_y = cursor_y + glyph.yoffset as f32;
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
        }

        // Render shield frame (without the actual masked flag which requires MaskedFlagRenderer)
        // This shows the shield positioning for isolated testing
        if let Some((ref shield_bind_group, overlay_w, overlay_h)) = self.shield_frame_bind_group
            && let Some(shield) = self
                .country_select
                .buttons
                .iter()
                .find(|b| b.name == "player_shield")
        {
            let shield_size = (overlay_w, overlay_h);
            let screen_pos = position_from_anchor(
                window_anchor,
                shield.position,
                shield.orientation,
                shield_size,
            );

            let (clip_x, clip_y, clip_w, clip_h) =
                rect_to_clip_space(screen_pos, shield_size, screen_size);

            sprite_renderer.draw(
                render_pass,
                shield_bind_group,
                queue,
                clip_x,
                clip_y,
                clip_w,
                clip_h,
            );
        }
    }

    /// Get the clip space rectangle for the player shield (flag position).
    ///
    /// Returns (x, y, width, height) in clip space if player_shield is defined,
    /// or None if not found in the topbar layout.
    pub fn get_player_shield_clip_rect(
        &self,
        screen_size: (u32, u32),
        flag_size: (u32, u32),
    ) -> Option<(f32, f32, f32, f32)> {
        let shield = self.topbar.player_shield.as_ref()?;

        let topbar_anchor =
            get_window_anchor(self.topbar.window_pos, self.topbar.orientation, screen_size);

        let screen_pos = position_from_anchor(
            topbar_anchor,
            shield.position,
            shield.orientation,
            flag_size,
        );

        Some(rect_to_clip_space(screen_pos, flag_size, screen_size))
    }

    /// Get the shield mask texture view and dimensions for masked flag rendering.
    /// Returns None if the mask hasn't been loaded yet.
    pub fn get_shield_mask(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<(&wgpu::TextureView, u32, u32)> {
        let mask_path = "gfx/interface/shield_fancy_mask.tga";
        let result = self.sprite_cache.get(mask_path, device, queue);
        if result.is_none() {
            log::warn!("Failed to load shield mask from {}", mask_path);
        }
        result
    }

    /// Get the shield overlay texture view and dimensions for drawing frame on top of flag.
    /// Returns None if the overlay hasn't been loaded yet.
    pub fn get_shield_overlay(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<(&wgpu::TextureView, u32, u32)> {
        let overlay_path = "gfx/interface/shield_fancy_overlay.dds";
        self.sprite_cache.get(overlay_path, device, queue)
    }

    /// Get the thin shield mask texture view and dimensions for country select.
    /// Returns None if the mask hasn't been loaded yet.
    #[allow(dead_code)] // API for future masked flag rendering
    pub fn get_thin_shield_mask(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<(&wgpu::TextureView, u32, u32)> {
        let mask_path = "gfx/interface/shield_mask.tga";
        self.sprite_cache.get(mask_path, device, queue)
    }

    /// Get the thin shield overlay (frame) texture view and dimensions.
    /// Returns None if the overlay hasn't been loaded yet.
    #[allow(dead_code)] // API for future masked flag rendering
    pub fn get_thin_shield_overlay(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<(&wgpu::TextureView, u32, u32)> {
        let overlay_path = "gfx/interface/shield_frame.dds";
        self.sprite_cache.get(overlay_path, device, queue)
    }

    /// Get the clip space rectangle for the country select player shield.
    ///
    /// The panel_top_left is the screen position of the country select panel.
    /// Returns (x, y, width, height) in clip space if player_shield button is defined.
    #[allow(dead_code)] // API for future masked flag rendering
    pub fn get_country_select_shield_clip_rect(
        &self,
        panel_top_left: (f32, f32),
        content_offset: (f32, f32),
        shield_size: (u32, u32),
        screen_size: (u32, u32),
    ) -> Option<(f32, f32, f32, f32)> {
        // Find player_shield button
        let shield = self
            .country_select
            .buttons
            .iter()
            .find(|b| b.name == "player_shield")?;

        // Window anchor is panel top-left plus content offset
        let window_anchor = (
            panel_top_left.0 + content_offset.0,
            panel_top_left.1 + content_offset.1,
        );

        let screen_pos = position_from_anchor(
            window_anchor,
            shield.position,
            shield.orientation,
            shield_size,
        );

        Some(rect_to_clip_space(screen_pos, shield_size, screen_size))
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
                    topbar.player_shield = Some(icon);
                } else if bg_names.contains(&name.as_str()) {
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
                // player_shield is a guiButtonType in topbar.gui
                if name == "player_shield" {
                    log::debug!(
                        "Parsed player_shield (button): pos={:?}, sprite={}",
                        position,
                        sprite_type
                    );
                    topbar.player_shield = Some(TopBarIcon {
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
                max_height,
                orientation,
                format,
                border_size,
                ..
            } => {
                // Text labels for resources
                if name.starts_with("text_") {
                    log::debug!(
                        "Parsed topbar text {}: pos={:?}, font={}, format={:?}",
                        name,
                        position,
                        font,
                        format
                    );
                    topbar.texts.push(TopBarText {
                        name: name.clone(),
                        position: *position,
                        font: font.clone(),
                        max_width: *max_width,
                        max_height: *max_height,
                        orientation: *orientation,
                        format: *format,
                        border_size: *border_size,
                    });
                }
            }
            _ => {}
        }
    }

    log::info!(
        "Loaded topbar: {} backgrounds, {} icons, {} texts, player_shield={}",
        topbar.backgrounds.len(),
        topbar.icons.len(),
        topbar.texts.len(),
        topbar.player_shield.is_some()
    );

    topbar
}

/// Load country selection panel layout from frontend.gui.
fn load_country_select(game_path: &Path) -> CountrySelectLayout {
    let gui_path = game_path.join("interface/frontend.gui");

    if !gui_path.exists() {
        log::warn!("frontend.gui not found, using defaults");
        return CountrySelectLayout::default();
    }

    match parse_gui_file(&gui_path) {
        Ok(elements) => {
            // The structure is: country_selection_panel > ... > singleplayer
            // We need to recursively search for the singleplayer window
            if let Some(layout) = find_singleplayer_window(&elements) {
                layout
            } else {
                log::warn!("singleplayer window not found in frontend.gui");
                CountrySelectLayout::default()
            }
        }
        Err(e) => {
            log::warn!("Failed to parse frontend.gui: {}", e);
            CountrySelectLayout::default()
        }
    }
}

/// Recursively search for the singleplayer window in GUI elements.
fn find_singleplayer_window(elements: &[GuiElement]) -> Option<CountrySelectLayout> {
    for element in elements {
        if let GuiElement::Window {
            name,
            position,
            size,
            orientation,
            children,
        } = element
        {
            if name == "singleplayer" {
                return Some(extract_country_select(
                    position,
                    size,
                    orientation,
                    children,
                ));
            }
            // Recurse into child windows
            if let Some(layout) = find_singleplayer_window(children) {
                return Some(layout);
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
mod tests {
    use super::parser::count_raw_gui_elements;
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

    enum RenderMode {
        SpeedControlsOnly,
        TopbarOnly,
    }

    /// Render a specific GUI component to an image for snapshot testing.
    fn render_component_to_image(
        gpu: &HeadlessGpu,
        game_path: &std::path::Path,
        gui_state: &GuiState,
        screen_size: (u32, u32),
        mode: RenderMode,
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
            match mode {
                RenderMode::SpeedControlsOnly => {
                    gui_renderer.render_speed_controls_only(
                        &mut render_pass,
                        &gpu.device,
                        &gpu.queue,
                        &sprite_renderer,
                        gui_state,
                        screen_size,
                    );
                }
                RenderMode::TopbarOnly => {
                    gui_renderer.render_topbar_only(
                        &mut render_pass,
                        &gpu.device,
                        &gpu.queue,
                        &sprite_renderer,
                        gui_state,
                        screen_size,
                    );
                }
            }
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

    /// Render country select panel to an image for snapshot testing.
    fn render_country_select_to_image(
        gpu: &HeadlessGpu,
        game_path: &std::path::Path,
        country_state: &SelectedCountryState,
        screen_size: (u32, u32),
    ) -> RgbaImage {
        let format = gpu.format;
        let sprite_renderer = SpriteRenderer::new(&gpu.device, format);
        let mut gui_renderer = GuiRenderer::new(game_path);

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
            gui_renderer.render_country_select_only(
                &mut render_pass,
                &gpu.device,
                &gpu.queue,
                &sprite_renderer,
                country_state,
                screen_size,
            );
        }

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

        let buffer_slice = output_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| tx.send(v).unwrap());
        gpu.device.poll(wgpu::Maintain::Wait);
        rx.recv().unwrap().unwrap();

        let data = buffer_slice.get_mapped_range();

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
    fn test_country_select_snapshot() {
        let Some((gpu, game_path)) = get_test_context() else {
            println!("Skipping test_country_select_snapshot: prerequisites not available");
            return;
        };

        // Austria at game start (1444)
        let austria_state = SelectedCountryState {
            tag: "HAB".to_string(),
            name: "Austria".to_string(),
            government_type: "Archduchy".to_string(),
            fog_status: String::new(), // Visible, not in fog
            government_rank: 2,        // Kingdom
            religion_frame: 0,         // Catholic
            tech_group_frame: 0,       // Western
            ruler_name: "Friedrich III".to_string(),
            ruler_adm: 3,
            ruler_dip: 3,
            ruler_mil: 3,
            adm_tech: 3,
            dip_tech: 3,
            mil_tech: 3,
            ideas_name: "Austrian Ideas".to_string(),
            ideas_unlocked: 0,
            province_count: 6,
            total_development: 70,
            fort_level: 2,
            diplomacy_header: "Diplomacy".to_string(),
        };

        // Render at a size that fits the full panel (content extends to ~400 pixels vertically)
        let screen_size = (450, 800);
        let image = render_country_select_to_image(&gpu, &game_path, &austria_state, screen_size);

        assert_snapshot(&image, "country_select");
    }

    #[test]
    fn test_speed_controls_snapshot() {
        let Some((gpu, game_path)) = get_test_context() else {
            println!("Skipping test_speed_controls_snapshot: prerequisites not available");
            return;
        };

        // Size to fit speed controls panel (centered)
        let screen_size = (512, 256);
        let gui_state = GuiState {
            date: "11 November 1444".to_string(),
            speed: 3,
            paused: false,
            country: None, // Speed controls don't need country data
        };

        let image = render_component_to_image(
            &gpu,
            &game_path,
            &gui_state,
            screen_size,
            RenderMode::SpeedControlsOnly,
        );
        assert_snapshot(&image, "speed_controls");
    }

    #[test]
    fn test_topbar_snapshot() {
        let Some((gpu, game_path)) = get_test_context() else {
            println!("Skipping test_topbar_snapshot: prerequisites not available");
            return;
        };

        // Wide enough for full topbar, short height since it's just the bar
        let screen_size = (1024, 128);
        let gui_state = GuiState {
            date: "11 November 1444".to_string(),
            speed: 1,
            paused: true,
            // Sample country data for Castile at game start
            country: Some(CountryResources {
                treasury: 150.0,
                income: 8.5,
                manpower: 25000,
                max_manpower: 30000,
                sailors: 5000,
                max_sailors: 8000,
                stability: 1,
                prestige: 25.0,
                corruption: 0.0,
                adm_power: 50,
                dip_power: 50,
                mil_power: 50,
                merchants: 2,
                max_merchants: 3,
                colonists: 0,
                max_colonists: 1,
                diplomats: 2,
                max_diplomats: 3,
                missionaries: 1,
                max_missionaries: 2,
            }),
        };

        let image = render_component_to_image(
            &gpu,
            &game_path,
            &gui_state,
            screen_size,
            RenderMode::TopbarOnly,
        );
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

    #[test]
    fn test_gui_gap_detection() {
        let Some((_, game_path)) = get_test_context() else {
            println!("Skipping test_gui_gap_detection: prerequisites not available");
            return;
        };

        println!("\n=== GUI Gap Detection Report ===\n");

        // Check speed_controls.gui
        let speed_controls_path = game_path.join("interface/speed_controls.gui");
        if speed_controls_path.exists() {
            let raw_counts = count_raw_gui_elements(&speed_controls_path)
                .expect("Failed to count speed_controls.gui elements");

            let gui_renderer = GuiRenderer::new(&game_path);
            let sc = &gui_renderer.speed_controls;

            // Count what we actually use
            // 1 = background icon, plus any additional icons we parsed
            let used_icons = 1 + sc.icons.len();
            let used_buttons = sc.buttons.len();
            // 1 = date text, plus any additional texts we parsed
            let used_texts = 1 + sc.texts.len();

            println!("speed_controls.gui:");
            println!(
                "  Raw: {} windows, {} icons, {} buttons, {} textboxes",
                raw_counts.windows, raw_counts.icons, raw_counts.buttons, raw_counts.textboxes
            );
            println!(
                "  Used: {} icons, {} buttons, {} texts",
                used_icons, used_buttons, used_texts
            );

            let icon_gap = raw_counts.icons.saturating_sub(used_icons);
            let button_gap = raw_counts.buttons.saturating_sub(used_buttons);
            let text_gap = raw_counts.textboxes.saturating_sub(used_texts);

            if icon_gap > 0 || button_gap > 0 || text_gap > 0 {
                println!(
                    "  GAPS: {} icons, {} buttons, {} textboxes not rendered",
                    icon_gap, button_gap, text_gap
                );
            } else {
                println!("  OK: All elements accounted for");
            }

            if !raw_counts.unknown_types.is_empty() {
                println!(
                    "  Unsupported element types: {:?}",
                    raw_counts.unknown_types
                );
            }
        }

        // Check topbar.gui
        let topbar_path = game_path.join("interface/topbar.gui");
        if topbar_path.exists() {
            let raw_counts =
                count_raw_gui_elements(&topbar_path).expect("Failed to count topbar.gui elements");

            let gui_renderer = GuiRenderer::new(&game_path);
            let tb = &gui_renderer.topbar;

            // Count what we actually use (backgrounds are icons in the raw file)
            let used_icons = tb.backgrounds.len() + tb.icons.len();
            let used_texts = tb.texts.len();

            println!("\ntopbar.gui:");
            println!(
                "  Raw: {} windows, {} icons, {} buttons, {} textboxes",
                raw_counts.windows, raw_counts.icons, raw_counts.buttons, raw_counts.textboxes
            );
            println!(
                "  Used: {} icons (incl. backgrounds), {} texts",
                used_icons, used_texts
            );

            let icon_gap = raw_counts.icons.saturating_sub(used_icons);
            let text_gap = raw_counts.textboxes.saturating_sub(used_texts);

            if icon_gap > 0 || text_gap > 0 {
                println!(
                    "  GAPS: {} icons, {} textboxes not rendered",
                    icon_gap, text_gap
                );
            } else {
                println!("  OK: All elements accounted for");
            }

            if !raw_counts.unknown_types.is_empty() {
                println!(
                    "  Unsupported element types: {:?}",
                    raw_counts.unknown_types
                );
            }
        }

        println!("\n=== End Gap Detection Report ===\n");

        // This test is informational - it doesn't fail CI
        // But we print the gaps so developers know what's missing
    }

    #[test]
    fn test_country_select_loading() {
        let Some((_, game_path)) = get_test_context() else {
            println!("Skipping test_country_select_loading: prerequisites not available");
            return;
        };

        let layout = load_country_select(&game_path);

        // Verify loading succeeded
        assert!(layout.loaded, "Country select layout should be loaded");

        // Check window position - should be UPPER_RIGHT anchored
        assert_eq!(
            layout.window_orientation,
            Orientation::UpperRight,
            "Window should be UPPER_RIGHT oriented"
        );

        // Check that we parsed some elements (from frontend.gui singleplayer window)
        assert!(
            !layout.icons.is_empty(),
            "Should have parsed at least some icons"
        );
        assert!(
            !layout.texts.is_empty(),
            "Should have parsed at least some text boxes"
        );
        assert!(
            !layout.buttons.is_empty(),
            "Should have parsed at least some buttons"
        );

        // Print what we found for debugging
        println!("\n=== Country Select Layout ===");
        println!(
            "Window: pos={:?}, size={:?}, orientation={:?}",
            layout.window_pos, layout.window_size, layout.window_orientation
        );
        println!("\nIcons ({}):", layout.icons.len());
        for icon in &layout.icons {
            println!(
                "  {}: sprite={}, pos={:?}",
                icon.name, icon.sprite, icon.position
            );
        }
        println!("\nTexts ({}):", layout.texts.len());
        for text in &layout.texts {
            println!(
                "  {}: font={}, pos={:?}, format={:?}",
                text.name, text.font, text.position, text.format
            );
        }
        println!("\nButtons ({}):", layout.buttons.len());
        for button in &layout.buttons {
            println!(
                "  {}: sprite={}, pos={:?}",
                button.name, button.sprite, button.position
            );
        }
        println!("=== End Country Select Layout ===\n");
    }
}
