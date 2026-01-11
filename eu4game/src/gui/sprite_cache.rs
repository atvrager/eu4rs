//! GUI sprite texture caching.
//!
//! Lazily loads DDS textures and caches them with LRU eviction.

use crate::dds::load_dds_texture;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Maximum sprites to cache in GPU memory.
const MAX_CACHED_SPRITES: usize = 128;

/// Border size for 9-slice (cornered) sprites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpriteBorder {
    pub x: u32,
    pub y: u32,
}

/// Type of cached sprite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachedSpriteType {
    Standard,
    Cornered(SpriteBorder),
}

/// Cached sprite texture data.
struct CachedSprite {
    #[allow(dead_code)]
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    #[allow(dead_code)]
    sprite_type: CachedSpriteType,
}

/// Sprite texture cache with LRU eviction.
pub struct SpriteCache {
    /// Base game path for resolving texture paths.
    game_path: PathBuf,
    /// Cached textures keyed by texture file path.
    cache: HashMap<String, CachedSprite>,
    /// Access order for LRU eviction.
    access_order: Vec<String>,
}

impl SpriteCache {
    /// Creates a new sprite cache.
    pub fn new(game_path: PathBuf) -> Self {
        Self {
            game_path,
            cache: HashMap::new(),
            access_order: Vec::new(),
        }
    }

