//! GPU rendering for the game.
//!
//! Uses wgpu to render the world map with shader-based political coloring.
//! All map rendering happens on the GPU for maximum performance.

use crate::camera::CameraUniform;
use crate::gui::layout::rect_to_clip_space;
use crate::gui::nine_slice::{NineSliceResult, generate_9_slice_quads};
use std::collections::HashMap;
use wgpu::util::DeviceExt;

// ============================================================================
// Render Target Abstraction (used by test harnesses)
// ============================================================================

/// Error type for render target operations.
#[allow(dead_code)] // Used by test harnesses
#[derive(Debug)]
pub enum RenderError {
    /// Surface error from wgpu.
    Surface(wgpu::SurfaceError),
}

impl From<wgpu::SurfaceError> for RenderError {
    fn from(e: wgpu::SurfaceError) -> Self {
        RenderError::Surface(e)
    }
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::Surface(e) => write!(f, "Surface error: {:?}", e),
        }
    }
}

impl std::error::Error for RenderError {}

/// Abstraction over render output destination.
///
/// Used by test harnesses to render to offscreen textures for verification.
#[allow(dead_code)] // Used by test harnesses
pub trait RenderTarget {
    /// Get a texture view to render into and the current dimensions.
    fn get_view(&mut self) -> Result<(wgpu::TextureView, u32, u32), RenderError>;

    /// Present the frame (no-op for offscreen targets).
    fn present(&mut self);

    /// Get the texture format.
    fn format(&self) -> wgpu::TextureFormat;
}

/// GPU context abstraction for device/queue access.
///
/// Used by test harnesses to abstract over different GPU initialization methods.
#[allow(dead_code)] // Used by test harnesses
pub trait GpuContext {
    /// Get the wgpu device.
    fn device(&self) -> &wgpu::Device;
    /// Get the wgpu queue.
    fn queue(&self) -> &wgpu::Queue;
    /// Get the surface/target format.
    fn format(&self) -> wgpu::TextureFormat;
}

// ============================================================================
// Original Render Module
// ============================================================================

/// Maximum number of army markers that can be rendered.
const MAX_ARMIES: usize = 1024;

/// Maximum number of fleet markers that can be rendered.
const MAX_FLEETS: usize = 512;

/// Size of the color lookup texture (must be power of 2, >= max province ID).
pub const LOOKUP_SIZE: u32 = 8192;

/// Creates a heightmap texture from a grayscale image.
/// The heightmap is used for terrain shading in the fragment shader.
pub fn create_heightmap_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    heightmap: &image::GrayImage,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
    let width = heightmap.width();
    let height = heightmap.height();

    // Convert grayscale to RGBA (R=height, GBA=255 for simplicity)
    let mut rgba_data: Vec<u8> = Vec::with_capacity((width * height * 4) as usize);
    for pixel in heightmap.pixels() {
        let h = pixel[0];
        rgba_data.extend_from_slice(&[h, h, h, 255]);
    }

    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Heightmap Texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm, // Linear for height values
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
        &rgba_data,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        size,
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::Repeat, // Match province texture wrapping
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear, // Smooth interpolation for terrain
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    (texture, view, sampler)
}

/// Creates a terrain color texture from an RGBA image.
/// Used for RealTerrain map mode that shows terrain type colors.
pub fn create_terrain_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    terrain: &image::RgbaImage,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
    let width = terrain.width();
    let height = terrain.height();

    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Terrain Texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb, // sRGB for color accuracy
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
        terrain,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        size,
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::Repeat, // Match province texture wrapping
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    (texture, view, sampler)
}

/// Map settings uniform for shader.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MapSettings {
    /// Province texture dimensions (width, height).
    pub texture_size: [f32; 2],
    /// Lookup texture width.
    pub lookup_size: f32,
    /// Border enabled (1.0 = yes, 0.0 = no).
    pub border_enabled: f32,
    /// Map mode (0.0 = political, 1.0 = terrain).
    pub map_mode: f32,
    /// Border thickness multiplier (1.0 = 1px, 2.0 = 2px, etc.).
    pub border_thickness: f32,
    /// Padding to align to 16 bytes (wgpu requirement).
    _padding: [f32; 2],
}

impl Default for MapSettings {
    fn default() -> Self {
        Self {
            texture_size: [5632.0, 2048.0],
            lookup_size: LOOKUP_SIZE as f32,
            border_enabled: 1.0,
            map_mode: 0.0, // Default to political mode
            border_thickness: 1.0,
            _padding: [0.0; 2],
        }
    }
}

/// Calculate border thickness based on zoom level.
/// At high zoom (zoomed in), borders are thinner so they don't dominate.
/// At low zoom (zoomed out), borders are slightly thicker so they remain visible.
pub fn calculate_border_thickness(zoom: f32) -> f32 {
    // zoom ranges from ~1 (zoomed out) to ~50 (zoomed in)
    // We want thickness from 1.2 (zoomed out) to 0.3 (zoomed in)
    // Invert the relationship: divide constant by zoom
    (8.0 / zoom).clamp(0.3, 1.2)
}

/// Army marker instance data for GPU instanced rendering.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ArmyInstance {
    /// World position in UV space (0..1).
    pub world_pos: [f32; 2],
    /// Marker color (RGBA, normalized 0..1).
    pub color: [f32; 4],
}

impl ArmyInstance {
    /// Vertex buffer layout for instanced rendering.
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ArmyInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // world_pos
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Fleet marker instance data for GPU instanced rendering.
/// Uses same layout as ArmyInstance but rendered as diamond shape.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FleetInstance {
    /// World position in UV space (0..1).
    pub world_pos: [f32; 2],
    /// Marker color (RGBA, normalized 0..1).
    pub color: [f32; 4],
}

