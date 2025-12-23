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

            let move_speed = (50.0 / self.scale) as u32;
            match key.code {
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
        Ok(())
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
        return Color::Indexed(17); // Dark blue (ocean/unknown)
    }

    let Some(prov) = state.provinces.get(&prov_id) else {
        // Province not in state - use ID-based color
        return id_to_color(prov_id);
    };

    if prov.is_sea {
        return Color::Indexed(18); // Darker blue for sea
    }

    match &prov.owner {
        Some(tag) => tag_to_color(tag),
        None => Color::Indexed(240), // Gray for uncolonized
    }
}

fn id_to_color(id: u32) -> Color {
    // Map province ID to 256-color palette (16-231 are the color cube)
    let idx = 16 + ((id as u8) % 216);
    Color::Indexed(idx)
}

fn tag_to_color(tag: &str) -> Color {
    let mut hasher = DefaultHasher::new();
    tag.hash(&mut hasher);
    let hash = hasher.finish();
    // Use color cube: 16-231 (216 colors)
    let idx = 16 + ((hash % 216) as u8);
    Color::Indexed(idx)
}
