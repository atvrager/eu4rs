//! GUI renderer implementation.
//!
//! This module contains the `GuiRenderer` struct which handles rendering
//! EU4's authentic layout and sprites using WGPU.

#[cfg(test)]
use super::country_select::SelectedCountryState;
use super::country_select::{CountrySelectLayout, CountrySelectPanel};
use super::country_select_left::CountrySelectLeftPanel;
use super::country_select_top::CountrySelectTopPanel;
use super::layout::{
    get_window_anchor, position_from_anchor, position_from_anchor_with_screen, rect_to_clip_space,
    resolve_position,
};
use super::layout_types::{SpeedControlsLayout, TopBarLayout};
use super::legacy_loaders::{
    load_country_select_split, load_frontend_panels, load_speed_controls_split, load_topbar_split,
};
use super::lobby_controls::LobbyControlsPanel;
use super::primitives;
use super::speed_controls;
use super::sprite_cache::{SpriteBorder, SpriteCache};
use super::topbar;
use super::types::{self, GfxDatabase, GuiAction, GuiState, HitBox, Orientation};
use super::{interner, parse_gfx_file};
use crate::bmfont::BitmapFontCache;
use crate::render::SpriteRenderer;
use crate::screen::Screen;
use eu4data::bookmarks::BookmarkEntry;
use std::path::Path;

/// GUI renderer that uses EU4's authentic layout and sprites.
pub struct GuiRenderer {
    /// Sprite database from .gfx files.
    gfx_db: GfxDatabase,
    /// String interner for efficient widget naming and lookups.
    #[allow(dead_code)]
    pub interner: interner::StringInterner,
    /// Sprite texture cache.
    sprite_cache: SpriteCache,
    /// Bitmap font cache.
    font_cache: BitmapFontCache,
    /// Legacy speed controls layout (Phase 3.5: rendering metadata only).
    pub(crate) speed_controls_layout: SpeedControlsLayout,
    /// Macro-based speed controls widgets (Phase 3.5).
    #[allow(dead_code)] // Used in render_speed_controls_only
    speed_controls: Option<speed_controls::SpeedControls>,
    /// Legacy topbar layout (Phase 3.5: rendering metadata only).
    pub(crate) topbar_layout: TopBarLayout,
    /// Macro-based topbar text widgets (Phase 3.5).
    topbar: Option<topbar::TopBar>,
    /// Legacy country selection panel layout (Phase 3.5: rendering metadata only).
    country_select_layout: CountrySelectLayout,
    /// Macro-based country select panel widgets (Phase 3.5).
    #[allow(dead_code)] // Used in render_country_select_only (test-only)
    country_select_panel: Option<CountrySelectPanel>,
    /// Country selection left panel (Phase 8.5.1).
    #[allow(dead_code)] // Phase 8.5.2 rendering integration
    left_panel: Option<CountrySelectLeftPanel>,
    /// Left panel window layout (Phase 8.5.2).
    #[allow(dead_code)] // Will be used for rendering in Part 2
    left_panel_layout: super::layout_types::FrontendPanelLayout,
    /// Country selection top panel (Phase 8.5.1).
    #[allow(dead_code)] // Phase 8.5.2 rendering integration
    top_panel: Option<CountrySelectTopPanel>,
    /// Top panel window layout (Phase 8.5.2).
    #[allow(dead_code)] // Will be used for rendering in Part 2
    top_panel_layout: super::layout_types::FrontendPanelLayout,
    /// Lobby controls panel (Phase 8.5.1).
    #[allow(dead_code)] // Phase 8.5.2 rendering integration
    lobby_controls: Option<LobbyControlsPanel>,
    /// Lobby controls window layout (Phase 8.5.2).
    lobby_controls_layout: super::layout_types::FrontendPanelLayout,
    /// Cached bind groups for frequently used sprites.
    bg_bind_group: Option<wgpu::BindGroup>,
    speed_bind_group: Option<wgpu::BindGroup>,
    /// Font texture bind group.
    font_bind_group: Option<wgpu::BindGroup>,
    /// Cached topbar icon bind groups: (sprite_name, bind_group, width, height).
    topbar_icons: Vec<(String, wgpu::BindGroup, u32, u32)>,
    /// Cached button bind groups: (button_name, bind_group, width, height).
    button_bind_groups: Vec<(String, wgpu::BindGroup, u32, u32)>,
    /// Cached frontend button bind groups: (button_name, bind_group, width, height).
    /// Used for country selection panel buttons (play, map modes, back, etc.).
    frontend_button_bind_groups: Vec<(String, wgpu::BindGroup, u32, u32)>,
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
    /// Cached font bind groups by font name.
    font_bind_groups: Vec<(String, wgpu::BindGroup)>,
    /// Hit boxes for interactive elements (screen pixel coords).
    hit_boxes: Vec<(String, HitBox)>,
    /// Background sprite dimensions.
    bg_size: (u32, u32),
    /// Speed indicator dimensions (per frame).
    speed_size: (u32, u32),
    /// Loaded bookmark entries for the bookmarks listbox.
    bookmarks: Vec<BookmarkEntry>,
    /// Scroll offset for bookmarks listbox (in pixels).
    bookmarks_scroll_offset: f32,
    /// Currently selected bookmark index (None = no selection).
    selected_bookmark: Option<usize>,
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
            "interface/core.gfx",          // button_type_1 (back, play buttons)
            "interface/general_stuff.gfx", // shield_thin, tech icons, ideas icon, arrow buttons
            "interface/countrydiplomacyview.gfx", // government_rank_strip
            "interface/countrygovernmentview.gfx", // tech_group_strip
            "interface/countryview.gfx",   // icon_religion
            "interface/endgamedialog.gfx", // province_icon
            "interface/provinceview.gfx",  // development_icon, fort_defense_icon
            "interface/ideas.gfx",         // GFX_idea_empty, national idea sprites
            "interface/frontend.gfx",      // GFX_country_selection_panel_bg (9-slice)
            "interface/menubar.gfx",       // GFX_mapmode_* sprites for top panel
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

        let interner = interner::StringInterner::new();

        // Load speed_controls.gui layout (Phase 3.5: split into layout + macro-based widgets)
        let (speed_controls_layout, speed_root) = load_speed_controls_split(game_path, &interner);
        let speed_controls =
            speed_root.map(|root| speed_controls::SpeedControls::bind(&root, &interner));

        // Load topbar.gui layout (Phase 3.5: split into layout + macro-based widgets)
        let (topbar_layout, topbar_root) = load_topbar_split(game_path, &interner);
        let topbar = topbar_root.map(|root| topbar::TopBar::bind(&root, &interner));

        // Load country select panel layout from frontend.gui (Phase 3.5: split into layout + macro-based widgets)
        let (country_select_layout, country_select_root) =
            load_country_select_split(game_path, &interner);
        let country_select_panel =
            country_select_root.map(|root| CountrySelectPanel::bind(&root, &interner));

        // Load frontend panels (Phase 8.5.1)
        let (left_data, top_data, right_data) = load_frontend_panels(game_path, &interner);
        log::info!(
            "Frontend panels loaded: left={}, top={}, right={}",
            left_data.is_some(),
            top_data.is_some(),
            right_data.is_some()
        );

        let (left_panel, left_panel_layout) = left_data
            .map(|(root, layout)| (Some(CountrySelectLeftPanel::bind(&root, &interner)), layout))
            .unwrap_or((None, Default::default()));

        let (top_panel, top_panel_layout) = top_data
            .map(|(root, layout)| (Some(CountrySelectTopPanel::bind(&root, &interner)), layout))
            .unwrap_or((None, Default::default()));

        let (lobby_controls, lobby_controls_layout) = right_data
            .map(|(root, layout)| (Some(LobbyControlsPanel::bind(&root, &interner)), layout))
            .unwrap_or((None, Default::default()));

        // Load bookmarks for the bookmarks listbox
        let bookmarks = eu4data::bookmarks::parse_bookmarks(game_path);
        log::info!("Loaded {} bookmarks", bookmarks.len());

