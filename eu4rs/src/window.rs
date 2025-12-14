use crate::args::MapMode;
use crate::camera::Camera;
use eu4data::countries::Country;
use eu4data::history::ProvinceHistory;
use image::{GenericImageView, RgbImage};
use std::collections::HashMap;
use wgpu::util::DeviceExt; // Now used for buffer creation
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

pub struct WorldData {
    pub province_map: RgbImage,
    pub political_map: RgbImage,
    pub tradegoods_map: RgbImage,
    pub religion_map: RgbImage,
    pub culture_map: RgbImage,
    pub color_to_id: HashMap<(u8, u8, u8), u32>,
    pub province_history: HashMap<u32, ProvinceHistory>,
    #[allow(dead_code)]
    pub countries: HashMap<String, Country>,
}

impl WorldData {
    pub fn get_province_id(&self, x: u32, y: u32) -> Option<u32> {
        if x >= self.province_map.width() || y >= self.province_map.height() {
            return None;
        }
        let pixel = self.province_map.get_pixel(x, y);
        let rgb = (pixel[0], pixel[1], pixel[2]);
        self.color_to_id.get(&rgb).copied()
    }

    pub fn get_province_tooltip(&self, id: u32) -> String {
        if let Some(hist) = self.province_history.get(&id) {
            let owner = hist.owner.as_deref().unwrap_or("---");
            let goods = hist.trade_goods.as_deref().unwrap_or("---");
            let religion = hist.religion.as_deref().unwrap_or("---");
            let culture = hist.culture.as_deref().unwrap_or("---");
            format!(
                "Province ID: {}\nOwner: {}\nGoods: {}\nReli: {}\nCult: {}",
                id, owner, goods, religion, culture
            )
        } else {
            format!("Province ID: {}\n(No History)", id)
        }
    }

    pub fn get_mode_specific_tooltip(&self, id: u32, mode: MapMode) -> String {
        if let Some(hist) = self.province_history.get(&id) {
            match mode {
                MapMode::Province => format!("Province ID: {}", id),
                MapMode::Political => format!("Owner: {}", hist.owner.as_deref().unwrap_or("---")),
                MapMode::TradeGoods => {
                    format!("Goods: {}", hist.trade_goods.as_deref().unwrap_or("---"))
                }
                MapMode::Religion => {
                    format!("Religion: {}", hist.religion.as_deref().unwrap_or("---"))
                }
                MapMode::Culture => {
                    format!("Culture: {}", hist.culture.as_deref().unwrap_or("---"))
                }
                _ => format!("Province ID: {}", id),
            }
        } else {
            format!("Province ID: {} (No History)", id)
        }
    }
}

/// Decoupled application state for logic testing
pub struct AppState {
    pub world_data: WorldData,
    pub window_size: (u32, u32),
    pub cursor_pos: Option<(f64, f64)>,
    pub camera: Camera,
    pub is_panning: bool,
    pub last_cursor_pos: Option<(f64, f64)>,
    pub current_map_mode: MapMode,
}

impl AppState {
    pub fn new(world_data: WorldData, width: u32, height: u32) -> Self {
        let (tex_w, tex_h) = world_data.province_map.dimensions();
        let content_aspect = if tex_h > 0 {
            tex_w as f64 / tex_h as f64
        } else {
            1.0
        };
        Self {
            world_data,
            window_size: (width, height),
            cursor_pos: None,
            camera: Camera::new(content_aspect),
            is_panning: false,
            last_cursor_pos: None,
            current_map_mode: MapMode::Province,
        }
    }

    pub fn get_hover_text(&self) -> Option<String> {
        if let Some((mx, my)) = self.cursor_pos {
            let (win_w, win_h) = self.window_size;
            let (tex_w, tex_h) = self.world_data.province_map.dimensions();
            if win_w == 0 || win_h == 0 {
                return None;
            }

            let (u_world, v_world) =
                self.camera
                    .screen_to_world(mx, my, win_w as f64, win_h as f64);
            if !(0.0..=1.0).contains(&v_world) {
                return None;
            }

            let x = (u_world * tex_w as f64) as u32;
            let y = (v_world * tex_h as f64) as u32;

            #[allow(clippy::collapsible_if)]
            if x < tex_w && y < tex_h {
                if let Some(id) = self.world_data.get_province_id(x, y) {
                    return Some(
                        self.world_data
                            .get_mode_specific_tooltip(id, self.current_map_mode),
                    );
                }
            }
        }
        None
    }

