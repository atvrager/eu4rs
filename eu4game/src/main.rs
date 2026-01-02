//! EU4 Source Port - Game Binary
//!
//! A playable EU4 experience using eu4sim-core for simulation
//! and wgpu for rendering.

mod bmfont;
mod camera;
mod dds;
mod flags;
mod gui;
mod input;
mod render;
mod screen;
mod sim_thread;
#[cfg(test)]
mod testing;
mod text;

use screen::Screen;
use sim_thread::{SimEvent, SimHandle, SimSpeed};
use std::sync::Arc;

/// Extracts GUI-displayable country resources from the simulation state.
fn extract_country_resources(
    world_state: &eu4sim_core::WorldState,
    tag: &str,
) -> Option<gui::CountryResources> {
    let country = world_state.countries.get(tag)?;

    // Calculate max manpower from owned provinces (base_manpower * 250 per dev)
    let max_manpower: i32 = world_state
        .provinces
        .values()
        .filter(|p| p.owner.as_deref() == Some(tag))
        .map(|p| (p.base_manpower.to_f32() * 250.0) as i32)
        .sum();

    // Calculate net monthly income
    let income_breakdown = &country.income;
    let net_income =
        (income_breakdown.taxation + income_breakdown.trade + income_breakdown.production
            - income_breakdown.expenses)
            .to_f32();

    Some(gui::CountryResources {
        treasury: country.treasury.to_f32(),
        income: net_income,
        manpower: country.manpower.to_f32() as i32,
        max_manpower,
        sailors: 0,     // Not yet implemented in sim
        max_sailors: 0, // Not yet implemented in sim
        stability: country.stability.get(),
        prestige: country.prestige.get().to_f32(),
        corruption: 0.0, // Not yet implemented in sim
        adm_power: country.adm_mana.to_int() as i32,
        dip_power: country.dip_mana.to_int() as i32,
        mil_power: country.mil_mana.to_int() as i32,
        merchants: 0,        // Not yet implemented in sim
        max_merchants: 0,    // Not yet implemented in sim
        colonists: 0,        // Not yet implemented in sim
        max_colonists: 0,    // Not yet implemented in sim
        diplomats: 0,        // Not yet implemented in sim
        max_diplomats: 0,    // Not yet implemented in sim
        missionaries: 0,     // Not yet implemented in sim
        max_missionaries: 0, // Not yet implemented in sim
    })
}

use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

/// Target resolution for OCR pipeline compatibility.
const TARGET_WIDTH: u32 = 1920;
const TARGET_HEIGHT: u32 = 1080;

/// Application state holding all game resources.
struct App {
    /// The winit window.
    window: winit::window::Window,
    /// wgpu surface for presenting frames.
    surface: wgpu::Surface<'static>,
    /// wgpu device for GPU operations.
    device: wgpu::Device,
    /// wgpu queue for submitting commands.
    queue: wgpu::Queue,
    /// Surface configuration.
    config: wgpu::SurfaceConfiguration,
    /// Window size.
    size: PhysicalSize<u32>,
    /// Renderer holding pipelines and textures.
    renderer: render::Renderer,
    /// Camera for pan/zoom.
    camera: camera::Camera,
    /// Current cursor position.
    cursor_pos: (f64, f64),
    /// Whether we're currently panning (middle mouse held).
    panning: bool,
    /// Last cursor position for delta calculation.
    last_cursor_pos: (f64, f64),
    /// Simulation thread handle.
    sim_handle: SimHandle,
    /// Current simulation speed.
    sim_speed: SimSpeed,
    /// Current game date (from last tick).
    current_date: eu4sim_core::state::Date,
    /// Province lookup table (color -> province ID).
    province_lookup: Option<eu4data::map::ProvinceLookup>,
    /// Province map image for pixel lookup.
    province_map: image::RgbaImage,
    /// Currently selected province.
    selected_province: Option<u32>,
    /// Screen state manager with navigation history.
    screen_manager: screen::ScreenManager,
    /// Frontend UI coordinator (manages main menu and screen transitions).
    frontend_ui: Option<gui::frontend::FrontendUI>,
    /// Player's country tag (set after selection).
    player_tag: Option<String>,
    /// List of playable countries (sorted by development).
    playable_countries: Vec<(String, String, i32)>, // (tag, name, development)
    /// Index of currently highlighted country in selection.
    country_selection_index: usize,
    /// Current input mode (Normal, MovingArmy, MovingFleet, etc.).
    input_mode: input::InputMode,
    /// Currently selected army (if any).
    selected_army: Option<u32>,
    /// Currently selected fleet (if any).
    selected_fleet: Option<u32>,
    /// Latest world state snapshot from sim thread.
    world_state: Option<Arc<eu4sim_core::WorldState>>,
    /// Province ID -> pixel center (for army markers).
    /// Will be used for instanced army rendering.
    #[allow(dead_code)]
    province_centers: std::collections::HashMap<u32, (u32, u32)>,
    /// Country tag -> RGB color (from game data).
    country_colors: std::collections::HashMap<String, [u8; 3]>,
    /// Whether the GPU lookup texture needs updating.
    lookup_dirty: bool,
    /// Flag texture cache.
    flag_cache: flags::FlagCache,
    /// Sprite renderer for UI elements.
    sprite_renderer: render::SpriteRenderer,
    /// Bind group for the player's flag (created when country is selected).
    player_flag_bind_group: Option<wgpu::BindGroup>,
    /// Masked flag bind group (flag + shield mask) for shield rendering.
    masked_flag_bind_group: Option<wgpu::BindGroup>,
    /// Shield overlay bind group (for drawing frame on top of flag).
    shield_overlay_bind_group: Option<wgpu::BindGroup>,
    /// Shield overlay dimensions (from texture).
    shield_overlay_size: (u32, u32),
    /// Shield mask dimensions (from texture).
    shield_mask_size: (u32, u32),
    /// Cached shield clip rect (position for player flag in topbar).
    shield_clip_rect: Option<(f32, f32, f32, f32)>,
    /// Text renderer for UI text.
    text_renderer: Option<text::TextRenderer>,
    /// EU4-authentic GUI renderer.
    gui_renderer: Option<gui::GuiRenderer>,
    /// Country selection left panel (Phase 8.2).
    country_select_left: Option<gui::CountrySelectLeftPanel>,
    /// Selected start date on country selection screen.
    start_date: eu4data::Eu4Date,
    /// Valid year range for start dates (min, max), derived from loaded bookmarks.
    /// Supports mod compatibility (e.g., Extended Timeline).
    year_range: (i32, i32),
}

