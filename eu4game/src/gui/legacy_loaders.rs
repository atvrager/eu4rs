//! Legacy layout loading functions.
//!
//! This module contains helper functions that parse EU4's .gui files
//! and extract layout metadata for speed controls, topbar, and country select.
//! These are gradually being replaced by the generic UI binder system.

use super::country_select::{
    CountrySelectButton, CountrySelectIcon, CountrySelectLayout, CountrySelectText,
};
use super::interner;
use super::layout_types::{
    SpeedControlsIcon, SpeedControlsLayout, SpeedControlsText, TopBarIcon, TopBarLayout,
};
use super::parser::parse_gui_file;
use super::types::{GuiElement, Orientation};
use std::path::Path;

pub(super) fn load_speed_controls_split(
    game_path: &Path,
    interner: &interner::StringInterner,
) -> (SpeedControlsLayout, Option<GuiElement>) {
    let gui_path = game_path.join("interface/speed_controls.gui");

    if !gui_path.exists() {
        log::warn!("speed_controls.gui not found, using defaults");
        return (SpeedControlsLayout::default(), None);
    }

    match parse_gui_file(&gui_path, interner) {
        Ok(db) => {
            // Find the speed_controls window
            let symbol = interner.intern("speed_controls");
            if let Some(root) = db.get(&symbol)
                && let GuiElement::Window {
                    position,
                    orientation,
                    children,
                    ..
                } = root
            {
                let layout = extract_speed_controls_layout(position, orientation, children);
                return (layout, Some(root.clone()));
            }
            log::warn!("speed_controls window not found in GUI file");
            (SpeedControlsLayout::default(), None)
        }
        Err(e) => {
            log::warn!("Failed to parse speed_controls.gui: {}", e);
            (SpeedControlsLayout::default(), None)
        }
    }
}

/// Extract speed controls layout data from parsed GUI elements (rendering metadata only).
fn extract_speed_controls_layout(
    window_pos: &(i32, i32),
    orientation: &Orientation,
    children: &[GuiElement],
) -> SpeedControlsLayout {
    let mut controls = SpeedControlsLayout {
        window_pos: *window_pos,
        orientation: *orientation,
        ..Default::default()
    };

    for child in children {
        match child {
            GuiElement::Icon {
                name,
                sprite_type,
                position,
                orientation,
                ..
            } => {
                if name == "date_bg" || name == "icon_date_bg" {
                    controls.bg_sprite = sprite_type.clone();
                    controls.bg_pos = *position;
                    controls.bg_orientation = *orientation;
                    log::debug!(
                        "Parsed icon_date_bg: pos={:?}, orientation={:?}, sprite={}",
                        position,
                        orientation,
                        sprite_type
                    );
                } else {
                    // Collect additional icons (e.g., icon_score)
                    controls.icons.push(SpeedControlsIcon {
                        name: name.clone(),
                        sprite: sprite_type.clone(),
                        position: *position,
                        orientation: *orientation,
                    });
                    log::debug!(
                        "Parsed icon {}: pos={:?}, orientation={:?}, sprite={}",
                        name,
                        position,
                        orientation,
                        sprite_type
                    );
                }
            }
            GuiElement::TextBox {
                name,
                position,
                orientation,
                max_width,
                max_height,
                font,
                border_size,
                ..
            } => {
                // EU4 uses "DateText" for the date display
                if name == "date" || name == "DateText" {
                    controls.date_pos = *position;
                    controls.date_orientation = *orientation;
                    controls.date_max_width = *max_width;
                    controls.date_max_height = *max_height;
                    controls.date_font = font.clone();
                    controls.date_border_size = *border_size;
                    log::debug!(
                        "Parsed DateText: pos={:?}, orientation={:?}, maxWidth={}, maxHeight={}, font={}, borderSize={:?}",
                        position,
                        orientation,
                        max_width,
                        max_height,
                        font,
                        border_size
                    );
                } else {
                    // Collect additional text labels (e.g., text_score, text_score_rank)
                    controls.texts.push(SpeedControlsText {
                        name: name.clone(),
                        position: *position,
                        font: font.clone(),
                        max_width: *max_width,
                        max_height: *max_height,
                        orientation: *orientation,
                        border_size: *border_size,
                    });
                    log::debug!(
                        "Parsed text {}: pos={:?}, orientation={:?}, font={}",
                        name,
                        position,
                        orientation,
                        font
                    );
                }
            }
            GuiElement::Button {
                name,
                position,
                sprite_type,
                orientation,
                ..
            } => {
                if name == "speed_indicator" {
                    controls.speed_sprite = sprite_type.clone();
                    controls.speed_pos = *position;
                    controls.speed_orientation = *orientation;
                    log::debug!(
                        "Parsed speed_indicator: pos={:?}, orientation={:?}, sprite={}",
                        position,
                        orientation,
                        sprite_type
                    );
                } else {
                    controls.buttons.push((
                        name.clone(),
                        *position,
                        *orientation,
                        sprite_type.clone(),
                    ));
                    log::debug!(
                        "Parsed button {}: pos={:?}, orientation={:?}",
                        name,
                        position,
                        orientation
                    );
                }
            }
            _ => {}
        }
    }

    controls
}

