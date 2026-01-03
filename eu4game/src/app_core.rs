//! Core application logic for eu4game.
//!
//! This module contains the window-independent game state and logic,
//! enabling both windowed and headless operation.
//!
//! Note: This module is currently scaffolding for the headless refactor.
//! Full integration with main.rs is pending.

// TODO: Remove when AppCore is integrated with main.rs
#![allow(dead_code)]

use crate::camera::Camera;
use crate::flags::FlagCache;
use crate::gui::{self, GuiRenderer};
use crate::input::InputMode;
use crate::render::{self, GpuContext, RenderError, Renderer, SpriteRenderer};
use crate::screen::{Screen, ScreenManager};
use crate::sim_thread::{SimEvent, SimHandle, SimSpeed};
use crate::text;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Core application state, independent of display mode.
///
/// This struct contains all game state and logic that doesn't depend on
/// whether we're rendering to a window or an offscreen buffer.
pub struct AppCore {
    // GPU resources
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub format: wgpu::TextureFormat,

    // Window dimensions (updated on resize)
    pub size: (u32, u32),

    // Rendering
    pub renderer: Renderer,
    pub camera: Camera,
    pub sprite_renderer: SpriteRenderer,
    pub text_renderer: Option<text::TextRenderer>,
    pub gui_renderer: Option<GuiRenderer>,

    // Input state
    pub cursor_pos: (f64, f64),
    pub panning: bool,
    pub last_cursor_pos: (f64, f64),
    pub input_mode: InputMode,

    // Game state
    pub sim_handle: SimHandle,
    pub sim_speed: SimSpeed,
    pub current_date: eu4sim_core::state::Date,
    pub world_state: Option<Arc<eu4sim_core::WorldState>>,

    // Screen/UI state
    pub screen_manager: ScreenManager,
    pub frontend_ui: Option<gui::frontend::FrontendUI>,

    // Province/map data
    pub province_lookup: Option<eu4data::map::ProvinceLookup>,
    pub province_map: image::RgbaImage,
    pub selected_province: Option<u32>,
    pub province_centers: HashMap<u32, (u32, u32)>,

    // Country/player state
    pub player_tag: Option<String>,
    pub playable_countries: Vec<(String, String, i32)>,
    pub country_selection_index: usize,
    pub country_colors: HashMap<String, [u8; 3]>,

    // Army/fleet selection
    pub selected_army: Option<u32>,
    pub selected_fleet: Option<u32>,

    // GPU lookup state
    pub lookup_dirty: bool,
    pub flag_cache: FlagCache,

    // Flag rendering
    pub player_flag_bind_group: Option<wgpu::BindGroup>,
    pub masked_flag_bind_group: Option<wgpu::BindGroup>,
    pub shield_overlay_bind_group: Option<wgpu::BindGroup>,
    pub shield_overlay_size: (u32, u32),
    pub shield_mask_size: (u32, u32),
    pub shield_clip_rect: Option<(f32, f32, f32, f32)>,

    // Country selection UI
    pub country_select_left: Option<gui::CountrySelectLeftPanel>,
    pub start_date: eu4data::Eu4Date,
    pub year_range: (i32, i32),

    // Map modes
    pub current_map_mode: gui::MapMode,
    pub trade_network: Option<eu4data::tradenodes::TradeNetwork>,
    pub sea_provinces: HashSet<u32>,
    pub religions: HashMap<String, eu4data::religions::Religion>,
    pub cultures: HashMap<String, eu4data::cultures::Culture>,
    pub region_mapping: Option<eu4data::regions::ProvinceRegionMapping>,
}

impl GpuContext for AppCore {
    fn device(&self) -> &wgpu::Device {
        &self.device
    }

    fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    fn format(&self) -> wgpu::TextureFormat {
        self.format
    }
}

/// Result of processing one frame.
#[derive(Debug, Clone)]
pub struct TickResult {
    /// Whether exit was requested.
    pub should_exit: bool,
    /// Whether a redraw is needed.
    pub needs_redraw: bool,
}

impl AppCore {
    /// Resize handler - updates internal size and recalculates shield clip rect.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.size = (width, height);
            log::debug!("AppCore resized to {}x{}", width, height);

