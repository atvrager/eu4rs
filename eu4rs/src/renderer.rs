use crate::args::MapMode;
use std::collections::HashMap;
use wgpu::util::DeviceExt;

/// A wrapper around WGPU texture resources (texture, view, sampler).
///
/// Handles creation from images and provides easy access to the view and sampler
/// needed for bind groups.
pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Texture {
    /// Creates a texture from a byte slice (e.g. file contents).
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

    /// Creates a texture from a dynamic image.
    ///
    /// Uploads the image data to the GPU immediately.
    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &image::DynamicImage,
        label: Option<&str>,
    ) -> Result<Self, image::ImageError> {
        let rgba = img.to_rgba8();
        let dimensions = (img.width(), img.height());

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
    /// Pipeline for rendering the map (provinces, political, etc.).
    pub render_pipeline: wgpu::RenderPipeline,
    /// Bind group for the map texture and camera.
    pub diffuse_bind_group: wgpu::BindGroup,
    /// Cache of loaded map textures keyed by MapMode.
    pub map_textures: HashMap<MapMode, Texture>,
    /// Uniform buffer for the map camera transform.
    pub camera_buffer: wgpu::Buffer,

    // UI Overlay components
    /// Pipeline for rendering the UI overlay (with alpha blending).
    pub ui_pipeline: wgpu::RenderPipeline,
    /// Bind group for the UI texture.
    pub ui_bind_group: wgpu::BindGroup,
    /// Texture holding the rendered UI (text, sidebar, console).
    pub ui_texture: Texture,
    /// Uniform buffer for the UI camera (usually identity).
    pub ui_camera_buffer: wgpu::Buffer,
}

impl Eu4Renderer {
    /// Switches the active map texture without re-uploading to GPU.
    pub fn set_map_mode(&mut self, device: &wgpu::Device, mode: MapMode) {
        if let Some(texture) = self.map_textures.get(&mode) {
            let camera_bind_group_layout = self.render_pipeline.get_bind_group_layout(0);

            self.diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &camera_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&texture.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.camera_buffer.as_entire_binding(),
                    },
                ],
                label: Some("diffuse_bind_group_updated"),
            });
        } else {
            eprintln!("Map mode texture not found in cache: {:?}", mode);
        }
    }

    /// Updates the UI texture with a new image.
    ///
    /// This should be called whenever the UI state changes (`dirty = true`).
    /// Currently creates a new texture for simplicity, but could be optimized to write to existing texture.
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

    /// Updates the camera buffer with new transform data.
    pub fn update_camera_buffer(&self, queue: &wgpu::Queue, data: crate::camera::CameraUniform) {
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[data]));
    }

    /// Reloads map textures (e.g. after world data is fully loaded).
    pub fn update_maps(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        input_images: HashMap<MapMode, image::DynamicImage>,
    ) {
        self.map_textures.clear();
        for (mode, img) in input_images {
            if let Ok(texture) =
                Texture::from_image(device, queue, &img, Some(&format!("{:?} Texture", mode)))
            {
                self.map_textures.insert(mode, texture);
            }
        }
        // Force refresh of bind group for default mode (Province)
        self.set_map_mode(device, MapMode::Province);
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
        input_images: HashMap<MapMode, image::DynamicImage>,
    ) -> Self {
        use std::io::Write;
        if verbose {
            print!("\r[1/4] Initialization complete. Loading textures...     ");
            std::io::stdout().flush().unwrap();
        }

        let mut map_textures = HashMap::new();

        for (mode, img) in input_images {
            if let Ok(texture) =
                Texture::from_image(device, queue, &img, Some(&format!("{:?} Texture", mode)))
            {
                map_textures.insert(mode, texture);
            }
        }

        if verbose {
            print!("\r[2/4] Textures cached. Uploading to GPU...             ");
            std::io::stdout().flush().unwrap();
        }

        // Create Camera Buffer
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[crate::camera::CameraUniform::default()]), // Initial Identity
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

        // Initial Bind Group (Province)
        let default_texture = map_textures
            .get(&MapMode::Province)
            .expect("Province texture missing");
        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&default_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&default_texture.sampler),
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
            contents: bytemuck::cast_slice(&[crate::camera::CameraUniform::default()]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Initial UI Texture: Plain dark background
        // Console will render properly on first update
        let mut initial_ui = image::RgbaImage::new(1920, 1080);
        for pixel in initial_ui.pixels_mut() {
            *pixel = image::Rgba([20, 20, 25, 255]); // Dark console background
        }

        let ui_texture = Texture::from_image(
            device,
            queue,
            &image::DynamicImage::ImageRgba8(initial_ui),
            Some("UI Texture"),
        )
        .unwrap();

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
            map_textures,
            camera_buffer,
            ui_pipeline,
            ui_bind_group,
            ui_texture,
            ui_camera_buffer,
        }
    }
}
