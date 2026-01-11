//! Unified widget caching for GUI rendering.
//!
//! This module provides a single cache for all GUI sprites and fonts,
//! replacing the multiple separate caches used in the manual renderer.

use super::layout::{position_from_anchor, rect_to_clip_space, resolve_position};
use super::primitives::{GuiEditBox, GuiText};
use super::sprite_cache::SpriteCache;
use super::types::{GfxDatabase, HitBox, Orientation, TextFormat};
use crate::bmfont::{BitmapFontCache, BmFont};
use crate::render::SpriteRenderer;
use std::collections::HashMap;
use std::sync::Arc;

/// Sprite draw command for two-phase rendering.
///
/// NOTE: Currently unused but kept for future use.
#[derive(Clone)]
#[allow(dead_code)]
pub struct SpriteDrawCmd {
    pub bind_group: Arc<wgpu::BindGroup>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Hit box registration for split-phase rendering.
///
/// Used by generated rendering code to collect hit boxes before registering them.
#[derive(Clone)]
#[allow(dead_code)]
pub struct HitBoxCmd {
    pub name: String,
    pub hit_box: HitBox,
}

/// Cached sprite with bind group and dimensions.
#[allow(dead_code)] // Used by generated rendering code in Phase 3
pub struct CachedSprite {
    pub bind_group: Arc<wgpu::BindGroup>,
    /// Actual texture dimensions.
    pub dimensions: (u32, u32),
    pub num_frames: u32,
    /// Optional border size for 9-slice rendering.
    pub border_size: Option<(u32, u32)>,
    /// Optional target rendered size (from corneredTileSpriteType).
    pub target_size: Option<(u32, u32)>,
}

/// Cached font with bind group and metrics.
#[allow(dead_code)]
pub struct CachedFont {
    pub bind_group: Arc<wgpu::BindGroup>,
    pub font: Arc<BmFont>,
}

/// Unified cache for all GUI widgets (sprites and fonts).
///
/// This cache wraps the existing SpriteCache and provides a higher-level
/// interface that returns fully-prepared sprites with bind groups.
#[allow(dead_code)] // Used by generated rendering code in Phase 3
pub struct WidgetCache {
    /// Sprite cache (name -> CachedSprite with bind group).
    pub(crate) sprites: HashMap<String, CachedSprite>,
    /// Font cache (name -> CachedFont with bind group).
    pub(crate) fonts: HashMap<String, CachedFont>,
}

impl WidgetCache {
    /// Creates a new empty widget cache.
    pub fn new() -> Self {
        Self {
            sprites: HashMap::new(),
            fonts: HashMap::new(),
        }
    }

    /// Gets or loads a sprite, creating bind group if needed.
    ///
    /// Returns Some if the sprite was loaded successfully, None if sprite not found in database.
    #[allow(dead_code)] // Used by generated rendering code in Phase 3
    #[allow(clippy::too_many_arguments)]
    pub fn get_or_load_sprite(
        &mut self,
        sprite_name: &str,
        gfx_db: &GfxDatabase,
        sprite_cache: &mut SpriteCache,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sprite_renderer: &SpriteRenderer,
    ) -> Option<&CachedSprite> {
        // Check if already cached
        if self.sprites.contains_key(sprite_name) {
            return self.sprites.get(sprite_name);
        }

        // Look up sprite in GFX database (check regular sprites first, then cornered tiles)
        let (texture_file, num_frames, border_size, target_size) =
            if let Some(info) = gfx_db.get(sprite_name) {
                (info.texture_file.clone(), info.num_frames, None, None)
            } else if let Some(info) = gfx_db.get_cornered_tile(sprite_name) {
                (
                    info.texture_file.clone(),
                    1,
                    Some(info.border_size),
                    Some(info.size),
                )
            } else {
                return None;
            };

        // Load texture from sprite cache
        let (view, w, h) = sprite_cache.get(&texture_file, device, queue)?;

        // Create bind group
        let bind_group = sprite_renderer.create_bind_group(device, view);

        log::debug!(
            "Loaded sprite '{}': {} -> tex {}x{}, target {:?}, 9-slice: {:?}",
            sprite_name,
            texture_file,
            w,
            h,
            target_size,
            border_size
        );

        let cached = CachedSprite {
            bind_group: Arc::new(bind_group),
            dimensions: (w, h),
            num_frames: num_frames.max(1),
            border_size,
            target_size,
        };

        self.sprites.insert(sprite_name.to_string(), cached);
        self.sprites.get(sprite_name)
    }