        Self {
            gfx_db,
            interner,
            sprite_cache: SpriteCache::new(game_path.to_path_buf()),
            font_cache: BitmapFontCache::new(game_path),
            speed_controls_layout,
            speed_controls,
            topbar_layout,
            topbar,
            country_select_layout,
            country_select_panel,
            left_panel,
            left_panel_layout,
            top_panel,
            top_panel_layout,
            lobby_controls,
            lobby_controls_layout,
            bg_bind_group: None,
            speed_bind_group: None,
            font_bind_group: None,
            topbar_icons: Vec::new(),
            button_bind_groups: Vec::new(),
            frontend_button_bind_groups: Vec::new(),
            speed_icon_bind_groups: Vec::new(),
            country_select_icons: Vec::new(),
            panel_bg_bind_group: None,
            shield_frame_bind_group: None,
            font_bind_groups: Vec::new(),
            hit_boxes: Vec::new(),
            bg_size: (1, 1),    // Updated from texture in ensure_textures()
            speed_size: (1, 1), // Updated from texture in ensure_textures()
            bookmarks,
            bookmarks_scroll_offset: 0.0,
            selected_bookmark: Some(0), // Default to first bookmark selected
        }
    }

    /// Take ownership of the country selection left panel (Phase 8.5.1).
    ///
    /// This is typically called once during FrontendUI initialization.
    /// Returns None if already taken or not loaded.
    #[allow(dead_code)] // Phase 8.5+ main.rs integration
    pub fn take_left_panel(&mut self) -> Option<CountrySelectLeftPanel> {
        self.left_panel.take()
    }

    /// Take ownership of the country selection top panel (Phase 8.5.1).
    ///
    /// This is typically called once during FrontendUI initialization.
    /// Returns None if already taken or not loaded.
    #[allow(dead_code)] // Phase 8.5+ main.rs integration
    pub fn take_top_panel(&mut self) -> Option<CountrySelectTopPanel> {
        self.top_panel.take()
    }

    /// Take ownership of the lobby controls panel (Phase 8.5.1).
    ///
    /// This is typically called once during FrontendUI initialization.
    /// Returns None if already taken or not loaded.
    #[allow(dead_code)] // Phase 8.5+ main.rs integration
    pub fn take_lobby_controls(&mut self) -> Option<LobbyControlsPanel> {
        self.lobby_controls.take()
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
            && let Some(sprite) = self.gfx_db.get(&self.speed_controls_layout.bg_sprite)
            && let Some((view, w, h)) = self.sprite_cache.get(&sprite.texture_file, device, queue)
        {
            log::debug!(
                "Loaded bg texture: {} -> {}x{} (window_pos={:?}, orientation={:?})",
                sprite.texture_file,
                w,
                h,
                self.speed_controls_layout.window_pos,
                self.speed_controls_layout.orientation
            );
            self.bg_size = (w, h);
            self.bg_bind_group = Some(sprite_renderer.create_bind_group(device, view));
        }

        // Load speed indicator texture
        if self.speed_bind_group.is_none()
            && let Some(sprite) = self.gfx_db.get(&self.speed_controls_layout.speed_sprite)
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
            for (name, _, _, sprite_name) in &self.speed_controls_layout.buttons {
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
            for icon in &self.speed_controls_layout.icons {
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
            let font_name = &self.speed_controls_layout.date_font;
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
            .topbar_layout
            .backgrounds
            .iter()
            .chain(self.topbar_layout.icons.iter())
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

        for icon in &self.country_select_layout.icons {
            if !sprites_to_load.contains(&icon.sprite.as_str()) {
                sprites_to_load.push(&icon.sprite);
            }
        }

        for button in &self.country_select_layout.buttons {
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
            && let Some((view, w, h)) = self.sprite_cache.get_cornered(
                &panel_bg.texture_file,
                SpriteBorder {
                    x: panel_bg.border_size.0,
                    y: panel_bg.border_size.1,
                },
                device,
                queue,
            )
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
    ///
    /// The `screen` parameter controls which UI elements are rendered:
    /// - `Screen::MainMenu`: No GUI elements rendered
    /// - `Screen::SinglePlayer`: Country selection panels only
    /// - `Screen::Playing`: Topbar + speed controls only
    #[allow(clippy::too_many_arguments)]
    pub fn render<'a>(
        &'a mut self,
        render_pass: &mut wgpu::RenderPass<'a>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sprite_renderer: &'a SpriteRenderer,
        state: &GuiState,
        screen: Screen,
        screen_size: (u32, u32),
    ) {
        self.hit_boxes.clear();

        match screen {
            Screen::MainMenu | Screen::Multiplayer => {
                // No GUI elements rendered for main menu or multiplayer lobby
            }
            Screen::SinglePlayer => {
                // Country selection panels only
                self.render_country_selection(
                    render_pass,
                    device,
                    queue,
                    sprite_renderer,
                    screen_size,
                );
            }
            Screen::Playing => {
                // Topbar + speed controls only
                self.render_gameplay_ui(
                    render_pass,
                    device,
                    queue,
                    sprite_renderer,
                    state,
                    screen_size,
                );
            }
        }
    }

    /// Render the gameplay UI (topbar + speed controls).
    /// Called when screen is `Screen::Playing`.
    #[allow(clippy::too_many_arguments)]
    fn render_gameplay_ui<'a>(
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

        // Collect topbar draw commands first (to avoid borrowing self during draw)
        let topbar_draws: Vec<(usize, f32, f32, f32, f32)> = {
            let topbar_anchor = get_window_anchor(
                self.topbar_layout.window_pos,
                self.topbar_layout.orientation,
                screen_size,
            );

            let mut draws = Vec::new();

            // Backgrounds first
            for bg in &self.topbar_layout.backgrounds {
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
            for icon in &self.topbar_layout.icons {
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
            let topbar_anchor = get_window_anchor(
                self.topbar_layout.window_pos,
                self.topbar_layout.orientation,
                screen_size,
            );

            // Update topbar widgets with current country state
            if let Some(ref mut topbar) = self.topbar {
                topbar.update(country);
            }

            // Get font for text rendering (reuse existing font from speed controls)
            let font_name = &self.speed_controls_layout.date_font; // vic_18
            if let Some(loaded) = self.font_cache.get(font_name, device, queue)
                && let Some(ref topbar) = self.topbar
            {
                let font = &loaded.font;

                // Helper closure to render a single text widget
                let mut render_text = |widget: &primitives::GuiText| {
                    let value = widget.text();
                    if value.is_empty() {
                        return; // Skip empty/placeholder widgets
                    }

                    let text_screen_pos = position_from_anchor(
                        topbar_anchor,
                        widget.position(),
                        widget.orientation(),
                        widget.max_dimensions(),
                    );

                    // Measure text width for alignment
                    let text_width = font.measure_width(value);

                    // Calculate starting X based on format (alignment)
                    let border_size = widget.border_size();
                    let (max_width, _max_height) = widget.max_dimensions();
                    let start_x = match widget.format() {
                        types::TextFormat::Left => text_screen_pos.0 + border_size.0 as f32,
                        types::TextFormat::Center => {
                            text_screen_pos.0 + (max_width as f32 - text_width) / 2.0
                        }
                        types::TextFormat::Right => {
                            text_screen_pos.0 + max_width as f32 - text_width - border_size.0 as f32
                        }
                    };

                    let mut cursor_x = start_x;
                    let cursor_y = text_screen_pos.1 + border_size.1 as f32;

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
                };

                // Render all topbar text widgets
                render_text(&topbar.text_gold);
                render_text(&topbar.text_manpower);
                render_text(&topbar.text_sailors);
                render_text(&topbar.text_stability);
                render_text(&topbar.text_prestige);
                render_text(&topbar.text_corruption);
                render_text(&topbar.text_ADM);
                render_text(&topbar.text_DIP);
                render_text(&topbar.text_MIL);
                render_text(&topbar.text_merchants);
                render_text(&topbar.text_settlers);
                render_text(&topbar.text_diplomats);
                render_text(&topbar.text_missionaries);
            }
        }

        // Get window anchor point - window is just an anchor, not a rectangle
        let window_anchor = get_window_anchor(
            self.speed_controls_layout.window_pos,
            self.speed_controls_layout.orientation,
            screen_size,
        );

        // Draw background at its own position relative to window anchor
        if let Some(ref bind_group) = self.bg_bind_group {
            let bg_screen_pos = position_from_anchor(
                window_anchor,
                self.speed_controls_layout.bg_pos,
                self.speed_controls_layout.bg_orientation,
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
            for (name, pos, orientation, _) in &self.speed_controls_layout.buttons {
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
        for icon in &self.speed_controls_layout.icons {
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
                self.speed_controls_layout.speed_pos,
                self.speed_controls_layout.speed_orientation,
                self.speed_size,
            );

            // Get UVs for this frame
            if let Some(sprite) = self.gfx_db.get(&self.speed_controls_layout.speed_sprite) {
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
            self.speed_controls_layout.date_max_width,
            self.speed_controls_layout.date_max_height,
        );
        let date_screen_pos = position_from_anchor(
            window_anchor,
            self.speed_controls_layout.date_pos,
            self.speed_controls_layout.date_orientation,
            text_box_size,
        );

        // Render text using bitmap font
        if let Some(ref font_bind_group) = self.font_bind_group {
            let font_name = &self.speed_controls_layout.date_font;
            if let Some(loaded) = self.font_cache.get(font_name, device, queue) {
                let font = &loaded.font;

                // Measure text width for centering
                let text_width = font.measure_width(&state.date);

                // Apply border/padding
                // In EU4, borderSize.y is top offset, format=centre is horizontal only
                let border = self.speed_controls_layout.date_border_size;

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
        for (name, pos, orientation, sprite_name) in &self.speed_controls_layout.buttons {
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

    /// Render the country selection UI (frontend panels).
    /// Called when screen is `Screen::SinglePlayer`.
    #[allow(clippy::too_many_arguments)]
    fn render_country_selection<'a>(
        &'a mut self,
        render_pass: &mut wgpu::RenderPass<'a>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sprite_renderer: &'a SpriteRenderer,
        screen_size: (u32, u32),
    ) {
        self.ensure_font(device, queue, sprite_renderer);

        log::info!(
            "Rendering country selection panels: left={}, top={}, lobby={}",
            self.left_panel.is_some(),
            self.top_panel.is_some(),
            self.lobby_controls.is_some()
        );

        // Update panel text labels before rendering
        // TODO: Pass actual start_year from game state (Phase 9)
        let start_year = 1444;
        if let Some(ref mut panel) = self.top_panel {
            let _ = panel.update(crate::gui::core::MapMode::Political, start_year);
        }

        // Phase 1: Load ALL textures for ALL panels (must be done before any rendering)
        // This avoids borrow checker issues from sprite_renderer.draw() extending borrows to lifetime 'a

        // Load top panel textures
        if let Some(ref panel) = self.top_panel {
            let buttons = vec![
                panel.mapmode_terrain.clone(),
                panel.mapmode_political.clone(),
                panel.mapmode_trade.clone(),
                panel.mapmode_religion.clone(),
                panel.mapmode_empire.clone(),
                panel.mapmode_diplomacy.clone(),
                panel.mapmode_economy.clone(),
                panel.mapmode_region.clone(),
                panel.mapmode_culture.clone(),
                panel.mapmode_players.clone(),
            ];

            for button in &buttons {
                if let Some(sprite_type) = button.sprite_type() {
                    let button_name = button.name();
                    if !self
                        .frontend_button_bind_groups
                        .iter()
                        .any(|(name, _, _, _)| name == button_name)
                    {
                        if let Some(sprite) = self.gfx_db.get(sprite_type) {
                            if let Some((view, w, h)) =
                                self.sprite_cache.get(&sprite.texture_file, device, queue)
                            {
                                log::info!("Loaded button texture: {} ({}x{})", button_name, w, h);
                                let bind_group = sprite_renderer.create_bind_group(device, view);
                                self.frontend_button_bind_groups.push((
                                    button_name.to_string(),
                                    bind_group,
                                    w,
                                    h,
                                ));
                            } else {
                                log::warn!(
                                    "Failed to load texture for button {}: texture file not found",
                                    button_name
                                );
                            }
                        } else {
                            log::warn!(
                                "Failed to load button {}: sprite '{}' not in gfx_db",
                                button_name,
                                sprite_type
                            );
                        }
                    }
                } else {
                    log::warn!("Button {} has no sprite_type", button.name());
                }
            }
        }

        // Load left panel textures (back button + date widget buttons + observe mode)
        if let Some(ref panel) = self.left_panel {
            // Collect all buttons to load
            let buttons_to_load = [
                &panel.back_button,
                &panel.year_up_1,
                &panel.year_down_1,
                &panel.year_up_2,
                &panel.year_down_2,
                &panel.year_up_3,
                &panel.year_down_3,
                &panel.month_up,
                &panel.month_down,
                &panel.day_up,
                &panel.day_down,
                &panel.observe_mode_button,
            ];

            for button in buttons_to_load {
                let button_name = button.name();
                if let Some(sprite_type) = button.sprite_type()
                    && !self
                        .frontend_button_bind_groups
                        .iter()
                        .any(|(name, _, _, _)| name == button_name)
                    && let Some(sprite) = self.gfx_db.get(sprite_type)
                    && let Some((view, w, h)) =
                        self.sprite_cache.get(&sprite.texture_file, device, queue)
                {
                    log::debug!(
                        "Loaded left panel button: {} -> {} ({}x{})",
                        button_name,
                        sprite_type,
                        w,
                        h
                    );
                    let bind_group = sprite_renderer.create_bind_group(device, view);
                    self.frontend_button_bind_groups.push((
                        button_name.to_string(),
                        bind_group,
                        w,
                        h,
                    ));
                }
            }
        }

        // Load lobby panel textures (all buttons)
        if let Some(ref panel) = self.lobby_controls {
            // Collect all lobby buttons to load
            let lobby_buttons: Vec<&super::primitives::GuiButton> = vec![
                &panel.play_button,
                &panel.random_country_button,
                &panel.nation_designer_button,
                &panel.random_new_world_button,
                &panel.enable_custom_nation_button,
            ];

            for button in lobby_buttons {
                if let Some(sprite_type) = button.sprite_type() {
                    let button_name = button.name();
                    if !self
                        .frontend_button_bind_groups
                        .iter()
                        .any(|(name, _, _, _)| name == button_name)
                        && let Some(sprite) = self.gfx_db.get(sprite_type)
                        && let Some((view, w, h)) =
                            self.sprite_cache.get(&sprite.texture_file, device, queue)
                    {
                        log::debug!(
                            "Loaded lobby button {}: {} ({}x{})",
                            button_name,
                            sprite_type,
                            w,
                            h
                        );
                        let bind_group = sprite_renderer.create_bind_group(device, view);
                        self.frontend_button_bind_groups.push((
                            button_name.to_string(),
                            bind_group,
                            w,
                            h,
                        ));
                    }
                }
            }
        }

        // Phase 2: Render all panels using loaded textures

        // Render top panel (map mode buttons, year label)
        if let Some(ref panel) = self.top_panel {
            let top_anchor = get_window_anchor(
                self.top_panel_layout.window_pos,
                self.top_panel_layout.orientation,
                screen_size,
            );

            // Clone widgets to avoid borrow conflicts
            let buttons_to_render = vec![
                (panel.mapmode_terrain.clone(), "mapmode_terrain"),
                (panel.mapmode_political.clone(), "mapmode_political"),
                (panel.mapmode_trade.clone(), "mapmode_trade"),
                (panel.mapmode_religion.clone(), "mapmode_religion"),
                (panel.mapmode_empire.clone(), "mapmode_empire"),
                (panel.mapmode_diplomacy.clone(), "mapmode_diplomacy"),
                (panel.mapmode_economy.clone(), "mapmode_economy"),
                (panel.mapmode_region.clone(), "mapmode_region"),
                (panel.mapmode_culture.clone(), "mapmode_culture"),
                (panel.mapmode_players.clone(), "mapmode_players"),
            ];
            let year_label = panel.year_label.clone();
            let select_label = panel.select_label.clone();
            let _ = panel;

            // Extract render data to avoid lifetime issues
            type ButtonRenderData = (usize, (i32, i32), Orientation, (u32, u32), String);
            let mut button_render_data: Vec<ButtonRenderData> = Vec::new();
            for (button, action_name) in &buttons_to_render {
                if let Some(pos) = button.position()
                    && let Some(orientation) = button.orientation()
                {
                    let button_name = button.name();
                    if let Some(idx) = self
                        .frontend_button_bind_groups
                        .iter()
                        .position(|(name, _, _, _)| name == button_name)
                    {
                        let (w, h) = (
                            self.frontend_button_bind_groups[idx].2,
                            self.frontend_button_bind_groups[idx].3,
                        );
                        button_render_data.push((
                            idx,
                            pos,
                            orientation,
                            (w, h),
                            action_name.to_string(),
                        ));
                    }
                }
            }

            // Render buttons using extracted data
            for (idx, pos, orientation, (w, h), action_name) in button_render_data {
                let button_screen_pos = position_from_anchor(top_anchor, pos, orientation, (w, h));
                let (clip_x, clip_y, clip_w, clip_h) =
                    rect_to_clip_space(button_screen_pos, (w, h), screen_size);

                let bind_group = &self.frontend_button_bind_groups[idx].1;
                sprite_renderer.draw(
                    render_pass,
                    bind_group,
                    queue,
                    clip_x,
                    clip_y,
                    clip_w,
                    clip_h,
                );

                self.hit_boxes.push((
                    action_name,
                    HitBox {
                        x: button_screen_pos.0,
                        y: button_screen_pos.1,
                        width: w as f32,
                        height: h as f32,
                    },
                ));
            }

            // Phase 1b: Load fonts for text labels
            for text_widget in &[&year_label, &select_label] {
                let font_name = text_widget.font();
                if !self
                    .font_bind_groups
                    .iter()
                    .any(|(name, _)| name == font_name)
                    && let Some(loaded) = self.font_cache.get(font_name, device, queue)
                {
                    let bind_group = sprite_renderer.create_bind_group(device, &loaded.view);
                    self.font_bind_groups
                        .push((font_name.to_string(), bind_group));
                    log::info!("Loaded font: {}", font_name);
                }
            }

            // Phase 1c: Load fonts for button text (left panel and lobby controls)
            // Collect all button fonts needed
            let mut button_fonts_to_load: Vec<String> = Vec::new();
            if let Some(ref panel) = self.left_panel {
                for button in [
                    &panel.back_button,
                    &panel.year_up_1,
                    &panel.year_down_1,
                    &panel.year_up_2,
                    &panel.year_down_2,
                    &panel.year_up_3,
                    &panel.year_down_3,
                    &panel.month_up,
                    &panel.month_down,
                    &panel.day_up,
                    &panel.day_down,
                    &panel.observe_mode_button,
                ] {
                    if let Some(font_name) = button.button_font()
                        && !button_fonts_to_load.contains(&font_name.to_string())
                    {
                        button_fonts_to_load.push(font_name.to_string());
                    }
                }
            }
            if let Some(ref panel) = self.lobby_controls {
                for button in [
                    &panel.play_button,
                    &panel.random_country_button,
                    &panel.nation_designer_button,
                    &panel.random_new_world_button,
                    &panel.enable_custom_nation_button,
                ] {
                    if let Some(font_name) = button.button_font()
                        && !button_fonts_to_load.contains(&font_name.to_string())
                    {
                        button_fonts_to_load.push(font_name.to_string());
                    }
                }
            }
            // Load collected button fonts
            for font_name in button_fonts_to_load {
                if !self
                    .font_bind_groups
                    .iter()
                    .any(|(name, _)| name == &font_name)
                    && let Some(loaded) = self.font_cache.get(&font_name, device, queue)
                {
                    let bind_group = sprite_renderer.create_bind_group(device, &loaded.view);
                    self.font_bind_groups.push((font_name.clone(), bind_group));
                    log::debug!("Loaded button font: {}", font_name);
                }
            }

            // Load fonts for bookmark listbox (vic_18 for title, Arial12 for date)
            for font_name in ["vic_18", "Arial12"] {
                if !self
                    .font_bind_groups
                    .iter()
                    .any(|(name, _)| name == font_name)
                    && let Some(loaded) = self.font_cache.get(font_name, device, queue)
                {
                    let bind_group = sprite_renderer.create_bind_group(device, &loaded.view);
                    self.font_bind_groups
                        .push((font_name.to_string(), bind_group));
                    log::debug!("Loaded bookmark font: {}", font_name);
                }
            }

            // Phase 2b: Render text labels
            for text_widget in &[year_label, select_label] {
                let font_name = text_widget.font();
                let font_bind_group_idx = self
                    .font_bind_groups
                    .iter()
                    .position(|(name, _)| name == font_name);

                if let Some(idx) = font_bind_group_idx
                    && let Some(loaded) = self.font_cache.get(font_name, device, queue)
                {
                    let font = &loaded.font;
                    let value = text_widget.text();

                    if !value.is_empty() {
                        let pos = text_widget.position();
                        let orientation = text_widget.orientation();
                        let format = text_widget.format();
                        let max_dimensions = text_widget.max_dimensions();
                        let border_size = text_widget.border_size();

                        // Use screen-aware positioning for CENTER_UP elements
                        let text_screen_pos = position_from_anchor_with_screen(
                            top_anchor,
                            pos,
                            orientation,
                            max_dimensions,
                            screen_size,
                        );
                        let text_width = font.measure_width(value);
                        let max_width = max_dimensions.0 as f32;
                        let screen_width = screen_size.0 as f32;

                        // For CENTER_UP orientation with format=centre, center on screen
                        // EU4 seems to center these text elements on screen rather than
                        // positioning relative to the textbox bounds
                        let start_x = match (format, orientation) {
                            (types::TextFormat::Center, types::Orientation::CenterUp) => {
                                // Center the text on screen
                                (screen_width - text_width) / 2.0
                            }
                            (types::TextFormat::Left, _) => {
                                text_screen_pos.0 + border_size.0 as f32
                            }
                            (types::TextFormat::Center, _) => {
                                text_screen_pos.0 + (max_width - text_width) / 2.0
                            }
                            (types::TextFormat::Right, _) => {
                                text_screen_pos.0 + max_width - text_width - border_size.0 as f32
                            }
                        };

                        let mut cursor_x = start_x;
                        let cursor_y = text_screen_pos.1 + border_size.1 as f32;

                        for c in value.chars() {
                            if let Some(glyph) = font.get_glyph(c) {
                                if glyph.width == 0 || glyph.height == 0 {
                                    cursor_x += glyph.xadvance as f32;
                                    continue;
                                }

                                let glyph_x = cursor_x + glyph.xoffset as f32;
                                let glyph_y = cursor_y + glyph.yoffset as f32;
                                let glyph_screen_pos = (glyph_x, glyph_y);
                                let glyph_size = (glyph.width, glyph.height);
                                let (clip_x, clip_y, clip_w, clip_h) =
                                    rect_to_clip_space(glyph_screen_pos, glyph_size, screen_size);

                                let atlas_width = font.scale_w as f32;
                                let atlas_height = font.scale_h as f32;
                                let u_min = glyph.x as f32 / atlas_width;
                                let v_min = glyph.y as f32 / atlas_height;
                                let u_max = (glyph.x + glyph.width) as f32 / atlas_width;
                                let v_max = (glyph.y + glyph.height) as f32 / atlas_height;

                                let font_bind_group = &self.font_bind_groups[idx].1;
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

                                cursor_x += glyph.xadvance as f32;
                            }
                        }
                    }
                }
            }
        }

        // Render left panel (back button + date widget buttons)
        if let Some(ref panel) = self.left_panel {
            let left_anchor = get_window_anchor(
                self.left_panel_layout.window_pos,
                self.left_panel_layout.orientation,
                screen_size,
            );

            // Clone all buttons to avoid borrow conflicts
            let buttons_with_actions = vec![
                (panel.back_button.clone(), "back_button"),
                (panel.year_up_1.clone(), "year_up_1"),
                (panel.year_down_1.clone(), "year_down_1"),
                (panel.year_up_2.clone(), "year_up_2"),
                (panel.year_down_2.clone(), "year_down_2"),
                (panel.year_up_3.clone(), "year_up_3"),
                (panel.year_down_3.clone(), "year_down_3"),
                (panel.month_up.clone(), "month_up"),
                (panel.month_down.clone(), "month_down"),
                (panel.day_up.clone(), "day_up"),
                (panel.day_down.clone(), "day_down"),
                (panel.observe_mode_button.clone(), "observe_mode_button"),
            ];
            let _ = panel;

            // Extract render data for all buttons (idx, pos, orientation, w, h, action_name, button_text, button_font)
            type LeftButtonRenderData = (
                usize,
                (i32, i32),
                Orientation,
                u32,
                u32,
                String,
                Option<String>,
                Option<String>,
            );
            let mut button_render_data: Vec<LeftButtonRenderData> = Vec::new();

            for (button, action_name) in &buttons_with_actions {
                if let Some(pos) = button.position()
                    && let Some(orientation) = button.orientation()
                {
                    let button_name = button.name();
                    if let Some(idx) = self
                        .frontend_button_bind_groups
                        .iter()
                        .position(|(name, _, _, _)| name == button_name)
                    {
                        let (w, h) = (
                            self.frontend_button_bind_groups[idx].2,
                            self.frontend_button_bind_groups[idx].3,
                        );
                        button_render_data.push((
                            idx,
                            pos,
                            orientation,
                            w,
                            h,
                            action_name.to_string(),
                            button.button_text().map(|s| s.to_string()),
                            button.button_font().map(|s| s.to_string()),
                        ));
                    }
                }
            }

            // Render all buttons using extracted data
            for (idx, pos, orientation, w, h, action_name, button_text, button_font) in
                button_render_data
            {
                // For LOWER_* orientations in fullscreen windows, use screen-relative positioning
                let button_screen_pos = match orientation {
                    Orientation::LowerLeft | Orientation::LowerRight => {
                        resolve_position(pos, orientation, (w, h), screen_size)
                    }
                    _ => position_from_anchor(left_anchor, pos, orientation, (w, h)),
                };
                let (clip_x, clip_y, clip_w, clip_h) =
                    rect_to_clip_space(button_screen_pos, (w, h), screen_size);

                // Draw button sprite
                let bind_group = &self.frontend_button_bind_groups[idx].1;
                sprite_renderer.draw(
                    render_pass,
                    bind_group,
                    queue,
                    clip_x,
                    clip_y,
                    clip_w,
                    clip_h,
                );

                // Render button text centered on button
                if let Some(text) = button_text
                    && let Some(font_name) = button_font
                    && let Some(font_idx) = self
                        .font_bind_groups
                        .iter()
                        .position(|(name, _)| name == &font_name)
                    && let Some(loaded) = self.font_cache.get(&font_name, device, queue)
                {
                    let font = &loaded.font;
                    // Resolve localization key if needed (FE_BACK -> Back)
                    let display_text = match text.as_str() {
                        "FE_BACK" => "Back",
                        other => other,
                    };
                    let text_width = font.measure_width(display_text);
                    let text_height = font.line_height as f32;

                    // Center text horizontally and vertically on button
                    let text_x = button_screen_pos.0 + (w as f32 - text_width) / 2.0;
                    let text_y = button_screen_pos.1 + (h as f32 - text_height) / 2.0;

                    let mut cursor_x = text_x;
                    for c in display_text.chars() {
                        if let Some(glyph) = font.get_glyph(c) {
                            if glyph.width == 0 || glyph.height == 0 {
                                cursor_x += glyph.xadvance as f32;
                                continue;
                            }

                            let glyph_x = cursor_x + glyph.xoffset as f32;
                            let glyph_y = text_y + glyph.yoffset as f32;
                            let glyph_screen_pos = (glyph_x, glyph_y);
                            let glyph_size = (glyph.width, glyph.height);
                            let (glyph_clip_x, glyph_clip_y, glyph_clip_w, glyph_clip_h) =
                                rect_to_clip_space(glyph_screen_pos, glyph_size, screen_size);

                            let atlas_width = font.scale_w as f32;
                            let atlas_height = font.scale_h as f32;
                            let u_min = glyph.x as f32 / atlas_width;
                            let v_min = glyph.y as f32 / atlas_height;
                            let u_max = (glyph.x + glyph.width) as f32 / atlas_width;
                            let v_max = (glyph.y + glyph.height) as f32 / atlas_height;

                            let font_bind_group = &self.font_bind_groups[font_idx].1;
                            sprite_renderer.draw_uv(
                                render_pass,
                                font_bind_group,
                                queue,
                                glyph_clip_x,
                                glyph_clip_y,
                                glyph_clip_w,
                                glyph_clip_h,
                                u_min,
                                v_min,
                                u_max,
                                v_max,
                            );

                            cursor_x += glyph.xadvance as f32;
                        }
                    }
                }

                self.hit_boxes.push((
                    action_name,
                    HitBox {
                        x: button_screen_pos.0,
                        y: button_screen_pos.1,
                        width: w as f32,
                        height: h as f32,
                    },
                ));
            }

            // Render bookmarks listbox
            if !self.bookmarks.is_empty() {
                let listbox = &panel.bookmarks_list;
                let listbox_pos = listbox.position();
                let listbox_size = listbox.size();

                // Calculate listbox screen position
                let listbox_screen_pos = position_from_anchor(
                    left_anchor,
                    (listbox_pos.0, listbox_pos.1),
                    Orientation::UpperLeft, // Listbox uses UPPER_LEFT
                    (listbox_size.0, listbox_size.1),
                );

                // Entry dimensions from bookmark_entry template
                const ENTRY_HEIGHT: f32 = 41.0;
                const TITLE_OFFSET_X: f32 = 20.0;
                const TITLE_OFFSET_Y: f32 = 5.0;
                const DATE_OFFSET_X: f32 = 21.0;
                const DATE_OFFSET_Y: f32 = 22.0;

                // Set scissor rect to clip to listbox bounds
                let scissor_x = listbox_screen_pos.0.max(0.0) as u32;
                let scissor_y = listbox_screen_pos.1.max(0.0) as u32;
                let scissor_w = listbox_size.0.min(screen_size.0 - scissor_x);
                let scissor_h = listbox_size.1.min(screen_size.1 - scissor_y);
                render_pass.set_scissor_rect(scissor_x, scissor_y, scissor_w, scissor_h);

                // Store listbox screen bounds for hit testing
                self.hit_boxes.push((
                    "bookmarks_list".to_string(),
                    HitBox {
                        x: listbox_screen_pos.0,
                        y: listbox_screen_pos.1,
                        width: listbox_size.0 as f32,
                        height: listbox_size.1 as f32,
                    },
                ));

                // Render each visible bookmark entry
                let visible_count = (listbox_size.1 as f32 / ENTRY_HEIGHT).ceil() as usize + 1;
                let scroll_offset = self.bookmarks_scroll_offset;
                let start_idx = (scroll_offset / ENTRY_HEIGHT).floor() as usize;
                let end_idx = (start_idx + visible_count).min(self.bookmarks.len());

                for idx in start_idx..end_idx {
                    let bookmark = &self.bookmarks[idx];
                    let entry_y =
                        listbox_screen_pos.1 + (idx as f32 * ENTRY_HEIGHT) - scroll_offset;

                    // Render selection highlight if this entry is selected
                    if self.selected_bookmark == Some(idx) {
                        // Draw a semi-transparent highlight bar using the font atlas
                        // (reusing existing white pixels from font texture for simple highlight)
                        let highlight_width = listbox_size.0 as f32 - 8.0; // Slight margin
                        let highlight_height = ENTRY_HEIGHT - 2.0;
                        let highlight_x = listbox_screen_pos.0 + 4.0;
                        let highlight_y = entry_y + 1.0;
                        let (clip_x, clip_y, clip_w, clip_h) = rect_to_clip_space(
                            (highlight_x, highlight_y),
                            (highlight_width as u32, highlight_height as u32),
                            screen_size,
                        );

                        // Use the first font bind group (vic_18) with UV pointing to a white area
                        if let Some((_, font_bg)) = self.font_bind_groups.first() {
                            // Draw with minimal UV to pick up font base color (acts as tint)
                            sprite_renderer.draw_uv(
                                render_pass,
                                font_bg,
                                queue,
                                clip_x,
                                clip_y,
                                clip_w,
                                clip_h,
                                0.0,
                                0.0,
                                0.01, // Tiny region to sample one color
                                0.01,
                            );
                        }
                    }

                    // Render bookmark title with vic_18 font
                    if let Some(font_idx) = self
                        .font_bind_groups
                        .iter()
                        .position(|(name, _)| name == "vic_18")
                        && let Some(loaded) = self.font_cache.get("vic_18", device, queue)
                    {
                        let font = &loaded.font;
                        let title_x = listbox_screen_pos.0 + TITLE_OFFSET_X;
                        let title_y = entry_y + TITLE_OFFSET_Y;

                        // Use bookmark name as display (localization key for now)
                        let display_name = bookmark.name.trim_start_matches("BMARK_");

                        let mut cursor_x = title_x;
                        for c in display_name.chars() {
                            if let Some(glyph) = font.get_glyph(c) {
                                if glyph.width == 0 || glyph.height == 0 {
                                    cursor_x += glyph.xadvance as f32;
                                    continue;
                                }

                                let glyph_x = cursor_x + glyph.xoffset as f32;
                                let glyph_y = title_y + glyph.yoffset as f32;
                                let glyph_screen_pos = (glyph_x, glyph_y);
                                let glyph_size = (glyph.width, glyph.height);
                                let (glyph_clip_x, glyph_clip_y, glyph_clip_w, glyph_clip_h) =
                                    rect_to_clip_space(glyph_screen_pos, glyph_size, screen_size);

                                let atlas_width = font.scale_w as f32;
                                let atlas_height = font.scale_h as f32;
                                let u_min = glyph.x as f32 / atlas_width;
                                let v_min = glyph.y as f32 / atlas_height;
                                let u_max = (glyph.x + glyph.width) as f32 / atlas_width;
                                let v_max = (glyph.y + glyph.height) as f32 / atlas_height;

                                let font_bind_group = &self.font_bind_groups[font_idx].1;
                                sprite_renderer.draw_uv(
                                    render_pass,
                                    font_bind_group,
                                    queue,
                                    glyph_clip_x,
                                    glyph_clip_y,
                                    glyph_clip_w,
                                    glyph_clip_h,
                                    u_min,
                                    v_min,
                                    u_max,
                                    v_max,
                                );
                                cursor_x += glyph.xadvance as f32;
                            }
                        }
                    }

                    // Render bookmark date with Arial12 font
                    if let Some(font_idx) = self
                        .font_bind_groups
                        .iter()
                        .position(|(name, _)| name == "Arial12")
                        && let Some(loaded) = self.font_cache.get("Arial12", device, queue)
                    {
                        let font = &loaded.font;
                        let date_x = listbox_screen_pos.0 + DATE_OFFSET_X;
                        let date_y = entry_y + DATE_OFFSET_Y;

                        // Format date as "dd Month yyyy"
                        let date_str = format!(
                            "{} {} {}",
                            bookmark.date.day(),
                            month_name(bookmark.date.month()),
                            bookmark.date.year()
                        );

                        let mut cursor_x = date_x;
                        for c in date_str.chars() {
                            if let Some(glyph) = font.get_glyph(c) {
                                if glyph.width == 0 || glyph.height == 0 {
                                    cursor_x += glyph.xadvance as f32;
                                    continue;
                                }

                                let glyph_x = cursor_x + glyph.xoffset as f32;
                                let glyph_y = date_y + glyph.yoffset as f32;
                                let glyph_screen_pos = (glyph_x, glyph_y);
                                let glyph_size = (glyph.width, glyph.height);
                                let (glyph_clip_x, glyph_clip_y, glyph_clip_w, glyph_clip_h) =
                                    rect_to_clip_space(glyph_screen_pos, glyph_size, screen_size);

                                let atlas_width = font.scale_w as f32;
                                let atlas_height = font.scale_h as f32;
                                let u_min = glyph.x as f32 / atlas_width;
                                let v_min = glyph.y as f32 / atlas_height;
                                let u_max = (glyph.x + glyph.width) as f32 / atlas_width;
                                let v_max = (glyph.y + glyph.height) as f32 / atlas_height;

                                let font_bind_group = &self.font_bind_groups[font_idx].1;
                                sprite_renderer.draw_uv(
                                    render_pass,
                                    font_bind_group,
                                    queue,
                                    glyph_clip_x,
                                    glyph_clip_y,
                                    glyph_clip_w,
                                    glyph_clip_h,
                                    u_min,
                                    v_min,
                                    u_max,
                                    v_max,
                                );
                                cursor_x += glyph.xadvance as f32;
                            }
                        }
                    }
                }

                // Reset scissor rect to full screen
                render_pass.set_scissor_rect(0, 0, screen_size.0, screen_size.1);
            }

            // TODO Part 3: Render year editor textbox, day/month label
        }

        // Render lobby controls (all buttons)
        if let Some(ref panel) = self.lobby_controls {
            let lobby_anchor = get_window_anchor(
                self.lobby_controls_layout.window_pos,
                self.lobby_controls_layout.orientation,
                screen_size,
            );

            // Clone all buttons to avoid borrow conflicts
            let lobby_buttons = vec![
                panel.play_button.clone(),
                panel.random_country_button.clone(),
                panel.nation_designer_button.clone(),
                panel.random_new_world_button.clone(),
                panel.enable_custom_nation_button.clone(),
            ];
            let _ = panel; // Release borrow

            // Extract render data for all buttons
            type LobbyButtonRenderData = (
                usize,
                (i32, i32),
                Orientation,
                u32,
                u32,
                String,
                Option<String>,
                Option<String>,
            );
            let mut button_render_data: Vec<LobbyButtonRenderData> = Vec::new();

            for button in &lobby_buttons {
                if let Some(pos) = button.position()
                    && let Some(orientation) = button.orientation()
                {
                    let button_name = button.name();
                    if let Some(idx) = self
                        .frontend_button_bind_groups
                        .iter()
                        .position(|(name, _, _, _)| name == button_name)
                    {
                        let (w, h) = (
                            self.frontend_button_bind_groups[idx].2,
                            self.frontend_button_bind_groups[idx].3,
                        );
                        button_render_data.push((
                            idx,
                            pos,
                            orientation,
                            w,
                            h,
                            button_name.to_string(),
                            button.button_text().map(|s| s.to_string()),
                            button.button_font().map(|s| s.to_string()),
                        ));
                    }
                }
            }

            // Render all buttons
            for (idx, pos, orientation, w, h, button_name, button_text, button_font) in
                button_render_data
            {
                // All lobby buttons use LOWER_RIGHT orientation
                let button_screen_pos = match orientation {
                    Orientation::LowerLeft | Orientation::LowerRight => {
                        resolve_position(pos, orientation, (w, h), screen_size)
                    }
                    _ => position_from_anchor(lobby_anchor, pos, orientation, (w, h)),
                };
                let (clip_x, clip_y, clip_w, clip_h) =
                    rect_to_clip_space(button_screen_pos, (w, h), screen_size);

                // Draw button sprite
                let bind_group = &self.frontend_button_bind_groups[idx].1;
                sprite_renderer.draw(
                    render_pass,
                    bind_group,
                    queue,
                    clip_x,
                    clip_y,
                    clip_w,
                    clip_h,
                );

                // Render button text centered on button
                if let Some(text) = button_text
                    && let Some(font_name) = button_font
                    && let Some(font_idx) = self
                        .font_bind_groups
                        .iter()
                        .position(|(name, _)| name == &font_name)
                    && let Some(loaded) = self.font_cache.get(&font_name, device, queue)
                {
                    let font = &loaded.font;
                    // Localize text
                    let display_text = match text.as_str() {
                        "FE_BACK" => "Back",
                        "PLAY" => "PLAY",
                        "RANDOM_COUNTRY" => "Random",
                        "CUSTOM_NATION" => "Custom",
                        "RANDOM_WORLD_START" => "Random World",
                        other => other,
                    };
                    let text_width = font.measure_width(display_text);
                    let text_height = font.line_height as f32;

                    // Center text horizontally and vertically on button
                    let text_x = button_screen_pos.0 + (w as f32 - text_width) / 2.0;
                    let text_y = button_screen_pos.1 + (h as f32 - text_height) / 2.0;

                    let mut cursor_x = text_x;
                    for c in display_text.chars() {
                        if let Some(glyph) = font.get_glyph(c) {
                            if glyph.width == 0 || glyph.height == 0 {
                                cursor_x += glyph.xadvance as f32;
                                continue;
                            }

                            let glyph_x = cursor_x + glyph.xoffset as f32;
                            let glyph_y = text_y + glyph.yoffset as f32;
                            let glyph_screen_pos = (glyph_x, glyph_y);
                            let glyph_size = (glyph.width, glyph.height);
                            let (glyph_clip_x, glyph_clip_y, glyph_clip_w, glyph_clip_h) =
                                rect_to_clip_space(glyph_screen_pos, glyph_size, screen_size);

                            let atlas_width = font.scale_w as f32;
                            let atlas_height = font.scale_h as f32;
                            let u_min = glyph.x as f32 / atlas_width;
                            let v_min = glyph.y as f32 / atlas_height;
                            let u_max = (glyph.x + glyph.width) as f32 / atlas_width;
                            let v_max = (glyph.y + glyph.height) as f32 / atlas_height;

                            let font_bind_group = &self.font_bind_groups[font_idx].1;
                            sprite_renderer.draw_uv(
                                render_pass,
                                font_bind_group,
                                queue,
                                glyph_clip_x,
                                glyph_clip_y,
                                glyph_clip_w,
                                glyph_clip_h,
                                u_min,
                                v_min,
                                u_max,
                                v_max,
                            );

                            cursor_x += glyph.xadvance as f32;
                        }
                    }
                }

                // Register hit box for this button
                self.hit_boxes.push((
                    button_name,
                    HitBox {
                        x: button_screen_pos.0,
                        y: button_screen_pos.1,
                        width: w as f32,
                        height: h as f32,
                    },
                ));
            }
        }
    }

    /// Handle a click at screen coordinates.
    /// Returns an action if a GUI element was clicked.
    pub fn handle_click(&mut self, x: f32, y: f32, current_state: &GuiState) -> Option<GuiAction> {
        const ENTRY_HEIGHT: f32 = 41.0;

        for (name, hit_box) in &self.hit_boxes {
            if hit_box.contains(x, y) {
                // Check for bookmarks list click first (needs special handling)
                if name == "bookmarks_list" {
                    // Calculate which entry was clicked
                    let relative_y = y - hit_box.y + self.bookmarks_scroll_offset;
                    let clicked_idx = (relative_y / ENTRY_HEIGHT).floor() as usize;

                    if clicked_idx < self.bookmarks.len() {
                        self.selected_bookmark = Some(clicked_idx);
                        return Some(GuiAction::SelectBookmark(clicked_idx));
                    }
                    return None;
                }

                return match name.as_str() {
                    // Speed controls
                    "speed_up" => {
                        let new_speed = (current_state.speed + 1).min(5);
                        Some(GuiAction::SetSpeed(new_speed))
                    }
                    "speed_down" => {
                        let new_speed = current_state.speed.saturating_sub(1).max(1);
                        Some(GuiAction::SetSpeed(new_speed))
                    }
                    "pause" => Some(GuiAction::TogglePause),
                    // Country selection: left panel
                    "back_button" => Some(GuiAction::Back),
                    "year_up_1" => Some(GuiAction::DateAdjust(types::DatePart::Year, 1)),
                    "year_down_1" => Some(GuiAction::DateAdjust(types::DatePart::Year, -1)),
                    "year_up_2" => Some(GuiAction::DateAdjust(types::DatePart::Year, 10)),
                    "year_down_2" => Some(GuiAction::DateAdjust(types::DatePart::Year, -10)),
                    "year_up_3" => Some(GuiAction::DateAdjust(types::DatePart::Year, 100)),
                    "year_down_3" => Some(GuiAction::DateAdjust(types::DatePart::Year, -100)),
                    "month_up" => Some(GuiAction::DateAdjust(types::DatePart::Month, 1)),
                    "month_down" => Some(GuiAction::DateAdjust(types::DatePart::Month, -1)),
                    "day_up" => Some(GuiAction::DateAdjust(types::DatePart::Day, 1)),
                    "day_down" => Some(GuiAction::DateAdjust(types::DatePart::Day, -1)),
                    // Country selection: lobby controls
                    "play_button" => Some(GuiAction::StartGame),
                    "random_country_button" => Some(GuiAction::RandomCountry),
                    "nation_designer_button" => Some(GuiAction::OpenNationDesigner),
                    "random_new_world_button" => Some(GuiAction::ToggleRandomNewWorld),
                    "enable_custom_nation_button" => Some(GuiAction::ToggleCustomNation),
                    // Country selection: left panel
                    "observe_mode_button" => Some(GuiAction::ToggleObserveMode),
                    // Country selection: top panel map modes
                    s if s.starts_with("mapmode_") => Some(GuiAction::SetMapMode(s.to_string())),
                    _ => None,
                };
            }
        }
        None
    }

    /// Handle mouse wheel scroll at screen coordinates.
    /// Returns true if the scroll was consumed by a GUI element (e.g., listbox).
    pub fn handle_mouse_wheel(&mut self, x: f32, y: f32, delta_y: f32) -> bool {
        const ENTRY_HEIGHT: f32 = 41.0;
        const SCROLL_SPEED: f32 = 40.0;

        // Check if mouse is over bookmarks list
        for (name, hit_box) in &self.hit_boxes {
            if name == "bookmarks_list" && hit_box.contains(x, y) {
                // Calculate max scroll based on content height
                let content_height = self.bookmarks.len() as f32 * ENTRY_HEIGHT;
                let viewport_height = hit_box.height;
                let max_scroll = (content_height - viewport_height).max(0.0);

                // Apply scroll (positive delta = scroll down)
                self.bookmarks_scroll_offset =
                    (self.bookmarks_scroll_offset + delta_y * SCROLL_SPEED).clamp(0.0, max_scroll);

                return true;
            }
        }
        false
    }

    /// Get the currently selected bookmark entry, if any.
    pub fn selected_bookmark(&self) -> Option<&BookmarkEntry> {
        self.selected_bookmark
            .and_then(|idx| self.bookmarks.get(idx))
    }
}

/// Convert month number (1-12) to month name abbreviation.
fn month_name(month: u8) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "???",
    }
}

impl GuiRenderer {
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
                self.speed_controls_layout.bg_pos,
                self.speed_controls_layout.bg_orientation,
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
        for (name, pos, orientation, _) in &self.speed_controls_layout.buttons {
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
        for icon in &self.speed_controls_layout.icons {
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

        // Update speed controls widgets with current state
        if let Some(ref mut speed_controls) = self.speed_controls {
            speed_controls.update(&state.date, state.speed as u8, state.paused);
        }

        // Draw speed indicator
        if let Some(ref bind_group) = self.speed_bind_group
            && let Some(ref speed_controls) = self.speed_controls
        {
            let speed_screen_pos = position_from_anchor(
                window_anchor,
                speed_controls.speed_indicator.position(),
                speed_controls.speed_indicator.orientation(),
                self.speed_size,
            );

            if let Some(sprite) = self.gfx_db.get(&self.speed_controls_layout.speed_sprite) {
                let frame = speed_controls.speed_indicator.frame();
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
        if let Some(ref font_bind_group) = self.font_bind_group
            && let Some(ref speed_controls) = self.speed_controls
        {
            let font_name = &self.speed_controls_layout.date_font;
            if let Some(loaded) = self.font_cache.get(font_name, device, queue) {
                let font = &loaded.font;
                let text = speed_controls.date_text.text();
                if !text.is_empty() {
                    let text_box_size = speed_controls.date_text.max_dimensions();
                    let text_screen_pos = position_from_anchor(
                        window_anchor,
                        speed_controls.date_text.position(),
                        speed_controls.date_text.orientation(),
                        text_box_size,
                    );

                    // Measure text width for centering
                    let text_width = font.measure_width(text);
                    let border = speed_controls.date_text.border_size();
                    let start_x = text_screen_pos.0 + (text_box_size.0 as f32 - text_width) / 2.0;
                    let start_y = text_screen_pos.1 + border.1 as f32;
                    let mut cursor_x = start_x;

                    for c in text.chars() {
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
        let topbar_anchor = get_window_anchor(
            self.topbar_layout.window_pos,
            self.topbar_layout.orientation,
            screen_size,
        );

        // Draw backgrounds
        for icon in &self.topbar_layout.backgrounds {
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
        for icon in &self.topbar_layout.icons {
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
            // Update topbar widgets with current country state
            if let Some(ref mut topbar) = self.topbar {
                topbar.update(country);
            }

            let font_name = &self.speed_controls_layout.date_font; // vic_18
            if let Some(loaded) = self.font_cache.get(font_name, device, queue)
                && let Some(ref topbar) = self.topbar
            {
                let font = &loaded.font;

                // Helper closure to render a single text widget
                let mut render_text = |widget: &primitives::GuiText| {
                    let value = widget.text();
                    if value.is_empty() {
                        return; // Skip empty/placeholder widgets
                    }

                    let text_screen_pos = position_from_anchor(
                        topbar_anchor,
                        widget.position(),
                        widget.orientation(),
                        widget.max_dimensions(),
                    );

                    // Measure text width for alignment
                    let text_width = font.measure_width(value);

                    // Calculate starting X based on format (alignment)
                    let border_size = widget.border_size();
                    let (max_width, _max_height) = widget.max_dimensions();
                    let start_x = match widget.format() {
                        types::TextFormat::Left => text_screen_pos.0 + border_size.0 as f32,
                        types::TextFormat::Center => {
                            text_screen_pos.0 + (max_width as f32 - text_width) / 2.0
                        }
                        types::TextFormat::Right => {
                            text_screen_pos.0 + max_width as f32 - text_width - border_size.0 as f32
                        }
                    };

                    let mut cursor_x = start_x;
                    let cursor_y = text_screen_pos.1 + border_size.1 as f32;

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
                };

                // Render all topbar text widgets
                render_text(&topbar.text_gold);
                render_text(&topbar.text_manpower);
                render_text(&topbar.text_sailors);
                render_text(&topbar.text_stability);
                render_text(&topbar.text_prestige);
                render_text(&topbar.text_corruption);
                render_text(&topbar.text_ADM);
                render_text(&topbar.text_DIP);
                render_text(&topbar.text_MIL);
                render_text(&topbar.text_merchants);
                render_text(&topbar.text_settlers);
                render_text(&topbar.text_diplomats);
                render_text(&topbar.text_missionaries);
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

        // Update panel widgets with current country state (Phase 3.5)
        if let Some(ref mut panel) = self.country_select_panel {
            panel.update(country_state);
        }

        // Window content area
        let window_size = (
            self.country_select_layout.window_size.0 as f32,
            self.country_select_layout.window_size.1 as f32,
        );

        // Get border size from panel definition
        let border_size = self
            .gfx_db
            .get_cornered_tile("GFX_country_selection_panel_bg")
            .map(|p| (p.border_size.0 as f32, p.border_size.1 as f32))
            .unwrap_or((32.0, 32.0));

        // Calculate panel size to fit content: window + border padding
        // The y_offset (40) positions content within the panel
        let y_offset = self.country_select_layout.window_pos.1 as f32;

        // Content extends beyond declared window_size (e.g., diplomacy label at y=402)
        // Calculate actual content height from element positions
        let max_content_y = self
            .country_select_layout
            .icons
            .iter()
            .map(|i| i.position.1)
            .chain(
                self.country_select_layout
                    .texts
                    .iter()
                    .map(|t| t.position.1),
            )
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
            sprite_renderer.draw_cornered_tile(
                render_pass,
                bind_group,
                queue,
                panel_top_left.0,
                panel_top_left.1,
                panel_size.0,
                panel_size.1,
                panel_bg.border_size.0 as f32,
                panel_bg.border_size.1 as f32,
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
        for icon in &self.country_select_layout.icons {
            if let Some(idx) = self
                .country_select_icons
                .iter()
                .position(|(name, _, _, _)| name == &icon.sprite)
            {
                let (sprite_name, bind_group, tex_w, tex_h) = &self.country_select_icons[idx];

                // Get sprite info for frame count
                let sprite = self.gfx_db.get(sprite_name);
                let num_frames = sprite.map(|s| s.num_frames).unwrap_or(1);

                // Determine frame: use panel widget for dynamic icons (Phase 3.5), else use layout
                let frame = if let Some(ref panel) = self.country_select_panel {
                    match icon.name.as_str() {
                        "government_rank" => panel.government_rank.frame().min(num_frames - 1),
                        "religion_icon" | "secondary_religion_icon" => {
                            panel.religion_icon.frame().min(num_frames - 1)
                        }
                        "techgroup_icon" => panel.techgroup_icon.frame().min(num_frames - 1),
                        _ => icon.frame.min(num_frames - 1),
                    }
                } else {
                    // Fallback to old logic if panel not loaded (CI mode)
                    match icon.name.as_str() {
                        "government_rank" => (country_state.government_rank.saturating_sub(1)
                            as u32)
                            .min(num_frames - 1),
                        "religion_icon" | "secondary_religion_icon" => {
                            country_state.religion_frame.min(num_frames - 1)
                        }
                        "techgroup_icon" => country_state.tech_group_frame.min(num_frames - 1),
                        _ => icon.frame.min(num_frames - 1),
                    }
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

                for text_elem in &self.country_select_layout.texts {
                    // Get text value from panel widget (Phase 3.5), else use old logic
                    let value = if let Some(ref panel) = self.country_select_panel {
                        match text_elem.name.as_str() {
                            "selected_nation_label" => {
                                panel.selected_nation_label.text().to_string()
                            }
                            "selected_nation_status_label" => {
                                panel.selected_nation_status_label.text().to_string()
                            }
                            "selected_fog" => panel.selected_fog.text().to_string(),
                            "selected_ruler" => panel.selected_ruler.text().to_string(),
                            "ruler_adm_value" => panel.ruler_adm_value.text().to_string(),
                            "ruler_dip_value" => panel.ruler_dip_value.text().to_string(),
                            "ruler_mil_value" => panel.ruler_mil_value.text().to_string(),
                            "admtech_value" => panel.admtech_value.text().to_string(),
                            "diptech_value" => panel.diptech_value.text().to_string(),
                            "miltech_value" => panel.miltech_value.text().to_string(),
                            "national_ideagroup_name" => {
                                panel.national_ideagroup_name.text().to_string()
                            }
                            "ideas_value" => panel.ideas_value.text().to_string(),
                            "provinces_value" => panel.provinces_value.text().to_string(),
                            "economy_value" => panel.economy_value.text().to_string(),
                            "fort_value" => panel.fort_value.text().to_string(),
                            "diplomacy_banner_label" => {
                                panel.diplomacy_banner_label.text().to_string()
                            }
                            _ => continue,
                        }
                    } else {
                        // Fallback to old logic if panel not loaded (CI mode)
                        match text_elem.name.as_str() {
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
                        }
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
                .country_select_layout
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
        let shield = self.topbar_layout.player_shield.as_ref()?;

        let topbar_anchor = get_window_anchor(
            self.topbar_layout.window_pos,
            self.topbar_layout.orientation,
            screen_size,
        );

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
            .country_select_layout
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
