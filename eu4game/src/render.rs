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
// Render Target Abstraction
// ============================================================================

// TODO: Remove when integrated with main.rs
#[allow(dead_code)]
/// Error type for render target operations.
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
/// This trait allows the application to render to either a window surface
/// or an offscreen texture (for headless testing).
#[allow(dead_code)]
pub trait RenderTarget {
    /// Get a texture view to render into and the current dimensions.
    fn get_view(&mut self) -> Result<(wgpu::TextureView, u32, u32), RenderError>;

    /// Present the frame (no-op for offscreen targets).
    fn present(&mut self);

    /// Get the texture format.
    fn format(&self) -> wgpu::TextureFormat;
}

/// Render target backed by a window surface.
#[allow(dead_code)]
pub struct SurfaceTarget {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    current_texture: Option<wgpu::SurfaceTexture>,
}

#[allow(dead_code)]
impl SurfaceTarget {
    /// Create a new surface target.
    pub fn new(surface: wgpu::Surface<'static>, config: wgpu::SurfaceConfiguration) -> Self {
        Self {
            surface,
            config,
            current_texture: None,
        }
    }

    /// Reconfigure the surface after resize.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(device, &self.config);
    }

    /// Reconfigure the surface (e.g., after outdated error).
    pub fn reconfigure(&mut self, device: &wgpu::Device) {
        self.surface.configure(device, &self.config);
    }

    /// Get the current dimensions.
    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// Get a reference to the config.
    pub fn config(&self) -> &wgpu::SurfaceConfiguration {
        &self.config
    }
}

impl RenderTarget for SurfaceTarget {
    fn get_view(&mut self) -> Result<(wgpu::TextureView, u32, u32), RenderError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let dims = (self.config.width, self.config.height);
        self.current_texture = Some(output);
        Ok((view, dims.0, dims.1))
    }

    fn present(&mut self) {
        if let Some(texture) = self.current_texture.take() {
            texture.present();
        }
    }

    fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }
}

/// GPU context abstraction for device/queue access.
///
/// This trait allows `AppCore` to be created with either a windowed
/// or headless GPU context.
#[allow(dead_code)]
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
    /// Padding to align to 16 bytes (wgpu requirement).
    _padding: [f32; 3],
}

impl Default for MapSettings {
    fn default() -> Self {
        Self {
            texture_size: [5632.0, 2048.0],
            lookup_size: LOOKUP_SIZE as f32,
            border_enabled: 1.0,
            map_mode: 0.0, // Default to political mode
            _padding: [0.0; 3],
        }
    }
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
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        province_map: &image::RgbaImage,
        province_lookup: &HashMap<(u8, u8, u8), u32>,
        heightmap: Option<&image::GrayImage>,
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
            _padding: [0.0; 3],
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
                // binding 6: heightmap texture
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // binding 7: heightmap sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
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
            depth_stencil: None,
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
            depth_stencil: None,
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
            depth_stencil: None,
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
    pub fn update_map_mode(&self, queue: &wgpu::Queue, map_mode: f32, map_size: (u32, u32)) {
        let settings = MapSettings {
            texture_size: [map_size.0 as f32, map_size.1 as f32],
            lookup_size: LOOKUP_SIZE as f32,
            border_enabled: 1.0,
            map_mode,
            _padding: [0.0; 3],
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
            depth_stencil: None,
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
            depth_stencil: None,
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