impl FleetInstance {
    /// Vertex buffer layout for instanced rendering.
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<FleetInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // world_pos
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Gets the color for a country tag.
/// Uses real colors from game data if available, otherwise falls back to hash-based color.
pub fn country_color(tag: &str, country_colors: &HashMap<String, [u8; 3]>) -> [u8; 4] {
    if let Some(&[r, g, b]) = country_colors.get(tag) {
        return [r, g, b, 255];
    }

    // Fallback: hash-based color generation
    let mut hash: u32 = 5381;
    for byte in tag.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u32);
    }

    let hue = (hash % 360) as f32;
    let sat = 0.7;
    let val = 0.8;

    let c = val * sat;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = val - c;

    let (r, g, b) = match (hue / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    [
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
        255,
    ]
}

/// Creates the province ID texture from provinces.bmp.
/// Each pixel encodes the province ID as RG8 (R = low byte, G = high byte).
pub fn create_province_id_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    province_map: &image::RgbaImage,
    lookup: &HashMap<(u8, u8, u8), u32>,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
    let width = province_map.width();
    let height = province_map.height();

    // Convert RGB colors to province IDs encoded as RG8
    let mut id_data: Vec<u8> = Vec::with_capacity((width * height * 4) as usize);
    for pixel in province_map.pixels() {
        let color_key = (pixel[0], pixel[1], pixel[2]);
        let province_id = lookup.get(&color_key).copied().unwrap_or(0);
        // Encode as RG8 (low byte, high byte, 0, 255)
        let low = (province_id & 0xFF) as u8;
        let high = ((province_id >> 8) & 0xFF) as u8;
        id_data.extend_from_slice(&[low, high, 0, 255]);
    }

    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Province ID Texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm, // Not sRGB - we need linear for ID encoding
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
        &id_data,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        size,
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::Repeat, // Wrap for world map
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest, // Nearest for crisp provinces
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    (texture, view, sampler)
}

/// Creates the color lookup texture (province ID -> RGBA color).
/// This is a 1D texture (LOOKUP_SIZE x 1).
pub fn create_lookup_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (
    wgpu::Texture,
    wgpu::TextureView,
    wgpu::Sampler,
    wgpu::Buffer,
) {
    let size = wgpu::Extent3d {
        width: LOOKUP_SIZE,
        height: 1,
        depth_or_array_layers: 1,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Color Lookup Texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    // Initialize with default colors (gray for unknown)
    let default_color = [60u8, 60, 60, 255];
    let initial_data: Vec<u8> = (0..LOOKUP_SIZE).flat_map(|_| default_color).collect();

    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &initial_data,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * LOOKUP_SIZE),
            rows_per_image: Some(1),
        },
        size,
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    // Create a staging buffer for efficient updates
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Lookup Staging Buffer"),
        size: (LOOKUP_SIZE * 4) as u64,
        usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::MAP_WRITE,
        mapped_at_creation: false,
    });

    (texture, view, sampler, staging_buffer)
}

/// Data needed to update the lookup texture.
pub struct LookupUpdateData<'a> {
    /// Province ID -> Owner tag.
    pub province_owners: &'a HashMap<u32, String>,
    /// Sea provinces (province IDs that are water).
    pub sea_provinces: &'a std::collections::HashSet<u32>,
    /// Country tag -> RGB color (from game data).
    pub country_colors: &'a HashMap<String, [u8; 3]>,
    /// Maximum province ID (reserved for future optimization).
    #[allow(dead_code)]
    pub max_province_id: u32,
}

/// Updates the lookup texture with current province colors.
/// This is called when ownership changes - much faster than regenerating the whole map.
pub fn update_lookup_texture(
    queue: &wgpu::Queue,
    lookup_texture: &wgpu::Texture,
    data: &LookupUpdateData,
) {
    // Build the lookup data
    let mut lookup_data: Vec<u8> = Vec::with_capacity((LOOKUP_SIZE * 4) as usize);

    // Default colors
    let wasteland_color = [60u8, 60, 60, 255]; // Gray for unowned
    let water_color = [30u8, 60, 100, 255]; // Dark blue for water
    let unknown_color = [80u8, 80, 80, 255]; // Gray for unknown

    for province_id in 0..LOOKUP_SIZE {
        let color = if province_id == 0 {
            unknown_color
        } else if data.sea_provinces.contains(&province_id) {
            water_color
        } else if let Some(owner) = data.province_owners.get(&province_id) {
            country_color(owner, data.country_colors)
        } else {
            wasteland_color
        };
        lookup_data.extend_from_slice(&color);
    }

    // Write directly to texture
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: lookup_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &lookup_data,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * LOOKUP_SIZE),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: LOOKUP_SIZE,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
}

/// Main renderer holding GPU resources.
pub struct Renderer {
    /// Render pipeline for the map.
    pub pipeline: wgpu::RenderPipeline,
    /// Bind group for textures and uniforms.
    pub bind_group: wgpu::BindGroup,
    /// Camera uniform buffer.
    pub camera_buffer: wgpu::Buffer,
    /// Map settings uniform buffer (reserved for future use).
    #[allow(dead_code)]
    pub settings_buffer: wgpu::Buffer,
    /// Province ID texture.
    #[allow(dead_code)]
    pub province_texture: wgpu::Texture,
    /// Color lookup texture.
    pub lookup_texture: wgpu::Texture,
    /// Map dimensions (for instanced rendering).
    pub map_size: (u32, u32),
    /// Army marker pipeline.
    pub army_pipeline: wgpu::RenderPipeline,
    /// Army marker bind group (just camera).
    pub army_bind_group: wgpu::BindGroup,
    /// Army instance buffer.
    pub army_instance_buffer: wgpu::Buffer,
    /// Current number of army instances.
    pub army_count: u32,
    /// Fleet marker pipeline.
    pub fleet_pipeline: wgpu::RenderPipeline,
    /// Fleet marker bind group (same as army, just camera).
    pub fleet_bind_group: wgpu::BindGroup,
    /// Fleet instance buffer.
    pub fleet_instance_buffer: wgpu::Buffer,
    /// Current number of fleet instances.
    pub fleet_count: u32,
}