    pub fn toggle_map_mode(&mut self) -> MapMode {
        self.current_map_mode = match self.current_map_mode {
            MapMode::Province => MapMode::Political,
            MapMode::Political => MapMode::TradeGoods,
            MapMode::TradeGoods => MapMode::Religion,
            MapMode::Religion => MapMode::Culture,
            MapMode::Culture => MapMode::Province,
            _ => MapMode::Province,
        };
        self.current_map_mode
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.window_size = (width, height);
        }
    }

    pub fn update_cursor(&mut self, x: f64, y: f64) {
        self.cursor_pos = Some((x, y));
    }

    pub fn get_selected_province(&self) -> Option<(u32, String)> {
        if let Some((mx, my)) = self.cursor_pos {
            // Map pos to texture coordinates
            let (win_w, win_h) = self.window_size;
            let (tex_w, tex_h) = self.world_data.province_map.dimensions();

            // Avoid divide by zero
            if win_w == 0 || win_h == 0 {
                return None;
            }

            // Camera Transform Logic
            let (u_world, v_world) =
                self.camera
                    .screen_to_world(mx, my, win_w as f64, win_h as f64);
            println!(
                "Screen ({}, {}) -> World ({:.4}, {:.4})",
                mx, my, u_world, v_world
            );

            if !(0.0..=1.0).contains(&v_world) {
                println!("Click Out of Bounds (Y)");
                return None;
            }

            let x = (u_world * tex_w as f64) as u32;
            let y = (v_world * tex_h as f64) as u32;
            println!("Texture Coords: ({}, {})", x, y);

            if x < tex_w && y < tex_h {
                if let Some(id) = self.world_data.get_province_id(x, y) {
                    return Some((id, self.world_data.get_province_tooltip(id)));
                } else {
                    return Some((0, "Unknown Province".to_string()));
                }
            }
        }
        None
    }
}

use crate::text::TextRenderer;

pub struct Texture {
    #[allow(dead_code)]
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Texture {
    #[allow(dead_code)]
    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        label: &str,
    ) -> Result<Self, image::ImageError> {
        let img = image::load_from_memory(bytes)?;
        Self::from_image(device, queue, &img, Some(label))
    }

    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &image::DynamicImage,
        label: Option<&str>,
    ) -> Result<Self, image::ImageError> {
        let rgba = img.to_rgba8();
        let dimensions = img.dimensions();

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
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &rgba,
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
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Ok(Self {
            texture,
            view,
            sampler,
        })
    }
}

/// Independent rendering state that can be used for both on-screen (winit) and headless (offscreen) rendering.
/// Holds the VRAM resources (textures, pipelines, buffers).
pub struct Eu4Renderer {
    pub render_pipeline: wgpu::RenderPipeline,
    pub diffuse_bind_group: wgpu::BindGroup,
    pub diffuse_texture: Texture,
    pub camera_buffer: wgpu::Buffer,

    // UI Overlay components
    pub ui_pipeline: wgpu::RenderPipeline,
    pub ui_bind_group: wgpu::BindGroup,
    pub ui_texture: Texture,
    pub ui_camera_buffer: wgpu::Buffer,
}

