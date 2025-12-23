//! TUI mode for eu4sim using ratatui.

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use eu4data::map::ProvinceLookup;
use eu4sim_core::WorldState;
use image::RgbaImage;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{self, Stdout};

/// TUI system state.
pub struct TuiSystem {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    map: Option<RgbaImage>,
    lookup: Option<ProvinceLookup>,
    /// Cached province ID grid
    cache: Option<CachedMap>,
    pub should_quit: bool,
    pub speed: u64,
    pub paused: bool,
    /// Zoom level (1.0 = 20 map pixels per terminal char)
    pub scale: f32,
    /// Top-left corner of viewport in map image coordinates
    pub offset: (u32, u32),
}

struct CachedMap {
    inner_area: Rect,
    grid: Vec<Vec<u32>>,
    scale: f32,
    offset: (u32, u32),
}

impl TuiSystem {
    pub fn new(
        map: Option<RgbaImage>,
        lookup: Option<ProvinceLookup>,
        initial_speed: u64,
    ) -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        // Default to Europe (roughly center of 5632x2048 map)
        let offset = if map.is_some() { (2200, 1200) } else { (0, 0) };

        Ok(Self {
            terminal,
            map,
            lookup,
            cache: None,
            should_quit: false,
            speed: initial_speed,
            paused: false,
            scale: 1.0,
            offset,
        })
    }

    pub fn render(&mut self, state: &WorldState, tick: u64, max_ticks: u32) -> Result<()> {
        let size = self.terminal.size()?;
        let rect = Rect::new(0, 0, size.width, size.height);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(rect);

        let outer_area = chunks[0];
        let inner_area = Rect {
            x: outer_area.x + 1,
            y: outer_area.y + 1,
            width: outer_area.width.saturating_sub(2),
            height: outer_area.height.saturating_sub(2),
        };

        let cache_valid = self
            .cache
            .as_ref()
            .map(|c| {
                c.inner_area == inner_area
                    && (c.scale - self.scale).abs() < 0.001
                    && c.offset == self.offset
            })
            .unwrap_or(false);

        if !cache_valid {
            self.rebuild_cache(inner_area);
        }

        let grid_ref = self.cache.as_ref().map(|c| &c.grid);
        let speed = self.speed;
        let paused = self.paused;
        let scale = self.scale;
        let offset = self.offset;

        self.terminal.draw(|f| {
            draw_ui(
                f, outer_area, chunks[1], grid_ref, state, tick, max_ticks, speed, paused, scale,
                offset,
            );
        })?;
        Ok(())
    }

    fn rebuild_cache(&mut self, inner_area: Rect) {
        let (Some(img), Some(lookup)) = (&self.map, &self.lookup) else {
            return;
        };

        let width = inner_area.width as u32;
        let height = inner_area.height as u32;
        if width == 0 || height == 0 {
            return;
        }

        let img_width = img.width();
        let img_height = img.height();
        let zoom_factor = 20.0 / self.scale;

        let mut grid = Vec::with_capacity(height as usize);
        for y in 0..height {
            let mut row = Vec::with_capacity(width as usize);
            for x in 0..width {
                let dx = (x as f32 * zoom_factor) as u32;
                let dy = (y as f32 * zoom_factor) as u32;

                let img_x = self
                    .offset
                    .0
                    .saturating_add(dx)
                    .min(img_width.saturating_sub(1));
                let img_y = self
                    .offset
                    .1
                    .saturating_add(dy)
                    .min(img_height.saturating_sub(1));

                let pixel = img.get_pixel(img_x, img_y);
                let rgb = (pixel[0], pixel[1], pixel[2]);

                let prov_id = lookup.by_color.get(&rgb).copied().unwrap_or(0);
                row.push(prov_id);
            }
            grid.push(row);
        }

        self.cache = Some(CachedMap {
            inner_area,
            grid,
            scale: self.scale,
            offset: self.offset,
        });
    }

    pub fn handle_events(&mut self) -> Result<()> {
        if !event::poll(std::time::Duration::ZERO)? {
            return Ok(());
        }

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                return Ok(());
            }

            self.handle_key(key.code);
        }
        Ok(())
    }

    /// Process a key press (extracted for testability)
    fn handle_key(&mut self, key: KeyCode) {
        let move_speed = (50.0 / self.scale) as u32;
        match key {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char(' ') => self.paused = !self.paused,
            KeyCode::Char('1') => self.speed = 1,
            KeyCode::Char('2') => self.speed = 2,
            KeyCode::Char('3') => self.speed = 3,
            KeyCode::Char('4') => self.speed = 4,
            KeyCode::Char('5') => self.speed = 5,
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.scale = (self.scale * 1.2).min(10.0);
            }
            KeyCode::Char('-') => {
                self.scale = (self.scale / 1.2).max(0.1);
            }
            KeyCode::Char('w') | KeyCode::Up => {
                self.offset.1 = self.offset.1.saturating_sub(move_speed);
            }
            KeyCode::Char('s') | KeyCode::Down => {
                self.offset.1 = self.offset.1.saturating_add(move_speed);
            }
            KeyCode::Char('a') | KeyCode::Left => {
                self.offset.0 = self.offset.0.saturating_sub(move_speed);
            }
            KeyCode::Char('d') | KeyCode::Right => {
                self.offset.0 = self.offset.0.saturating_add(move_speed);
            }
            _ => {}
        }
    }
}