impl Renderer {
    /// Creates a new GPU-based renderer.
    ///
    /// If `heightmap` is provided, terrain shading will be enabled.
    /// If `terrain_texture` is provided, RealTerrain map mode will be available.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        province_map: &image::RgbaImage,
        province_lookup: &HashMap<(u8, u8, u8), u32>,
        heightmap: Option<&image::GrayImage>,
        terrain_texture: Option<&image::RgbaImage>,
    ) -> Self {
        let map_size = (province_map.width(), province_map.height());
        log::info!(
            "Creating GPU renderer with {}x{} map",
            map_size.0,
            map_size.1
        );

        // Create province ID texture
        let (province_texture, province_view, province_sampler) =
            create_province_id_texture(device, queue, province_map, province_lookup);
        log::info!("Created province ID texture");

        // Create lookup texture
        let (lookup_texture, lookup_view, lookup_sampler, _staging) =
            create_lookup_texture(device, queue);
        log::info!("Created color lookup texture ({}x1)", LOOKUP_SIZE);

        // Create camera buffer
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[CameraUniform::default()]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create settings buffer
        let settings = MapSettings {
            texture_size: [map_size.0 as f32, map_size.1 as f32],
            lookup_size: LOOKUP_SIZE as f32,
            border_enabled: 1.0,
            map_mode: 0.0, // Default to political mode
            border_thickness: 1.0,
            _padding: [0.0; 2],
        };
        let settings_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Settings Buffer"),
            contents: bytemuck::cast_slice(&[settings]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create heightmap texture (or fallback to flat gray if not provided)
        let (heightmap_view, heightmap_sampler) = if let Some(hm) = heightmap {
            log::info!(
                "Creating heightmap texture ({}x{})",
                hm.width(),
                hm.height()
            );
            let (_tex, view, sampler) = create_heightmap_texture(device, queue, hm);
            (view, sampler)
        } else {
            // Create a 1x1 flat gray heightmap as fallback (no terrain shading)
            log::info!("No heightmap provided, using flat terrain");
            let flat = image::GrayImage::from_pixel(1, 1, image::Luma([128u8]));
            let (_tex, view, sampler) = create_heightmap_texture(device, queue, &flat);
            (view, sampler)
        };

        // Create terrain texture (or fallback to 1x1 gray if not provided)
        let (terrain_view, terrain_sampler) = if let Some(terrain) = terrain_texture {
            log::info!(
                "Creating terrain texture ({}x{})",
                terrain.width(),
                terrain.height()
            );
            let (_tex, view, sampler) = create_terrain_texture(device, queue, terrain);
            (view, sampler)
        } else {
            // Create a 1x1 gray fallback (RealTerrain mode won't look useful)
            log::info!("No terrain texture provided");
            let gray = image::RgbaImage::from_pixel(1, 1, image::Rgba([128, 128, 128, 255]));
            let (_tex, view, sampler) = create_terrain_texture(device, queue, &gray);
            (view, sampler)
        };

        // Bind group layout matching shader
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Map Bind Group Layout"),
            entries: &[
                // binding 0: province texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // binding 1: province sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 2: lookup texture
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // binding 3: lookup sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 4: camera uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 5: settings uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 6: heightmap texture (VERTEX for terrain displacement, FRAGMENT for shading)
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // binding 7: heightmap sampler (VERTEX for terrain displacement, FRAGMENT for shading)
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 8: terrain color texture (RealTerrain mode)
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // binding 9: terrain color sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Map Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&province_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&province_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&lookup_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&lookup_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: settings_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&heightmap_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&heightmap_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&terrain_view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(&terrain_sampler),
                },
            ],
        });

        // Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Map Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Map Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Map Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            // Depth stencil required for compatibility with render pass.
            // Map renders at full depth (always visible).
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        // =====================================================================
        // Army marker pipeline
        // =====================================================================

        // Army bind group layout (just camera uniform)
        let army_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Army Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        // Army bind group
        let army_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Army Bind Group"),
            layout: &army_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // Army pipeline layout
        let army_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Army Pipeline Layout"),
            bind_group_layouts: &[&army_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Army instance buffer
        let army_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Army Instance Buffer"),
            size: (MAX_ARMIES * std::mem::size_of::<ArmyInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Army render pipeline
        let army_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Army Render Pipeline"),
            layout: Some(&army_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_army",
                buffers: &[ArmyInstance::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_army",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            // Depth stencil required for compatibility with render pass.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        log::info!("Created army marker pipeline");

        // =====================================================================
        // Fleet marker pipeline (diamond shape, same layout as army)
        // =====================================================================

        // Fleet bind group (reuses same camera buffer via same layout)
        let fleet_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Fleet Bind Group"),
            layout: &army_bind_group_layout, // Same layout - just camera uniform
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // Fleet instance buffer
        let fleet_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Fleet Instance Buffer"),
            size: (MAX_FLEETS * std::mem::size_of::<FleetInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Fleet render pipeline (same layout as army, different shader entry points)
        let fleet_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Fleet Render Pipeline"),
            layout: Some(&army_pipeline_layout), // Same layout - just camera uniform
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_fleet",
                buffers: &[FleetInstance::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_fleet",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            // Depth stencil required for compatibility with render pass.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        log::info!("Created fleet marker pipeline");

        Self {
            pipeline,
            bind_group,
            camera_buffer,
            settings_buffer,
            province_texture,
            lookup_texture,
            map_size,
            army_pipeline,
            army_bind_group,
            army_instance_buffer,
            army_count: 0,
            fleet_pipeline,
            fleet_bind_group,
            fleet_instance_buffer,
            fleet_count: 0,
        }
    }

    /// Updates the camera uniform buffer.
    pub fn update_camera(&self, queue: &wgpu::Queue, uniform: CameraUniform) {
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[uniform]));
    }

    /// Updates the map mode in the settings buffer.
    ///
    /// # Arguments
    /// * `map_mode` - 0.0 for political mode, 1.0 for terrain mode
    /// * `zoom` - Current camera zoom level (used for border thickness)
    pub fn update_map_mode(
        &self,
        queue: &wgpu::Queue,
        map_mode: f32,
        map_size: (u32, u32),
        zoom: f32,
    ) {
        let settings = MapSettings {
            texture_size: [map_size.0 as f32, map_size.1 as f32],
            lookup_size: LOOKUP_SIZE as f32,
            border_enabled: 1.0,
            map_mode,
            border_thickness: calculate_border_thickness(zoom),
            _padding: [0.0; 2],
        };
        queue.write_buffer(&self.settings_buffer, 0, bytemuck::cast_slice(&[settings]));
    }

    /// Updates the color lookup texture with new province colors.
    /// Call this when province ownership changes.
    pub fn update_lookup(&self, queue: &wgpu::Queue, data: &LookupUpdateData) {
        update_lookup_texture(queue, &self.lookup_texture, data);
    }

    /// Updates the army instance buffer with current army positions.
    /// Returns the number of armies to render.
    pub fn update_armies(&mut self, queue: &wgpu::Queue, instances: &[ArmyInstance]) -> u32 {
        let count = instances.len().min(MAX_ARMIES);
        if count > 0 {
            queue.write_buffer(
                &self.army_instance_buffer,
                0,
                bytemuck::cast_slice(&instances[..count]),
            );
        }
        self.army_count = count as u32;
        count as u32
    }

    /// Updates the fleet instance buffer with current fleet positions.
    /// Returns the number of fleets to render.
    pub fn update_fleets(&mut self, queue: &wgpu::Queue, instances: &[FleetInstance]) -> u32 {
        let count = instances.len().min(MAX_FLEETS);
        if count > 0 {
            queue.write_buffer(
                &self.fleet_instance_buffer,
                0,
                bytemuck::cast_slice(&instances[..count]),
            );
        }
        self.fleet_count = count as u32;
        count as u32
    }
}

// =============================================================================
// Sprite Renderer for UI elements (flags, icons, etc.)
// =============================================================================

/// Sprite instance data for rendering a textured quad.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpriteInstance {
    /// Position in clip space (-1..1).
    pub pos: [f32; 2],
    /// Size in clip space.
    pub size: [f32; 2],
    /// UV coordinates: min (top-left).
    pub uv_min: [f32; 2],
    /// UV coordinates: max (bottom-right).
    pub uv_max: [f32; 2],
}

impl SpriteInstance {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SpriteInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // pos
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // size
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // uv_min
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 2]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // uv_max
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 2]>() * 3) as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// Maximum sprites per frame.
/// Sized for complex UI: country selection with bookmarks, saves, date widget,
/// topbar, text glyphs, etc. Each text character is one sprite draw.
const MAX_SPRITES_PER_FRAME: usize = 2048;