impl Eu4Renderer {
    pub fn update_texture(&mut self, device: &wgpu::Device, texture: Texture) {
        self.diffuse_texture = texture;

        let camera_bind_group_layout = self.render_pipeline.get_bind_group_layout(0);

        self.diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.diffuse_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.camera_buffer.as_entire_binding(),
                },
            ],
            label: Some("diffuse_bind_group_updated"),
        });
    }

    pub fn update_ui_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &image::RgbaImage,
    ) {
        // Option A: Write to existing texture if size matches (faster) or recreate?
        // Let's assume size might change, so we recreate or reuse intelligently.
        // For simplicity: new texture from image.
        let dyn_img = image::DynamicImage::ImageRgba8(img.clone());
        if let Ok(new_tex) = Texture::from_image(device, queue, &dyn_img, Some("UI Texture")) {
            self.ui_texture = new_tex;

            let bind_group_layout = self.ui_pipeline.get_bind_group_layout(0);
            self.ui_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.ui_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.ui_texture.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.ui_camera_buffer.as_entire_binding(),
                    },
                ],
                label: Some("ui_bind_group"),
            });
        }
    }

    pub fn update_camera_buffer(&self, queue: &wgpu::Queue, data: [f32; 4]) {
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[data]));
    }

    /// Creates a new renderer.
    ///
    /// This function:
    /// 1. Takes ownership of loading `provinces.bmp` logic (with fallbacks).
    /// 2. Creating `wgpu` textures, views, and samplers.
    /// 3. Compiling the WGSL shader.
    /// 4. Creating the render pipeline and bind groups.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config_format: wgpu::TextureFormat,
        verbose: bool,
    ) -> Self {
        use std::io::Write;
        if verbose {
            print!("\r[1/4] Initialization complete. Loading texture...      ");
            std::io::stdout().flush().unwrap();
        }

        // Load Texture
        // Try local first, then hardcoded path
        let province_path = std::path::Path::new("provinces.bmp");
        let img = if province_path.exists() {
            if verbose {
                print!("\r[2/4] Found local provinces.bmp...                   ");
                std::io::stdout().flush().unwrap();
            }
            image::open(province_path).unwrap()
        } else {
            // Fallback or panic? For now let's try to load from typical install if local missing
            let steam_path = std::path::Path::new(
                "C:/Program Files (x86)/Steam/steamapps/common/Europa Universalis IV/map/provinces.bmp",
            );
            if steam_path.exists() {
                if verbose {
                    print!("\r[2/4] Found Steam installation...                    ");
                    std::io::stdout().flush().unwrap();
                }
                image::open(steam_path).unwrap()
            } else {
                if verbose {
                    print!("\r[2/4] Texture not found! Using fallback...           ");
                    std::io::stdout().flush().unwrap();
                }
                // Create a dummy pink texture if missing
                image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                    100,
                    100,
                    image::Rgba([255, 0, 255, 255]),
                ))
            }
        };

        if verbose {
            print!("\r[3/4] Uploading texture to GPU...                  ");
            std::io::stdout().flush().unwrap();
        }
        let diffuse_texture =
            Texture::from_image(device, queue, &img, Some("provinces.bmp")).unwrap();
        if verbose {
            print!("\r[4/4] Texture uploaded. Starting loop...           ");
            std::io::stdout().flush().unwrap();
        }

        // Create Camera Buffer
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[0.5f32, 0.5f32, 1.0f32, 1.0f32]), // Initial Identity (Center 0.5, Scale 1.0)
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: camera_buffer.as_entire_binding(),
                },
            ],
            label: Some("diffuse_bind_group"),
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&texture_bind_group_layout],
                push_constant_ranges: &[],
            });

        // 1. Regular Map Pipeline (No Blending / Replace)
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
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
                    format: config_format,
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

        // 2. UI Pipeline (Alpha Blending)
        let ui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("UI Pipeline"),
            layout: Some(&render_pipeline_layout), // Same layout (texture+sampler+camera)
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main", // Same shader
                targets: &[Some(wgpu::ColorTargetState {
                    format: config_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING), // Enable Blending
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

        // UI Camera (Identity for overlay 1:1)
        // Center 0.5, Scale 1.0 means the texture covers the screen exactly (if UVs are 0-1).
        // Since we generate the UI texture to match screen size, this is perfect.
        let ui_camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("UI Camera Buffer"),
            contents: bytemuck::cast_slice(&[0.5f32, 0.5f32, 1.0f32, 1.0f32]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Initial Dummy UI Texture (Transparent 1x1)
        let dummy_ui = image::DynamicImage::ImageRgba8(image::RgbaImage::new(1, 1));
        let ui_texture = Texture::from_image(device, queue, &dummy_ui, Some("UI Texture")).unwrap();

        let ui_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&ui_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&ui_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ui_camera_buffer.as_entire_binding(),
                },
            ],
            label: Some("ui_bind_group"),
        });

        Self {
            render_pipeline,
            diffuse_bind_group,
            diffuse_texture,
            camera_buffer,
            ui_pipeline,
            ui_bind_group,
            ui_texture,
            ui_camera_buffer,
        }
    }
}

struct State<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: &'a winit::window::Window,
    renderer: Eu4Renderer,
    app_state: AppState, // Encapsulated logic state
    text_renderer: TextRenderer,
    ui_state: crate::ui::UIState,
}

impl<'a> State<'a> {
    // Creating some of the wgpu types requires async code
    async fn new(
        window: &'a winit::window::Window,
        verbose: bool,
        world_data: WorldData,
    ) -> State<'a> {
        // Enforce 1920x1080 Physical or specific size if possible, otherwise use existing.
        // The user specifically asked for "Physical" to avoid DPI issues.
        // Winit doesn't easily let us "force" exact physical pixels if OS scaling is active,
        // but we can try to request initialization at a size that matches.
        // For strict "No DPI Scaling" behavior, we often just ignore scale factor or request specific inner physical size.
        // However, on Windows, the OS decorates the window.
        // Let's try to request the physical size immediately.