impl App {
    /// Creates the application with wgpu initialized.
    async fn new(window: winit::window::Window) -> Self {
        let size = window.inner_size();

        // wgpu instance with Vulkan preference
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::METAL | wgpu::Backends::DX12,
            ..Default::default()
        });

        // SAFETY: window lives as long as surface (both in App struct)
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::from_window(&window).unwrap())
                .unwrap()
        };

        // Request adapter
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find a suitable GPU adapter");

        log::info!("Using adapter: {:?}", adapter.get_info().name);

        // Request device
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("eu4game device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .expect("Failed to create device");

        // Surface configuration
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Load province map, lookup table, and heightmap
        let (province_img, province_lookup, heightmap) = Self::load_province_data();
        let province_map = province_img.to_rgba8();
        let content_aspect = province_img.width() as f64 / province_img.height() as f64;

        // Compute province centers (for army markers)
        let province_centers = Self::compute_province_centers(&province_map, &province_lookup);
        log::info!("Computed {} province centers", province_centers.len());

        // Create GPU renderer with province lookup for shader-based coloring
        let lookup_map = province_lookup.as_ref().map(|l| &l.by_color);
        let renderer = render::Renderer::new(
            &device,
            &queue,
            surface_format,
            &province_map,
            lookup_map.unwrap_or(&std::collections::HashMap::new()),
            heightmap.as_ref(),
        );

        // Create camera
        let camera = camera::Camera::new(content_aspect);

        // Load world state from game files (or use default if unavailable)
        let (initial_state, playable_countries, country_colors) = Self::load_world_state();
        let initial_date = initial_state.date;
        let sim_handle = sim_thread::spawn_sim_thread(initial_state);

        // Initialize flag cache with fallback texture
        let flags_dir = eu4data::path::detect_game_path()
            .map(|p| p.join("gfx/flags"))
            .unwrap_or_default();
        let flag_cache = flags::FlagCache::with_fallback(flags_dir, &device, &queue);

        // Create sprite renderer for UI elements
        let sprite_renderer = render::SpriteRenderer::new(&device, config.format);

        // Create text renderer with EU4 font
        let text_renderer = eu4data::path::detect_game_path()
            .and_then(|game_path| {
                let font_path = game_path.join("assets/font.ttf");
                if font_path.exists() {
                    log::info!("Loading font from: {}", font_path.display());
                    std::fs::read(&font_path).ok()
                } else {
                    log::warn!("Font not found at: {}", font_path.display());
                    None
                }
            })
            .and_then(|font_data| {
                text::TextRenderer::new(&device, &queue, config.format, &font_data)
            });

        if text_renderer.is_some() {
            log::info!("Text renderer initialized");
        } else {
            log::warn!("Text rendering unavailable");
        }

        // Create EU4 GUI renderer
        let gui_renderer = eu4data::path::detect_game_path().map(|game_path| {
            log::info!("Initializing GUI renderer from: {}", game_path.display());
            gui::GuiRenderer::new(&game_path)
        });

        if gui_renderer.is_some() {
            log::info!("GUI renderer initialized");
        } else {
            log::warn!("GUI rendering unavailable (game path not found)");
        }

        // Derive valid year range from loaded bookmarks (supports mod compatibility)
        let year_range = gui_renderer
            .as_ref()
            .map(|gr| eu4data::bookmarks::get_year_range_from_bookmarks(gr.bookmarks()))
            .unwrap_or((
                eu4data::Eu4Date::VANILLA_MIN_YEAR,
                eu4data::Eu4Date::VANILLA_MAX_YEAR,
            ));
        log::info!(
            "Valid start year range: {}-{} (from {} bookmarks)",
            year_range.0,
            year_range.1,
            gui_renderer
                .as_ref()
                .map(|gr| gr.bookmarks().len())
                .unwrap_or(0)
        );

        // Create FrontendUI with panels from GuiRenderer (Phase 8.5.1)
        // Main menu remains a placeholder until Phase 8.5.4
        // Left/top/lobby panels are loaded from frontend.gui when available
        let frontend_ui = {
            use gui::binder::Bindable;
            let panel = gui::main_menu::MainMenuPanel {
                single_player: gui::primitives::GuiButton::placeholder(),
                multi_player: gui::primitives::GuiButton::placeholder(),
                tutorial: None,
                credits: None,
                settings: None,
                exit: gui::primitives::GuiButton::placeholder(),
            };

            // Phase 8.5.2: Panels are rendered by GuiRenderer, not FrontendUI
            // FrontendUI uses placeholders for now - rendering is handled separately
            // TODO: Consolidate panel ownership when interactive button handling is added
            Some(gui::frontend::FrontendUI::new(panel, None, None, None))
        };

        Self {
            window,
            surface,
            device,
            queue,
            config,
            size,
            renderer,
            camera,
            cursor_pos: (0.0, 0.0),
            panning: false,
            last_cursor_pos: (0.0, 0.0),
            sim_handle,
            sim_speed: SimSpeed::Paused,
            current_date: initial_date,
            province_lookup,
            province_map,
            selected_province: None,
            screen_manager: {
                // Phase 8.1: Start at main menu
                screen::ScreenManager::new() // Starts at MainMenu by default
            },
            player_tag: None,
            playable_countries,
            country_selection_index: 0,
            input_mode: input::InputMode::Normal,
            selected_army: None,
            selected_fleet: None,
            world_state: None,
            province_centers,
            country_colors,
            lookup_dirty: true, // Update lookup on first tick
            flag_cache,
            sprite_renderer,
            player_flag_bind_group: None,
            masked_flag_bind_group: None,
            shield_overlay_bind_group: None,
            shield_overlay_size: (1, 1),
            shield_mask_size: (1, 1),
            shield_clip_rect: None,
            text_renderer,
            gui_renderer,
            frontend_ui,
            country_select_left: None,
            start_date: eu4data::Eu4Date::from_ymd(1444, 11, 11), // Default EU4 start
            year_range,
        }
    }

    /// Computes the center point of each province for marker placement.
    fn compute_province_centers(
        province_map: &image::RgbaImage,
        province_lookup: &Option<eu4data::map::ProvinceLookup>,
    ) -> std::collections::HashMap<u32, (u32, u32)> {
        let Some(lookup) = province_lookup else {
            return std::collections::HashMap::new();
        };

        // Accumulate pixel positions for each province
        let mut sums: std::collections::HashMap<u32, (u64, u64, u64)> =
            std::collections::HashMap::new();

        for (x, y, pixel) in province_map.enumerate_pixels() {
            let color = (pixel[0], pixel[1], pixel[2]);
            if let Some(&province_id) = lookup.by_color.get(&color) {
                let entry = sums.entry(province_id).or_insert((0, 0, 0));
                entry.0 += x as u64;
                entry.1 += y as u64;
                entry.2 += 1;
            }
        }

        // Calculate centers
        sums.into_iter()
            .filter_map(|(id, (sum_x, sum_y, count))| {
                if count > 0 {
                    Some((id, ((sum_x / count) as u32, (sum_y / count) as u32)))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Loads the world state from game files.
    /// Returns (world_state, playable_countries, country_colors).
    #[allow(clippy::type_complexity)]
    fn load_world_state() -> (
        eu4sim_core::WorldState,
        Vec<(String, String, i32)>,
        std::collections::HashMap<String, [u8; 3]>,
    ) {
        use eu4sim_core::state::Date;

        // Try to load from EU4 game path
        if let Some(game_path) = eu4data::path::detect_game_path() {
            log::info!("Loading world state from: {}", game_path.display());
            let start_date = Date::new(1444, 11, 11);

            // Load country colors from game data
            let country_colors: std::collections::HashMap<String, [u8; 3]> =
                match eu4data::countries::load_tags(&game_path) {
                    Ok(tags) => {
                        let colors: std::collections::HashMap<String, [u8; 3]> =
                            eu4data::countries::load_country_map(&game_path, &tags)
                                .into_iter()
                                .filter_map(|(tag, country)| {
                                    if country.color.len() >= 3 {
                                        Some((
                                            tag,
                                            [country.color[0], country.color[1], country.color[2]],
                                        ))
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                        log::info!("Loaded {} country colors", colors.len());
                        colors
                    }
                    Err(e) => {
                        log::warn!("Failed to load country colors: {}", e);
                        std::collections::HashMap::new()
                    }
                };

            match eu4sim::loader::load_initial_state(&game_path, start_date, 42) {
                Ok((world, _adjacency)) => {
                    log::info!(
                        "Loaded world: {} provinces, {} countries",
                        world.provinces.len(),
                        world.countries.len()
                    );

                    // Calculate development for each country by summing owned province development
                    let mut country_dev: std::collections::HashMap<String, i32> =
                        std::collections::HashMap::new();
                    for (_, prov) in &world.provinces {
                        if let Some(ref owner) = prov.owner {
                            let dev = (prov.base_tax + prov.base_production + prov.base_manpower)
                                .to_f32() as i32;
                            *country_dev.entry(owner.clone()).or_insert(0) += dev;
                        }
                    }

                    // Build list of playable countries (only those with provinces)
                    let mut playable: Vec<(String, String, i32)> = country_dev
                        .iter()
                        .filter(|(_, dev)| **dev > 0) // Only countries with positive development
                        .map(|(tag, dev)| {
                            let dev = *dev;
                            // Use tag as name for now (country definitions may have proper names)
                            (tag.clone(), tag.clone(), dev)
                        })
                        .collect();

                    // Sort by development (descending)
                    playable.sort_by(|a, b| b.2.cmp(&a.2));

                    log::info!("Found {} playable countries", playable.len());
                    if !playable.is_empty() {
                        log::info!("Top 5: {:?}", playable.iter().take(5).collect::<Vec<_>>());
                    }

                    return (world, playable, country_colors);
                }
                Err(e) => {
                    log::warn!("Failed to load world state: {}", e);
                }
            }
        }

        // Fallback: empty world
        log::warn!("Using empty world state");
        (
            eu4sim_core::WorldState::default(),
            Vec::new(),
            std::collections::HashMap::new(),
        )
    }

    /// Returns a reference to the window.
    fn window(&self) -> &winit::window::Window {
        &self.window
    }

    /// Loads province map, lookup table, and heightmap from EU4 game files.
    fn load_province_data() -> (
        image::DynamicImage,
        Option<eu4data::map::ProvinceLookup>,
        Option<image::GrayImage>,
    ) {
        // Try to load from EU4 game path
        if let Some(game_path) = eu4data::path::detect_game_path() {
            let provinces_path = game_path.join("map/provinces.bmp");
            let definitions_path = game_path.join("map/definition.csv");
            let heightmap_path = game_path.join("map/heightmap.bmp");

            if provinces_path.exists() {
                log::info!("Loading province map from: {}", provinces_path.display());
                if let Ok(img) = image::open(&provinces_path) {
                    // Try to load province definitions
                    let lookup = if definitions_path.exists() {
                        match eu4data::map::ProvinceLookup::load(&definitions_path) {
                            Ok(lookup) => {
                                log::info!("Loaded {} province definitions", lookup.by_id.len());
                                Some(lookup)
                            }
                            Err(e) => {
                                log::warn!("Failed to load province definitions: {}", e);
                                None
                            }
                        }
                    } else {
                        log::warn!(
                            "Province definitions not found at: {}",
                            definitions_path.display()
                        );
                        None
                    };

                    // Try to load heightmap for terrain shading
                    let heightmap = if heightmap_path.exists() {
                        log::info!("Loading heightmap from: {}", heightmap_path.display());
                        match image::open(&heightmap_path) {
                            Ok(hm) => {
                                let gray = hm.to_luma8();
                                log::info!("Loaded heightmap ({}x{})", gray.width(), gray.height());
                                Some(gray)
                            }
                            Err(e) => {
                                log::warn!("Failed to load heightmap: {}", e);
                                None
                            }
                        }
                    } else {
                        log::warn!("Heightmap not found at: {}", heightmap_path.display());
                        None
                    };

                    return (img, lookup, heightmap);
                }
            }
        }

        // Fallback: generate a simple test pattern
        log::warn!("Could not load provinces.bmp, using test pattern");
        let mut img = image::RgbaImage::new(5632, 2048);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            let r = ((x * 7) % 256) as u8;
            let g = ((y * 11) % 256) as u8;
            let b = ((x + y) % 256) as u8;
            *pixel = image::Rgba([r, g, b, 255]);
        }
        (image::DynamicImage::ImageRgba8(img), None, None)
    }

    /// Handles window resize.
    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            log::debug!("Resized to {}x{}", new_size.width, new_size.height);

            // Recalculate shield clip rect for new screen size
            if let Some(gui_renderer) = &self.gui_renderer {
                let screen_size = (new_size.width, new_size.height);
                self.shield_clip_rect =
                    gui_renderer.get_player_shield_clip_rect(screen_size, self.shield_overlay_size);
            }
        }
    }

    /// Renders a frame.
    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Update camera uniform
        let camera_uniform = self
            .camera
            .to_uniform(self.config.width as f32, self.config.height as f32);
        self.renderer.update_camera(&self.queue, camera_uniform);

        // Create command encoder
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Render pass
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Reset sprite slot counter for this frame
            self.sprite_renderer.begin_frame();

            let screen_size = (self.config.width, self.config.height);
            let current_screen = self.screen_manager.current();

            // Phase 8.1: Render different content based on current screen
            log::debug!("=== RENDERING PHASE: {:?} ===", current_screen);
            match current_screen {
                screen::Screen::MainMenu => {
                    // Main menu: Just render the menu UI, no map
                    log::debug!("MainMenu: Skipping all map/sprite rendering");

                    // DIAGNOSTIC: Draw a bright green test rectangle to verify rendering works
                    // If you see a green box at (500,100), the pipeline works and something
                    // else is drawing black over our text
                    log::debug!("Drawing diagnostic green rectangle");
                    // TODO: Actually draw a test rectangle here if needed

                    // TODO: Render main menu panel
                }
                screen::Screen::SinglePlayer | screen::Screen::Playing => {
                    // Game screens: Render map and game elements
                    // Draw map (big triangle)
                    render_pass.set_pipeline(&self.renderer.pipeline);
                    render_pass.set_bind_group(0, &self.renderer.bind_group, &[]);
                    render_pass.draw(0..3, 0..1);

                    // Draw army markers (instanced squares)
                    if self.renderer.army_count > 0 {
                        render_pass.set_pipeline(&self.renderer.army_pipeline);
                        render_pass.set_bind_group(0, &self.renderer.army_bind_group, &[]);
                        render_pass
                            .set_vertex_buffer(0, self.renderer.army_instance_buffer.slice(..));
                        // 6 vertices per square, army_count instances
                        render_pass.draw(0..6, 0..self.renderer.army_count);
                    }

                    // Draw fleet markers (instanced diamonds)
                    if self.renderer.fleet_count > 0 {
                        render_pass.set_pipeline(&self.renderer.fleet_pipeline);
                        render_pass.set_bind_group(0, &self.renderer.fleet_bind_group, &[]);
                        render_pass
                            .set_vertex_buffer(0, self.renderer.fleet_instance_buffer.slice(..));
                        // 6 vertices per diamond, fleet_count instances
                        render_pass.draw(0..6, 0..self.renderer.fleet_count);
                    }
                }
                _ => {
                    // Other screens: no rendering yet
                }
            }

            // Prepare GUI state
            let date_str = format!(
                "{} {} {}",
                self.current_date.day,
                match self.current_date.month {
                    1 => "January",
                    2 => "February",
                    3 => "March",
                    4 => "April",
                    5 => "May",
                    6 => "June",
                    7 => "July",
                    8 => "August",
                    9 => "September",
                    10 => "October",
                    11 => "November",
                    _ => "December",
                },
                self.current_date.year
            );

            // Extract country resources from simulation state if we have a player
            let country_resources = self.player_tag.as_ref().and_then(|tag| {
                self.world_state
                    .as_ref()
                    .and_then(|ws| extract_country_resources(ws, tag))
            });

            let gui_state = gui::GuiState {
                date: date_str.clone(),
                speed: match self.sim_speed {
                    SimSpeed::Paused => 0,
                    SimSpeed::Speed1 => 1,
                    SimSpeed::Speed2 => 2,
                    SimSpeed::Speed3 => 3,
                    SimSpeed::Speed4 => 4,
                    SimSpeed::Speed5 => 5,
                },
                paused: self.sim_speed == SimSpeed::Paused,
                country: country_resources,
            };

            // Render screen-specific GUI
            log::debug!("Current screen for rendering: {:?}", current_screen);
            match current_screen {
                screen::Screen::MainMenu => {
                    // Phase 8.1: Main menu rendering
                    // Show simple text instructions
                    log::debug!("Rendering MainMenu screen");
                    if let Some(text_renderer) = &self.text_renderer {
                        log::debug!(
                            "Rendering main menu text (screen size: {}x{})",
                            screen_size.0,
                            screen_size.1
                        );
                        // Phase 8.1: Main menu text
                        let white = [1.0, 1.0, 1.0, 1.0];
                        let screen_f32 = (screen_size.0 as f32, screen_size.1 as f32);

                        // Collect ALL quads first, then draw once (avoids buffer sync issues)
                        let mut all_quads = Vec::new();
                        all_quads.extend(text_renderer.layout_text(
                            "EUROPA UNIVERSALIS IV",
                            400.0,
                            200.0,
                            white,
                            screen_f32,
                        ));
                        all_quads.extend(text_renderer.layout_text(
                            "Main Menu",
                            400.0,
                            250.0,
                            white,
                            screen_f32,
                        ));
                        all_quads.extend(text_renderer.layout_text(
                            "Press 'S' for Single Player",
                            400.0,
                            350.0,
                            white,
                            screen_f32,
                        ));
                        all_quads.extend(text_renderer.layout_text(
                            "Press ESC to Exit",
                            400.0,
                            400.0,
                            white,
                            screen_f32,
                        ));

                        text_renderer.draw(&mut render_pass, &self.queue, &all_quads);
                    } else {
                        log::warn!("text_renderer is None - cannot render main menu text");
                    }
                }
                screen::Screen::SinglePlayer | screen::Screen::Playing => {
                    log::debug!("Rendering {:?} screen", current_screen);

                    // Create country state before mut borrow (Phase 9.4)
                    let country_state = if current_screen == screen::Screen::SinglePlayer {
                        self.create_selected_country_state()
                    } else {
                        None
                    };

                    // Render EU4 GUI overlay (topbar, speed controls, country select)
                    if let Some(gui_renderer) = &mut self.gui_renderer {
                        log::debug!("Calling gui_renderer.render()");
                        // Pass start_date for SinglePlayer screen (date widget display)
                        let start_date = if current_screen == screen::Screen::SinglePlayer {
                            Some(&self.start_date)
                        } else {
                            None
                        };
                        // Enable/disable play button based on country selection (Phase 9.3)
                        gui_renderer.set_play_button_enabled(self.player_tag.is_some());

                        // Update country selection right panel (Phase 9.4)
                        gui_renderer.update_selected_country(country_state.as_ref());

                        gui_renderer.render(
                            &mut render_pass,
                            &self.device,
                            &self.queue,
                            &self.sprite_renderer,
                            &gui_state,
                            current_screen,
                            screen_size,
                            start_date,
                        );
                    } else {
                        log::warn!("gui_renderer is None");
                    }
                }
                _ => {}
            }

            // Draw player flag with shield mask and overlay (only on game screens)
            // (uses App-owned bind groups to avoid borrow issues with gui_renderer)
            if current_screen == screen::Screen::Playing
                && let Some(overlay_rect) = self.shield_clip_rect
            {
                // Compute flag rect scaled and centered to match mask within overlay
                let flag_rect = gui::compute_masked_flag_rect(
                    overlay_rect,
                    self.shield_mask_size,
                    self.shield_overlay_size,
                );

                // Draw masked flag (flag clipped to shield shape)
                if let Some(ref masked_bind_group) = self.masked_flag_bind_group {
                    self.sprite_renderer.draw_masked_flag(
                        &mut render_pass,
                        masked_bind_group,
                        &self.queue,
                        flag_rect.0,
                        flag_rect.1,
                        flag_rect.2,
                        flag_rect.3,
                    );
                }

                // Draw shield overlay on top (decorative frame) at full size
                if let Some(ref overlay_bind_group) = self.shield_overlay_bind_group {
                    self.sprite_renderer.draw(
                        &mut render_pass,
                        overlay_bind_group,
                        &self.queue,
                        overlay_rect.0,
                        overlay_rect.1,
                        overlay_rect.2,
                        overlay_rect.3,
                    );
                }
            }

            // Draw input mode indicator (left side, below flag area)
            if self.screen_manager.current() == screen::Screen::Playing
                && let Some(ref text_renderer) = self.text_renderer
            {
                let mode_str = self.input_mode.description();
                if mode_str != "Normal" {
                    text_renderer.draw_text(
                        &mut render_pass,
                        &self.queue,
                        mode_str,
                        20.0,
                        screen_size.1 as f32 - 40.0, // Bottom left
                        [1.0, 1.0, 0.0, 1.0],        // Yellow for mode indicator
                        (screen_size.0 as f32, screen_size.1 as f32),
                    );
                }
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    /// Handles input events. Returns (consumed, should_exit).
    fn input(&mut self, event: &WindowEvent) -> (bool, bool) {
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key),
                        state,
                        ..
                    },
                ..
            } => {
                let should_exit = self.handle_key(*key, *state);
                (true, should_exit)
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.handle_scroll(*delta);
                self.window.request_redraw();
                (true, false)
            }
            WindowEvent::MouseInput { button, state, .. } => {
                self.handle_mouse_button(*button, *state);
                (true, false)
            }
            WindowEvent::CursorMoved { position, .. } => {
                let needs_redraw = self.handle_cursor_move(position.x, position.y);
                if needs_redraw {
                    self.window.request_redraw();
                }
                (true, false)
            }
            _ => (false, false),
        }
    }

    /// Handles keyboard input. Returns true if should exit.
    fn handle_key(&mut self, key: KeyCode, state: ElementState) -> bool {
        if state != ElementState::Pressed {
            return false;
        }

        // Phase 8.1: Handle main menu input
        if self.screen_manager.current() == screen::Screen::MainMenu {
            match key {
                KeyCode::KeyS => {
                    // Single Player - transition to country selection
                    log::info!("Starting Single Player mode");
                    self.screen_manager
                        .transition_to(screen::Screen::SinglePlayer);

                    self.update_window_title();
                    return false;
                }
                KeyCode::Escape => {
                    // Exit from main menu
                    log::info!("Exiting from main menu");
                    self.sim_handle.shutdown();
                    return true;
                }
                _ => return false,
            }
        }

        // Handle Escape key for back navigation (Phase 6.1.4)
        if key == KeyCode::Escape {
            // On SinglePlayer screen with country selected: deselect country
            if self.screen_manager.current() == screen::Screen::SinglePlayer
                && self.player_tag.is_some()
            {
                log::info!("Escape pressed - deselecting country");
                self.player_tag = None;
                if let Some(gui_renderer) = &mut self.gui_renderer {
                    gui_renderer.update_selected_country(None);
                    gui_renderer.set_play_button_enabled(false);
                }
                return false;
            }

            // Try to go back in navigation history
            if self.screen_manager.can_go_back() {
                log::info!("Escape pressed - going back");
                self.screen_manager.go_back();
                self.update_window_title();
                return false;
            }

            // No history - exit the game
            log::info!("Escape pressed with no history - exiting");
            self.sim_handle.shutdown();
            return true;
        }

        // Handle country selection mode (Single Player screen)
        if self.screen_manager.current() == screen::Screen::SinglePlayer {
            match key {
                KeyCode::ArrowUp => {
                    if self.country_selection_index > 0 {
                        self.country_selection_index -= 1;
                        self.log_country_selection();
                    }
                }
                KeyCode::ArrowDown => {
                    if self.country_selection_index + 1 < self.playable_countries.len() {
                        self.country_selection_index += 1;
                        self.log_country_selection();
                    }
                }
                KeyCode::PageUp => {
                    self.country_selection_index = self.country_selection_index.saturating_sub(10);
                    self.log_country_selection();
                }
                KeyCode::PageDown => {
                    self.country_selection_index = (self.country_selection_index + 10)
                        .min(self.playable_countries.len().saturating_sub(1));
                    self.log_country_selection();
                }
                KeyCode::Home => {
                    self.country_selection_index = 0;
                    self.log_country_selection();
                }
                KeyCode::End => {
                    self.country_selection_index = self.playable_countries.len().saturating_sub(1);
                    self.log_country_selection();
                }
                KeyCode::Enter => {
                    self.start_game_with_selected_country();
                }
                _ => {}
            }
            return false;
        }

        // Normal game mode
        match key {
            KeyCode::Escape => {
                // Cancel current mode or exit
                if self.input_mode.is_cancellable() {
                    log::info!("Cancelled {}", self.input_mode.description());
                    self.input_mode = input::InputMode::Normal;
                    self.update_window_title();
                } else {
                    log::info!("Escape pressed, exiting");
                    self.sim_handle.shutdown();
                    return true;
                }
            }
            KeyCode::Space => {
                self.sim_handle.toggle_pause();
                log::info!("Toggled pause");
            }
            KeyCode::Digit1 => self.set_speed(SimSpeed::Speed1),
            KeyCode::Digit2 => self.set_speed(SimSpeed::Speed2),
            KeyCode::Digit3 => self.set_speed(SimSpeed::Speed3),
            KeyCode::Digit4 => self.set_speed(SimSpeed::Speed4),
            KeyCode::Digit5 => self.set_speed(SimSpeed::Speed5),
            KeyCode::KeyM => {
                // Enter move army mode if we have a selected army
                if let Some(army_id) = self.selected_army {
                    self.input_mode = input::InputMode::MovingArmy { army_id };
                    log::info!("Move mode: click destination for army {}", army_id);
                    self.update_window_title();
                } else {
                    log::info!("No army selected - click on a province with your army first");
                }
            }
            KeyCode::KeyF => {
                // Enter move fleet mode if we have a selected fleet
                if let Some(fleet_id) = self.selected_fleet {
                    self.input_mode = input::InputMode::MovingFleet { fleet_id };
                    log::info!(
                        "Fleet move mode: click sea zone destination for fleet {}",
                        fleet_id
                    );
                    self.update_window_title();
                } else {
                    log::info!("No fleet selected - click on a sea zone with your fleet first");
                }
            }
            KeyCode::KeyW => {
                // Enter declare war mode
                self.input_mode = input::InputMode::DeclaringWar;
                log::info!("Declare War mode: click on enemy province to declare war");
                self.update_window_title();
            }
            KeyCode::KeyR => {
                // Force refresh political map
                self.lookup_dirty = true;
                log::info!("Refreshing map lookup texture");
            }
            _ => {}
        }
        false
    }

    /// Sets the simulation speed.
    fn set_speed(&mut self, speed: SimSpeed) {
        self.sim_handle.set_speed(speed);
        log::info!("Set speed to {:?}", speed);
    }

    /// Logs the current country selection to console.
    fn log_country_selection(&self) {
        if let Some((tag, _name, dev)) = self.playable_countries.get(self.country_selection_index) {
            log::info!(
                "SELECT COUNTRY [{}/{}]: {} (Dev: {}) - Up/Down/PgUp/PgDn to browse, Enter to select",
                self.country_selection_index + 1,
                self.playable_countries.len(),
                tag,
                dev
            );
        }
    }

    /// Polls and processes events from the simulation thread.
    fn poll_sim_events(&mut self) {
        for event in self.sim_handle.poll_events() {
            match event {
                SimEvent::Tick { state, tick } => {
                    self.current_date = state.date;
                    self.world_state = Some(state);
                    // Update lookup on every tick (GPU rendering is fast - only 32KB texture)
                    // This ensures army markers move when armies move
                    self.lookup_dirty = true;
                    log::debug!("Tick {} - Date: {}", tick, self.current_date);
                }
                SimEvent::SpeedChanged(speed) => {
                    self.sim_speed = speed;
                    self.update_window_title();
                }
                SimEvent::Shutdown => {
                    log::info!("Sim thread shutdown acknowledged");
                }
            }
        }
    }

    /// Poll frontend UI for button actions.
    /// Returns true if the game should exit.
    fn poll_frontend_ui(&mut self) -> bool {
        let Some(ref mut frontend_ui) = self.frontend_ui else {
            return false;
        };

        // Poll for button click actions from main menu
        if let Some(action) = frontend_ui.poll_main_menu() {
            return self.handle_ui_action(action);
        }

        // Poll for actions from country select left panel (Phase 8.2)
        if let Some(ref mut left_panel) = self.country_select_left
            && let Some(action) = left_panel.poll_actions()
        {
            return self.handle_ui_action(action);
        }

        false
    }

    /// Handle a UI action from any panel.
    /// Returns true if the game should exit.
    fn handle_ui_action(&mut self, action: gui::core::UiAction) -> bool {
        use gui::core::UiAction;

        // Handle the action - App is the single source of truth for screen state
        match action {
            UiAction::ShowSinglePlayer => {
                self.screen_manager
                    .transition_to(screen::Screen::SinglePlayer);
                self.update_window_title();
                false
            }
            UiAction::ShowMultiplayer => {
                self.screen_manager
                    .transition_to(screen::Screen::Multiplayer);
                self.update_window_title();
                false
            }
            UiAction::Exit => true,
            UiAction::Back => {
                if self.screen_manager.can_go_back() {
                    self.screen_manager.go_back();
                    self.update_window_title();
                }
                false
            }
            UiAction::StartGame => {
                self.screen_manager.transition_to(screen::Screen::Playing);
                self.screen_manager.clear_history();
                self.update_window_title();
                false
            }
            UiAction::ShowTutorial | UiAction::ShowCredits | UiAction::ShowSettings => {
                // Not implemented yet
                false
            }
            UiAction::DateAdjust(part, delta) => {
                use gui::core::DatePart;
                match part {
                    DatePart::Year => {
                        self.start_date
                            .adjust_year(delta, self.year_range.0, self.year_range.1);
                    }
                    DatePart::Month => {
                        self.start_date.adjust_month(delta);
                        // Month adjustment can wrap the year, so clamp it back to range
                        let year = self.start_date.year();
                        self.start_date
                            .set_year(year, self.year_range.0, self.year_range.1);
                    }
                    DatePart::Day => {
                        self.start_date.adjust_day(delta);
                        // Day adjustment can wrap the year, so clamp it back to range
                        let year = self.start_date.year();
                        self.start_date
                            .set_year(year, self.year_range.0, self.year_range.1);
                    }
                }
                log::info!(
                    "Date adjusted to: {}.{}.{} (range: {}-{})",
                    self.start_date.year(),
                    self.start_date.month(),
                    self.start_date.day(),
                    self.year_range.0,
                    self.year_range.1
                );
                self.lookup_dirty = true;
                false
            }
            UiAction::SelectBookmark(idx) => {
                log::info!("Select bookmark: {}", idx);
                // Update start date from selected bookmark
                if let Some(gui_renderer) = &self.gui_renderer
                    && let Some(bookmark) = gui_renderer.bookmarks().get(idx)
                {
                    self.start_date = bookmark.date;
                    log::info!(
                        "Date set to bookmark '{}': {}.{}.{}",
                        bookmark.name,
                        self.start_date.year(),
                        self.start_date.month(),
                        self.start_date.day()
                    );
                    self.lookup_dirty = true;
                }
                false
            }
            UiAction::SelectSaveGame(idx) => {
                log::info!("Select save game: {}", idx);
                // TODO: Implement save game selection when we have saves list
                false
            }
            UiAction::SetMapMode(mode) => {
                log::info!("Set map mode: {:?}", mode);
                // TODO: Implement map mode switching when rendering supports multiple modes
                false
            }
            UiAction::RandomCountry => {
                // Select a random country from playable countries
                if !self.playable_countries.is_empty() {
                    use rand::Rng;
                    let idx = rand::thread_rng().gen_range(0..self.playable_countries.len());
                    self.country_selection_index = idx;
                    self.log_country_selection();
                }
                false
            }
            UiAction::OpenNationDesigner => {
                log::info!("Open nation designer - not yet implemented");
                // TODO: Implement nation designer screen (Phase 10+)
                false
            }
            UiAction::ToggleRandomNewWorld => {
                log::info!("Toggle Random New World - not yet implemented");
                // TODO: Implement RNW toggle when game setup state is added
                false
            }
            UiAction::ToggleObserveMode => {
                log::info!("Toggle Observe Mode - not yet implemented");
                // TODO: Implement observer mode toggle when game setup state is added
                false
            }
            UiAction::ToggleCustomNation => {
                log::info!("Toggle Custom Nation - not yet implemented");
                // TODO: Implement custom nation toggle when game setup state is added
                false
            }
            UiAction::None => false,
        }
    }

    /// Updates the GPU lookup texture and army markers if needed.
    /// This is fast - only updates a 32KB texture and army instance buffer.
    fn update_lookup(&mut self) {
        if !self.lookup_dirty {
            return;
        }
        self.lookup_dirty = false;

        let Some(world_state) = &self.world_state else {
            return;
        };

        // Build province owners map and sea provinces set
        let mut province_owners = std::collections::HashMap::new();
        let mut sea_provinces = std::collections::HashSet::new();
        let mut max_province_id: u32 = 0;

        for (&province_id, province) in &world_state.provinces {
            max_province_id = max_province_id.max(province_id);
            if province.is_sea {
                sea_provinces.insert(province_id);
            }
            if let Some(ref owner) = province.owner {
                province_owners.insert(province_id, owner.clone());
            }
        }

        // Update GPU lookup texture (fast - only 32KB!)
        let data = render::LookupUpdateData {
            province_owners: &province_owners,
            sea_provinces: &sea_provinces,
            country_colors: &self.country_colors,
            max_province_id,
        };

        self.renderer.update_lookup(&self.queue, &data);

        // Update army markers for player
        if let Some(ref player_tag) = self.player_tag {
            let map_width = self.renderer.map_size.0 as f32;
            let map_height = self.renderer.map_size.1 as f32;

            // Count player armies for debugging
            let player_armies: Vec<_> = world_state
                .armies
                .values()
                .filter(|army| &army.owner == player_tag)
                .collect();

            let instances: Vec<render::ArmyInstance> = player_armies
                .iter()
                .filter_map(|army| {
                    // Get province center in pixel coordinates
                    let center = self.province_centers.get(&army.location);
                    if center.is_none() {
                        log::warn!(
                            "Army '{}' in province {} has no center point",
                            army.name,
                            army.location
                        );
                        return None;
                    }
                    let (px, py) = center.unwrap();
                    // Convert to UV space (0..1)
                    let u = *px as f32 / map_width;
                    let v = *py as f32 / map_height;
                    Some(render::ArmyInstance {
                        world_pos: [u, v],
                        color: [1.0, 0.0, 0.0, 1.0], // Red for player armies
                    })
                })
                .collect();

            if instances.len() != player_armies.len() {
                log::warn!(
                    "Player {} has {} armies but only {} have province centers",
                    player_tag,
                    player_armies.len(),
                    instances.len()
                );
            }

            let count = self.renderer.update_armies(&self.queue, &instances);
            log::trace!(
                "Updated {} army markers for {} ({} total player armies)",
                count,
                player_tag,
                player_armies.len()
            );

            // Update fleet markers for player (diamond shape, blue color)
            let player_fleets: Vec<_> = world_state
                .fleets
                .values()
                .filter(|fleet| &fleet.owner == player_tag)
                .collect();

            let fleet_instances: Vec<render::FleetInstance> = player_fleets
                .iter()
                .filter_map(|fleet| {
                    // Get sea zone center in pixel coordinates
                    let center = self.province_centers.get(&fleet.location);
                    if center.is_none() {
                        log::warn!(
                            "Fleet '{}' in sea zone {} has no center point",
                            fleet.name,
                            fleet.location
                        );
                        return None;
                    }
                    let (px, py) = center.unwrap();
                    // Convert to UV space (0..1)
                    let u = *px as f32 / map_width;
                    let v = *py as f32 / map_height;
                    Some(render::FleetInstance {
                        world_pos: [u, v],
                        color: [0.0, 0.5, 1.0, 1.0], // Blue for player fleets
                    })
                })
                .collect();

            let fleet_count = self.renderer.update_fleets(&self.queue, &fleet_instances);
            log::trace!(
                "Updated {} fleet markers for {} ({} total player fleets)",
                fleet_count,
                player_tag,
                player_fleets.len()
            );
        }

        log::debug!("Updated GPU lookup texture ({} provinces)", max_province_id);
    }

    /// Updates the window title with current date and speed.
    fn update_window_title(&self) {
        let title = match self.screen_manager.current() {
            screen::Screen::MainMenu => "EU4 Source Port - Main Menu".to_string(),
            screen::Screen::Multiplayer => "EU4 Source Port - Multiplayer Setup".to_string(),
            screen::Screen::SinglePlayer => {
                if let Some((tag, name, dev)) =
                    self.playable_countries.get(self.country_selection_index)
                {
                    format!(
                        "EU4 Source Port - SELECT COUNTRY ({}/{}) - {} ({}) [Dev: {}] - Up/Down to browse, Enter to select",
                        self.country_selection_index + 1,
                        self.playable_countries.len(),
                        name,
                        tag,
                        dev
                    )
                } else {
                    "EU4 Source Port - No countries available".to_string()
                }
            }
            screen::Screen::Playing => {
                let province_info = if let Some(id) = self.selected_province {
                    if let Some(lookup) = &self.province_lookup {
                        if let Some(def) = lookup.by_id.get(&id) {
                            format!(" - {} ({})", def.name, id)
                        } else {
                            format!(" - Province {}", id)
                        }
                    } else {
                        format!(" - Province {}", id)
                    }
                } else {
                    String::new()
                };
                let player_info = if let Some(ref tag) = self.player_tag {
                    format!(" [{}]", tag)
                } else {
                    String::new()
                };
                let mode_info = if self.input_mode != input::InputMode::Normal {
                    format!(" | {}", self.input_mode.description())
                } else {
                    String::new()
                };
                format!(
                    "EU4 Source Port - {} - {}{}{}{}",
                    self.current_date,
                    self.sim_speed.name(),
                    player_info,
                    province_info,
                    mode_info
                )
            }
        };
        self.window.set_title(&title);
    }

    /// Create SelectedCountryState for the current player_tag (Phase 9.4).
    ///
    /// Returns None if no country selected or world state unavailable.
    fn create_selected_country_state(&self) -> Option<gui::country_select::SelectedCountryState> {
        let player_tag = self.player_tag.as_ref()?;
        let world_state = self.world_state.as_ref()?;

        // Get country data from world state
        let country = world_state.countries.get(player_tag)?;

        // Count provinces owned by this country
        let province_count = world_state
            .provinces
            .values()
            .filter(|p| p.owner.as_deref() == Some(player_tag))
            .count();

        // Calculate total development (Fixed -> f32 -> i32)
        let total_development: i32 = world_state
            .provinces
            .values()
            .filter(|p| p.owner.as_deref() == Some(player_tag))
            .map(|p| (p.base_tax + p.base_production + p.base_manpower).to_f32())
            .sum::<f32>() as i32;

        Some(gui::country_select::SelectedCountryState {
            tag: player_tag.clone(),
            name: player_tag.clone(), // TODO: localize country name
            government_type: "Feudal Monarchy".to_string(), // TODO: get from country data
            fog_status: String::new(), // Always visible for selected country
            government_rank: 2,       // TODO: get from country data (1=Duchy, 2=Kingdom, 3=Empire)
            religion_frame: 0,        // TODO: get from religion data
            tech_group_frame: 0,      // TODO: get from tech group data
            ruler_name: format!("{} (placeholder)", player_tag), // TODO: get actual ruler
            ruler_adm: country.ruler_adm,
            ruler_dip: country.ruler_dip,
            ruler_mil: country.ruler_mil,
            adm_tech: country.adm_tech,
            dip_tech: country.dip_tech,
            mil_tech: country.mil_tech,
            ideas_name: format!("{} Ideas", player_tag), // TODO: get actual idea group
            ideas_unlocked: 0,                           // TODO: count unlocked ideas
            province_count: province_count as u32,
            total_development,
            fort_level: 0, // TODO: calculate max fort level
            diplomacy_header: "Diplomacy".to_string(),
        })
    }

    /// Selects the province at the given world coordinates.
    /// Select a country by clicking on the map (SinglePlayer mode).
    ///
    /// Looks up the province at the clicked position and selects the country
    /// that owns it. Clicking ocean/wasteland or the same country deselects.
    fn select_country_at(&mut self, world_x: f64, world_y: f64) {
        // Wrap X to 0..1 range
        let world_x = world_x.rem_euclid(1.0);

        // Check bounds for Y
        if !(0.0..=1.0).contains(&world_y) {
            log::debug!("Click outside map bounds: y={:.4}", world_y);
            return;
        }

        // Convert to pixel coordinates
        let map_width = self.province_map.width();
        let map_height = self.province_map.height();
        let pixel_x = (world_x * map_width as f64) as u32;
        let pixel_y = (world_y * map_height as f64) as u32;

        // Clamp to valid range
        let pixel_x = pixel_x.min(map_width - 1);
        let pixel_y = pixel_y.min(map_height - 1);

        // Sample pixel color
        let pixel = self.province_map.get_pixel(pixel_x, pixel_y);
        let color = (pixel[0], pixel[1], pixel[2]);

        // Look up province ID
        let province_id = if let Some(lookup) = &self.province_lookup {
            lookup.by_color.get(&color).copied()
        } else {
            None
        };

        let Some(province_id) = province_id else {
            log::debug!(
                "No province at pixel ({}, {}) - color {:?}. Deselecting country.",
                pixel_x,
                pixel_y,
                color
            );
            self.player_tag = None;
            self.lookup_dirty = true;
            return;
        };

        // Get province owner from world state
        let province_owner = self
            .world_state
            .as_ref()
            .and_then(|ws| ws.provinces.get(&province_id).and_then(|p| p.owner.clone()));

        let Some(owner_tag) = province_owner else {
            log::debug!(
                "Province {} has no owner (ocean/wasteland). Deselecting country.",
                province_id
            );
            self.player_tag = None;
            self.lookup_dirty = true;
            return;
        };

        // Check if clicking the same country (toggle deselection)
        if self.player_tag.as_deref() == Some(&owner_tag) {
            log::info!("Deselected country: {}", owner_tag);
            self.player_tag = None;
            self.lookup_dirty = true;
            return;
        }

        // Select the new country
        log::info!("Selected country: {}", owner_tag);
        self.player_tag = Some(owner_tag.clone());

        // Update country_selection_index to match
        if let Some((idx, _)) = self
            .playable_countries
            .iter()
            .enumerate()
            .find(|(_, (tag, _, _))| tag == &owner_tag)
        {
            self.country_selection_index = idx;
            log::info!(
                "Country selection index updated to {} for tag {}",
                idx,
                owner_tag
            );
        } else {
            log::warn!("Country {} not found in playable_countries list", owner_tag);
        }

        self.lookup_dirty = true;
    }

    fn select_province_at(&mut self, world_x: f64, world_y: f64) {
        // Wrap X to 0..1 range
        let world_x = world_x.rem_euclid(1.0);

        // Check bounds for Y (provinces.bmp is in 0..1 texture space)
        if !(0.0..=1.0).contains(&world_y) {
            log::debug!("Click outside map bounds: y={:.4}", world_y);
            self.selected_province = None;
            self.update_window_title();
            return;
        }

        // Convert to pixel coordinates
        let map_width = self.province_map.width();
        let map_height = self.province_map.height();
        let pixel_x = (world_x * map_width as f64) as u32;
        let pixel_y = (world_y * map_height as f64) as u32;

        // Clamp to valid range
        let pixel_x = pixel_x.min(map_width - 1);
        let pixel_y = pixel_y.min(map_height - 1);

        // Sample pixel color
        let pixel = self.province_map.get_pixel(pixel_x, pixel_y);
        let color = (pixel[0], pixel[1], pixel[2]);

        // Look up province ID
        let province_id = if let Some(lookup) = &self.province_lookup {
            lookup.by_color.get(&color).copied()
        } else {
            None
        };

        let Some(province_id) = province_id else {
            log::debug!(
                "No province at pixel ({}, {}) - color {:?}",
                pixel_x,
                pixel_y,
                color
            );
            self.selected_province = None;
            self.update_window_title();
            return;
        };

        // Get province owner from world state
        let province_owner = self
            .world_state
            .as_ref()
            .and_then(|ws| ws.provinces.get(&province_id).and_then(|p| p.owner.clone()));

        // Get player tag
        let player_tag = match &self.player_tag {
            Some(tag) => tag.clone(),
            None => {
                self.selected_province = Some(province_id);
                self.update_window_title();
                return;
            }
        };

        // Handle click based on input mode
        let (new_mode, action) = input::handle_province_click(
            &self.input_mode,
            province_id,
            province_owner.as_deref(),
            &player_tag,
        );

        self.input_mode = new_mode;

        // Process the action
        match action {
            input::PlayerAction::SelectProvince(pid) => {
                self.selected_province = Some(pid);

                // Check if player has an army or fleet in this province
                if let Some(ws) = &self.world_state {
                    // Check for army
                    let player_army = ws
                        .armies
                        .iter()
                        .find(|(_, army)| army.location == pid && army.owner == player_tag);

                    // Check for fleet (in sea zones)
                    let player_fleet = ws
                        .fleets
                        .iter()
                        .find(|(_, fleet)| fleet.location == pid && fleet.owner == player_tag);

                    if let Some((&army_id, army)) = player_army {
                        self.selected_army = Some(army_id);
                        self.selected_fleet = None;
                        log::info!(
                            "Selected {} ({} regiments) in province {} - press M to move",
                            army.name,
                            army.regiments.len(),
                            pid
                        );
                    } else if let Some((&fleet_id, fleet)) = player_fleet {
                        self.selected_fleet = Some(fleet_id);
                        self.selected_army = None;
                        log::info!(
                            "Selected {} ({} ships) in sea zone {} - press F to move",
                            fleet.name,
                            fleet.ships.len(),
                            pid
                        );
                    } else {
                        self.selected_army = None;
                        self.selected_fleet = None;
                        // Log province info
                        if let Some(lookup) = &self.province_lookup
                            && let Some(def) = lookup.by_id.get(&pid)
                        {
                            log::info!("Selected province {} ({})", def.name, pid);
                        }
                    }
                }
            }
            input::PlayerAction::MoveArmy {
                army_id,
                destination,
            } => {
                log::info!("Moving army {} to province {}", army_id, destination);
                self.send_move_command(army_id, destination);
                self.selected_army = None;
            }
            input::PlayerAction::MoveFleet {
                fleet_id,
                destination,
            } => {
                log::info!("Moving fleet {} to sea zone {}", fleet_id, destination);
                self.send_fleet_move_command(fleet_id, destination);
                self.selected_fleet = None;
            }
            input::PlayerAction::DeclareWar { target } => {
                log::info!("Declaring war on {}!", target);
                self.send_declare_war_command(&target);
            }
            input::PlayerAction::Cancel | input::PlayerAction::None => {}
        }

        self.update_window_title();
    }

    /// Sends a move command to the simulation thread.
    fn send_move_command(&self, army_id: u32, destination: u32) {
        use eu4sim_core::input::{Command, PlayerInputs};

        let player_tag = match &self.player_tag {
            Some(tag) => tag.clone(),
            None => return,
        };

        let inputs = PlayerInputs {
            country: player_tag,
            commands: vec![Command::Move {
                army_id,
                destination,
            }],
            available_commands: Vec::new(),
            visible_state: None,
        };

        self.sim_handle.enqueue_commands(inputs);
        log::info!(
            "Sent Move command: army {} -> province {}",
            army_id,
            destination
        );
    }

    /// Sends a fleet move command to the simulation thread.
    fn send_fleet_move_command(&self, fleet_id: u32, destination: u32) {
        use eu4sim_core::input::{Command, PlayerInputs};

        let player_tag = match &self.player_tag {
            Some(tag) => tag.clone(),
            None => return,
        };

        let inputs = PlayerInputs {
            country: player_tag,
            commands: vec![Command::MoveFleet {
                fleet_id,
                destination,
            }],
            available_commands: Vec::new(),
            visible_state: None,
        };

        self.sim_handle.enqueue_commands(inputs);
        log::info!(
            "Sent MoveFleet command: fleet {} -> sea zone {}",
            fleet_id,
            destination
        );
    }

    /// Sends a declare war command to the simulation thread.
    fn send_declare_war_command(&self, target: &str) {
        use eu4sim_core::input::{Command, PlayerInputs};

        let player_tag = match &self.player_tag {
            Some(tag) => tag.clone(),
            None => return,
        };

        let inputs = PlayerInputs {
            country: player_tag,
            commands: vec![Command::DeclareWar {
                target: target.to_string(),
                cb: None,
            }],
            available_commands: Vec::new(),
            visible_state: None,
        };

        self.sim_handle.enqueue_commands(inputs);
        log::info!(
            "Sent DeclareWar command: {} -> {}",
            self.player_tag.as_deref().unwrap_or("?"),
            target
        );
    }

    /// Handles mouse scroll for zooming or GUI scrolling.
    fn handle_scroll(&mut self, delta: MouseScrollDelta) {
        // Extract scroll delta
        let scroll_delta = match delta {
            MouseScrollDelta::LineDelta(_, y) => y,
            MouseScrollDelta::PixelDelta(pos) => {
                // Convert pixel delta to line delta (approximate)
                (pos.y / 40.0) as f32
            }
        };

        if scroll_delta == 0.0 {
            return;
        }

        // First, check if GUI wants to handle the scroll (e.g., listboxes)
        let current_screen = self.screen_manager.current();
        if matches!(current_screen, Screen::SinglePlayer)
            && let Some(gui_renderer) = &mut self.gui_renderer
        {
            let gui_consumed = gui_renderer.handle_mouse_wheel(
                self.cursor_pos.0 as f32,
                self.cursor_pos.1 as f32,
                -scroll_delta, // Negate: positive wheel delta = scroll up (decrease offset)
            );
            if gui_consumed {
                log::debug!("GUI consumed mouse wheel scroll");
                return;
            }
        }

        // Otherwise, apply map zoom
        let zoom_factor = if scroll_delta > 0.0 { 1.1 } else { 0.9 };

        log::debug!(
            "Zooming by factor {}, cursor at {:?}",
            zoom_factor,
            self.cursor_pos
        );
        self.camera.zoom(
            zoom_factor,
            self.cursor_pos.0,
            self.cursor_pos.1,
            self.config.width as f64,
            self.config.height as f64,
        );
        log::debug!("New camera zoom: {}", self.camera.zoom);
    }

    /// Handles mouse button events.
    fn handle_mouse_button(&mut self, button: MouseButton, state: ElementState) {
        log::debug!("Mouse button {:?} {:?}", button, state);
        match button {
            MouseButton::Middle => {
                self.panning = state == ElementState::Pressed;
                self.last_cursor_pos = self.cursor_pos;
                log::debug!("Panning: {}", self.panning);
            }
            MouseButton::Left => {
                if state == ElementState::Pressed {
                    let current_screen = self.screen_manager.current();

                    // Check GUI clicks for screens with UI panels
                    if matches!(
                        current_screen,
                        screen::Screen::Playing | screen::Screen::SinglePlayer
                    ) && let Some(ref mut gui_renderer) = self.gui_renderer
                    {
                        // Create current GUI state for hit testing
                        let gui_state = gui::GuiState {
                            date: String::new(), // Not needed for hit testing
                            speed: match self.sim_speed {
                                SimSpeed::Paused => 0,
                                SimSpeed::Speed1 => 1,
                                SimSpeed::Speed2 => 2,
                                SimSpeed::Speed3 => 3,
                                SimSpeed::Speed4 => 4,
                                SimSpeed::Speed5 => 5,
                            },
                            paused: self.sim_speed == SimSpeed::Paused,
                            country: None, // Not needed for hit testing
                        };

                        if let Some(action) = gui_renderer.handle_click(
                            self.cursor_pos.0 as f32,
                            self.cursor_pos.1 as f32,
                            &gui_state,
                        ) {
                            self.handle_gui_action(action);
                            return; // Don't process as province click
                        }
                    }

                    // Map clicks: different behavior per screen
                    match current_screen {
                        screen::Screen::Playing => {
                            // Playing mode: select provinces for viewing info
                            let world_pos = self.camera.screen_to_world(
                                self.cursor_pos.0,
                                self.cursor_pos.1,
                                self.config.width as f64,
                                self.config.height as f64,
                            );
                            self.select_province_at(world_pos.0, world_pos.1);
                        }
                        screen::Screen::SinglePlayer => {
                            // SinglePlayer mode: select countries for game start
                            let world_pos = self.camera.screen_to_world(
                                self.cursor_pos.0,
                                self.cursor_pos.1,
                                self.config.width as f64,
                                self.config.height as f64,
                            );
                            self.select_country_at(world_pos.0, world_pos.1);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    /// Handles GUI actions from button clicks.
    fn handle_gui_action(&mut self, action: gui::GuiAction) {
        match action {
            gui::GuiAction::SetSpeed(speed) => {
                self.sim_speed = match speed {
                    0 => SimSpeed::Paused,
                    1 => SimSpeed::Speed1,
                    2 => SimSpeed::Speed2,
                    3 => SimSpeed::Speed3,
                    4 => SimSpeed::Speed4,
                    _ => SimSpeed::Speed5,
                };
                self.sim_handle.set_speed(self.sim_speed);
                log::info!("Speed set to {:?} via GUI click", self.sim_speed);
            }
            gui::GuiAction::TogglePause => {
                self.sim_speed = if self.sim_speed == SimSpeed::Paused {
                    SimSpeed::Speed3
                } else {
                    SimSpeed::Paused
                };
                self.sim_handle.set_speed(self.sim_speed);
                log::info!("Pause toggled via GUI click: {:?}", self.sim_speed);
            }
            gui::GuiAction::Back => {
                // Return to main menu from country selection
                self.screen_manager.transition_to(Screen::MainMenu);
                log::info!("Returning to main menu via Back button");
            }
            gui::GuiAction::StartGame => {
                self.start_game_with_selected_country();
            }
            gui::GuiAction::DateAdjust(part, delta) => {
                use gui::types::DatePart;
                match part {
                    DatePart::Year => {
                        self.start_date
                            .adjust_year(delta, self.year_range.0, self.year_range.1);
                    }
                    DatePart::Month => {
                        self.start_date.adjust_month(delta);
                        // Month adjustment can wrap the year, so clamp it back to range
                        let year = self.start_date.year();
                        self.start_date
                            .set_year(year, self.year_range.0, self.year_range.1);
                    }
                    DatePart::Day => {
                        self.start_date.adjust_day(delta);
                        // Day adjustment can wrap the year, so clamp it back to range
                        let year = self.start_date.year();
                        self.start_date
                            .set_year(year, self.year_range.0, self.year_range.1);
                    }
                }
                log::info!(
                    "Date adjusted to: {}.{}.{} (range: {}-{})",
                    self.start_date.year(),
                    self.start_date.month(),
                    self.start_date.day(),
                    self.year_range.0,
                    self.year_range.1
                );
                // Mark lookup dirty to trigger political map update
                self.lookup_dirty = true;
            }
            gui::GuiAction::SetMapMode(mode) => {
                // TODO: Implement map mode switching (Phase 9)
                log::info!("Set map mode: {}", mode);
            }
            gui::GuiAction::RandomCountry => {
                // Select a random country from playable countries
                if !self.playable_countries.is_empty() {
                    use rand::Rng;
                    let idx = rand::thread_rng().gen_range(0..self.playable_countries.len());
                    self.country_selection_index = idx;
                    self.log_country_selection();
                }
            }
            gui::GuiAction::OpenNationDesigner => {
                log::info!("Open nation designer - not yet implemented");
            }
            gui::GuiAction::ToggleRandomNewWorld => {
                log::info!("Toggle Random New World - not yet implemented");
            }
            gui::GuiAction::ToggleObserveMode => {
                log::info!("Toggle Observe Mode - not yet implemented");
            }
            gui::GuiAction::ToggleCustomNation => {
                log::info!("Toggle Custom Nation - not yet implemented");
            }
            gui::GuiAction::SelectBookmark(idx) => {
                // Log bookmark selection; actual date application happens in Phase 9
                if let Some(gui_renderer) = &self.gui_renderer
                    && let Some(bookmark) = gui_renderer.selected_bookmark()
                {
                    log::info!(
                        "Selected bookmark {}: {} ({:?})",
                        idx,
                        bookmark.name,
                        bookmark.date
                    );
                }
            }
            gui::GuiAction::SelectSaveGame(idx) => {
                // Log save game selection; actual save loading happens in Phase 9
                if let Some(gui_renderer) = &self.gui_renderer
                    && let Some(save) = gui_renderer.selected_save_game()
                {
                    log::info!(
                        "Selected save game {}: {} (modified: {})",
                        idx,
                        save.name,
                        save.modified_str()
                    );
                }
            }
        }
        self.window.request_redraw();
    }

    /// Starts the game with the currently selected country.
    ///
    /// Called from both Enter key handler and Play button click.
    fn start_game_with_selected_country(&mut self) {
        if let Some((tag, _name, dev)) = self.playable_countries.get(self.country_selection_index) {
            log::info!(
                ">>> SELECTED: {} with {} development - GAME STARTING <<<",
                tag,
                dev
            );
            self.player_tag = Some(tag.clone());
            self.screen_manager.transition_to(screen::Screen::Playing);
            self.screen_manager.clear_history(); // Can't go back from gameplay
            self.lookup_dirty = true; // Update GPU lookup texture
            self.update_window_title();

            // Load the player's flag and create bind groups
            if let Some(flag_view) = self.flag_cache.get(tag, &self.device, &self.queue) {
                // Regular flag bind group (for fallback)
                self.player_flag_bind_group = Some(
                    self.sprite_renderer
                        .create_bind_group(&self.device, flag_view),
                );

                // Also create masked flag bind group and overlay if available
                if let Some(gui_renderer) = &mut self.gui_renderer {
                    // Masked flag bind group (flag + shield mask)
                    if let Some((mask_view, mask_w, mask_h)) =
                        gui_renderer.get_shield_mask(&self.device, &self.queue)
                    {
                        self.shield_mask_size = (mask_w, mask_h);
                        self.masked_flag_bind_group =
                            Some(self.sprite_renderer.create_masked_bind_group(
                                &self.device,
                                flag_view,
                                mask_view,
                            ));
                        log::info!(
                            "Created masked flag bind group for {} (mask {}x{})",
                            tag,
                            mask_w,
                            mask_h
                        );
                    }

                    // Shield overlay bind group
                    if let Some((overlay_view, overlay_w, overlay_h)) =
                        gui_renderer.get_shield_overlay(&self.device, &self.queue)
                    {
                        self.shield_overlay_size = (overlay_w, overlay_h);
                        self.shield_overlay_bind_group = Some(
                            self.sprite_renderer
                                .create_bind_group(&self.device, overlay_view),
                        );
                        log::info!(
                            "Created shield overlay bind group ({}x{})",
                            overlay_w,
                            overlay_h
                        );
                    }

                    // Cache the shield clip rect (position in topbar)
                    // Use overlay size for positioning since that's the full frame
                    let screen_size = (self.size.width, self.size.height);
                    self.shield_clip_rect = gui_renderer
                        .get_player_shield_clip_rect(screen_size, self.shield_overlay_size);
                }

                log::info!("Loaded flag for {}", tag);
            }
        } else {
            log::warn!("No country selected - cannot start game");
        }
    }

    /// Handles cursor movement. Returns true if a redraw is needed.
    fn handle_cursor_move(&mut self, x: f64, y: f64) -> bool {
        self.cursor_pos = (x, y);

        if self.panning {
            let dx = x - self.last_cursor_pos.0;
            let dy = y - self.last_cursor_pos.1;
            self.last_cursor_pos = (x, y);

            self.camera
                .pan(dx, dy, self.config.width as f64, self.config.height as f64);
            true
        } else {
            false
        }
    }

    /// Reconfigures the surface (needed after Outdated error).
    fn reconfigure_surface(&mut self) {
        self.surface.configure(&self.device, &self.config);
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("EU4 Source Port starting...");

    let event_loop = EventLoop::new().unwrap();

    // Create borderless window at target resolution
    let window = WindowBuilder::new()
        .with_title("EU4 Source Port")
        .with_inner_size(PhysicalSize::new(TARGET_WIDTH, TARGET_HEIGHT))
        .with_decorations(false) // Borderless
        .with_resizable(false)
        .build(&event_loop)
        .expect("Failed to create window");

    log::info!(
        "Created borderless window: {}x{}",
        TARGET_WIDTH,
        TARGET_HEIGHT
    );

    // Initialize app
    let mut app = pollster::block_on(App::new(window));

    // Set initial window title and show country selection prompt
    app.update_window_title();
    if app.screen_manager.current() == screen::Screen::SinglePlayer {
        app.log_country_selection();
    }

    // Run event loop
    let _ = event_loop.run(move |event, control_flow| match event {
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == app.window().id() => {
            let (consumed, should_exit) = app.input(event);
            if should_exit {
                control_flow.exit();
                return;
            }
            if !consumed {
                match event {
                    WindowEvent::CloseRequested => control_flow.exit(),
                    WindowEvent::Resized(physical_size) => {
                        app.resize(*physical_size);
                    }
                    WindowEvent::RedrawRequested => match app.render() {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => app.resize(app.size),
                        Err(wgpu::SurfaceError::Outdated) => app.reconfigure_surface(),
                        Err(wgpu::SurfaceError::OutOfMemory) => control_flow.exit(),
                        Err(e) => log::warn!("Render error: {:?}", e),
                    },
                    _ => {}
                }
            }
        }
        Event::AboutToWait => {
            app.poll_sim_events();
            app.update_lookup();

            // Poll frontend UI for button clicks (Phase 6.1.3)
            if app.poll_frontend_ui() {
                log::info!("Exit requested from frontend UI");
                control_flow.exit();
                return;
            }

            app.window().request_redraw();
        }
        _ => {}
    });
}