    /// Gets or loads a font, creating bind group if needed.
    ///
    /// Returns a cached font with bind group ready for text rendering.
    #[allow(dead_code)]
    pub fn get_or_load_font(
        &mut self,
        font_name: &str,
        font_cache: &mut BitmapFontCache,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sprite_renderer: &SpriteRenderer,
    ) -> Option<&CachedFont> {
        // Check if already cached
        if self.fonts.contains_key(font_name) {
            return self.fonts.get(font_name);
        }

        // Load via font cache
        let loaded = font_cache.get(font_name, device, queue)?;

        // Create bind group
        let bind_group = sprite_renderer.create_bind_group(device, &loaded.view);

        let cached = CachedFont {
            bind_group: Arc::new(bind_group),
            // We need to clone the BmFont. BmFont currently doesn't implement Clone easily
            // because it has a HashMap. Let's assume we can use Arc for the font data itself if needed,
            // or just Clone it if we add #[derive(Clone)].
            // Looking at BmFont in bmfont.rs, it has HashMap<u32, BmGlyph>.
            // I will update bmfont.rs to support Clone or BmFont should be wrapped in Arc.
            // BmFont is already in LoadedFont.
            font: Arc::new(BmFont {
                face: loaded.font.face.clone(),
                size: loaded.font.size,
                line_height: loaded.font.line_height,
                base: loaded.font.base,
                scale_w: loaded.font.scale_w,
                scale_h: loaded.font.scale_h,
                texture_file: loaded.font.texture_file.clone(),
                glyphs: loaded.font.glyphs.clone(),
            }),
        };

        self.fonts.insert(font_name.to_string(), cached);
        self.fonts.get(font_name)
    }
}

impl Default for WidgetCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Render a GuiText widget using its bound metadata.
///
/// This is the GENERIC text rendering helper that uses the widget's stored
/// font and format info. Position is passed explicitly because the bound
/// GuiText has local position, but we need the accumulated position from codegen.
///
/// # Arguments
/// * `text_widget` - The GuiText with bound font/format from .gui file
/// * `widget_position` - Accumulated position from codegen (includes all parent container offsets)
/// * `window_anchor` - The parent window's anchor point
/// * `widget_cache` - Cache containing loaded fonts
/// * `sprite_renderer` - Renderer for drawing glyphs
/// * `render_pass` - wgpu render pass
/// * `queue` - wgpu queue for uniforms
/// * `screen_size` - Current screen dimensions
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
pub fn render_gui_text<'a>(
    text_widget: &GuiText,
    widget_position: (i32, i32),
    window_anchor: (f32, f32),
    widget_cache: &'a WidgetCache,
    sprite_renderer: &'a SpriteRenderer,
    render_pass: &mut wgpu::RenderPass<'a>,
    queue: &wgpu::Queue,
    screen_size: (u32, u32),
) {
    let text = text_widget.text();
    if text.is_empty() {
        return;
    }

    let font_name = text_widget.font();
    let cached_font = match widget_cache.fonts.get(font_name) {
        Some(f) => f,
        None => {
            log::warn!("Font '{}' not loaded for text widget", font_name);
            return;
        }
    };

    let font = &cached_font.font;
    let text_width = font.measure_width(text);
    // Use the passed-in widget_position (which has accumulated parent offsets from codegen)
    // instead of text_widget.position() (which only has local position)
    let position = widget_position;
    let orientation = text_widget.orientation();
    let format = text_widget.format();
    let (widget_width, _widget_height) = text_widget.max_dimensions();

    // Calculate base position based on orientation
    let base_pos = match orientation {
        Orientation::LowerLeft | Orientation::LowerRight => {
            resolve_position(position, orientation, (text_width as u32, 20), screen_size)
        }
        _ => position_from_anchor(
            window_anchor,
            position,
            orientation,
            (text_width as u32, 20),
        ),
    };

    // Apply text alignment within widget bounds
    // For Left: start at base_pos
    // For Center: center text within widget_width (if specified)
    // For Right: align text to right edge of widget_width (if specified)
    let text_x = match format {
        TextFormat::Left => base_pos.0,
        TextFormat::Center => {
            if widget_width > 0 {
                // Center within the widget's width
                base_pos.0 + (widget_width as f32 - text_width) / 2.0
            } else {
                // No widget width, center around position
                base_pos.0 - text_width / 2.0
            }
        }
        TextFormat::Right => {
            if widget_width > 0 {
                // Align to right edge of widget
                base_pos.0 + widget_width as f32 - text_width
            } else {
                base_pos.0 - text_width
            }
        }
    };
    let text_y = base_pos.1;

    // Render each glyph
    let mut cursor_x = text_x;
    for c in text.chars() {
        if let Some(glyph) = font.get_glyph(c) {
            let glyph_x = cursor_x + glyph.xoffset as f32;
            let glyph_y = text_y + glyph.yoffset as f32;

            let (clip_x, clip_y, clip_w, clip_h) =
                rect_to_clip_space((glyph_x, glyph_y), (glyph.width, glyph.height), screen_size);

            let (u_min, v_min, u_max, v_max) = font.glyph_uv(glyph);

            sprite_renderer.draw_uv(
                render_pass,
                cached_font.bind_group.as_ref(),
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

/// Render a GuiEditBox widget's text using cached fonts.
///
/// Similar to render_gui_text but for editbox widgets which display
/// editable text like the year in the date widget.
///
/// `widget_size` is the size from the GUI file (passed from codegen since
/// editbox.size() returns (0,0) in placeholder mode).
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
pub fn render_gui_editbox<'a>(
    editbox: &GuiEditBox,
    widget_position: (i32, i32),
    widget_size: (u32, u32),
    window_anchor: (f32, f32),
    widget_cache: &'a WidgetCache,
    sprite_renderer: &'a SpriteRenderer,
    render_pass: &mut wgpu::RenderPass<'a>,
    queue: &wgpu::Queue,
    screen_size: (u32, u32),
) {
    let text = editbox.text();
    if text.is_empty() {
        return;
    }

    let font_name = editbox.font();
    // Try the requested font, fall back to similar fonts if not found
    // (some fonts like vic_22s use DDS textures which may not be supported)
    let cached_font = widget_cache
        .fonts
        .get(font_name)
        .or_else(|| {
            // Try fallback: vic_22s -> vic_22, vic_18s -> vic_18, etc.
            font_name
                .strip_suffix('s')
                .and_then(|fallback| widget_cache.fonts.get(fallback))
        })
        .or_else(|| {
            // Ultimate fallback: vic_18 (most common font)
            widget_cache.fonts.get("vic_18")
        });

    let cached_font = match cached_font {
        Some(f) => f,
        None => {
            log::warn!(
                "render_gui_editbox: no font available for '{}' (and no fallbacks)",
                font_name
            );
            return;
        }
    };

    let font = &cached_font.font;
    let text_width = font.measure_width(text);
    let position = widget_position;
    let orientation = editbox.orientation();
    // Use explicit widget_size from codegen (editbox.size() returns (0,0) in placeholder mode)
    let (widget_width, _widget_height) = if widget_size.0 > 0 {
        widget_size
    } else {
        editbox.size()
    };

    // Calculate base position based on orientation
    let base_pos = match orientation {
        Orientation::LowerLeft | Orientation::LowerRight => {
            resolve_position(position, orientation, (text_width as u32, 20), screen_size)
        }
        _ => position_from_anchor(
            window_anchor,
            position,
            orientation,
            (text_width as u32, 20),
        ),
    };

    // EditBoxes center their text within the widget width
    let text_x = if widget_width > 0 {
        base_pos.0 + (widget_width as f32 - text_width) / 2.0
    } else {
        base_pos.0
    };
    let text_y = base_pos.1;

    // Render each glyph
    let mut cursor_x = text_x;
    for c in text.chars() {
        if let Some(glyph) = font.get_glyph(c) {
            let glyph_x = cursor_x + glyph.xoffset as f32;
            let glyph_y = text_y + glyph.yoffset as f32;

            let (clip_x, clip_y, clip_w, clip_h) =
                rect_to_clip_space((glyph_x, glyph_y), (glyph.width, glyph.height), screen_size);

            let (u_min, v_min, u_max, v_max) = font.glyph_uv(glyph);

            sprite_renderer.draw_uv(
                render_pass,
                cached_font.bind_group.as_ref(),
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
