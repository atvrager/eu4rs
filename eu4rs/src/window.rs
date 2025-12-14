use crate::args::MapMode;
use std::collections::HashMap;

use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

pub use crate::renderer::Eu4Renderer;
pub use crate::state::{AppState, WorldData};

use crate::text::TextRenderer;

use crate::logger::ConsoleLog;
use std::sync::mpsc::Receiver;

enum AppFlow {
    Loading(Receiver<Result<WorldData, String>>),
    Running(Box<AppState>),
}

struct State<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: &'a winit::window::Window,
    renderer: Eu4Renderer,
    text_renderer: TextRenderer,
    ui_state: crate::ui::UIState,

    flow: AppFlow,
    console_log: ConsoleLog,
}

impl<'a> State<'a> {
    // Creating some of the wgpu types requires async code
    async fn new(
        window: &'a winit::window::Window,
        log_level: log::LevelFilter,
        eu4_path: &std::path::Path,
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

        // Load font immediately for console (TextRenderer)
        let font_path = std::path::Path::new("assets/Roboto-Regular.ttf");
        let font_data = std::fs::read(font_path).expect("Failed to load assets/Roboto-Regular.ttf");
        let text_renderer = TextRenderer::new(font_data);

        // Initialize Logger
        let console_log = match crate::logger::init(log_level) {
            Ok(cl) => cl,
            Err(_) => crate::logger::ConsoleLog::new(50),
        };

        // Create Dummy Maps for Initial Loading State
        let mut map_images = HashMap::new();
        let black_pixel = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            1,
            1,
            image::Rgb([0, 0, 0]),
        ));

        map_images.insert(MapMode::Province, black_pixel.clone());
        map_images.insert(MapMode::Political, black_pixel.clone());
        map_images.insert(MapMode::TradeGoods, black_pixel.clone());
        map_images.insert(MapMode::Religion, black_pixel.clone());
        map_images.insert(MapMode::Culture, black_pixel.clone());

        // Initialize Renderer with Dummy Maps
        let verbose = log_level >= log::LevelFilter::Info;
        let renderer = Eu4Renderer::new(&device, &queue, config.format, verbose, map_images);

        // Spawn async loading thread
        let (tx, rx) = std::sync::mpsc::channel();
        let eu4_path_clone = eu4_path.to_path_buf();
        std::thread::spawn(move || {
            let res = crate::ops::load_world_data(&eu4_path_clone);
            let _ = tx.send(res);
        });

        Self {
            window,
            surface,
            device,
            queue,
            config,
            size,
            renderer,
            text_renderer,
            ui_state: crate::ui::UIState::new(),
            flow: AppFlow::Loading(rx),
            console_log,
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

            // Forward resize to AppState if Running
            if let AppFlow::Running(ref mut app_state) = self.flow {
                app_state.resize(new_size.width, new_size.height);
            }
        }
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        let app_state = match &mut self.flow {
            AppFlow::Running(state) => state,
            AppFlow::Loading(_) => return false,
        };

        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let old_pos = app_state.last_cursor_pos;
                app_state.last_cursor_pos = Some((position.x, position.y));
                app_state.update_cursor(position.x, position.y);

                // Update UI State Cursor
                self.ui_state.set_cursor_pos(Some((position.x, position.y)));

                // Update Hover Tooltip if strictly over map
                if !self.ui_state.sidebar_open || position.x < (self.size.width as f64 - 300.0) {
                    if let Some(text) = app_state.get_hover_text() {
                        self.ui_state.set_hovered_tooltip(Some(text));
                    } else {
                        self.ui_state.set_hovered_tooltip(None);
                    }
                } else {
                    // Over sidebar
                    self.ui_state.set_hovered_tooltip(None);
                }

                #[allow(clippy::collapsible_if)]
                if app_state.is_panning {
                    if let Some((old_x, old_y)) = old_pos {
                        let dx = position.x - old_x;
                        let dy = position.y - old_y;
                        app_state.camera.pan(
                            dx,
                            dy,
                            self.size.width as f64,
                            self.size.height as f64,
                        );
                    }
                }
            }
            WindowEvent::MouseInput {
                state,
                button: winit::event::MouseButton::Middle,
                ..
            } => {
                app_state.is_panning = *state == winit::event::ElementState::Pressed;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let zoom_amount = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => *y * 0.1,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.01,
                };
                // Zoom towards cursor
                if let Some((mx, my)) = app_state.cursor_pos {
                    app_state.camera.zoom(
                        zoom_amount.into(),
                        mx,
                        my,
                        self.size.width as f64,
                        self.size.height as f64,
                    );
                }
            }
            WindowEvent::MouseInput {
                state: winit::event::ElementState::Pressed,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                // If not clicking on sidebar
                #[allow(clippy::collapsible_if)]
                if !self.ui_state.sidebar_open
                    || app_state.cursor_pos.unwrap().0 < (self.size.width as f64 - 300.0)
                {
                    if let Some((id, text)) = app_state.get_selected_province() {
                        println!("Clicked Province: {} - {}", id, text);
                        self.ui_state.set_selected_province(Some((id, text)));
                        self.ui_state.set_sidebar_open(true); // Open sidebar on click
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        state: winit::event::ElementState::Pressed,
                        physical_key: winit::keyboard::PhysicalKey::Code(keycode),
                        ..
                    },
                ..
            } => match keycode {
                winit::keyboard::KeyCode::KeyC => {
                    self.ui_state.toggle_console();
                }
                winit::keyboard::KeyCode::Tab => {
                    let new_mode = app_state.toggle_map_mode();
                    println!("Switched Map Mode to: {:?}", new_mode);
                    self.renderer.set_map_mode(&self.device, new_mode);
                }
                _ => {}
            },
            _ => {
                return false;
            }
        }
        true
    }

    fn update(&mut self) {
        match &mut self.flow {
            AppFlow::Loading(rx) => {
                // Check if loading is done
                if let Ok(result) = rx.try_recv() {
                    match result {
                        Ok(world_data) => {
                            println!("World Data Loaded! Initializing AppState...");

                            // Prepare renderer with real maps
                            let mut map_images = HashMap::new();
                            map_images.insert(
                                MapMode::Province,
                                image::DynamicImage::ImageRgb8(world_data.province_map.clone()),
                            );
                            map_images.insert(
                                MapMode::Political,
                                image::DynamicImage::ImageRgb8(world_data.political_map.clone()),
                            );
                            map_images.insert(
                                MapMode::TradeGoods,
                                image::DynamicImage::ImageRgb8(world_data.tradegoods_map.clone()),
                            );
                            map_images.insert(
                                MapMode::Religion,
                                image::DynamicImage::ImageRgb8(world_data.religion_map.clone()),
                            );
                            map_images.insert(
                                MapMode::Culture,
                                image::DynamicImage::ImageRgb8(world_data.culture_map.clone()),
                            );

                            self.renderer
                                .update_maps(&self.device, &self.queue, map_images);

                            let app_state =
                                AppState::new(world_data, self.size.width, self.size.height);
                            self.flow = AppFlow::Running(Box::new(app_state));
                        }
                        Err(e) => {
                            // Render Error Screen?
                            println!("FATAL ERROR LOADING DATA: {}", e);
                            // Keep spinner spinning, but maybe log it?
                        }
                    }
                }
            }
            AppFlow::Running(app_state) => {
                // Update Camera Buffer (only if running)
                self.renderer.update_camera_buffer(
                    &self.queue,
                    app_state
                        .camera
                        .to_uniform_data(self.config.width as f32, self.config.height as f32),
                );
            }
        }
    }

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

        // 1. Render Map (or Loading Screen)
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

            match &self.flow {
                AppFlow::Loading(_) => {
                    // Just clear (blue screen) or render a "Loading..." text?
                    // We will render UI overlay on top which can show "Loading..."
                    // For now, map logic assumes textures exist.
                    // Since we init renderer with dummy textures, we CAN draw.
                    self.renderer.set_map_mode(&self.device, MapMode::Province); // Ensure dummy texture
                    render_pass.set_pipeline(&self.renderer.render_pipeline);
                    render_pass.set_bind_group(0, &self.renderer.diffuse_bind_group, &[]);
                    render_pass.draw(0..3, 0..1); // Draw full screen quad
                }
                AppFlow::Running(_) => {
                    render_pass.set_pipeline(&self.renderer.render_pipeline);
                    render_pass.set_bind_group(0, &self.renderer.diffuse_bind_group, &[]);
                    render_pass.draw(0..3, 0..1);
                }
            }
        } // End Map Pass

        // 2. Render UI Overlay (Console, Sidebar, Loading Spinner)
        // We render to an RgbaImage using TextRenderer, upload to texture, then draw quad
        let ui_img = if let AppFlow::Loading(_) = self.flow {
            self.ui_state.render_loading_screen(
                &self.text_renderer,
                self.size.width,
                self.size.height,
                &self.console_log,
            )
        } else {
            self.ui_state.render(
                &self.text_renderer,
                self.size.width,
                self.size.height,
                &self.console_log,
            )
        };

        self.renderer
            .update_ui_texture(&self.device, &self.queue, &ui_img);

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("UI Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Load the map we just drew
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.renderer.ui_pipeline);
            render_pass.set_bind_group(0, &self.renderer.ui_bind_group, &[]);
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
pub fn run(log_level: log::LevelFilter, eu4_path: &std::path::Path) {
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("eu4rs - Map Viewer")
        .with_inner_size(winit::dpi::PhysicalSize::new(1920, 1080)) // Request 1080p
        .build(&event_loop)
        .unwrap();

    let eu4_path = eu4_path.to_path_buf();

    // Init State
    let mut state = pollster::block_on(State::new(&window, log_level, &eu4_path));

    let _ = event_loop.run(move |event, control_flow| {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == state.window().id() => {
                if !state.input(event) {
                    // If input didn't consume it, handle window events
                    match event {
                        WindowEvent::CloseRequested
                        | WindowEvent::KeyboardInput {
                            event:
                                winit::event::KeyEvent {
                                    state: winit::event::ElementState::Pressed,
                                    physical_key:
                                        winit::keyboard::PhysicalKey::Code(
                                            winit::keyboard::KeyCode::Escape,
                                        ),
                                    ..
                                },
                            ..
                        } => control_flow.exit(),
                        WindowEvent::Resized(physical_size) => {
                            state.resize(*physical_size);
                        }
                        WindowEvent::RedrawRequested => {
                            state.update();
                            match state.render() {
                                Ok(_) => {}
                                Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                                Err(wgpu::SurfaceError::OutOfMemory) => control_flow.exit(),
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
        }
    });
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
    log_level: log::LevelFilter,
) -> Result<(), String> {
    // We need to re-init logger if it hasn't been initialized?
    let _ = crate::logger::init(log_level);

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

    let mut map_images = HashMap::new();

    if mode == MapMode::Province {
        let p1 = eu4_path.join("map/provinces.bmp");
        let p2 = eu4_path.join("provinces.bmp");
        let province_path = if p1.exists() { p1 } else { p2 };

        println!("Loading province map from {:?}", province_path);
        let img = image::open(&province_path)
            .or_else(|_| image::open("provinces.bmp"))
            .unwrap_or_else(|_| {
                println!("Warning: Texture not found! Using fallback pink texture.");
                image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                    5632,
                    2048,
                    image::Rgba([255, 0, 255, 255]),
                ))
            });
        map_images.insert(MapMode::Province, img);
    } else {
        println!("Loading world data for Snapshot...");
        let world_data = crate::ops::load_world_data(eu4_path)?;
        let img = match mode {
            MapMode::Political => world_data.political_map.clone(),
            MapMode::TradeGoods => world_data.tradegoods_map.clone(),
            MapMode::Religion => world_data.religion_map.clone(),
            MapMode::Culture => world_data.culture_map.clone(),
            _ => world_data.province_map.clone(),
        };
        map_images.insert(mode, image::DynamicImage::ImageRgb8(img));
        // Ensure Province map exists as it is required for the default bind group in Eu4Renderer
        map_images
            .entry(MapMode::Province)
            .or_insert_with(|| image::DynamicImage::ImageRgb8(world_data.province_map));
    }

    // Verbose = true for logs if Info or below
    let verbose = log_level >= log::LevelFilter::Info;
    let mut renderer = Eu4Renderer::new(&device, &queue, format, verbose, map_images);
    renderer.set_map_mode(&device, mode);

    let size = renderer.map_textures[&mode].texture.size();
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
        let steam_path = std::path::Path::new(
            "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Europa Universalis IV",
        );

        let path = if steam_path.exists() {
            steam_path
        } else {
            std::path::Path::new(".")
        };

        // Block on the async snapshot function
        // Use current dir and Province mode for regression test
        match pollster::block_on(crate::window::snapshot(
            path,
            &output_path,
            MapMode::Province,
            log::LevelFilter::Info,
        )) {
            Ok(_) => {
                // Load the result and assert
                let img = image::open(&output_path)
                    .expect("Failed to load map snapshot output")
                    .to_rgba8();
                if steam_path.exists() {
                    testing::assert_snapshot(&img, "map_province");
                } else {
                    println!("Skipping snapshot match assertion as we are using fallback path");
                }
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
                log::LevelFilter::Info,
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
                log::LevelFilter::Info,
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
                log::LevelFilter::Info,
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
                log::LevelFilter::Info,
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
