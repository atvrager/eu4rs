use crate::args::MapMode;
use crate::camera::Camera;
use eu4data::Tradegoods;
use eu4data::countries::Country;
use eu4data::cultures::Culture;
use eu4data::history::ProvinceHistory;
use eu4data::religions::Religion;
use image::RgbImage;
use std::collections::{HashMap, HashSet};

/// Holds all static world data loaded from the game files.
///
/// This struct is the "database" of the application, containing the parsed maps
/// and history data required for visualization.
pub struct WorldData {
    /// The base province map (provinces.bmp), used for ID lookups.
    pub province_map: RgbImage,
    /// Political map colors (based on owner countries).
    pub political_map: RgbImage,
    /// Trade goods map colors.
    pub tradegoods_map: RgbImage,
    /// Religion map colors.
    pub religion_map: RgbImage,
    /// Culture map colors.
    pub culture_map: RgbImage,
    /// Mapping from unique RGB colors in province_map to Province IDs.
    pub color_to_id: HashMap<(u8, u8, u8), u32>,
    /// History data keyed by Province ID.
    pub province_history: HashMap<u32, ProvinceHistory>,
    /// Country definitions (tags, colors, names).
    #[allow(dead_code)]
    pub countries: HashMap<String, Country>,
    /// Religion definitions.
    #[allow(dead_code)]
    pub religions: HashMap<String, Religion>,
    /// Culture definitions.
    #[allow(dead_code)]
    pub cultures: HashMap<String, Culture>,
    /// Tradegood definitions.
    #[allow(dead_code)]
    pub tradegoods: HashMap<String, Tradegoods>,
    /// Set of Province IDs considered water (seas/lakes).
    #[allow(dead_code)]
    pub water_ids: HashSet<u32>,
    /// Adjacency graph for pathfinding and movement visualization.
    #[allow(dead_code)]
    pub adjacency_graph: eu4data::adjacency::AdjacencyGraph,
}

impl WorldData {
    /// Looks up the Province ID at a specific pixel coordinate in the province map.
    ///
    /// Returns `None` if coordinates are out of bounds or the color is not mapped.
    pub fn get_province_id(&self, x: u32, y: u32) -> Option<u32> {
        if x >= self.province_map.width() || y >= self.province_map.height() {
            return None;
        }
        let pixel = self.province_map.get_pixel(x, y);
        let rgb = (pixel[0], pixel[1], pixel[2]);
        self.color_to_id.get(&rgb).copied()
    }

    /// Generates a multiline debug tooltip for a province.
    ///
    /// Shows ID, owner, goods, religion, and culture.
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

    /// Generates a concise tooltip relevant to the active map mode.
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

/// Decoupled application state for logic testing.
///
/// This struct manages the dynamic state of the interactive viewer, including
/// camera position, window size, input state, and the active dataset (`WorldData`).
pub struct AppState {
    /// The static game data.
    pub world_data: WorldData,
    /// Current window dimensions (width, height).
    pub window_size: (u32, u32),
    /// Current mouse cursor position (if inside window).
    pub cursor_pos: Option<(f64, f64)>,
    /// The camera controller for zooming and panning.
    pub camera: Camera,
    /// Whether the user is currently panning the map (Middle Mouse click).
    pub is_panning: bool,
    /// The cursor position recorded at the start of the last frame/event.
    pub last_cursor_pos: Option<(f64, f64)>,
    /// The currently active map mode.
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

    /// Calculates the tooltip text for the province under the cursor.
    ///
    /// Returns `None` if the cursor is invalid or not over a province.
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

            if x < tex_w && y < tex_h {
                return self.world_data.get_province_id(x, y).map(|id| {
                    self.world_data
                        .get_mode_specific_tooltip(id, self.current_map_mode)
                });
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

    /// Identifies the province selected (clicked) by the user.
    ///
    /// Returns the Province ID and its full details text.
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
                let (id, text) = self
                    .world_data
                    .get_province_id(x, y)
                    .map(|id| (id, self.world_data.get_province_tooltip(id)))
                    .unwrap_or((0, "Unknown Province".to_string()));
                return Some((id, text));
            }
        }
        None
    }
}
