use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};
// use wgpu::util::DeviceExt; // Unused for now
use image::GenericImageView;

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
    #[allow(dead_code)]
    pub diffuse_texture: Texture,
}

impl Eu4Renderer {
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
            println!(); // Newline at the end
        }

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

        Self {
            render_pipeline,
            diffuse_bind_group,
            diffuse_texture,
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
}

impl<'a> State<'a> {
    // Creating some of the wgpu types requires async code
    async fn new(window: &'a winit::window::Window, verbose: bool) -> State<'a> {
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
            .find(|f| f.is_srgb())
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

        // We need dimensions data to resize...
        // Eu4Renderer has diffuse_texture.
        let renderer = Eu4Renderer::new(&device, &queue, config.format, verbose);
        let tex_size = renderer.diffuse_texture.texture.size();
        let (width, height) = (tex_size.width, tex_size.height);

        // Cap width at 1280 (720p standard) or screen width (heuristic)
        let target_width = if width > 1280 { 1280 } else { width };
        let target_height = (target_width as f64 * (height as f64 / width as f64)) as u32;

        if verbose {
            use std::io::Write;
            print!(
                "\r[2.5/4] Resizing window to {}x{}...                ",
                target_width, target_height
            );
            std::io::stdout().flush().unwrap();
        }
        let _ =
            window.request_inner_size(winit::dpi::PhysicalSize::new(target_width, target_height));

        Self {
            window,
            surface,
            device,
            queue,
            config,
            size,
            renderer,
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
        }
    }

    fn input(&mut self, _event: &WindowEvent) -> bool {
        false
    }

    fn update(&mut self) {}

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
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

            render_pass.set_pipeline(&self.renderer.render_pipeline);
            render_pass.set_bind_group(0, &self.renderer.diffuse_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
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
pub async fn run(verbose: bool) {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("eu4rs Source Port")
        .build(&event_loop)
        .unwrap();

    let mut state = State::new(&window, verbose).await;

    event_loop
        .run(move |event, elwt| {
            match event {
                Event::WindowEvent {
                    ref event,
                    window_id,
                } if window_id == state.window().id() => {
                    if !state.input(event) {
                        match event {
                            WindowEvent::CloseRequested => elwt.exit(),
                            WindowEvent::Resized(physical_size) => state.resize(*physical_size),
                            WindowEvent::RedrawRequested => {
                                state.update();
                                match state.render() {
                                    Ok(_) => {}
                                    // Reconfigure the surface if lost
                                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                                    // The system is out of memory, we should probably quit
                                    Err(wgpu::SurfaceError::OutOfMemory) => elwt.exit(),
                                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                                    Err(e) => eprintln!("{:?}", e),
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Event::AboutToWait => {
                    // RedrawRequested will only trigger once unless we manually request it.
                    state.window().request_redraw();
                }
                _ => {}
            }
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
pub async fn snapshot(output_path: &std::path::Path) {
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
        eprintln!("No suitable graphics adapter found. Skipping snapshot test (CI waiver).");
        std::process::exit(0);
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
    let renderer = Eu4Renderer::new(&device, &queue, format, true);
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
}
