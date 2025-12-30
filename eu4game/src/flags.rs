//! Country flag loading and caching.
//!
//! EU4 stores country flags as 128x128 TGA files in `gfx/flags/`.
//! This module provides lazy loading and GPU texture caching.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Flag dimensions (EU4 uses 128x128).
pub const FLAG_SIZE: u32 = 128;

/// Maximum flags to cache in GPU memory.
const MAX_CACHED_FLAGS: usize = 256;

/// Flag texture cache with LRU eviction.
pub struct FlagCache {
    /// Base path to flags directory.
    flags_dir: PathBuf,
    /// Cached flag textures (tag -> texture view).
    cache: HashMap<String, CachedFlag>,
    /// Access order for LRU eviction.
    access_order: Vec<String>,
    /// Fallback texture for missing flags.
    fallback: Option<wgpu::TextureView>,
}

struct CachedFlag {
    #[allow(dead_code)]
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl FlagCache {
    /// Creates a new flag cache.
    pub fn new(flags_dir: PathBuf) -> Self {
        Self {
            flags_dir,
            cache: HashMap::new(),
            access_order: Vec::new(),
            fallback: None,
        }
    }

    /// Creates a flag cache with a fallback texture for missing flags.
    pub fn with_fallback(flags_dir: PathBuf, device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let mut cache = Self::new(flags_dir);
        cache.fallback = Some(create_fallback_flag(device, queue));
        cache
    }

    /// Gets a flag texture, loading and caching if needed.
    pub fn get(
        &mut self,
        tag: &str,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<&wgpu::TextureView> {
        // Check cache first
        if self.cache.contains_key(tag) {
            // Update LRU order
            self.touch(tag);
            return self.cache.get(tag).map(|f| &f.view);
        }

        // Try to load
        let path = self.flags_dir.join(format!("{}.tga", tag));
        if !path.exists() {
            log::debug!("Flag not found: {}", tag);
            return self.fallback.as_ref();
        }

        // Evict if cache full
        if self.cache.len() >= MAX_CACHED_FLAGS {
            self.evict_lru();
        }

        // Load and cache
        match load_flag_texture(device, queue, &path, tag) {
            Ok((texture, view)) => {
                self.cache
                    .insert(tag.to_string(), CachedFlag { texture, view });
                self.access_order.push(tag.to_string());
                self.cache.get(tag).map(|f| &f.view)
            }
            Err(e) => {
                log::warn!("Failed to load flag {}: {}", tag, e);
                self.fallback.as_ref()
            }
        }
    }

    /// Preloads flags for a list of country tags.
    #[allow(dead_code)] // Will be used for batch preloading
    pub fn preload(&mut self, tags: &[&str], device: &wgpu::Device, queue: &wgpu::Queue) {
        for tag in tags {
            let _ = self.get(tag, device, queue);
        }
    }

    /// Updates LRU order when a flag is accessed.
    fn touch(&mut self, tag: &str) {
        if let Some(pos) = self.access_order.iter().position(|t| t == tag) {
            self.access_order.remove(pos);
            self.access_order.push(tag.to_string());
        }
    }

    /// Evicts the least recently used flag.
    fn evict_lru(&mut self) {
        if let Some(tag) = self.access_order.first().cloned() {
            self.cache.remove(&tag);
            self.access_order.remove(0);
            log::trace!("Evicted flag from cache: {}", tag);
        }
    }

    /// Returns the number of cached flags.
    #[allow(dead_code)]
    pub fn cached_count(&self) -> usize {
        self.cache.len()
    }
}

/// Loads a TGA flag file as a wgpu texture.
fn load_flag_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path: &Path,
    tag: &str,
) -> Result<(wgpu::Texture, wgpu::TextureView), FlagError> {
    let img = image::open(path)
        .map_err(|e| FlagError::Load {
            tag: tag.to_string(),
            source: e.to_string(),
        })?
        .into_rgba8();

    let (width, height) = img.dimensions();

    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(&format!("Flag: {}", tag)),
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
        &img,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        size,
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    Ok((texture, view))
}

/// Creates a fallback flag texture (gray with "?" pattern).
fn create_fallback_flag(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    let mut data = vec![0u8; (FLAG_SIZE * FLAG_SIZE * 4) as usize];

    // Fill with dark gray
    for pixel in data.chunks_exact_mut(4) {
        pixel[0] = 60; // R
        pixel[1] = 60; // G
        pixel[2] = 60; // B
        pixel[3] = 255; // A
    }

    // Draw a simple "?" pattern (lighter gray)
    let center = FLAG_SIZE / 2;
    for y in 30..50 {
        for x in 50..78 {
            let idx = ((y * FLAG_SIZE + x) * 4) as usize;
            data[idx] = 120;
            data[idx + 1] = 120;
            data[idx + 2] = 120;
        }
    }
    for y in 50..80 {
        for x in 64..78 {
            let idx = ((y * FLAG_SIZE + x) * 4) as usize;
            data[idx] = 120;
            data[idx + 1] = 120;
            data[idx + 2] = 120;
        }
    }
    // Dot
    for y in 90..100 {
        for x in (center - 5)..(center + 5) {
            let idx = ((y * FLAG_SIZE + x) * 4) as usize;
            data[idx] = 120;
            data[idx + 1] = 120;
            data[idx + 2] = 120;
        }
    }

    let size = wgpu::Extent3d {
        width: FLAG_SIZE,
        height: FLAG_SIZE,
        depth_or_array_layers: 1,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Flag: Fallback"),
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
        &data,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * FLAG_SIZE),
            rows_per_image: Some(FLAG_SIZE),
        },
        size,
    );

    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// Flag loading errors.
#[derive(Debug)]
pub enum FlagError {
    /// Failed to load flag image.
    Load { tag: String, source: String },
}

impl std::fmt::Display for FlagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FlagError::Load { tag, source } => {
                write!(f, "Failed to load flag '{}': {}", tag, source)
            }
        }
    }
}

impl std::error::Error for FlagError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flag_cache_lru() {
        // Test LRU order tracking (without GPU)
        let mut cache = FlagCache::new(PathBuf::from("/tmp"));

        cache.access_order.push("ENG".to_string());
        cache.access_order.push("FRA".to_string());
        cache.access_order.push("SWE".to_string());

        cache.touch("ENG");

        assert_eq!(cache.access_order, vec!["FRA", "SWE", "ENG"]);
    }

    #[test]
    #[ignore = "Requires EU4 installation"]
    fn test_load_flag_image() {
        let game_path = std::env::var("EU4_GAME_PATH").expect("EU4_GAME_PATH not set");
        // Use FRA.tga which is 24-bit RGB (ENG.tga is 16-bit which image crate doesn't support)
        let path = Path::new(&game_path).join("gfx/flags/FRA.tga");

        let img = image::open(&path).expect("Failed to load flag");
        let rgba = img.into_rgba8();

        assert_eq!(rgba.width(), FLAG_SIZE);
        assert_eq!(rgba.height(), FLAG_SIZE);
    }
}
