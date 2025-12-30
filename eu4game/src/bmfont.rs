//! BMFont bitmap font parsing and rendering.
//!
//! Parses EU4's .fnt files (BMFont text format) and loads the corresponding
//! glyph atlas textures for authentic text rendering.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// Information about a single glyph in the font atlas.
#[derive(Debug, Clone, Copy)]
pub struct BmGlyph {
    /// X position in atlas texture.
    pub x: u32,
    /// Y position in atlas texture.
    pub y: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// X offset when rendering.
    pub xoffset: i32,
    /// Y offset when rendering.
    pub yoffset: i32,
    /// How much to advance cursor after this glyph.
    pub xadvance: u32,
}

/// Parsed BMFont data.
#[derive(Debug)]
pub struct BmFont {
    /// Font face name (e.g., "Adobe Garamond Pro").
    #[allow(dead_code)]
    pub face: String,
    /// Font size in points.
    #[allow(dead_code)]
    pub size: u32,
    /// Line height in pixels.
    #[allow(dead_code)]
    pub line_height: u32,
    /// Baseline offset from top.
    #[allow(dead_code)]
    pub base: u32,
    /// Atlas texture width.
    pub scale_w: u32,
    /// Atlas texture height.
    pub scale_h: u32,
    /// Atlas texture filename.
    pub texture_file: String,
    /// Glyph data keyed by character code.
    pub glyphs: HashMap<u32, BmGlyph>,
}

impl BmFont {
    /// Parse a BMFont .fnt file.
    pub fn parse(path: &Path) -> Result<Self, String> {
        let file =
            File::open(path).map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;
        let reader = BufReader::new(file);

        let mut face = String::new();
        let mut size = 0u32;
        let mut line_height = 0u32;
        let mut base = 0u32;
        let mut scale_w = 256u32;
        let mut scale_h = 256u32;
        let mut texture_file = String::new();
        let mut glyphs = HashMap::new();

        for line in reader.lines() {
            let line = line.map_err(|e| format!("Read error: {}", e))?;
            let line = line.trim();

            if line.starts_with("info ") {
                // Parse: info face="Adobe Garamond Pro" size=18 bold=1 ...
                if let Some(f) = extract_quoted("face", line) {
                    face = f;
                }
                if let Some(s) = extract_int("size", line) {
                    size = s as u32;
                }
            } else if line.starts_with("common ") {
                // Parse: common lineHeight=18 base=13 scaleW=256 scaleH=256 ...
                if let Some(lh) = extract_int("lineHeight", line) {
                    line_height = lh as u32;
                }
                if let Some(b) = extract_int("base", line) {
                    base = b as u32;
                }
                if let Some(sw) = extract_int("scaleW", line) {
                    scale_w = sw as u32;
                }
                if let Some(sh) = extract_int("scaleH", line) {
                    scale_h = sh as u32;
                }
            } else if line.starts_with("page ") {
                // Parse: page id=0 file="vic_18.tga"
                if let Some(f) = extract_quoted("file", line) {
                    texture_file = f;
                }
            } else if line.starts_with("char ") {
                // Parse: char id=32 x=170 y=83 width=1 height=0 xoffset=0 yoffset=18 xadvance=4 page=0
                if let Some(id) = extract_int("id", line) {
                    let glyph = BmGlyph {
                        x: extract_int("x", line).unwrap_or(0) as u32,
                        y: extract_int("y", line).unwrap_or(0) as u32,
                        width: extract_int("width", line).unwrap_or(0) as u32,
                        height: extract_int("height", line).unwrap_or(0) as u32,
                        xoffset: extract_int("xoffset", line).unwrap_or(0),
                        yoffset: extract_int("yoffset", line).unwrap_or(0),
                        xadvance: extract_int("xadvance", line).unwrap_or(0) as u32,
                    };
                    glyphs.insert(id as u32, glyph);
                }
            }
        }

        // If no explicit page line, infer texture filename from font filename
        // EU4 fonts use this convention: vic_18.fnt -> vic_18.tga
        if texture_file.is_empty()
            && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
        {
            texture_file = format!("{}.tga", stem);
        }

        if texture_file.is_empty() {
            return Err("Could not determine texture file for font".to_string());
        }

        log::debug!(
            "Parsed BMFont '{}' size={}, {} glyphs, atlas={}x{}, texture={}",
            face,
            size,
            glyphs.len(),
            scale_w,
            scale_h,
            texture_file
        );

        Ok(Self {
            face,
            size,
            line_height,
            base,
            scale_w,
            scale_h,
            texture_file,
            glyphs,
        })
    }

    /// Get glyph info for a character.
    pub fn get_glyph(&self, c: char) -> Option<&BmGlyph> {
        self.glyphs.get(&(c as u32))
    }