/// Sprite renderer for drawing textured quads (flags, icons).
///
/// Uses a ring buffer to support multiple draw calls per frame without
/// overwriting instance data before the GPU reads it.
pub struct SpriteRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    /// Pipeline for masked flag rendering (flag + mask + overlay).
    masked_flag_pipeline: wgpu::RenderPipeline,
    /// Bind group layout for masked flag (4 bindings: sprite + sampler + mask + sampler).
    masked_flag_bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    instance_buffer: wgpu::Buffer,
    /// Current slot in the instance buffer (reset each frame).
    current_slot: std::cell::Cell<usize>,
}

impl SpriteRenderer {
    /// Creates a new sprite renderer.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sprite Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Sprite Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Sprite Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Sprite Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_sprite",
                buffers: &[SpriteInstance::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_sprite",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            // Depth stencil required for compatibility with 3D terrain render pass.
            // Sprites render on top of everything (Always pass, no depth write).
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Sprite Instance Buffer"),
            size: std::mem::size_of::<SpriteInstance>() as u64 * MAX_SPRITES_PER_FRAME as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Masked flag pipeline (for shield-style flag rendering)
        let masked_flag_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Masked Flag Bind Group Layout"),
                entries: &[
                    // binding 0: flag texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    // binding 1: flag sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // binding 2: mask texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    // binding 3: mask sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let masked_flag_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Masked Flag Pipeline Layout"),
                bind_group_layouts: &[&masked_flag_bind_group_layout],
                push_constant_ranges: &[],
            });