/// Load topbar layout from game files (Phase 3.5: returns layout + root element).
///
/// Returns a tuple of (TopBarLayout, Option<GuiElement>) where:
/// - TopBarLayout contains rendering metadata (icons, backgrounds, positions)
/// - GuiElement is the root window for macro-based text widget binding
pub(super) fn load_topbar_split(
    game_path: &Path,
    interner: &interner::StringInterner,
) -> (TopBarLayout, Option<GuiElement>) {
    let gui_path = game_path.join("interface/topbar.gui");

    if !gui_path.exists() {
        log::warn!("topbar.gui not found, using defaults");
        return (TopBarLayout::default(), None);
    }

    match parse_gui_file(&gui_path, interner) {
        Ok(db) => {
            // Find the topbar window
            let symbol = interner.intern("topbar");
            if let Some(root) = db.get(&symbol)
                && let GuiElement::Window {
                    position,
                    orientation,
                    children,
                    ..
                } = root
            {
                let layout = extract_topbar_layout(position, orientation, children);
                return (layout, Some(root.clone()));
            }
            log::warn!("topbar window not found in GUI file");
            (TopBarLayout::default(), None)
        }
        Err(e) => {
            log::warn!("Failed to parse topbar.gui: {}", e);
            (TopBarLayout::default(), None)
        }
    }
}

