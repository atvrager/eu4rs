use ab_glyph::{Font, FontArc, PxScale, ScaleFont, point};
use image::{Rgba, RgbaImage};

pub struct TextRenderer {
    font: FontArc,
}

impl TextRenderer {
    pub fn new(font_data: Vec<u8>) -> Self {
        Self {
            font: FontArc::try_from_vec(font_data).expect("Error loading font"),
        }
    }

    /// Renders multiline text to an image.
    pub fn render(&self, text: &str, width: u32, height: u32) -> RgbaImage {
        let mut image = RgbaImage::new(width, height);
        // Fill with transparent background for proper compositing
        for pixel in image.pixels_mut() {
            *pixel = Rgba([0, 0, 0, 0]);
        }

        let scale = PxScale { x: 24.0, y: 24.0 };
        let scaled_font = self.font.as_scaled(scale);
        let mut y_pos = 10.0;

        for line in text.lines() {
            let ascent = scaled_font.ascent();
            let offset_y = y_pos + ascent;

            let mut x_pos = 10.0;
            for c in line.chars() {
                if c.is_control() {
                    continue;
                }

                let glyph_id = self.font.glyph_id(c);
                let h_advance = scaled_font.h_advance(glyph_id);

                let glyph = glyph_id.with_scale_and_position(scale, point(x_pos, offset_y));
                if let Some(outlined) = self.font.outline_glyph(glyph) {
                    let bounds = outlined.px_bounds();

                    outlined.draw(|x, y, coverage| {
                        let px = bounds.min.x as u32 + x;
                        let py = bounds.min.y as u32 + y;

                        if px < width && py < height {
                            let pixel = image.get_pixel_mut(px, py);
                            let alpha = (coverage * 255.0) as u8;
                            if alpha > 0 {
                                *pixel = Rgba([255, 255, 255, alpha]);
                            }
                        }
                    });
                }
                x_pos += h_advance;
            }
            y_pos += 30.0; // Line height
        }

        image
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_text_render() {
        // Resolve font path relative to package or workspace root
        let font_path = if Path::new("../assets/Roboto-Regular.ttf").exists() {
            Path::new("../assets/Roboto-Regular.ttf")
        } else if Path::new("assets/Roboto-Regular.ttf").exists() {
            Path::new("assets/Roboto-Regular.ttf")
        } else {
            eprintln!(
                "Skipping text test, font not found. CWD: {:?}",
                std::env::current_dir().unwrap()
            );
            return;
        };

        let font_data = std::fs::read(font_path).unwrap();
        let renderer = TextRenderer::new(font_data);

        let img = renderer.render("Province: Stockholm\nOwner: SWE\nGoods: Iron", 400, 200);

        crate::testing::assert_snapshot(&img, "text_render_stockholm");
    }
}