        let masked_flag_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Masked Flag Render Pipeline"),
            layout: Some(&masked_flag_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_sprite",
                buffers: &[SpriteInstance::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_masked_flag",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            // Depth stencil required for compatibility with 3D terrain render pass.
            // UI elements render on top of everything (Always pass, no depth write).
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Self {
            pipeline,
            bind_group_layout,
            masked_flag_pipeline,
            masked_flag_bind_group_layout,
            sampler,
            instance_buffer,
            current_slot: std::cell::Cell::new(0),
        }
    }

    /// Reset the slot counter at the start of each frame.
    pub fn begin_frame(&self) {
        self.current_slot.set(0);
    }

    /// Creates a bind group for a texture.
    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        texture_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sprite Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    /// Creates a bind group for masked flag rendering (flag + mask textures).
    pub fn create_masked_bind_group(
        &self,
        device: &wgpu::Device,
        flag_view: &wgpu::TextureView,
        mask_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Masked Flag Bind Group"),
            layout: &self.masked_flag_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(flag_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(mask_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    /// Draws a masked flag (flag texture clipped by mask).
    /// Position is in clip space (-1..1), size is in clip space units.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_masked_flag<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        bind_group: &'a wgpu::BindGroup,
        queue: &wgpu::Queue,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) {
        // Get next slot and advance counter
        let slot = self.current_slot.get();
        if slot >= MAX_SPRITES_PER_FRAME {
            log::warn!(
                "Too many sprites in one frame (max {})",
                MAX_SPRITES_PER_FRAME
            );
            return;
        }
        self.current_slot.set(slot + 1);

        let instance = SpriteInstance {
            pos: [x, y],
            size: [width, height],
            uv_min: [0.0, 0.0],
            uv_max: [1.0, 1.0],
        };

        // Write to this slot's offset in the buffer
        let offset = (slot * std::mem::size_of::<SpriteInstance>()) as u64;
        queue.write_buffer(
            &self.instance_buffer,
            offset,
            bytemuck::cast_slice(&[instance]),
        );

        // Draw using masked flag pipeline
        let start = offset;
        let end = offset + std::mem::size_of::<SpriteInstance>() as u64;

        render_pass.set_pipeline(&self.masked_flag_pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.instance_buffer.slice(start..end));
        render_pass.draw(0..6, 0..1);
    }

    /// Draws a sprite at the given position and size (full texture).
    /// Position is in clip space (-1..1), size is in clip space units.
    #[allow(clippy::too_many_arguments)]
    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        bind_group: &'a wgpu::BindGroup,
        queue: &wgpu::Queue,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) {
        self.draw_uv(
            render_pass,
            bind_group,
            queue,
            x,
            y,
            width,
            height,
            0.0,
            0.0,
            1.0,
            1.0,
        );
    }

    /// Draws a sprite with custom UV coordinates (for sprite strips).
    /// Position is in clip space (-1..1), size is in clip space units.
    /// UV coords specify the texture region: (u_min, v_min, u_max, v_max).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_uv<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        bind_group: &'a wgpu::BindGroup,
        queue: &wgpu::Queue,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        u_min: f32,
        v_min: f32,
        u_max: f32,
        v_max: f32,
    ) {
        // Get next slot and advance counter
        let slot = self.current_slot.get();
        if slot >= MAX_SPRITES_PER_FRAME {
            log::warn!(
                "Too many sprites in one frame (max {})",
                MAX_SPRITES_PER_FRAME
            );
            return;
        }
        self.current_slot.set(slot + 1);

        let instance = SpriteInstance {
            pos: [x, y],
            size: [width, height],
            uv_min: [u_min, v_min],
            uv_max: [u_max, v_max],
        };

        // Write to this slot's offset in the buffer
        let offset = (slot * std::mem::size_of::<SpriteInstance>()) as u64;
        queue.write_buffer(
            &self.instance_buffer,
            offset,
            bytemuck::cast_slice(&[instance]),
        );

        // Draw using just this one instance from its slot
        let start = offset;
        let end = offset + std::mem::size_of::<SpriteInstance>() as u64;

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.instance_buffer.slice(start..end));
        render_pass.draw(0..6, 0..1);
    }

    /// Draw a 9-slice (cornered tile) sprite in pixel coordinates.
    ///
    /// 9-slice divides a texture into 9 regions using border sizes:
    /// - 4 corners: fixed size, never stretched
    /// - 4 edges: stretched along one axis
    /// - 1 center: stretched along both axes
    ///
    /// # Arguments
    /// * `x, y, width, height` - Target rectangle in PIXELS
    /// * `border_x, border_y` - Border size in pixels
    /// * `tex_w, tex_h` - Actual texture dimensions in pixels
    /// * `screen_size` - Current screen resolution
    #[allow(clippy::too_many_arguments)]
    pub fn draw_cornered_tile<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        bind_group: &'a wgpu::BindGroup,
        queue: &wgpu::Queue,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        border_x: f32,
        border_y: f32,
        tex_w: u32,
        tex_h: u32,
        screen_size: (u32, u32),
    ) {
        let result = generate_9_slice_quads(
            (x, y),
            (width, height),
            (border_x, border_y),
            (tex_w, tex_h),
        );

        match result {
            NineSliceResult::Full(quads) => {
                for quad in quads.iter() {
                    let (clip_x, clip_y, clip_w, clip_h) = rect_to_clip_space(
                        (quad.pos[0], quad.pos[1]),
                        (quad.size[0] as u32, quad.size[1] as u32),
                        screen_size,
                    );
                    self.draw_uv(
                        render_pass,
                        bind_group,
                        queue,
                        clip_x,
                        clip_y,
                        clip_w,
                        clip_h,
                        quad.uv_pos[0],
                        quad.uv_pos[1],
                        quad.uv_pos[0] + quad.uv_size[0],
                        quad.uv_pos[1] + quad.uv_size[1],
                    );
                }
            }
            NineSliceResult::Fallback(quad) => {
                let (clip_x, clip_y, clip_w, clip_h) = rect_to_clip_space(
                    (quad.pos[0], quad.pos[1]),
                    (quad.size[0] as u32, quad.size[1] as u32),
                    screen_size,
                );
                self.draw_uv(
                    render_pass,
                    bind_group,
                    queue,
                    clip_x,
                    clip_y,
                    clip_w,
                    clip_h,
                    quad.uv_pos[0],
                    quad.uv_pos[1],
                    quad.uv_pos[0] + quad.uv_size[0],
                    quad.uv_pos[1] + quad.uv_size[1],
                );
            }
        }
    }

    /// Backwards compatibility wrapper for draw_nine_slice.
    /// Deprecated: use draw_cornered_tile with pixel coordinates.
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    pub fn draw_nine_slice<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        bind_group: &'a wgpu::BindGroup,
        queue: &wgpu::Queue,
        clip_x: f32,
        clip_y: f32,
        clip_width: f32,
        clip_height: f32,
        border_x: u32,
        border_y: u32,
        tex_w: u32,
        tex_h: u32,
        screen_size: (u32, u32),
    ) {
        // Convert clip space back to pixels for draw_cornered_tile
        let x = (clip_x + 1.0) / 2.0 * screen_size.0 as f32;
        let y = (1.0 - clip_y) / 2.0 * screen_size.1 as f32;
        let width = clip_width / 2.0 * screen_size.0 as f32;
        let height = clip_height / 2.0 * screen_size.1 as f32;

        self.draw_cornered_tile(
            render_pass,
            bind_group,
            queue,
            x,
            y,
            width,
            height,
            border_x as f32,
            border_y as f32,
            tex_w,
            tex_h,
            screen_size,
        );
    }
}

