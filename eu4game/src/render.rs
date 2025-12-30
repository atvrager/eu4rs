//! GPU rendering for the game.
//!
//! Uses wgpu to render the world map with shader-based political coloring.
//! All map rendering happens on the GPU for maximum performance.

use crate::camera::CameraUniform;
use std::collections::HashMap;
use wgpu::util::DeviceExt;

/// Maximum number of army markers that can be rendered.
const MAX_ARMIES: usize = 1024;

/// Maximum number of fleet markers that can be rendered.
const MAX_FLEETS: usize = 512;

/// Size of the color lookup texture (must be power of 2, >= max province ID).
pub const LOOKUP_SIZE: u32 = 8192;

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
}

impl Default for MapSettings {
    fn default() -> Self {
        Self {
            texture_size: [5632.0, 2048.0],
            lookup_size: LOOKUP_SIZE as f32,
            border_enabled: 1.0,
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
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        province_map: &image::RgbaImage,
        province_lookup: &HashMap<(u8, u8, u8), u32>,
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
        };
        let settings_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Settings Buffer"),
            contents: bytemuck::cast_slice(&[settings]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

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
}

impl SpriteInstance {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SpriteInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// Sprite renderer for drawing textured quads (flags, icons).
pub struct SpriteRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    instance_buffer: wgpu::Buffer,
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
            size: std::mem::size_of::<SpriteInstance>() as u64 * 16, // Up to 16 sprites
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            instance_buffer,
        }
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

    /// Draws a sprite at the given position and size.
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
        let instance = SpriteInstance {
            pos: [x, y],
            size: [width, height],
        };
        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&[instance]));

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        render_pass.draw(0..6, 0..1);
    }
}
