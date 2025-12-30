//! Text rendering system using ab_glyph for font rasterization.
//!
//! Provides GPU-accelerated text rendering via a pre-rasterized glyph atlas.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use std::collections::HashMap;

/// Default font size for the glyph cache.
const DEFAULT_FONT_SIZE: f32 = 24.0;

/// Atlas texture dimensions (must be power of 2).
const ATLAS_SIZE: u32 = 512;

/// Information about a cached glyph.
#[derive(Debug, Clone, Copy)]
pub struct GlyphInfo {
    /// UV coordinates in atlas (min_u, min_v, max_u, max_v).
    pub uv: [f32; 4],
    /// Glyph dimensions in pixels.
    pub size: [f32; 2],
    /// Horizontal advance after this glyph.
    pub advance: f32,
    /// Offset from baseline to top of glyph.
    pub bearing_y: f32,
    /// Offset from cursor to left edge of glyph.
    pub bearing_x: f32,
}

/// Pre-rasterized glyph atlas for fast text rendering.
#[allow(dead_code)]
pub struct GlyphCache {
    /// Glyph info lookup by character.
    glyphs: HashMap<char, GlyphInfo>,
    /// Atlas texture.
    pub texture: wgpu::Texture,
    /// Atlas texture view.
    pub view: wgpu::TextureView,
    /// Atlas sampler.
    pub sampler: wgpu::Sampler,
    /// Font line height (ascent + descent).
    pub line_height: f32,
    /// Font ascent (baseline to top).
    pub ascent: f32,
}

impl GlyphCache {
    /// Creates a new glyph cache from font data.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, font_data: &[u8]) -> Option<Self> {
        let font = FontRef::try_from_slice(font_data).ok()?;
        let scale = PxScale::from(DEFAULT_FONT_SIZE);
        let scaled_font = font.as_scaled(scale);

        let ascent = scaled_font.ascent();
        let descent = scaled_font.descent();
        let line_height = ascent - descent;

        // Characters to pre-rasterize (printable ASCII + common symbols)
        let chars: Vec<char> = (32u8..=126u8).map(|c| c as char).collect();

        // Rasterize all glyphs and pack into atlas
        let mut atlas_data = vec![0u8; (ATLAS_SIZE * ATLAS_SIZE * 4) as usize];
        let mut glyphs = HashMap::new();

        let mut cursor_x = 1u32;
        let mut cursor_y = 1u32;
        let mut row_height = 0u32;

        for c in chars {
            let glyph_id = font.glyph_id(c);
            let glyph = glyph_id.with_scale(scale);

            if let Some(outlined) = font.outline_glyph(glyph.clone()) {
                let bounds = outlined.px_bounds();
                let width = bounds.width().ceil() as u32;
                let height = bounds.height().ceil() as u32;

                // Check if we need to wrap to next row
                if cursor_x + width + 1 >= ATLAS_SIZE {
                    cursor_x = 1;
                    cursor_y += row_height + 1;
                    row_height = 0;
                }

                // Skip if atlas is full
                if cursor_y + height >= ATLAS_SIZE {
                    log::warn!("Glyph atlas full, skipping character '{}'", c);
                    continue;
                }

                // Rasterize glyph into atlas
                outlined.draw(|x, y, coverage| {
                    let px = cursor_x + x;
                    let py = cursor_y + y;
                    if px < ATLAS_SIZE && py < ATLAS_SIZE {
                        let idx = ((py * ATLAS_SIZE + px) * 4) as usize;
                        let alpha = (coverage * 255.0) as u8;
                        atlas_data[idx] = 255; // R
                        atlas_data[idx + 1] = 255; // G
                        atlas_data[idx + 2] = 255; // B
                        atlas_data[idx + 3] = alpha; // A
                    }
                });

                // Store glyph info
                let h_metrics = scaled_font.h_advance(glyph_id);
                glyphs.insert(
                    c,
                    GlyphInfo {
                        uv: [
                            cursor_x as f32 / ATLAS_SIZE as f32,
                            cursor_y as f32 / ATLAS_SIZE as f32,
                            (cursor_x + width) as f32 / ATLAS_SIZE as f32,
                            (cursor_y + height) as f32 / ATLAS_SIZE as f32,
                        ],
                        size: [width as f32, height as f32],
                        advance: h_metrics,
                        bearing_y: bounds.min.y,
                        bearing_x: bounds.min.x,
                    },
                );

                cursor_x += width + 1;
                row_height = row_height.max(height);
            } else {
                // Space or other non-visible glyph
                let h_metrics = scaled_font.h_advance(glyph_id);
                glyphs.insert(
                    c,
                    GlyphInfo {
                        uv: [0.0, 0.0, 0.0, 0.0],
                        size: [0.0, 0.0],
                        advance: h_metrics,
                        bearing_y: 0.0,
                        bearing_x: 0.0,
                    },
                );
            }
        }