// =============================================================================
// 3D Terrain Renderer (Phase 4)
// =============================================================================

use crate::camera::{CameraUniform3D, Frustum, TerrainSettings};
use crate::terrain_mesh::{TerrainChunk, TerrainMeshConfig, TerrainVertex};

/// Depth texture format for terrain rendering.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Creates a depth texture for terrain rendering.
pub fn create_depth_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    (texture, view)
}

/// GPU buffers for a terrain chunk.
pub struct TerrainChunkBuffers {
    /// Vertex buffer.
    pub vertex_buffer: wgpu::Buffer,
    /// Index buffer.
    pub index_buffer: wgpu::Buffer,
    /// Number of indices to draw.
    pub index_count: u32,
    /// Chunk position for debugging.
    #[allow(dead_code)]
    pub chunk_pos: (u32, u32),
    /// Axis-aligned bounding box for frustum culling.
    pub aabb_min: glam::Vec3,
    pub aabb_max: glam::Vec3,
}

impl TerrainChunkBuffers {
    /// Creates GPU buffers from a terrain chunk.
    pub fn from_chunk(device: &wgpu::Device, chunk: &TerrainChunk) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!(
                "Terrain Vertex Buffer ({}, {})",
                chunk.chunk_pos.0, chunk.chunk_pos.1
            )),
            contents: bytemuck::cast_slice(&chunk.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!(
                "Terrain Index Buffer ({}, {})",
                chunk.chunk_pos.0, chunk.chunk_pos.1
            )),
            contents: bytemuck::cast_slice(&chunk.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            index_count: chunk.indices.len() as u32,
            chunk_pos: chunk.chunk_pos,
            aabb_min: chunk.aabb.min,
            aabb_max: chunk.aabb.max,
        }
    }
}

/// 3D terrain renderer using heightmap displacement.
pub struct TerrainRenderer {
    /// Render pipeline for terrain.
    pub pipeline: wgpu::RenderPipeline,
    /// Bind group layout for terrain-specific uniforms (group 1).
    /// Kept for potential future use when recreating bind groups.
    #[allow(dead_code)]
    pub bind_group_layout: wgpu::BindGroupLayout,
    /// Bind groups for each horizontal copy (left, center, right).
    /// Each has a different x_offset for horizontal wrapping.
    pub bind_groups: [wgpu::BindGroup; 3],
    /// Camera uniform buffer (shared across all copies).
    pub camera_buffer: wgpu::Buffer,
    /// Terrain settings buffers for each copy.
    /// Index 0: left copy (-map_width), 1: center (0), 2: right (+map_width).
    pub settings_buffers: [wgpu::Buffer; 3],
    /// Terrain chunk buffers.
    pub chunks: Vec<TerrainChunkBuffers>,
    /// Depth texture.
    pub depth_texture: wgpu::Texture,
    /// Depth texture view.
    pub depth_view: wgpu::TextureView,
    /// Map width for horizontal wrapping calculations.
    pub map_width: f32,
}