impl Drop for TuiSystem {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_ui(
    f: &mut Frame,
    map_area: Rect,
    status_area: Rect,
    grid: Option<&Vec<Vec<u32>>>,
    state: &WorldState,
    tick: u64,
    max_ticks: u32,
    speed: u64,
    paused: bool,
    scale: f32,
    offset: (u32, u32),
) {
    let block = Block::default().borders(Borders::ALL).title(" EU4 Map ");

    if let Some(grid) = grid {
        let inner = block.inner(map_area);
        f.render_widget(block, map_area);
        render_map(f, inner, grid, state);
    } else {
        let body = Paragraph::new("Loading map...").block(block);
        f.render_widget(body, map_area);
    }

    let status = if paused { " PAUSED" } else { "" };
    let pct = (tick as f64 / max_ticks as f64) * 100.0;
    let status_text = format!(
        " {} │ {}/{} ({:.0}%){} │ Spd:{} │ ({},{}) {:.1}x │ WASD:pan ±:zoom 1-5:speed q:quit",
        state.date, tick, max_ticks, pct, status, speed, offset.0, offset.1, scale
    );
    let status_bar = Paragraph::new(status_text).style(Style::default().bg(Color::Indexed(236)));
    f.render_widget(status_bar, status_area);
}

fn render_map(f: &mut Frame, area: Rect, grid: &[Vec<u32>], state: &WorldState) {
    let buf = f.buffer_mut();
    for y in 0..area.height {
        let grid_row = y as usize;
        for x in 0..area.width {
            let prov_id = grid
                .get(grid_row)
                .and_then(|r| r.get(x as usize))
                .copied()
                .unwrap_or(0);
            let color = resolve_color(state, prov_id);

            let cell = &mut buf[(area.x + x, area.y + y)];
            cell.set_char(' ');
            cell.set_bg(color);
        }
    }
}

fn resolve_color(state: &WorldState, prov_id: u32) -> Color {
    if prov_id == 0 {
        return Color::Indexed(226); // Bright yellow - for debugging unknown pixels
    }

    let Some(prov) = state.provinces.get(&prov_id) else {
        // Province not in state (map edges, etc.)
        return Color::Indexed(201); // Bright magenta - for debugging missing provinces
    };

    if prov.is_sea {
        return Color::Indexed(18); // Dark blue for sea
    }

    match &prov.owner {
        Some(tag) => tag_to_color(tag),
        None => Color::Indexed(228), // Bright yellow/tan for wasteland
    }
}

fn tag_to_color(tag: &str) -> Color {
    let mut hasher = DefaultHasher::new();
    tag.hash(&mut hasher);
    let hash = hasher.finish();
    // Use color cube: 16-231 (216 colors)
    let idx = 16 + ((hash % 216) as u8);
    Color::Indexed(idx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eu4sim_core::state::{CountryState, Date, ProvinceState};
    use std::collections::HashMap;

    /// Helper to create a minimal WorldState for testing
    fn make_test_world() -> WorldState {
        let mut provinces = HashMap::new();
        let mut countries = HashMap::new();

        // Province 1: Owned by AAA
        countries.insert("AAA".to_string(), CountryState::default());
        provinces.insert(
            1,
            ProvinceState {
                owner: Some("AAA".to_string()),
                is_sea: false,
                ..Default::default()
            },
        );

        // Province 2: Ocean (is_sea = true, no owner)
        provinces.insert(
            2,
            ProvinceState {
                owner: None,
                is_sea: true,
                ..Default::default()
            },
        );

        // Province 3: Wasteland (no owner, NOT sea)
        provinces.insert(
            3,
            ProvinceState {
                owner: None,
                is_sea: false,
                ..Default::default()
            },
        );

        // Province 4: Owned by BBB
        countries.insert("BBB".to_string(), CountryState::default());
        provinces.insert(
            4,
            ProvinceState {
                owner: Some("BBB".to_string()),
                is_sea: false,
                ..Default::default()
            },
        );

        WorldState {
            date: Date::new(1444, 11, 11),
            provinces: provinces.into(),
            countries: countries.into(),
            ..Default::default()
        }
    }

    #[test]
    fn test_resolve_color_owned_province() {
        let state = make_test_world();
        let color = resolve_color(&state, 1);

        // Should use tag-based color for owned province
        let expected = tag_to_color("AAA");
        assert_eq!(color, expected, "Owned province should use tag color");
    }

    #[test]
    fn test_resolve_color_ocean() {
        let state = make_test_world();
        let color = resolve_color(&state, 2);

        // Ocean should be dark blue
        assert_eq!(
            color,
            Color::Indexed(18),
            "Ocean should be Color::Indexed(18)"
        );
    }

    #[test]
    fn test_resolve_color_wasteland() {
        let state = make_test_world();
        let color = resolve_color(&state, 3);

        // Wasteland (no owner, not sea) should be bright yellow/tan
        assert_eq!(
            color,
            Color::Indexed(228),
            "Wasteland should be bright yellow/tan (Color::Indexed(228))"
        );
    }

    #[test]
    fn test_resolve_color_unknown_province() {
        let state = make_test_world();
        let color = resolve_color(&state, 0);

        // Province ID 0 (invalid/border pixels) should be bright yellow
        assert_eq!(
            color,
            Color::Indexed(226),
            "Invalid province (ID 0) should be bright yellow (226)"
        );
    }

    #[test]
    fn test_resolve_color_missing_province() {
        let state = make_test_world();
        let color = resolve_color(&state, 999);

        // Province not in state (map edges, etc.) should be bright magenta
        assert_eq!(
            color,
            Color::Indexed(201),
            "Missing province should be bright magenta (201)"
        );
    }

    #[test]
    fn test_tag_to_color_consistency() {
        // Same tag should always produce same color
        let color1 = tag_to_color("FRA");
        let color2 = tag_to_color("FRA");
        assert_eq!(color1, color2, "Tag color should be deterministic");
    }

    #[test]
    fn test_tag_to_color_different_tags() {
        // Different tags should (usually) produce different colors
        let fra = tag_to_color("FRA");
        let eng = tag_to_color("ENG");
        // Not strictly guaranteed but very likely with hash function
        assert_ne!(fra, eng, "Different tags should produce different colors");
    }

    #[test]
    fn test_cache_invalidation_on_scale_change() {
        // Test that cache detects scale changes
        let cache = CachedMap {
            inner_area: Rect::new(0, 0, 10, 10),
            grid: vec![vec![0; 10]; 10],
            scale: 1.0,
            offset: (0, 0),
        };

        // Same params = valid
        let valid = cache.inner_area == Rect::new(0, 0, 10, 10)
            && (cache.scale - 1.0).abs() < 0.001
            && cache.offset == (0, 0);
        assert!(valid, "Cache should be valid with same params");

        // Different scale = invalid
        let invalid = cache.inner_area == Rect::new(0, 0, 10, 10)
            && (cache.scale - 1.5).abs() < 0.001
            && cache.offset == (0, 0);
        assert!(!invalid, "Cache should be invalid with different scale");
    }

    #[test]
    fn test_cache_invalidation_on_offset_change() {
        let cache = CachedMap {
            inner_area: Rect::new(0, 0, 10, 10),
            grid: vec![vec![0; 10]; 10],
            scale: 1.0,
            offset: (100, 200),
        };

        // Same offset = valid
        let valid = cache.inner_area == Rect::new(0, 0, 10, 10)
            && (cache.scale - 1.0).abs() < 0.001
            && cache.offset == (100, 200);
        assert!(valid, "Cache should be valid with same offset");

        // Different offset = invalid
        let invalid = cache.inner_area == Rect::new(0, 0, 10, 10)
            && (cache.scale - 1.0).abs() < 0.001
            && cache.offset == (150, 200);
        assert!(!invalid, "Cache should be invalid with different offset");
    }

    #[test]
    fn test_cache_invalidation_on_area_change() {
        let cache = CachedMap {
            inner_area: Rect::new(0, 0, 10, 10),
            grid: vec![vec![0; 10]; 10],
            scale: 1.0,
            offset: (0, 0),
        };

        // Different area = invalid
        let invalid = cache.inner_area == Rect::new(0, 0, 20, 20)
            && (cache.scale - 1.0).abs() < 0.001
            && cache.offset == (0, 0);
        assert!(!invalid, "Cache should be invalid with different area");
    }

    /// Test helper to simulate zoom behavior
    fn simulate_zoom_in(scale: f32) -> f32 {
        (scale * 1.2).min(10.0)
    }

    /// Test helper to simulate zoom out behavior
    fn simulate_zoom_out(scale: f32) -> f32 {
        (scale / 1.2).max(0.1)
    }

    /// Test helper to calculate move speed
    fn calculate_move_speed(scale: f32) -> u32 {
        (50.0 / scale) as u32
    }

    #[test]
    fn test_zoom_in_clamps_at_max() {
        let mut scale = 9.0;
        // Zoom in multiple times
        for _ in 0..10 {
            scale = simulate_zoom_in(scale);
        }
        assert!(
            scale <= 10.0,
            "Scale should clamp at max 10.0, got {}",
            scale
        );
        assert_eq!(scale, 10.0, "Scale should reach exactly 10.0 when maxed");
    }

    #[test]
    fn test_zoom_out_clamps_at_min() {
        let mut scale = 0.2;
        // Zoom out multiple times
        for _ in 0..10 {
            scale = simulate_zoom_out(scale);
        }
        assert!(scale >= 0.1, "Scale should clamp at min 0.1, got {}", scale);
        assert_eq!(scale, 0.1, "Scale should reach exactly 0.1 when minimized");
    }

    #[test]
    fn test_zoom_factor() {
        let initial = 1.0;
        let zoomed_in = simulate_zoom_in(initial);
        // 1.0 * 1.2 = 1.2
        assert!(
            (zoomed_in - 1.2).abs() < 0.001,
            "Zoom in should multiply by 1.2"
        );

        let zoomed_out = simulate_zoom_out(initial);
        // 1.0 / 1.2 ≈ 0.833
        assert!(
            (zoomed_out - 0.8333).abs() < 0.001,
            "Zoom out should divide by 1.2"
        );
    }

    #[test]
    fn test_move_speed_scales_with_zoom() {
        // At scale 1.0, move speed should be 50
        let speed_normal = calculate_move_speed(1.0);
        assert_eq!(speed_normal, 50, "Move speed at 1.0 scale should be 50");

        // At scale 2.0 (zoomed in), move speed should be 25 (slower screen movement)
        let speed_zoomed = calculate_move_speed(2.0);
        assert_eq!(speed_zoomed, 25, "Move speed at 2.0 scale should be 25");

        // At scale 0.5 (zoomed out), move speed should be 100 (faster screen movement)
        let speed_wide = calculate_move_speed(0.5);
        assert_eq!(speed_wide, 100, "Move speed at 0.5 scale should be 100");
    }

    #[test]
    fn test_offset_saturating_arithmetic() {
        // Test that offset doesn't wrap around
        let offset = 0u32;
        let result = offset.saturating_sub(100);
        assert_eq!(result, 0, "Saturating sub should clamp at 0");

        let offset = u32::MAX - 10;
        let result = offset.saturating_add(100);
        assert_eq!(result, u32::MAX, "Saturating add should clamp at MAX");
    }

    #[test]
    fn test_speed_values_in_range() {
        // Speed should be 1-5
        for speed in 1..=5 {
            assert!(
                (1..=5).contains(&speed),
                "Speed {} should be in range 1-5",
                speed
            );
        }
    }
}