/// Extract topbar layout data from parsed GUI elements (Phase 3.5).
///
/// This extracts only rendering metadata (icon positions, backgrounds).
/// Text widgets are handled by the macro-based topbar::TopBar.
fn extract_topbar_layout(
    window_pos: &(i32, i32),
    orientation: &Orientation,
    children: &[GuiElement],
) -> TopBarLayout {
    let mut layout = TopBarLayout {
        window_pos: *window_pos,
        orientation: *orientation,
        ..Default::default()
    };

    // Background icon names - rendered first
    let bg_names = [
        "topbar_upper_left_bg",
        "topbar_upper_left_bg2",
        "topbar_upper_left_bg4",
        "brown_bg",
        "topbar_1",
        "topbar_2",
        "topbar_3",
    ];

    // Resource icon names we want to render
    let icon_names = [
        // Core resources
        "icon_gold",
        "icon_manpower",
        "icon_sailors",
        "icon_stability",
        "icon_prestige",
        "icon_corruption",
        // Monarch power
        "icon_ADM",
        "icon_DIP",
        "icon_MIL",
        // Envoys
        "icon_merchant",
        "icon_settler",
        "icon_diplomat",
        "icon_missionary",
    ];

    for child in children {
        match child {
            GuiElement::Icon {
                name,
                sprite_type,
                position,
                orientation,
                ..
            } => {
                let icon = TopBarIcon {
                    name: name.clone(),
                    sprite: sprite_type.clone(),
                    position: *position,
                    orientation: *orientation,
                };

                if name == "player_shield" {
                    log::debug!(
                        "Parsed player_shield: pos={:?}, sprite={}",
                        position,
                        sprite_type
                    );
                    layout.player_shield = Some(icon);
                } else if bg_names.contains(&name.as_str()) {
                    log::debug!(
                        "Parsed topbar bg {}: pos={:?}, sprite={}",
                        name,
                        position,
                        sprite_type
                    );
                    layout.backgrounds.push(icon);
                } else if icon_names.contains(&name.as_str()) {
                    log::debug!(
                        "Parsed topbar icon {}: pos={:?}, sprite={}",
                        name,
                        position,
                        sprite_type
                    );
                    layout.icons.push(icon);
                }
            }
            GuiElement::Button {
                name,
                sprite_type,
                position,
                orientation,
                ..
            } => {
                // player_shield is a guiButtonType in topbar.gui
                if name == "player_shield" {
                    log::debug!(
                        "Parsed player_shield (button): pos={:?}, sprite={}",
                        position,
                        sprite_type
                    );
                    layout.player_shield = Some(TopBarIcon {
                        name: name.clone(),
                        sprite: sprite_type.clone(),
                        position: *position,
                        orientation: *orientation,
                    });
                } else if icon_names.contains(&name.as_str()) {
                    // Some icons are buttons (like mana icons)
                    log::debug!(
                        "Parsed topbar button-icon {}: pos={:?}, sprite={}",
                        name,
                        position,
                        sprite_type
                    );
                    layout.icons.push(TopBarIcon {
                        name: name.clone(),
                        sprite: sprite_type.clone(),
                        position: *position,
                        orientation: *orientation,
                    });
                }
            }
            // Phase 3.5: Text widgets now handled by macro-based topbar::TopBar
            _ => {}
        }
    }

    log::info!(
        "Loaded topbar layout: {} backgrounds, {} icons, player_shield={}",
        layout.backgrounds.len(),
        layout.icons.len(),
        layout.player_shield.is_some()
    );

    layout
}

/// Load country selection panel layout from frontend.gui (Phase 3.5: returns layout + root element).
///
/// Returns a tuple of (CountrySelectLayout, Option<GuiElement>) where:
/// - CountrySelectLayout contains rendering metadata (window size, icon vectors, text vectors, etc.)
/// - GuiElement is the root window for macro-based widget binding (CountrySelectPanel)
pub(super) fn load_country_select_split(
    game_path: &Path,
    interner: &interner::StringInterner,
) -> (CountrySelectLayout, Option<GuiElement>) {
    let gui_path = game_path.join("interface/frontend.gui");

    if !gui_path.exists() {
        log::warn!("frontend.gui not found, using defaults");
        return (CountrySelectLayout::default(), None);
    }

    match parse_gui_file(&gui_path, interner) {
        Ok(db) => {
            // The structure is: country_selection_panel > ... > singleplayer
            // We search all top-level windows in the database
            for element in db.values() {
                if let Some((layout, root)) = find_singleplayer_window_in_node_split(element) {
                    return (layout, Some(root));
                }
            }
            log::warn!("singleplayer window not found in frontend.gui");
            (CountrySelectLayout::default(), None)
        }
        Err(e) => {
            log::warn!("Failed to parse frontend.gui: {}", e);
            (CountrySelectLayout::default(), None)
        }
    }
}

/// Recursively search for the singleplayer window and return both layout and root (Phase 3.5).
fn find_singleplayer_window_in_node_split(
    element: &GuiElement,
) -> Option<(CountrySelectLayout, GuiElement)> {
    if let GuiElement::Window {
        name,
        position,
        size,
        orientation,
        children,
    } = element
    {
        if name == "singleplayer" {
            let layout = extract_country_select(position, size, orientation, children);
            return Some((layout, element.clone()));
        }
        // Recurse into child windows
        for child in children {
            if let Some(result) = find_singleplayer_window_in_node_split(child) {
                return Some(result);
            }
        }
    }
    None
}