impl TerrainRenderer {
    /// Creates a new terrain renderer.
    ///
    /// # Arguments
    /// * `device` - GPU device
    /// * `surface_format` - Render target format
    /// * `map_bind_group_layout` - Bind group layout from main map renderer (group 0)
    /// * `width` - Initial viewport width
    /// * `height` - Initial viewport height
    /// * `config` - Terrain mesh configuration
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        map_bind_group_layout: &wgpu::BindGroupLayout,
        width: u32,
        height: u32,
        config: &TerrainMeshConfig,
    ) -> Self {
        // Create depth texture
        let (depth_texture, depth_view) = create_depth_texture(device, width, height);

        let map_width = config.map_width;

        // Create camera buffer (shared across all 3 copies)
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Terrain Camera Buffer"),
            contents: bytemuck::cast_slice(&[CameraUniform3D::default()]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create 3 settings buffers with different x_offsets for horizontal wrapping
        // Index 0: left copy (-map_width), 1: center (0), 2: right (+map_width)
        let x_offsets = [-map_width, 0.0, map_width];
        let settings_buffers: [wgpu::Buffer; 3] = std::array::from_fn(|i| {
            let settings =
                TerrainSettings::with_x_offset(TerrainSettings::DEFAULT_HEIGHT_SCALE, x_offsets[i]);
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Terrain Settings Buffer (offset {})", i)),
                contents: bytemuck::cast_slice(&[settings]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            })
        });

        // Bind group layout for terrain-specific uniforms (group 1)
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Terrain Bind Group Layout"),
            entries: &[
                // binding 0: camera uniform (view_proj matrix)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 1: terrain settings (height_scale, x_offset)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create 3 bind groups (one per x_offset copy)
        let bind_groups: [wgpu::BindGroup; 3] = std::array::from_fn(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Terrain Bind Group (offset {})", i)),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: camera_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: settings_buffers[i].as_entire_binding(),
                    },
                ],
            })
        });

        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Terrain Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        // Pipeline layout: group 0 = map textures, group 1 = terrain uniforms
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Terrain Pipeline Layout"),
            bind_group_layouts: &[map_bind_group_layout, &bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline with depth test
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Terrain Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_terrain",
                buffers: &[TerrainVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_terrain",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back), // Cull back faces for terrain
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        // Generate terrain chunks
        let terrain_chunks = crate::terrain_mesh::generate_all_chunks(config);
        let chunks: Vec<TerrainChunkBuffers> = terrain_chunks
            .iter()
            .map(|c| TerrainChunkBuffers::from_chunk(device, c))
            .collect();

        log::info!(
            "Created terrain renderer with {} chunks ({} total triangles)",
            chunks.len(),
            chunks.iter().map(|c| c.index_count / 3).sum::<u32>()
        );

        Self {
            pipeline,
            bind_group_layout,
            bind_groups,
            camera_buffer,
            settings_buffers,
            chunks,
            depth_texture,
            depth_view,
            map_width,
        }
    }

    /// Resizes the depth texture for a new viewport size.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let (depth_texture, depth_view) = create_depth_texture(device, width, height);
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
    }

    /// Updates the camera uniform buffer.
    pub fn update_camera(&self, queue: &wgpu::Queue, uniform: CameraUniform3D) {
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[uniform]));
    }

    /// Updates the height scale in all terrain settings buffers.
    ///
    /// Preserves the x_offsets that were set during construction.
    pub fn update_settings(&self, queue: &wgpu::Queue, settings: TerrainSettings) {
        let x_offsets = [-self.map_width, 0.0, self.map_width];
        for (i, buffer) in self.settings_buffers.iter().enumerate() {
            let adjusted = TerrainSettings::with_x_offset(settings.height_scale, x_offsets[i]);
            queue.write_buffer(buffer, 0, bytemuck::cast_slice(&[adjusted]));
        }
    }

    /// Renders all terrain chunks with horizontal wrapping.
    ///
    /// Renders 3 copies of the terrain at different X offsets:
    /// - Left copy at x - map_width
    /// - Center copy at x (original position)
    /// - Right copy at x + map_width
    ///
    /// This creates seamless horizontal wrapping as the camera pans.
    /// Frustum culling ensures only visible copies are drawn.
    ///
    /// # Arguments
    /// * `render_pass` - Active render pass with depth attachment
    /// * `map_bind_group` - Bind group 0 with map textures
    /// * `frustum` - View frustum for culling
    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        map_bind_group: &'a wgpu::BindGroup,
        frustum: &Frustum,
    ) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, map_bind_group, &[]);

        // X-offsets for horizontal wrapping: left copy, center, right copy
        let x_offsets = [-self.map_width, 0.0, self.map_width];
        let mut chunks_drawn = 0u32;
        let mut chunks_culled = 0u32;

        // Render 3 copies with different x_offsets for horizontal wrapping
        for (copy_idx, bind_group) in self.bind_groups.iter().enumerate() {
            let x_offset = x_offsets[copy_idx];

            for chunk in &self.chunks {
                // Offset the chunk's AABB by the copy's x_offset
                let min = glam::Vec3::new(
                    chunk.aabb_min.x + x_offset,
                    chunk.aabb_min.y,
                    chunk.aabb_min.z,
                );
                let max = glam::Vec3::new(
                    chunk.aabb_max.x + x_offset,
                    chunk.aabb_max.y,
                    chunk.aabb_max.z,
                );

                // Frustum cull: skip chunks outside the view
                if !frustum.intersects_aabb(min, max) {
                    chunks_culled += 1;
                    continue;
                }

                chunks_drawn += 1;
                render_pass.set_bind_group(1, bind_group, &[]);
                render_pass.set_vertex_buffer(0, chunk.vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(chunk.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..chunk.index_count, 0, 0..1);
            }
        }

        // Log culling stats occasionally (every ~60 frames would be ideal, but we don't track)
        if chunks_culled > 0 {
            log::trace!(
                "Frustum culling: {} chunks drawn, {} culled",
                chunks_drawn,
                chunks_culled
            );
        }
    }
}

#[cfg(test)]
mod terrain_tests {
    use super::*;

    #[test]
    fn test_depth_format() {
        // Verify we're using Depth32Float as planned
        assert_eq!(
            DEPTH_FORMAT,
            wgpu::TextureFormat::Depth32Float,
            "Depth format should be Depth32Float for precision"
        );
    }

    #[test]
    fn test_terrain_vertex_buffer_layout() {
        let layout = TerrainVertex::desc();
        assert_eq!(
            layout.array_stride, 20,
            "TerrainVertex stride should be 20 bytes"
        );
        assert_eq!(
            layout.attributes.len(),
            2,
            "TerrainVertex should have 2 attributes (position, tex_coords)"
        );
    }

    #[test]
    fn test_camera_uniform_3d_size() {
        // 4x4 f32 matrix = 64 bytes
        assert_eq!(
            std::mem::size_of::<CameraUniform3D>(),
            64,
            "CameraUniform3D should be 64 bytes"
        );
    }

    #[test]
    fn test_terrain_settings_size() {
        // f32 + vec3 padding = 16 bytes
        assert_eq!(
            std::mem::size_of::<TerrainSettings>(),
            16,
            "TerrainSettings should be 16 bytes"
        );
    }

    #[test]
    fn test_map_settings_size() {
        // 6 f32 fields + 2 f32 padding = 32 bytes
        assert_eq!(
            std::mem::size_of::<MapSettings>(),
            32,
            "MapSettings should be 32 bytes (8 f32s for GPU alignment)"
        );
    }

    #[test]
    fn test_border_thickness_calculation() {
        // Zoomed out (low zoom) → slightly thicker borders (so they remain visible)
        assert_eq!(calculate_border_thickness(1.0), 1.2); // Clamped to max
        assert_eq!(calculate_border_thickness(4.0), 1.2); // 8/4 = 2.0, clamps to 1.2

        // Normal zoom → ~1.0 thickness
        assert_eq!(calculate_border_thickness(8.0), 1.0); // 8/8 = 1.0

        // Zoomed in (high zoom) → thin borders (so they don't dominate)
        assert_eq!(calculate_border_thickness(16.0), 0.5); // 8/16 = 0.5
        assert_eq!(calculate_border_thickness(50.0), 0.3); // Clamped to min
        assert_eq!(calculate_border_thickness(100.0), 0.3); // Clamped to min
    }
}

#[cfg(test)]
mod mode_switching_tests {
    use super::*;
    use crate::testing::HeadlessGpu;