    /// Calculate UV coordinates for a glyph.
    pub fn glyph_uv(&self, glyph: &BmGlyph) -> (f32, f32, f32, f32) {
        let u_min = glyph.x as f32 / self.scale_w as f32;
        let v_min = glyph.y as f32 / self.scale_h as f32;
        let u_max = (glyph.x + glyph.width) as f32 / self.scale_w as f32;
        let v_max = (glyph.y + glyph.height) as f32 / self.scale_h as f32;
        (u_min, v_min, u_max, v_max)
    }

    /// Measure text width in pixels.
    pub fn measure_width(&self, text: &str) -> f32 {
        text.chars()
            .filter_map(|c| self.get_glyph(c))
            .map(|g| g.xadvance as f32)
            .sum()
    }
}

/// Extract a quoted string value from a BMFont line.
/// e.g., extract_quoted("face", "info face=\"Arial\" size=12") -> Some("Arial")
fn extract_quoted(key: &str, line: &str) -> Option<String> {
    let pattern = format!("{}=\"", key);
    let start = line.find(&pattern)? + pattern.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Extract an integer value from a BMFont line.
/// e.g., extract_int("size", "info face=\"Arial\" size=12") -> Some(12)
fn extract_int(key: &str, line: &str) -> Option<i32> {
    let pattern = format!("{}=", key);
    let start = line.find(&pattern)? + pattern.len();
    let rest = &line[start..];
    // Find end of number (space or end of string)
    let end = rest.find(' ').unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// Cached bitmap font with loaded GPU texture.
pub struct BitmapFontCache {
    /// Base path for font files.
    fonts_path: PathBuf,
    /// Loaded fonts keyed by font name (e.g., "vic_18").
    fonts: HashMap<String, LoadedFont>,
}

/// A loaded bitmap font ready for rendering.
pub struct LoadedFont {
    /// Parsed font data.
    pub font: BmFont,
    /// GPU texture (kept alive for bind group lifetime).
    #[allow(dead_code)]
    pub texture: wgpu::Texture,
    /// Texture view.
    pub view: wgpu::TextureView,
}

impl BitmapFontCache {
    /// Create a new font cache.
    pub fn new(game_path: &Path) -> Self {
        Self {
            fonts_path: game_path.join("gfx/fonts"),
            fonts: HashMap::new(),
        }
    }

    /// Get or load a font by name (e.g., "vic_18").
    pub fn get(
        &mut self,
        font_name: &str,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<&LoadedFont> {
        // Check if already loaded
        if self.fonts.contains_key(font_name) {
            return self.fonts.get(font_name);
        }

        // Try to load
        let fnt_path = self.fonts_path.join(format!("{}.fnt", font_name));
        if !fnt_path.exists() {
            log::warn!("Font file not found: {}", fnt_path.display());
            return None;
        }

        match BmFont::parse(&fnt_path) {
            Ok(font) => {
                // Load the texture atlas
                let texture_path = self.fonts_path.join(&font.texture_file);
                match self.load_font_texture(device, queue, &texture_path, &font) {
                    Ok((texture, view)) => {
                        log::info!(
                            "Loaded bitmap font '{}': {} glyphs, atlas {}x{}",
                            font_name,
                            font.glyphs.len(),
                            font.scale_w,
                            font.scale_h
                        );
                        self.fonts.insert(
                            font_name.to_string(),
                            LoadedFont {
                                font,
                                texture,
                                view,
                            },
                        );
                        self.fonts.get(font_name)
                    }
                    Err(e) => {
                        log::warn!("Failed to load font texture: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to parse font {}: {}", font_name, e);
                None
            }
        }
    }

    /// Load font atlas texture.
    fn load_font_texture(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: &Path,
        font: &BmFont,
    ) -> Result<(wgpu::Texture, wgpu::TextureView), String> {
        // Try .tga first, then .dds
        let path = if path.exists() {
            path.to_path_buf()
        } else {
            // Try alternative extension
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let parent = path.parent().unwrap_or(Path::new(""));
            let dds_path = parent.join(format!("{}.dds", stem));
            if dds_path.exists() {
                dds_path
            } else {
                return Err(format!("Font texture not found: {}", path.display()));
            }
        };

        // Load image
        let img = image::open(&path)
            .map_err(|e| format!("Failed to load {}: {}", path.display(), e))?
            .to_rgba8();

        let (width, height) = img.dimensions();

        // Verify dimensions match font definition
        if width != font.scale_w || height != font.scale_h {
            log::warn!(
                "Font texture size {}x{} doesn't match font definition {}x{}",
                width,
                height,
                font.scale_w,
                font.scale_h
            );
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("BMFont Atlas"),
            size: wgpu::Extent3d {
                width,
                height,
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
            &img,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Ok((texture, view))
    }
}
