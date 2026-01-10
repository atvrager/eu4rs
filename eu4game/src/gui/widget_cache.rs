//! Unified widget caching for GUI rendering.
//!
//! This module provides a single cache for all GUI sprites and fonts,
//! replacing the multiple separate caches used in the manual renderer.

use super::sprite_cache::SpriteCache;
use super::types::GfxDatabase;
use crate::render::SpriteRenderer;
use std::collections::HashMap;

/// Cached sprite with bind group and dimensions.
#[allow(dead_code)] // Used by generated rendering code in Phase 3
pub struct CachedSprite {
    pub bind_group: wgpu::BindGroup,
    pub dimensions: (u32, u32),
    pub num_frames: u32,
}

/// Cached font with bind group.
#[allow(dead_code)]
pub struct CachedFont {
    pub bind_group: wgpu::BindGroup,
}

/// Unified cache for all GUI widgets (sprites and fonts).
///
/// This cache wraps the existing SpriteCache and provides a higher-level
/// interface that returns fully-prepared sprites with bind groups.
#[allow(dead_code)] // Used by generated rendering code in Phase 3
pub struct WidgetCache {
    /// Sprite cache (name -> CachedSprite with bind group).
    sprites: HashMap<String, CachedSprite>,
    /// Font cache (name -> CachedFont with bind group).
    fonts: HashMap<String, CachedFont>,
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
    /// Returns a cached sprite with bind group and dimensions ready for rendering.
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
    ) -> &CachedSprite {
        // Use HashMap::entry API for lazy loading
        self.sprites
            .entry(sprite_name.to_string())
            .or_insert_with(|| {
                // Look up sprite in GFX database
                let sprite_info = gfx_db.get(sprite_name).unwrap_or_else(|| {
                    panic!("Sprite '{}' not found in GFX database", sprite_name)
                });

                // Load texture from sprite cache
                let (view, w, h) = sprite_cache
                    .get(&sprite_info.texture_file, device, queue)
                    .unwrap_or_else(|| {
                        panic!("Failed to load texture for sprite '{}'", sprite_name)
                    });

                // Create bind group
                let bind_group = sprite_renderer.create_bind_group(device, view);

                log::debug!(
                    "Loaded sprite '{}': {} -> {}x{} ({} frames)",
                    sprite_name,
                    sprite_info.texture_file,
                    w,
                    h,
                    sprite_info.num_frames
                );

                CachedSprite {
                    bind_group,
                    dimensions: (w, h),
                    num_frames: sprite_info.num_frames.max(1),
                }
            })
    }

    /// Gets or loads a font, creating bind group if needed.
    ///
    /// Returns a cached font with bind group ready for text rendering.
    #[allow(dead_code)]
    pub fn get_or_load_font(
        &mut self,
        _font_name: &str,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _sprite_renderer: &SpriteRenderer,
    ) -> &CachedFont {
        // TODO: Implement font loading when needed for Phase 3
        todo!("Font loading not yet implemented")
    }
}

impl Default for WidgetCache {
    fn default() -> Self {
        Self::new()
    }
}