    /// Gets a texture, loading and caching if needed.
    /// Returns (texture_view, width, height) or None if not found.
    pub fn get(
        &mut self,
        texture_path: &str,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<(&wgpu::TextureView, u32, u32)> {
        self.get_with_type(texture_path, CachedSpriteType::Standard, device, queue)
    }

    /// Gets a cornered (9-slice) texture, loading and caching if needed.
    pub fn get_cornered(
        &mut self,
        texture_path: &str,
        border: SpriteBorder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<(&wgpu::TextureView, u32, u32)> {
        self.get_with_type(
            texture_path,
            CachedSpriteType::Cornered(border),
            device,
            queue,
        )
    }

    /// Internal implementation for getting/loading sprites.
    fn get_with_type(
        &mut self,
        texture_path: &str,
        sprite_type: CachedSpriteType,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<(&wgpu::TextureView, u32, u32)> {
        // Use a combined key of path + type for caching
        let cache_key = match sprite_type {
            CachedSpriteType::Standard => texture_path.to_string(),
            CachedSpriteType::Cornered(b) => format!("{}:{}x{}", texture_path, b.x, b.y),
        };

        // Check cache first
        if self.cache.contains_key(&cache_key) {
            self.touch(&cache_key);
            let cached = self.cache.get(&cache_key)?;
            return Some((&cached.view, cached.width, cached.height));
        }

        // Try to find the texture file, with extension fallbacks
        let full_path = self.resolve_texture_path(texture_path)?;

        // Evict if cache full
        if self.cache.len() >= MAX_CACHED_SPRITES {
            self.evict_lru();
        }

        // Load texture
        match self.load_texture(device, queue, &full_path, texture_path, sprite_type) {
            Ok(cached) => {
                let width = cached.width;
                let height = cached.height;
                self.cache.insert(cache_key.clone(), cached);
                self.access_order.push(cache_key.clone());
                let cached = self.cache.get(&cache_key)?;
                Some((&cached.view, width, height))
            }
            Err(e) => {
                log::warn!("Failed to load sprite texture {}: {}", texture_path, e);
                None
            }
        }
    }

    /// Load a texture from file.
    fn load_texture(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: &Path,
        label: &str,
        sprite_type: CachedSpriteType,
    ) -> Result<CachedSprite, String> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let (texture, view, width, height) = match ext.to_lowercase().as_str() {
            "dds" => {
                let (tex, view, _sampler) = load_dds_texture(device, queue, path, Some(label))
                    .map_err(|e| format!("DDS load error: {}", e))?;
                let size = tex.size();
                (tex, view, size.width, size.height)
            }
            "tga" => {
                let img = image::open(path)
                    .map_err(|e| format!("TGA load error: {}", e))?
                    .to_rgba8();
                let (width, height) = img.dimensions();
                let (tex, view) = create_rgba_texture(device, queue, &img, width, height, label);
                (tex, view, width, height)
            }
            _ => {
                return Err(format!("Unsupported texture format: {}", ext));
            }
        };

        log::debug!(
            "Loaded sprite {}: {}x{} ({:?})",
            label,
            width,
            height,
            sprite_type
        );

        Ok(CachedSprite {
            texture,
            view,
            width,
            height,
            sprite_type,
        })
    }

    /// Update LRU order.
    fn touch(&mut self, key: &str) {
        if let Some(pos) = self.access_order.iter().position(|k| k == key) {
            self.access_order.remove(pos);
            self.access_order.push(key.to_string());
        }
    }

    /// Evict least recently used entry.
    fn evict_lru(&mut self) {
        if let Some(oldest) = self.access_order.first().cloned() {
            self.cache.remove(&oldest);
            self.access_order.remove(0);
            log::debug!("Evicted sprite from cache: {}", oldest);
        }
    }

    /// Get cached sprite dimensions without loading the texture.
    /// Returns None if not cached or file doesn't exist.
    #[allow(dead_code)]
    pub fn get_dimensions(&self, texture_path: &str) -> Option<(u32, u32)> {
        // Check cache first
        if let Some(cached) = self.cache.get(texture_path) {
            return Some((cached.width, cached.height));
        }

        // Try to resolve path and get dimensions from file
        let full_path = self.resolve_texture_path(texture_path)?;

        // For DDS files, we can read dimensions from header without full load
        let ext = full_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match ext.to_lowercase().as_str() {
            "dds" => {
                // Read just the DDS header to get dimensions
                use std::fs::File;
                use std::io::Read;

                let mut file = File::open(&full_path).ok()?;
                let mut header = [0u8; 128]; // DDS header is 128 bytes
                file.read_exact(&mut header).ok()?;

                // DDS magic number check
                if &header[0..4] != b"DDS " {
                    return None;
                }

                // Height is at offset 12, Width at offset 16 (little endian u32)
                let height = u32::from_le_bytes([header[12], header[13], header[14], header[15]]);
                let width = u32::from_le_bytes([header[16], header[17], header[18], header[19]]);

                Some((width, height))
            }
            "tga" | "png" => {
                // For other formats, we'd need to load the full image
                // For now, return None and let the caller use fallback
                None
            }
            _ => None,
        }
    }

    /// Resolve texture path with extension fallbacks.
    ///
    /// EU4 .gfx files often specify .tga but actual files are .dds,
    /// so we try alternative extensions if the specified one isn't found.
    /// Also normalizes paths (EU4 .gfx files use `//' double slashes).
    fn resolve_texture_path(&self, texture_path: &str) -> Option<PathBuf> {
        // Normalize double slashes (common in EU4 .gfx files)
        let normalized = texture_path.replace("//", "/");
        let full_path = self.game_path.join(&normalized);

        // Try original path first
        if full_path.exists() {
            return Some(full_path);
        }

        // Try alternative extensions
        let path = Path::new(&normalized);
        let stem = path.file_stem()?.to_str()?;
        let parent = path.parent().unwrap_or(Path::new(""));
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        // Define fallback extensions
        let alternatives: &[&str] = match ext.to_lowercase().as_str() {
            "tga" => &["dds", "png"],
            "dds" => &["tga", "png"],
            "png" => &["dds", "tga"],
            _ => &["dds", "tga", "png"],
        };

        for alt_ext in alternatives {
            let alt_path = parent.join(format!("{}.{}", stem, alt_ext));
            let full_alt = self.game_path.join(&alt_path);
            if full_alt.exists() {
                log::debug!(
                    "Texture extension fallback: {} -> {}",
                    texture_path,
                    alt_path.display()
                );
                return Some(full_alt);
            }
        }

        log::warn!(
            "Sprite texture not found: {} (tried alternatives)",
            texture_path
        );
        None
    }
}

/// Create an RGBA texture from image data.
fn create_rgba_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    data: &[u8],
    width: u32,
    height: u32,
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        size,
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