        log::info!(
            "Created glyph atlas with {} characters ({}x{})",
            glyphs.len(),
            ATLAS_SIZE,
            ATLAS_SIZE
        );

        // Create GPU texture
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Glyph Atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
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
            &atlas_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * ATLAS_SIZE),
                rows_per_image: Some(ATLAS_SIZE),
            },
            wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Some(Self {
            glyphs,
            texture,
            view,
            sampler,
            line_height,
            ascent,
        })
    }

    /// Gets glyph info for a character.
    pub fn get(&self, c: char) -> Option<&GlyphInfo> {
        self.glyphs.get(&c)
    }

    /// Measures the width of a text string in pixels.
    pub fn measure_width(&self, text: &str) -> f32 {
        text.chars()
            .filter_map(|c| self.glyphs.get(&c))
            .map(|g| g.advance)
            .sum()
    }
}

/// Text instance data for instanced rendering.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextQuad {
    /// Position in clip space.
    pub pos: [f32; 2],
    /// Size in clip space.
    pub size: [f32; 2],
    /// UV min (top-left).
    pub uv_min: [f32; 2],
    /// UV max (bottom-right).
    pub uv_max: [f32; 2],
    /// Color (RGBA).
    pub color: [f32; 4],
}

impl TextQuad {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TextQuad>() as wgpu::BufferAddress,
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
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // uv_min
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // uv_max
                wgpu::VertexAttribute {
                    offset: 24,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Maximum number of text quads per draw call.
const MAX_TEXT_QUADS: usize = 1024;

/// GPU text renderer using instanced quads.
pub struct TextRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    /// Glyph cache reference for text layout.
    glyph_cache: GlyphCache,
}

impl TextRenderer {
    /// Creates a new text renderer with the given glyph cache.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        font_data: &[u8],
    ) -> Option<Self> {
        let glyph_cache = GlyphCache::new(device, queue, font_data)?;

        // Shader for text rendering
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Text Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("text_shader.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Text Bind Group Layout"),
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Text Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&glyph_cache.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&glyph_cache.sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Text Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Text Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_text",
                buffers: &[TextQuad::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_text",
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

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Text Instance Buffer"),
            size: (MAX_TEXT_QUADS * std::mem::size_of::<TextQuad>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Some(Self {
            pipeline,
            bind_group,
            instance_buffer,
            glyph_cache,
        })
    }

    /// Prepares text quads for a string at the given screen position.
    /// Returns quads that can be batched with other text.
    ///
    /// `x`, `y` are in pixels from top-left of screen.
    /// `screen_size` is (width, height) in pixels.
    pub fn layout_text(
        &self,
        text: &str,
        x: f32,
        y: f32,
        color: [f32; 4],
        screen_size: (f32, f32),
    ) -> Vec<TextQuad> {
        let mut quads = Vec::new();
        let mut cursor_x = x;
        let baseline_y = y + self.glyph_cache.ascent;

        for c in text.chars() {
            if let Some(glyph) = self.glyph_cache.get(c) {
                if glyph.size[0] > 0.0 && glyph.size[1] > 0.0 {
                    // Convert pixel position to clip space (-1 to 1)
                    let px = cursor_x + glyph.bearing_x;
                    let py = baseline_y + glyph.bearing_y;

                    let clip_x = (px / screen_size.0) * 2.0 - 1.0;
                    let clip_y = 1.0 - (py / screen_size.1) * 2.0;
                    let clip_w = (glyph.size[0] / screen_size.0) * 2.0;
                    let clip_h = (glyph.size[1] / screen_size.1) * 2.0;

                    quads.push(TextQuad {
                        pos: [clip_x, clip_y],
                        size: [clip_w, clip_h],
                        uv_min: [glyph.uv[0], glyph.uv[1]],
                        uv_max: [glyph.uv[2], glyph.uv[3]],
                        color,
                    });
                }
                cursor_x += glyph.advance;
            }
        }

        quads
    }

    /// Draws text quads to the render pass.
    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        quads: &[TextQuad],
    ) {
        if quads.is_empty() {
            return;
        }

        let count = quads.len().min(MAX_TEXT_QUADS);
        queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&quads[..count]),
        );

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        render_pass.draw(0..6, 0..count as u32);
    }

    /// Convenience method to draw text directly.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        text: &str,
        x: f32,
        y: f32,
        color: [f32; 4],
        screen_size: (f32, f32),
    ) {
        let quads = self.layout_text(text, x, y, color, screen_size);
        self.draw(render_pass, queue, &quads);
    }

    /// Gets the line height in pixels.
    #[allow(dead_code)]
    pub fn line_height(&self) -> f32 {
        self.glyph_cache.line_height
    }

    /// Measures text width in pixels.
    pub fn measure_width(&self, text: &str) -> f32 {
        self.glyph_cache.measure_width(text)
    }
}