/// Extract country select data from the singleplayer window.
fn extract_country_select(
    window_pos: &(i32, i32),
    window_size: &(u32, u32),
    orientation: &Orientation,
    children: &[GuiElement],
) -> CountrySelectLayout {
    let mut layout = CountrySelectLayout {
        window_pos: *window_pos,
        window_size: *window_size,
        window_orientation: *orientation,
        loaded: true,
        ..Default::default()
    };

    for child in children {
        match child {
            GuiElement::Icon {
                name,
                sprite_type,
                position,
                orientation,
                frame,
                scale,
            } => {
                log::debug!(
                    "Parsed country select icon {}: pos={:?}, sprite={}, scale={}",
                    name,
                    position,
                    sprite_type,
                    scale
                );
                layout.icons.push(CountrySelectIcon {
                    name: name.clone(),
                    sprite: sprite_type.clone(),
                    position: *position,
                    orientation: *orientation,
                    frame: *frame,
                    scale: *scale,
                });
            }
            GuiElement::Button {
                name,
                sprite_type,
                position,
                orientation,
                ..
            } => {
                log::debug!(
                    "Parsed country select button {}: pos={:?}, sprite={}",
                    name,
                    position,
                    sprite_type
                );
                layout.buttons.push(CountrySelectButton {
                    name: name.clone(),
                    sprite: sprite_type.clone(),
                    position: *position,
                    orientation: *orientation,
                });
            }
            GuiElement::TextBox {
                name,
                position,
                font,
                max_width,
                max_height,
                orientation,
                format,
                border_size,
                ..
            } => {
                log::debug!(
                    "Parsed country select text {}: pos={:?}, font={}, format={:?}",
                    name,
                    position,
                    font,
                    format
                );
                layout.texts.push(CountrySelectText {
                    name: name.clone(),
                    position: *position,
                    font: font.clone(),
                    max_width: *max_width,
                    max_height: *max_height,
                    format: *format,
                    orientation: *orientation,
                    border_size: *border_size,
                });
            }
            GuiElement::Window { .. } => {
                // Skip nested windows (like listboxes) for now
            }
            GuiElement::Checkbox { .. } => {
                // Skip checkboxes for now (not used in country select)
            }
            GuiElement::EditBox { .. } => {
                // Skip editboxes for now (not used in country select)
            }
            GuiElement::Listbox { .. } => {
                // Skip listboxes for now (Phase 7 - not yet implemented in country select)
            }
            GuiElement::Scrollbar { .. } => {
                // Skip scrollbars for now (Phase 7 - not yet implemented in country select)
            }
        }
    }

    log::info!(
        "Loaded country select: {} icons, {} texts, {} buttons",
        layout.icons.len(),
        layout.texts.len(),
        layout.buttons.len(),
    );

    layout
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::parser::count_raw_gui_elements;
    use crate::gui::{CountryResources, GuiRenderer, GuiState, SelectedCountryState};
    use crate::render::SpriteRenderer;
    use crate::testing::{HeadlessGpu, assert_snapshot};
    use image::RgbaImage;

    fn get_test_context() -> Option<(HeadlessGpu, std::path::PathBuf)> {
        // Try to get GPU
        let gpu = pollster::block_on(HeadlessGpu::new())?;

        // Try to get game path
        let game_path = eu4data::path::detect_game_path()?;

        Some((gpu, game_path))
    }

    enum RenderMode {
        SpeedControlsOnly,
        TopbarOnly,
    }

    /// Render a specific GUI component to an image for snapshot testing.
    fn render_component_to_image(
        gpu: &HeadlessGpu,
        game_path: &std::path::Path,
        gui_state: &GuiState,
        screen_size: (u32, u32),
        mode: RenderMode,
    ) -> RgbaImage {
        let format = gpu.format;
        let sprite_renderer = SpriteRenderer::new(&gpu.device, format);
        let mut gui_renderer = GuiRenderer::new(game_path);

        // Create offscreen texture
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Test Texture"),
            size: wgpu::Extent3d {
                width: screen_size.0,
                height: screen_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create readback buffer with proper alignment
        // wgpu requires COPY_BYTES_PER_ROW_ALIGNMENT (256 bytes)
        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = bytes_per_pixel * screen_size.0;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
        let buffer_size = (padded_bytes_per_row * screen_size.1) as wgpu::BufferAddress;
        let output_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Readback Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Render
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Test Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Test Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            sprite_renderer.begin_frame();
            match mode {
                RenderMode::SpeedControlsOnly => {
                    gui_renderer.render_speed_controls_only(
                        &mut render_pass,
                        &gpu.device,
                        &gpu.queue,
                        &sprite_renderer,
                        gui_state,
                        screen_size,
                    );
                }
                RenderMode::TopbarOnly => {
                    gui_renderer.render_topbar_only(
                        &mut render_pass,
                        &gpu.device,
                        &gpu.queue,
                        &sprite_renderer,
                        gui_state,
                        screen_size,
                    );
                }
            }
        }

        // Copy to buffer
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &output_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(screen_size.1),
                },
            },
            wgpu::Extent3d {
                width: screen_size.0,
                height: screen_size.1,
                depth_or_array_layers: 1,
            },
        );

        gpu.queue.submit(Some(encoder.finish()));

        // Read back
        let buffer_slice = output_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| tx.send(v).unwrap());
        gpu.device.poll(wgpu::Maintain::Wait);
        rx.recv().unwrap().unwrap();

        let data = buffer_slice.get_mapped_range();

        // Strip row padding if present
        let image = if padded_bytes_per_row != unpadded_bytes_per_row {
            let mut pixels = Vec::with_capacity((unpadded_bytes_per_row * screen_size.1) as usize);
            for row in 0..screen_size.1 {
                let row_start = (row * padded_bytes_per_row) as usize;
                let row_end = row_start + unpadded_bytes_per_row as usize;
                pixels.extend_from_slice(&data[row_start..row_end]);
            }
            RgbaImage::from_raw(screen_size.0, screen_size.1, pixels).unwrap()
        } else {
            RgbaImage::from_raw(screen_size.0, screen_size.1, data.to_vec()).unwrap()
        };

        drop(data);
        output_buffer.unmap();

        image
    }

    /// Render country select panel to an image for snapshot testing.
    fn render_country_select_to_image(
        gpu: &HeadlessGpu,
        game_path: &std::path::Path,
        country_state: &SelectedCountryState,
        screen_size: (u32, u32),
    ) -> RgbaImage {
        let format = gpu.format;
        let sprite_renderer = SpriteRenderer::new(&gpu.device, format);
        let mut gui_renderer = GuiRenderer::new(game_path);

        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Test Texture"),
            size: wgpu::Extent3d {
                width: screen_size.0,
                height: screen_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = bytes_per_pixel * screen_size.0;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
        let buffer_size = (padded_bytes_per_row * screen_size.1) as wgpu::BufferAddress;
        let output_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Readback Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Test Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Test Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            sprite_renderer.begin_frame();
            gui_renderer.render_country_select_only(
                &mut render_pass,
                &gpu.device,
                &gpu.queue,
                &sprite_renderer,
                country_state,
                screen_size,
            );
        }

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &output_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(screen_size.1),
                },
            },
            wgpu::Extent3d {
                width: screen_size.0,
                height: screen_size.1,
                depth_or_array_layers: 1,
            },
        );

        gpu.queue.submit(Some(encoder.finish()));

        let buffer_slice = output_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| tx.send(v).unwrap());
        gpu.device.poll(wgpu::Maintain::Wait);
        rx.recv().unwrap().unwrap();

        let data = buffer_slice.get_mapped_range();

        let image = if padded_bytes_per_row != unpadded_bytes_per_row {
            let mut pixels = Vec::with_capacity((unpadded_bytes_per_row * screen_size.1) as usize);
            for row in 0..screen_size.1 {
                let row_start = (row * padded_bytes_per_row) as usize;
                let row_end = row_start + unpadded_bytes_per_row as usize;
                pixels.extend_from_slice(&data[row_start..row_end]);
            }
            RgbaImage::from_raw(screen_size.0, screen_size.1, pixels).unwrap()
        } else {
            RgbaImage::from_raw(screen_size.0, screen_size.1, data.to_vec()).unwrap()
        };

        drop(data);
        output_buffer.unmap();

        image
    }

    #[test]
    fn test_country_select_snapshot() {
        let Some((gpu, game_path)) = get_test_context() else {
            println!("Skipping test_country_select_snapshot: prerequisites not available");
            return;
        };

        // Austria at game start (1444)
        let austria_state = SelectedCountryState {
            tag: "HAB".to_string(),
            name: "Austria".to_string(),
            government_type: "Archduchy".to_string(),
            fog_status: String::new(), // Visible, not in fog
            government_rank: 2,        // Kingdom
            religion_frame: 0,         // Catholic
            tech_group_frame: 0,       // Western
            ruler_name: "Friedrich III".to_string(),
            ruler_adm: 3,
            ruler_dip: 3,
            ruler_mil: 3,
            adm_tech: 3,
            dip_tech: 3,
            mil_tech: 3,
            ideas_name: "Austrian Ideas".to_string(),
            ideas_unlocked: 0,
            province_count: 6,
            total_development: 70,
            fort_level: 2,
            diplomacy_header: "Diplomacy".to_string(),
        };

        // Render at a size that fits the full panel (content extends to ~400 pixels vertically)
        let screen_size = (450, 800);
        let image = render_country_select_to_image(&gpu, &game_path, &austria_state, screen_size);

        assert_snapshot(&image, "country_select");
    }

    #[test]
    fn test_speed_controls_snapshot() {
        let Some((gpu, game_path)) = get_test_context() else {
            println!("Skipping test_speed_controls_snapshot: prerequisites not available");
            return;
        };

        // Size to fit speed controls panel (centered)
        let screen_size = (512, 256);
        let gui_state = GuiState {
            date: "11 November 1444".to_string(),
            speed: 3,
            paused: false,
            country: None, // Speed controls don't need country data
        };

        let image = render_component_to_image(
            &gpu,
            &game_path,
            &gui_state,
            screen_size,
            RenderMode::SpeedControlsOnly,
        );
        assert_snapshot(&image, "speed_controls");
    }

    #[test]
    fn test_topbar_snapshot() {
        let Some((gpu, game_path)) = get_test_context() else {
            println!("Skipping test_topbar_snapshot: prerequisites not available");
            return;
        };

        // Wide enough for full topbar, short height since it's just the bar
        let screen_size = (1024, 128);
        let gui_state = GuiState {
            date: "11 November 1444".to_string(),
            speed: 1,
            paused: true,
            // Sample country data for Castile at game start
            country: Some(CountryResources {
                treasury: 150.0,
                income: 8.5,
                manpower: 25000,
                max_manpower: 30000,
                sailors: 5000,
                max_sailors: 8000,
                stability: 1,
                prestige: 25.0,
                corruption: 0.0,
                adm_power: 50,
                dip_power: 50,
                mil_power: 50,
                merchants: 2,
                max_merchants: 3,
                colonists: 0,
                max_colonists: 1,
                diplomats: 2,
                max_diplomats: 3,
                missionaries: 1,
                max_missionaries: 2,
            }),
        };

        let image = render_component_to_image(
            &gpu,
            &game_path,
            &gui_state,
            screen_size,
            RenderMode::TopbarOnly,
        );
        assert_snapshot(&image, "topbar");
    }

    #[test]
    fn test_gui_layout_coverage() {
        let Some((_, game_path)) = get_test_context() else {
            println!("Skipping test_gui_layout_coverage: prerequisites not available");
            return;
        };

        let gui_renderer = GuiRenderer::new(&game_path);

        // Check speed controls coverage
        let sc = &gui_renderer.speed_controls_layout;
        assert!(
            !sc.bg_sprite.is_empty(),
            "Background sprite should be loaded"
        );
        assert!(!sc.speed_sprite.is_empty(), "Speed sprite should be loaded");
        assert!(!sc.date_font.is_empty(), "Date font should be specified");
        assert!(!sc.buttons.is_empty(), "Buttons should be parsed");

        println!("Speed controls layout coverage:");
        println!("  Background: {} at {:?}", sc.bg_sprite, sc.bg_pos);
        println!(
            "  Speed indicator: {} at {:?}",
            sc.speed_sprite, sc.speed_pos
        );
        println!("  Date text at {:?}, font: {}", sc.date_pos, sc.date_font);
        println!("  Buttons: {}", sc.buttons.len());
        for (name, pos, _, sprite) in &sc.buttons {
            println!("    - {} at {:?} ({})", name, pos, sprite);
        }

        // Check topbar coverage
        let tb = &gui_renderer.topbar_layout;
        println!("\nTopbar layout coverage:");
        println!(
            "  Window pos: {:?}, orientation: {:?}",
            tb.window_pos, tb.orientation
        );
        println!("  Backgrounds: {}", tb.backgrounds.len());
        for bg in &tb.backgrounds {
            println!("    - {} at {:?} ({})", bg.name, bg.position, bg.sprite);
        }
        println!("  Icons: {}", tb.icons.len());
        for icon in &tb.icons {
            println!(
                "    - {} at {:?} ({})",
                icon.name, icon.position, icon.sprite
            );
        }
        // Note: Text widgets now managed by macro-based TopBar struct (13 widgets)
        println!("  Texts: managed by TopBar (13 widgets)");

        // Assert minimum expected elements
        assert!(
            !tb.backgrounds.is_empty(),
            "Should have at least 1 background"
        );
        assert!(tb.icons.len() >= 5, "Should have at least 5 icons");
    }

    #[test]
    fn test_gui_gap_detection() {
        let Some((_, game_path)) = get_test_context() else {
            println!("Skipping test_gui_gap_detection: prerequisites not available");
            return;
        };

        println!("\n=== GUI Gap Detection Report ===\n");

        // Check speed_controls.gui
        let speed_controls_path = game_path.join("interface/speed_controls.gui");
        if speed_controls_path.exists() {
            let raw_counts = count_raw_gui_elements(&speed_controls_path)
                .expect("Failed to count speed_controls.gui elements");

            let gui_renderer = GuiRenderer::new(&game_path);
            let sc = &gui_renderer.speed_controls_layout;

            // Count what we actually use
            // 1 = background icon, plus any additional icons we parsed
            let used_icons = 1 + sc.icons.len();
            let used_buttons = sc.buttons.len();
            // 1 = date text (macro-based), plus any additional texts we parsed (layout)
            let used_texts = 1 + sc.texts.len();

            println!("speed_controls.gui:");
            println!(
                "  Raw: {} windows, {} icons, {} buttons, {} textboxes",
                raw_counts.windows, raw_counts.icons, raw_counts.buttons, raw_counts.textboxes
            );
            println!(
                "  Used: {} icons, {} buttons, {} texts",
                used_icons, used_buttons, used_texts
            );

            let icon_gap = raw_counts.icons.saturating_sub(used_icons);
            let button_gap = raw_counts.buttons.saturating_sub(used_buttons);
            let text_gap = raw_counts.textboxes.saturating_sub(used_texts);

            if icon_gap > 0 || button_gap > 0 || text_gap > 0 {
                println!(
                    "  GAPS: {} icons, {} buttons, {} textboxes not rendered",
                    icon_gap, button_gap, text_gap
                );
            } else {
                println!("  OK: All elements accounted for");
            }

            if !raw_counts.unknown_types.is_empty() {
                println!(
                    "  Unsupported element types: {:?}",
                    raw_counts.unknown_types
                );
            }
        }

        // Check topbar.gui
        let topbar_path = game_path.join("interface/topbar.gui");
        if topbar_path.exists() {
            let raw_counts =
                count_raw_gui_elements(&topbar_path).expect("Failed to count topbar.gui elements");

            let gui_renderer = GuiRenderer::new(&game_path);
            let tb = &gui_renderer.topbar_layout;

            // Count what we actually use (backgrounds are icons in the raw file)
            let used_icons = tb.backgrounds.len() + tb.icons.len();
            let used_texts = 13; // Macro-based TopBar has 13 text widgets

            println!("\ntopbar.gui:");
            println!(
                "  Raw: {} windows, {} icons, {} buttons, {} textboxes",
                raw_counts.windows, raw_counts.icons, raw_counts.buttons, raw_counts.textboxes
            );
            println!(
                "  Used: {} icons (incl. backgrounds), {} texts (macro-based)",
                used_icons, used_texts
            );

            let icon_gap = raw_counts.icons.saturating_sub(used_icons);
            let text_gap = raw_counts.textboxes.saturating_sub(used_texts);

            if icon_gap > 0 || text_gap > 0 {
                println!(
                    "  GAPS: {} icons, {} textboxes not rendered",
                    icon_gap, text_gap
                );
            } else {
                println!("  OK: All elements accounted for");
            }

            if !raw_counts.unknown_types.is_empty() {
                println!(
                    "  Unsupported element types: {:?}",
                    raw_counts.unknown_types
                );
            }
        }

        println!("\n=== End Gap Detection Report ===\n");

        // This test is informational - it doesn't fail CI
        // But we print the gaps so developers know what's missing
    }

    #[test]
    fn test_country_select_loading() {
        let Some((_, game_path)) = get_test_context() else {
            println!("Skipping test_country_select_loading: prerequisites not available");
            return;
        };

        let interner = interner::StringInterner::new();
        let (layout, _) = load_country_select_split(&game_path, &interner);

        // Verify loading succeeded
        assert!(layout.loaded, "Country select layout should be loaded");

        // Check window position - should be UPPER_RIGHT anchored
        assert_eq!(
            layout.window_orientation,
            Orientation::UpperRight,
            "Window should be UPPER_RIGHT oriented"
        );

        // Check that we parsed some elements (from frontend.gui singleplayer window)
        assert!(
            !layout.icons.is_empty(),
            "Should have parsed at least some icons"
        );
        assert!(
            !layout.texts.is_empty(),
            "Should have parsed at least some text boxes"
        );
        assert!(
            !layout.buttons.is_empty(),
            "Should have parsed at least some buttons"
        );

        // Print what we found for debugging
        println!("\n=== Country Select Layout ===");
        println!(
            "Window: pos={:?}, size={:?}, orientation={:?}",
            layout.window_pos, layout.window_size, layout.window_orientation
        );
        println!("\nIcons ({}):", layout.icons.len());
        for icon in &layout.icons {
            println!(
                "  {}: sprite={}, pos={:?}",
                icon.name, icon.sprite, icon.position
            );
        }
        println!("\nTexts ({}):", layout.texts.len());
        for text in &layout.texts {
            println!(
                "  {}: font={}, pos={:?}, format={:?}",
                text.name, text.font, text.position, text.format
            );
        }
        println!("\nButtons ({}):", layout.buttons.len());
        for button in &layout.buttons {
            println!(
                "  {}: sprite={}, pos={:?}",
                button.name, button.sprite, button.position
            );
        }
        println!("=== End Country Select Layout ===\n");
    }
}