        let target_width = 1920;
        let target_height = 1080;
        let _ =
            window.request_inner_size(winit::dpi::PhysicalSize::new(target_width, target_height));

        // Wait for resize? No, we proceed.
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });

        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        let renderer = Eu4Renderer::new(&device, &queue, config.format, verbose);

        // Load font
        let font_path = std::path::Path::new("assets/Roboto-Regular.ttf");
        let font_data = std::fs::read(font_path).expect("Failed to load assets/Roboto-Regular.ttf");
        let text_renderer = TextRenderer::new(font_data);

        // Create AppState
        let mut app_state = AppState::new(world_data, size.width, size.height);

        // Initial Camera Setup: "Fit Height"
        // view_height_tex = 1.0 (covering full map height).
        // view_height_tex = (1.0 / zoom) * content_aspect / screen_aspect
        // 1.0 = (1.0 / zoom) * content_aspect / screen_aspect
        // zoom = content_aspect / screen_aspect

        let screen_aspect = size.width as f64 / size.height as f64;
        let content_aspect = app_state.camera.content_aspect;
        app_state.camera.zoom = content_aspect / screen_aspect;
        // Make sure we clamp? Camera::pan logic clamps.
        // We'll trust this calculation is close enough to start.

        Self {
            window,
            surface,
            device,
            queue,
            config,
            size,
            renderer,
            app_state,
            text_renderer,
            ui_state: crate::ui::UIState::new(),
        }
    }

    pub fn window(&self) -> &winit::window::Window {
        self.window
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);

            // Forward resize to AppState
            self.app_state.resize(new_size.width, new_size.height);
        }
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let old_pos = self.app_state.last_cursor_pos;
                self.app_state.last_cursor_pos = Some((position.x, position.y));
                self.app_state.update_cursor(position.x, position.y);

                // Update UI State Cursor
                self.ui_state.set_cursor_pos(Some((position.x, position.y)));

                // Update Hover Tooltip if strictly over map
                if !self.ui_state.sidebar_open || position.x < (self.size.width as f64 - 300.0) {
                    if let Some(text) = self.app_state.get_hover_text() {
                        self.ui_state.set_hovered_tooltip(Some(text));
                    } else {
                        self.ui_state.set_hovered_tooltip(None);
                    }
                } else {
                    self.ui_state.set_hovered_tooltip(None);
                }

                #[allow(clippy::collapsible_if)]
                if self.app_state.is_panning {
                    if let Some((ox, oy)) = old_pos {
                        let dx = position.x - ox;
                        let dy = position.y - oy;
                        self.app_state.camera.pan(
                            dx,
                            dy,
                            self.size.width as f64,
                            self.size.height as f64,
                        );
                        return true;
                    }
                }
                false
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let zoom_factor = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => {
                        if *y > 0.0 {
                            1.1
                        } else {
                            0.9
                        }
                    }
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        if pos.y > 0.0 {
                            1.1
                        } else {
                            0.9
                        }
                    }
                };

                if let Some((cx, cy)) = self.app_state.cursor_pos {
                    self.app_state.camera.zoom(
                        zoom_factor,
                        cx,
                        cy,
                        self.size.width as f64,
                        self.size.height as f64,
                    );
                    return true;
                }
                false
            }
            WindowEvent::MouseInput {
                state,
                button: winit::event::MouseButton::Middle,
                ..
            } => {
                self.app_state.is_panning = *state == winit::event::ElementState::Pressed;
                true
            }
            WindowEvent::MouseInput {
                state: winit::event::ElementState::Pressed,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                let mx = self.app_state.cursor_pos.unwrap_or((0.0, 0.0)).0;

                // Check UI first
                if self.ui_state.on_click(mx, self.size.width as f64) {
                    println!("Click consumed by UI");
                    return true;
                }

                // Map Logic
                println!("Click detected at {:?}", self.app_state.cursor_pos);
                if let Some((id, text)) = self.app_state.get_selected_province() {
                    println!(
                        "Picked Province: {} ({})",
                        id,
                        text.lines().next().unwrap_or("")
                    );

                    // Update UI Selection
                    self.ui_state.set_selected_province(Some((id, text)));
                    self.ui_state.set_sidebar_open(true);

                    return true;
                }
                false
            }

            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        state: winit::event::ElementState::Pressed,
                        physical_key:
                            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Tab),
                        ..
                    },
                ..
            } => {
                let new_mode = self.app_state.toggle_map_mode();
                println!("Switched Map Mode to: {:?}", new_mode);

                self.ui_state.map_mode = new_mode;
                self.ui_state.set_dirty();

                // Update Texture
                let img = match new_mode {
                    MapMode::Province => &self.app_state.world_data.province_map,
                    MapMode::Political => &self.app_state.world_data.political_map,
                    MapMode::TradeGoods => &self.app_state.world_data.tradegoods_map,
                    MapMode::Religion => &self.app_state.world_data.religion_map,
                    MapMode::Culture => &self.app_state.world_data.culture_map,
                    _ => &self.app_state.world_data.province_map,
                };

                let dynamic_img = image::DynamicImage::ImageRgb8(img.clone());
                if let Ok(texture) = Texture::from_image(
                    &self.device,
                    &self.queue,
                    &dynamic_img,
                    Some("Map Texture"),
                ) {
                    self.renderer.update_texture(&self.device, texture);
                }
                true
            }

            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        state: winit::event::ElementState::Pressed,
                        physical_key:
                            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Escape),
                        ..
                    },
                ..
            } => {
                if self.ui_state.sidebar_open {
                    self.ui_state.set_sidebar_open(false);
                    println!("Closed Sidebar");
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn update(&mut self) {}

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // Main render loop
        // Update Camera Buffer logic
        let uniform = self
            .app_state
            .camera
            .to_uniform_data(self.size.width as f32, self.size.height as f32);
        self.renderer.update_camera_buffer(&self.queue, uniform);

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // 1. Update UI Texture if Dirty
        if self.ui_state.dirty {
            let ui_img = crate::ui::draw_ui(
                &self.ui_state,
                &self.text_renderer,
                self.size.width,
                self.size.height,
            );
            self.renderer
                .update_ui_texture(&self.device, &self.queue, &ui_img);
            self.ui_state.dirty = false;
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // Draw Map
            render_pass.set_pipeline(&self.renderer.render_pipeline);
            render_pass.set_bind_group(0, &self.renderer.diffuse_bind_group, &[]);
            render_pass.draw(0..3, 0..1); // Full screen triangle for map

            // Draw UI Overlay
            render_pass.set_pipeline(&self.renderer.ui_pipeline);
            render_pass.set_bind_group(0, &self.renderer.ui_bind_group, &[]);
            render_pass.draw(0..3, 0..1); // Full screen triangle for UI
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

/// Main entry point for the Windowed (GUI) mode.
///
/// Initializes `winit` event loop, opens a window, creates the `State` (which wraps `Eu4Renderer`),
/// and starts the render loop.
pub async fn run(verbose: bool, world_data: WorldData) {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("eu4rs Source Port")
        .build(&event_loop)
        .unwrap();

    let mut state = State::new(&window, verbose, world_data).await;

    event_loop
        .run(move |event, elwt| match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } => {
                if window_id == state.window().id() && !state.input(event) {
                    match event {
                        WindowEvent::CloseRequested => elwt.exit(),
                        WindowEvent::Resized(physical_size) => state.resize(*physical_size),
                        WindowEvent::RedrawRequested => {
                            state.update();
                            match state.render() {
                                Ok(_) => {}
                                Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                                Err(wgpu::SurfaceError::OutOfMemory) => elwt.exit(),
                                Err(e) => eprintln!("{:?}", e),
                            }
                        }
                        _ => {}
                    }
                }
            }
            Event::AboutToWait => {
                state.window().request_redraw();
            }
            _ => {}
        })
        .unwrap();
}

/// Headless entry point for rendering to a file.
///
/// Encapsulates the entire process of:
/// 1. Initializing `wgpu` (Instance, Adapter, Device) without a surface.
/// 2. Creating `Eu4Renderer` to handle assets and pipelines.
/// 3. Rendering to an offscreen texture.
/// 4. Reading back the texture data and saving it as a PNG.
///
/// Returns immediately if no GPU adapter is found (CI waiver).
pub async fn snapshot(
    eu4_path: &std::path::Path,
    output_path: &std::path::Path,
    mode: MapMode,
) -> Result<(), String> {
    // We need to re-init logger if it hasn't been initialized?
    // Actually env_logger::init() panics if called twice.
    // Let's assume it might be called.
    let _ = env_logger::try_init();

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..Default::default()
    });

    // For headless, we don't have a surface. We just need an adapter.
    // Try to find one compatible with our needs.
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None, // Headless!
            force_fallback_adapter: false,
        })
        .await;

    // Graceful degradation for CI
    if adapter.is_none() {
        // Return a specific error string that callers can check against
        return Err("No suitable graphics adapter found (CI waiver)".to_string());
    }
    let adapter = adapter.unwrap();

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        )
        .await
        .unwrap();

    // Use a standard texture format for offscreen rendering
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    // Verbose = true for logs
    let mut renderer = Eu4Renderer::new(&device, &queue, format, true);

    match mode {
        MapMode::Political => {
            println!("Loading world data for Political Snapshot...");
            // We use ops::load_world_data which requires crates::ops
            let world_data = crate::ops::load_world_data(eu4_path)?;
            let img = &world_data.political_map;
            let dynamic_img = image::DynamicImage::ImageRgb8(img.clone());
            if let Ok(texture) =
                Texture::from_image(&device, &queue, &dynamic_img, Some("Snapshot Texture"))
            {
                renderer.update_texture(&device, texture);
            }
        }
        _ => {
            // Default Eu4Renderer loads province map
        }
    }

    let size = renderer.diffuse_texture.texture.size();
    let (width, height) = (size.width, size.height);

    // Create offscreen texture to render to
    let texture_desc = wgpu::TextureDescriptor {
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
        label: Some("Offscreen Texture"),
        view_formats: &[],
    };
    let texture = device.create_texture(&texture_desc);
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    // Create buffer to read back data
    let u32_size = std::mem::size_of::<u32>() as u32;
    let output_buffer_size = (u32_size * width * height) as wgpu::BufferAddress;
    let output_buffer_desc = wgpu::BufferDescriptor {
        size: output_buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        label: Some("Output Buffer"),
        mapped_at_creation: false,
    };
    let output_buffer = device.create_buffer(&output_buffer_desc);

    // Render
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Render Encoder"),
    });

    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        render_pass.set_pipeline(&renderer.render_pipeline);
        render_pass.set_bind_group(0, &renderer.diffuse_bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    // Copy texture to buffer
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            aspect: wgpu::TextureAspect::All,
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
        },
        wgpu::ImageCopyBuffer {
            buffer: &output_buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(u32_size * width),
                rows_per_image: Some(height),
            },
        },
        texture_desc.size,
    );

    queue.submit(Some(encoder.finish()));

    // Read buffer
    let buffer_slice = output_buffer.slice(..);

    // NOTE: We have to map the buffer to access it
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |v| tx.send(v).unwrap());
    device.poll(wgpu::Maintain::Wait);
    rx.recv().unwrap().unwrap();

    let data = buffer_slice.get_mapped_range();

    use image::{ImageBuffer, Rgba};
    let buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, data.to_vec()).unwrap();
    buffer.save(output_path).unwrap();
    println!("Snapshot saved to {:?}", output_path);

    drop(data);
    output_buffer.unmap();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing;
    use crate::text::TextRenderer;
    use eu4data::history::ProvinceHistory;
    use image::RgbImage;
    use std::collections::HashMap;
    use std::path::Path;

    #[test]
    fn test_province_inspector() {
        // 1. Setup Mock WorldData
        let mut color_to_id = HashMap::new();
        color_to_id.insert((255, 0, 0), 1); // Red -> ID 1

        // Create 2x2 map
        let mut province_map = RgbImage::new(2, 2);
        province_map.put_pixel(0, 0, image::Rgb([255, 0, 0])); // (0,0) is ID 1
        province_map.put_pixel(1, 1, image::Rgb([0, 255, 0])); // (1,1) is Unknown

        let mut province_history = HashMap::new();
        province_history.insert(
            1,
            ProvinceHistory {
                // id: 1, // ID is the key, not in the value struct
                owner: Some("SWE".to_string()),
                trade_goods: Some("grain".to_string()),
                base_tax: Some(0.0),
                base_production: Some(0.0),
                base_manpower: Some(0.0),
                religion: Some("catholic".to_string()),
                culture: Some("swedish".to_string()),
                // events: vec![], // Not in struct
            },
        );

        let world_data = WorldData {
            province_map,
            political_map: RgbImage::new(1, 1),
            tradegoods_map: RgbImage::new(1, 1),
            religion_map: RgbImage::new(1, 1),
            culture_map: RgbImage::new(1, 1),
            color_to_id,
            province_history,
            countries: HashMap::new(),
        };

        // 2. Verify Data Retrieval
        let id = world_data.get_province_id(0, 0).expect("Should find ID 1");
        assert_eq!(id, 1);

        let tooltip = world_data.get_province_tooltip(id);
        assert!(tooltip.contains("SWE"));
        assert!(tooltip.contains("grain"));

        // 3. Render Inspector Image
        // Load font (borrowed from assets)
        let font_path = Path::new("../assets/Roboto-Regular.ttf");
        let font_path = if font_path.exists() {
            font_path
        } else {
            Path::new("assets/Roboto-Regular.ttf")
        };

        if !font_path.exists() {
            eprintln!("Skipping inspector test, font not found at {:?}", font_path);
            return;
        }

        let font_data = std::fs::read(font_path).unwrap();
        let renderer = TextRenderer::new(font_data);

        // Use a consistent text for snapshot
        let img = renderer.render(&tooltip, 400, 300);

        // 4. Assert Snapshot
        testing::assert_snapshot(&img, "inspector_province_1");
    }

    #[test]
    fn test_map_snapshot() {
        // This test runs the full headless map render

        let output_path = std::env::temp_dir().join("test_map_snapshot.png");

        // Block on the async snapshot function
        // Use current dir and Province mode for regression test
        match pollster::block_on(crate::window::snapshot(
            std::path::Path::new("."),
            &output_path,
            MapMode::Province,
        )) {
            Ok(_) => {
                // Load the result and assert
                let img = image::open(&output_path)
                    .expect("Failed to load map snapshot output")
                    .to_rgba8();
                testing::assert_snapshot(&img, "map_province");
            }
            Err(e) => {
                if e.contains("CI waiver") {
                    println!("Skipping test_map_snapshot: {}", e);
                    return;
                }
                panic!("Snapshot generation failed: {}", e);
            }
        }
    }

    #[test]
    fn test_political_snapshot() {
        let output_path = std::env::temp_dir().join("test_map_political.png");
        let steam_path = std::path::Path::new(
            "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Europa Universalis IV",
        );

        // Use Steam path and Political mode
        if steam_path.exists() {
            match pollster::block_on(crate::window::snapshot(
                steam_path,
                &output_path,
                MapMode::Political,
            )) {
                Ok(_) => {
                    let img = image::open(&output_path)
                        .expect("Failed to load political snapshot output")
                        .to_rgba8();
                    testing::assert_snapshot(&img, "map_political");
                }
                Err(e) => {
                    if e.contains("CI waiver") {
                        println!("Skipping test_political_snapshot: {}", e);
                        return;
                    }
                    panic!("Political Snapshot generation failed: {}", e);
                }
            }
        }
    }

    #[test]
    fn test_tradegoods_snapshot() {
        let output_path = std::env::temp_dir().join("test_map_tradegoods.png");
        let steam_path = std::path::Path::new(
            "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Europa Universalis IV",
        );

        if steam_path.exists() {
            match pollster::block_on(crate::window::snapshot(
                steam_path,
                &output_path,
                MapMode::TradeGoods,
            )) {
                Ok(_) => {
                    let img = image::open(&output_path)
                        .expect("Failed to load tradegoods snapshot output")
                        .to_rgba8();
                    testing::assert_snapshot(&img, "map_tradegoods");
                }
                Err(e) => {
                    if e.contains("CI waiver") {
                        println!("Skipping test_tradegoods_snapshot: {}", e);
                        return;
                    }
                    panic!("Tradegoods Snapshot generation failed: {}", e);
                }
            }
        } else {
            println!("Skipping test_tradegoods_snapshot: Steam path not found");
        }
    }

    #[test]
    fn test_religion_snapshot() {
        let output_path = std::env::temp_dir().join("test_map_religion.png");
        let steam_path = std::path::Path::new(
            "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Europa Universalis IV",
        );

        if steam_path.exists() {
            match pollster::block_on(crate::window::snapshot(
                steam_path,
                &output_path,
                MapMode::Religion,
            )) {
                Ok(_) => {
                    let img = image::open(&output_path)
                        .expect("Failed to load religion snapshot output")
                        .to_rgba8();
                    testing::assert_snapshot(&img, "map_religion");
                }
                Err(e) => {
                    if e.contains("CI waiver") {
                        println!("Skipping test_religion_snapshot: {}", e);
                        return;
                    }
                    panic!("Religion Snapshot generation failed: {}", e);
                }
            }
        } else {
            println!("Skipping test_religion_snapshot: Steam path not found");
        }
    }

    #[test]
    fn test_culture_snapshot() {
        let output_path = std::env::temp_dir().join("test_map_culture.png");
        let steam_path = std::path::Path::new(
            "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Europa Universalis IV",
        );

        if steam_path.exists() {
            match pollster::block_on(crate::window::snapshot(
                steam_path,
                &output_path,
                MapMode::Culture,
            )) {
                Ok(_) => {
                    let img = image::open(&output_path)
                        .expect("Failed to load culture snapshot output")
                        .to_rgba8();
                    testing::assert_snapshot(&img, "map_culture");
                }
                Err(e) => {
                    if e.contains("CI waiver") {
                        println!("Skipping test_culture_snapshot: {}", e);
                        return;
                    }
                    panic!("Culture Snapshot generation failed: {}", e);
                }
            }
        } else {
            println!("Skipping test_culture_snapshot: Steam path not found");
        }
    }

    #[test]
    fn test_world_data_logic() {
        use crate::window::WorldData;
        use eu4data::history::ProvinceHistory;
        use image::{Rgb, RgbImage};
        use std::collections::HashMap;

        // 1. Create a 2x2 Image
        // (0,0) = Red -> ID 1
        // (1,0) = Green -> ID 2
        // (0,1) = Blue -> Unknown
        // (1,1) = Black -> Unknown
        let mut img = RgbImage::new(2, 2);
        img.put_pixel(0, 0, Rgb([255, 0, 0]));
        img.put_pixel(1, 0, Rgb([0, 255, 0]));
        img.put_pixel(0, 1, Rgb([0, 0, 255]));
        img.put_pixel(1, 1, Rgb([0, 0, 0]));

        // 2. Map Colors to IDs
        let mut color_to_id = HashMap::new();
        color_to_id.insert((255, 0, 0), 1);
        color_to_id.insert((0, 255, 0), 2);

        // 3. History
        let mut history = HashMap::new();
        history.insert(
            1,
            ProvinceHistory {
                owner: Some("SWE".to_string()),
                trade_goods: Some("grain".to_string()),
                base_tax: None,
                base_production: None,
                base_manpower: None,
                religion: Some("catholic".to_string()),
                culture: Some("swedish".to_string()),
            },
        );

        let world = WorldData {
            province_map: img,
            political_map: RgbImage::new(1, 1),
            tradegoods_map: RgbImage::new(1, 1),
            religion_map: RgbImage::new(1, 1),
            culture_map: RgbImage::new(1, 1),
            color_to_id,
            province_history: history,
            countries: HashMap::new(),
        };

        // Test ID lookup
        assert_eq!(world.get_province_id(0, 0), Some(1));
        assert_eq!(world.get_province_id(1, 0), Some(2));
        assert_eq!(world.get_province_id(0, 1), None); // Blue not in map
        assert_eq!(world.get_province_id(100, 100), None); // Out of bounds

        // Test Tooltip
        let tt1 = world.get_province_tooltip(1);
        assert!(tt1.contains("ID: 1"));
        assert!(tt1.contains("Owner: SWE"));
        assert!(tt1.contains("Goods: grain"));

        let tt2 = world.get_province_tooltip(2);
        assert!(tt2.contains("ID: 2"));
        assert!(tt2.contains("(No History)"));
    }

    #[test]
    fn test_app_state_logic() {
        use crate::window::{AppState, WorldData};
        use image::RgbImage;
        use std::collections::HashMap;

        // Tests Interaction Logic without Window/WGPU dependency

        // 1. Setup Mock
        let mut color_to_id = HashMap::new();
        color_to_id.insert((255, 0, 0), 1);
        let mut img = RgbImage::new(100, 100);
        // Fill Red
        for p in img.pixels_mut() {
            *p = image::Rgb([255, 0, 0]);
        }

        let world = WorldData {
            province_map: img,
            political_map: RgbImage::new(1, 1),
            tradegoods_map: RgbImage::new(1, 1),
            religion_map: RgbImage::new(1, 1),
            culture_map: RgbImage::new(1, 1),
            color_to_id,
            province_history: HashMap::new(),
            countries: HashMap::new(),
        };

        // 2. Init AppState (Window size 100x100)
        let mut app = AppState::new(world, 100, 100);

        // 3. Test Selection (Middle of map -> ID 1)
        app.update_cursor(50.0, 50.0);
        let sel = app.get_selected_province();
        assert!(sel.is_some());
        let (id, text) = sel.unwrap();
        assert_eq!(id, 1);
        assert!(text.contains("ID: 1"));

        // 4. Test Resize (Window 200x200, but map still 100x100)
        // Cursor at 100.0, 100.0 (Middle of window) should still be ID 1
        app.resize(200, 200);
        app.update_cursor(100.0, 100.0);

        let sel2 = app.get_selected_province();
        assert!(sel2.is_some());
        assert_eq!(sel2.unwrap().0, 1);

        // 5. Test Update Cursor
        app.update_cursor(0.0, 0.0);
        assert_eq!(app.cursor_pos, Some((0.0, 0.0)));
    }
}