            // Recalculate shield clip rect for new screen size
            if let Some(gui_renderer) = &self.gui_renderer {
                self.shield_clip_rect =
                    gui_renderer.get_player_shield_clip_rect(self.size, self.shield_overlay_size);
            }
        }
    }

    /// Get the current screen.
    pub fn current_screen(&self) -> Screen {
        self.screen_manager.current()
    }

    /// Process one tick: poll simulation events, update lookup, poll UI.
    /// Returns whether exit was requested.
    pub fn tick(&mut self) -> TickResult {
        self.poll_sim_events();
        self.update_lookup();

        let should_exit = self.poll_frontend_ui();
        TickResult {
            should_exit,
            needs_redraw: true,
        }
    }

    /// Render to the provided texture view.
    ///
    /// This is the core render logic, extracted to work with any render target.
    pub fn render_to_view(
        &mut self,
        view: &wgpu::TextureView,
    ) -> Result<wgpu::CommandBuffer, RenderError> {
        let (width, height) = self.size;

        // Update camera uniform
        let camera_uniform = self.camera.to_uniform(width as f32, height as f32);
        self.renderer.update_camera(&self.queue, camera_uniform);

        // Update map mode uniform
        let map_mode_value = match self.current_map_mode {
            gui::MapMode::Political => 0.0,
            gui::MapMode::Terrain => 1.0,
            gui::MapMode::Trade => 2.0,
            gui::MapMode::Religion => 3.0,
            gui::MapMode::Culture => 4.0,
            gui::MapMode::Economy => 5.0,
            gui::MapMode::Empire => 6.0,
            gui::MapMode::Region => 7.0,
            _ => 0.0,
        };
        self.renderer
            .update_map_mode(&self.queue, map_mode_value, (width, height));

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
                    view,
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

            let screen_size = (width, height);
            let current_screen = self.screen_manager.current();

            // Render based on current screen
            match current_screen {
                Screen::MainMenu => {
                    // Main menu: Just render the menu UI, no map
                }
                Screen::SinglePlayer | Screen::Playing => {
                    // Game screens: Render map and game elements
                    render_pass.set_pipeline(&self.renderer.pipeline);
                    render_pass.set_bind_group(0, &self.renderer.bind_group, &[]);
                    render_pass.draw(0..3, 0..1);

                    // Draw army markers
                    if self.renderer.army_count > 0 {
                        render_pass.set_pipeline(&self.renderer.army_pipeline);
                        render_pass.set_bind_group(0, &self.renderer.army_bind_group, &[]);
                        render_pass
                            .set_vertex_buffer(0, self.renderer.army_instance_buffer.slice(..));
                        render_pass.draw(0..6, 0..self.renderer.army_count);
                    }

                    // Draw fleet markers
                    if self.renderer.fleet_count > 0 {
                        render_pass.set_pipeline(&self.renderer.fleet_pipeline);
                        render_pass.set_bind_group(0, &self.renderer.fleet_bind_group, &[]);
                        render_pass
                            .set_vertex_buffer(0, self.renderer.fleet_instance_buffer.slice(..));
                        render_pass.draw(0..6, 0..self.renderer.fleet_count);
                    }
                }
                _ => {}
            }

            // Prepare GUI state (must be computed before borrowing gui_renderer)
            let gui_state = self.create_gui_state();
            let country_state = self.create_selected_country_state();
            let play_button_enabled = self.player_tag.is_some();
            let start_date_ref = if current_screen == Screen::SinglePlayer {
                Some(self.start_date)
            } else {
                None
            };

            // Render GUI for current screen
            if let Some(gui_renderer) = &mut self.gui_renderer {
                gui_renderer.update_selected_country(country_state.as_ref());
                gui_renderer.set_play_button_enabled(play_button_enabled);

                gui_renderer.render(
                    &mut render_pass,
                    &self.device,
                    &self.queue,
                    &self.sprite_renderer,
                    &gui_state,
                    current_screen,
                    screen_size,
                    start_date_ref.as_ref(),
                );
            }
        }

        Ok(encoder.finish())
    }

    /// Create GUI state from current app state.
    fn create_gui_state(&self) -> gui::GuiState {
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

        let speed_index = match self.sim_speed {
            SimSpeed::Paused => 0,
            SimSpeed::Speed1 => 1,
            SimSpeed::Speed2 => 2,
            SimSpeed::Speed3 => 3,
            SimSpeed::Speed4 => 4,
            SimSpeed::Speed5 => 5,
        };

        let country = self.player_tag.as_ref().and_then(|tag| {
            self.world_state
                .as_ref()
                .and_then(|ws| crate::world_loader::extract_country_resources(ws, tag))
        });

        gui::GuiState {
            date: date_str,
            speed: speed_index,
            paused: matches!(self.sim_speed, SimSpeed::Paused),
            country,
        }
    }

    /// Create selected country state for country selection screen.
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

    // ========================================================================
    // Simulation polling (moved from App)
    // ========================================================================

    fn poll_sim_events(&mut self) {
        for event in self.sim_handle.poll_events() {
            match event {
                SimEvent::Tick { state, tick } => {
                    self.current_date = state.date;
                    self.world_state = Some(state);
                    self.lookup_dirty = true;
                    log::debug!("Tick {} - Date: {}", tick, self.current_date);
                }
                SimEvent::SpeedChanged(speed) => {
                    self.sim_speed = speed;
                }
                SimEvent::Shutdown => {
                    log::info!("Sim thread shutdown acknowledged");
                }
            }
        }
    }

    fn poll_frontend_ui(&mut self) -> bool {
        let Some(ref mut frontend_ui) = self.frontend_ui else {
            return false;
        };

        // Poll for button click actions from main menu
        if let Some(action) = frontend_ui.poll_main_menu() {
            return self.handle_ui_action(action);
        }

        // Poll for actions from country select left panel
        if let Some(ref mut left_panel) = self.country_select_left
            && let Some(action) = left_panel.poll_actions()
        {
            return self.handle_ui_action(action);
        }

        false
    }

    fn handle_ui_action(&mut self, action: gui::core::UiAction) -> bool {
        use gui::core::UiAction;

        match action {
            UiAction::ShowSinglePlayer => {
                self.screen_manager.transition_to(Screen::SinglePlayer);
                self.current_map_mode = gui::MapMode::Political;
                false
            }
            UiAction::ShowMultiplayer => {
                self.screen_manager.transition_to(Screen::Multiplayer);
                false
            }
            UiAction::Exit => {
                self.sim_handle.shutdown();
                true
            }
            UiAction::Back => {
                if self.screen_manager.can_go_back() {
                    self.screen_manager.go_back();
                }
                false
            }
            UiAction::StartGame => {
                self.screen_manager.transition_to(Screen::Playing);
                self.screen_manager.clear_history();
                false
            }
            _ => false, // Other actions not handled here
        }
    }

    // ========================================================================
    // Lookup texture updates (moved from App)
    // ========================================================================

    /// Write raw RGBA lookup data to the GPU texture.
    fn write_lookup_raw(&self, lookup_data: &[u8]) {
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.renderer.lookup_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            lookup_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * render::LOOKUP_SIZE),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: render::LOOKUP_SIZE,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }

    fn update_lookup(&mut self) {
        if !self.lookup_dirty {
            return;
        }

        match self.current_map_mode {
            gui::MapMode::Trade => self.update_lookup_trade(),
            gui::MapMode::Religion => self.update_lookup_religion(),
            gui::MapMode::Culture => self.update_lookup_culture(),
            gui::MapMode::Economy => self.update_lookup_economy(),
            gui::MapMode::Empire => self.update_lookup_empire(),
            gui::MapMode::Region => self.update_lookup_region(),
            _ => self.update_lookup_political(),
        }

        self.lookup_dirty = false;
    }

    fn update_lookup_political(&mut self) {
        let Some(world_state) = &self.world_state else {
            return;
        };

        // Build province owners map for the helper struct
        let mut province_owners = HashMap::new();
        let mut sea_provinces_local = HashSet::new();
        let mut max_province_id: u32 = 0;

        for (&province_id, province) in &world_state.provinces {
            max_province_id = max_province_id.max(province_id);
            if province.is_sea {
                sea_provinces_local.insert(province_id);
            }
            if let Some(ref owner) = province.owner {
                province_owners.insert(province_id, owner.clone());
            }
        }

        let data = render::LookupUpdateData {
            province_owners: &province_owners,
            sea_provinces: &sea_provinces_local,
            country_colors: &self.country_colors,
            max_province_id,
        };

        self.renderer.update_lookup(&self.queue, &data);
    }

    fn update_lookup_trade(&mut self) {
        let Some(ref trade_network) = self.trade_network else {
            return;
        };

        let mut lookup_data: Vec<u8> = Vec::with_capacity((render::LOOKUP_SIZE * 4) as usize);

        let wasteland_color = [60u8, 60, 60, 255];
        let water_color = [30u8, 60, 100, 255];
        let unknown_color = [80u8, 80, 80, 255];

        for province_id in 0..render::LOOKUP_SIZE {
            let color = if province_id == 0 {
                unknown_color
            } else if self.sea_provinces.contains(&province_id) {
                water_color
            } else if let Some(node_id) = trade_network.province_to_node.get(&province_id) {
                // Use node ID index for color
                let hue = (node_id.0 as f32 * 137.5) % 360.0;
                let rgb = hsl_to_rgb(hue, 0.7, 0.5);
                [rgb[0], rgb[1], rgb[2], 255]
            } else {
                wasteland_color
            };
            lookup_data.extend_from_slice(&color);
        }

        self.write_lookup_raw(&lookup_data);
    }

    fn update_lookup_religion(&mut self) {
        let Some(world_state) = &self.world_state else {
            // Show empty map
            let mut lookup_data: Vec<u8> = Vec::with_capacity((render::LOOKUP_SIZE * 4) as usize);
            for _ in 0..render::LOOKUP_SIZE {
                lookup_data.extend_from_slice(&[60, 60, 60, 255]);
            }
            self.write_lookup_raw(&lookup_data);
            return;
        };

        let mut lookup_data: Vec<u8> = Vec::with_capacity((render::LOOKUP_SIZE * 4) as usize);

        let wasteland_color = [60u8, 60, 60, 255];
        let water_color = [30u8, 60, 100, 255];
        let unknown_color = [120u8, 120, 120, 255];

        for province_id in 0..render::LOOKUP_SIZE {
            let color = if province_id == 0 {
                unknown_color
            } else if self.sea_provinces.contains(&province_id) {
                water_color
            } else if let Some(province) = world_state.provinces.get(&province_id) {
                if let Some(ref religion_key) = province.religion {
                    if let Some(religion) = self.religions.get(religion_key) {
                        [religion.color[0], religion.color[1], religion.color[2], 255]
                    } else {
                        let hash = religion_key
                            .bytes()
                            .fold(0u32, |acc, b| acc.wrapping_add(b as u32));
                        let rgb = hsl_to_rgb((hash % 360) as f32, 0.6, 0.5);
                        [rgb[0], rgb[1], rgb[2], 255]
                    }
                } else {
                    unknown_color
                }
            } else {
                wasteland_color
            };
            lookup_data.extend_from_slice(&color);
        }

        self.write_lookup_raw(&lookup_data);
    }

    fn update_lookup_culture(&mut self) {
        let Some(world_state) = &self.world_state else {
            return;
        };

        let mut lookup_data: Vec<u8> = Vec::with_capacity((render::LOOKUP_SIZE * 4) as usize);

        let wasteland_color = [60u8, 60, 60, 255];
        let water_color = [30u8, 60, 100, 255];
        let unknown_color = [100u8, 100, 100, 255];

        for province_id in 0..render::LOOKUP_SIZE {
            let color = if province_id == 0 {
                unknown_color
            } else if self.sea_provinces.contains(&province_id) {
                water_color
            } else if let Some(province) = world_state.provinces.get(&province_id) {
                if let Some(ref culture_key) = province.culture {
                    if let Some(culture) = self.cultures.get(culture_key) {
                        [culture.color[0], culture.color[1], culture.color[2], 255]
                    } else {
                        let hash = culture_key
                            .bytes()
                            .fold(0u32, |acc, b| acc.wrapping_add(b as u32));
                        let rgb = hsl_to_rgb((hash % 360) as f32, 0.6, 0.5);
                        [rgb[0], rgb[1], rgb[2], 255]
                    }
                } else {
                    unknown_color
                }
            } else {
                wasteland_color
            };
            lookup_data.extend_from_slice(&color);
        }

        self.write_lookup_raw(&lookup_data);
    }

    fn update_lookup_economy(&mut self) {
        let Some(world_state) = &self.world_state else {
            return;
        };

        let mut lookup_data: Vec<u8> = Vec::with_capacity((render::LOOKUP_SIZE * 4) as usize);

        let wasteland_color = [30u8, 30, 30, 255];
        let water_color = [30u8, 60, 100, 255];
        let unknown_color = [30u8, 30, 30, 255];

        for province_id in 0..render::LOOKUP_SIZE {
            let color = if province_id == 0 {
                unknown_color
            } else if self.sea_provinces.contains(&province_id) {
                water_color
            } else if let Some(province) = world_state.provinces.get(&province_id) {
                let total_dev = province.base_tax.to_f32()
                    + province.base_production.to_f32()
                    + province.base_manpower.to_f32();

                if total_dev <= 3.0 {
                    [30, 60, 30, 255]
                } else if total_dev <= 9.0 {
                    let t = (total_dev - 3.0) / 6.0;
                    [
                        (30.0 + t * 170.0) as u8,
                        (60.0 + t * 140.0) as u8,
                        (30.0 - t * 30.0) as u8,
                        255,
                    ]
                } else if total_dev <= 20.0 {
                    let t = (total_dev - 9.0) / 11.0;
                    [200, (200.0 - t * 80.0) as u8, 0, 255]
                } else {
                    [220, 100, 0, 255]
                }
            } else {
                wasteland_color
            };
            lookup_data.extend_from_slice(&color);
        }

        self.write_lookup_raw(&lookup_data);
    }

    fn update_lookup_empire(&mut self) {
        let Some(world_state) = &self.world_state else {
            return;
        };

        let mut lookup_data: Vec<u8> = Vec::with_capacity((render::LOOKUP_SIZE * 4) as usize);

        let wasteland_color = [50u8, 50, 50, 255];
        let water_color = [30u8, 60, 100, 255];
        let hre_color = [255u8, 215, 0, 255]; // Gold for HRE
        let non_hre_color = [80u8, 80, 80, 255]; // Dark gray for non-HRE

        for province_id in 0..render::LOOKUP_SIZE {
            let color = if province_id == 0 {
                wasteland_color
            } else if self.sea_provinces.contains(&province_id) {
                water_color
            } else if let Some(province) = world_state.provinces.get(&province_id) {
                if province.is_in_hre {
                    hre_color
                } else {
                    non_hre_color
                }
            } else {
                wasteland_color
            };
            lookup_data.extend_from_slice(&color);
        }

        self.write_lookup_raw(&lookup_data);
    }

    fn update_lookup_region(&mut self) {
        let Some(world_state) = &self.world_state else {
            return;
        };

        let mut lookup_data: Vec<u8> = Vec::with_capacity((render::LOOKUP_SIZE * 4) as usize);

        let wasteland_color = [50u8, 50, 50, 255];
        let water_color = [30u8, 60, 100, 255];

        // Pre-compute region colors
        let region_colors: HashMap<&str, [u8; 3]> = if let Some(rm) = &self.region_mapping {
            rm.regions
                .keys()
                .enumerate()
                .map(|(i, name)| {
                    let hue = (i as f32 * 137.5) % 360.0;
                    (name.as_str(), hsl_to_rgb(hue, 0.6, 0.5))
                })
                .collect()
        } else {
            HashMap::new()
        };

        for province_id in 0..render::LOOKUP_SIZE {
            let color = if province_id == 0 {
                wasteland_color
            } else if self.sea_provinces.contains(&province_id) {
                water_color
            } else if world_state.provinces.contains_key(&province_id) {
                if let Some(rm) = &self.region_mapping {
                    if let Some(region_name) = rm.province_to_region.get(&province_id) {
                        if let Some(&rgb) = region_colors.get(region_name.as_str()) {
                            [rgb[0], rgb[1], rgb[2], 255]
                        } else {
                            wasteland_color
                        }
                    } else {
                        wasteland_color
                    }
                } else {
                    wasteland_color
                }
            } else {
                wasteland_color
            };
            lookup_data.extend_from_slice(&color);
        }

        self.write_lookup_raw(&lookup_data);
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Convert HSL to RGB.
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [u8; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    [
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    ]
}
