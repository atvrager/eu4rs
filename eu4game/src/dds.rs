//! DDS texture loading for EU4 assets.
//!
//! EU4 uses DDS files for UI elements (buttons, panels, icons) in two main formats:
//! - ARGB8888 (uncompressed) - most UI elements
//! - DXT1/DXT5 (BC1/BC3 compressed) - event pictures, particles

use image_dds::{ddsfile::Dds, image_from_dds};
use std::path::Path;

/// Loads a DDS file and returns an RGBA image.
///
/// Handles both uncompressed (ARGB8888) and compressed (DXT1/DXT5) formats.
pub fn load_dds(path: &Path) -> Result<image::RgbaImage, DdsError> {
    let file = std::fs::File::open(path).map_err(|e| DdsError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    let reader = std::io::BufReader::new(file);
    let dds = Dds::read(reader).map_err(|e| DdsError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;

    let image = image_from_dds(&dds, 0).map_err(|e| DdsError::Decode {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;

    Ok(image)
}

/// Loads a DDS file and creates a wgpu texture.
pub fn load_dds_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path: &Path,
    label: Option<&str>,
) -> Result<(wgpu::Texture, wgpu::TextureView, wgpu::Sampler), DdsError> {
    let image = load_dds(path)?;
    let dimensions = (image.width(), image.height());

    let size = wgpu::Extent3d {
        width: dimensions.0,
        height: dimensions.1,
        depth_or_array_layers: 1,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label,
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
        &image,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * dimensions.0),
            rows_per_image: Some(dimensions.1),
        },
        size,
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    Ok((texture, view, sampler))
}

/// Information about a loaded DDS file.
#[derive(Debug, Clone)]
pub struct DdsInfo {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Format description.
    pub format: String,
}

/// Gets information about a DDS file without fully loading it.
pub fn dds_info(path: &Path) -> Result<DdsInfo, DdsError> {
    let file = std::fs::File::open(path).map_err(|e| DdsError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    let reader = std::io::BufReader::new(file);
    let dds = Dds::read(reader).map_err(|e| DdsError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;

    // Get format description from D3D or DXGI format
    let format = dds
        .get_d3d_format()
        .map(|f| format!("{:?}", f))
        .or_else(|| dds.get_dxgi_format().map(|f| format!("{:?}", f)))
        .unwrap_or_else(|| "Unknown".to_string());

    Ok(DdsInfo {
        width: dds.get_width(),
        height: dds.get_height(),
        format,
    })
}

/// DDS loading errors.
#[derive(Debug)]
pub enum DdsError {
    /// File I/O error.
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    /// DDS parsing error.
    Parse {
        path: std::path::PathBuf,
        message: String,
    },
    /// DDS decoding error.
    Decode {
        path: std::path::PathBuf,
        message: String,
    },
}

impl std::fmt::Display for DdsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DdsError::Io { path, source } => {
                write!(f, "Failed to open DDS file {:?}: {}", path, source)
            }
            DdsError::Parse { path, message } => {
                write!(f, "Failed to parse DDS file {:?}: {}", path, message)
            }
            DdsError::Decode { path, message } => {
                write!(f, "Failed to decode DDS file {:?}: {}", path, message)
            }
        }
    }
}

impl std::error::Error for DdsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DdsError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dds_info_missing_file() {
        let result = dds_info(Path::new("/nonexistent/file.dds"));
        assert!(result.is_err());
    }

    // Integration test with actual EU4 files (requires game installation)
    #[test]
    #[ignore = "Requires EU4 installation"]
    fn test_load_eu4_dds_uncompressed() {
        let game_path = std::env::var("EU4_GAME_PATH")
            .expect("EU4_GAME_PATH environment variable not set");
        let path = Path::new(&game_path).join("gfx/interface/menu.dds");

        let info = dds_info(&path).expect("Failed to get DDS info");
        assert!(info.width > 0);
        assert!(info.height > 0);
        // This file is ARGB8888 format
        assert!(info.format.contains("A8R8G8B8") || info.format.contains("A8B8G8R8"));

        let image = load_dds(&path).expect("Failed to load DDS");
        assert_eq!(image.width(), info.width);
        assert_eq!(image.height(), info.height);
    }

    // Test compressed DDS file (DXT1)
    #[test]
    #[ignore = "Requires EU4 installation"]
    fn test_load_eu4_dds_compressed() {
        let game_path = std::env::var("EU4_GAME_PATH")
            .expect("EU4_GAME_PATH environment variable not set");
        // Event pictures use DXT1 compression
        let path = Path::new(&game_path)
            .join("gfx/event_pictures/event_pictures_EUROPEAN/EXPLORERS_eventPicture.dds");

        let info = dds_info(&path).expect("Failed to get DDS info");
        assert!(info.width > 0);
        assert!(info.height > 0);
        // This file is DXT1 format
        assert!(info.format.contains("DXT1") || info.format.contains("Bc1"));

        let image = load_dds(&path).expect("Failed to load compressed DDS");
        assert_eq!(image.width(), info.width);
        assert_eq!(image.height(), info.height);
    }
}