    /// Test that sprite pipeline works with depth stencil attachment (3D mode).
    ///
    /// This is a critical test to prevent regressions where pipelines become
    /// incompatible with render passes that use depth buffers.
    #[test]
    fn test_sprite_pipeline_with_depth_attachment() {
        let Some(gpu) = pollster::block_on(HeadlessGpu::new()) else {
            eprintln!("Skipping test: no GPU available");
            return;
        };

        // Create sprite renderer
        let sprite_renderer = SpriteRenderer::new(&gpu.device, gpu.format);

        // Create offscreen texture
        let width = 64u32;
        let height = 64u32;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Test Color Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let color_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create depth texture
        let (_, depth_view) = create_depth_texture(&gpu.device, width, height);

        // Create render pass WITH depth attachment (simulating 3D terrain mode)
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Test Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("3D Mode Test Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // Set the sprite pipeline - this should NOT panic
            render_pass.set_pipeline(&sprite_renderer.pipeline);
            // We don't actually draw, just verify the pipeline is compatible
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Test that sprite pipeline also works WITHOUT depth stencil (2D mode).
    ///
    /// This ensures the pipeline is compatible with both 2D and 3D render passes.
    #[test]
    fn test_sprite_pipeline_without_depth_attachment() {
        let Some(gpu) = pollster::block_on(HeadlessGpu::new()) else {
            eprintln!("Skipping test: no GPU available");
            return;
        };

        // Create sprite renderer
        let sprite_renderer = SpriteRenderer::new(&gpu.device, gpu.format);

        // Create offscreen texture
        let width = 64u32;
        let height = 64u32;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Test Color Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let color_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create depth texture (needed even for 2D mode now)
        let (_, depth_view) = create_depth_texture(&gpu.device, width, height);

        // Create render pass with depth attachment (all modes now use depth)
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Test Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("2D Mode Test Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // Set the sprite pipeline - this should NOT panic
            render_pass.set_pipeline(&sprite_renderer.pipeline);
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Test that masked flag pipeline works with depth attachment.
    #[test]
    fn test_masked_flag_pipeline_with_depth_attachment() {
        let Some(gpu) = pollster::block_on(HeadlessGpu::new()) else {
            eprintln!("Skipping test: no GPU available");
            return;
        };

        // Create sprite renderer (contains masked flag pipeline)
        let sprite_renderer = SpriteRenderer::new(&gpu.device, gpu.format);

        // Create offscreen texture
        let width = 64u32;
        let height = 64u32;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Test Color Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let color_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create depth texture
        let (_, depth_view) = create_depth_texture(&gpu.device, width, height);

        // Create render pass WITH depth attachment
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Test Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Masked Flag Test Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // Set the masked flag pipeline - this should NOT panic
            render_pass.set_pipeline(&sprite_renderer.masked_flag_pipeline);
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Test that map pipeline works with depth attachment.
    ///
    /// Tests the Renderer's main map pipeline.
    #[test]
    fn test_map_pipeline_with_depth_attachment() {
        let Some(gpu) = pollster::block_on(HeadlessGpu::new()) else {
            eprintln!("Skipping test: no GPU available");
            return;
        };

        // Create minimal test textures for Renderer
        let province_img = image::RgbaImage::new(64, 64);
        let province_lookup = std::collections::HashMap::new();

        // Create renderer
        let renderer = Renderer::new(
            &gpu.device,
            &gpu.queue,
            gpu.format,
            &province_img,
            &province_lookup,
            None, // no heightmap
            None, // no terrain texture
        );

        // Create offscreen texture
        let width = 64u32;
        let height = 64u32;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Test Color Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let color_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create depth texture
        let (_, depth_view) = create_depth_texture(&gpu.device, width, height);

        // Create render pass WITH depth attachment
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Test Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Map Pipeline Test Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // Set the map pipeline - this should NOT panic
            render_pass.set_pipeline(&renderer.pipeline);
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Test that army pipeline works with depth attachment.
    #[test]
    fn test_army_pipeline_with_depth_attachment() {
        let Some(gpu) = pollster::block_on(HeadlessGpu::new()) else {
            eprintln!("Skipping test: no GPU available");
            return;
        };

        // Create minimal test textures for Renderer
        let province_img = image::RgbaImage::new(64, 64);
        let province_lookup = std::collections::HashMap::new();

        // Create renderer
        let renderer = Renderer::new(
            &gpu.device,
            &gpu.queue,
            gpu.format,
            &province_img,
            &province_lookup,
            None, // no heightmap
            None, // no terrain texture
        );

        // Create offscreen texture
        let width = 64u32;
        let height = 64u32;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Test Color Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let color_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create depth texture
        let (_, depth_view) = create_depth_texture(&gpu.device, width, height);

        // Create render pass WITH depth attachment
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Test Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Army Pipeline Test Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // Set the army pipeline - this should NOT panic
            render_pass.set_pipeline(&renderer.army_pipeline);
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Test that fleet pipeline works with depth attachment.
    #[test]
    fn test_fleet_pipeline_with_depth_attachment() {
        let Some(gpu) = pollster::block_on(HeadlessGpu::new()) else {
            eprintln!("Skipping test: no GPU available");
            return;
        };

        // Create minimal test textures for Renderer
        let province_img = image::RgbaImage::new(64, 64);
        let province_lookup = std::collections::HashMap::new();

        // Create renderer
        let renderer = Renderer::new(
            &gpu.device,
            &gpu.queue,
            gpu.format,
            &province_img,
            &province_lookup,
            None, // no heightmap
            None, // no terrain texture
        );

        // Create offscreen texture
        let width = 64u32;
        let height = 64u32;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Test Color Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let color_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create depth texture
        let (_, depth_view) = create_depth_texture(&gpu.device, width, height);

        // Create render pass WITH depth attachment
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Test Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Fleet Pipeline Test Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // Set the fleet pipeline - this should NOT panic
            render_pass.set_pipeline(&renderer.fleet_pipeline);
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));
    }
}
