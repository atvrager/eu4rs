//! OCR extraction from calibrated UI regions.
//!
//! Uses `ocrs` (pure Rust) for text recognition.

use crate::regions::{self, Region};
use anyhow::{Context, Result};
use image::DynamicImage;
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten::Model;
use std::path::Path;

/// OCR extractor using the ocrs engine.
pub struct Extractor {
    engine: OcrEngine,
}

impl Extractor {
    /// Create a new extractor, loading models from the given directory.
    ///
    /// If `model_dir` is None, looks for models in `~/.cache/ocrs/`.
    pub fn new(model_dir: Option<&Path>) -> Result<Self> {
        let cache_dir = model_dir.map(|p| p.to_path_buf()).unwrap_or_else(|| {
            dirs::cache_dir()
                .unwrap_or_else(|| std::env::current_dir().unwrap())
                .join("ocrs")
        });

        let detection_path = cache_dir.join("text-detection.rten");
        let recognition_path = cache_dir.join("text-recognition.rten");

        // Check if models exist
        if !detection_path.exists() || !recognition_path.exists() {
            anyhow::bail!(
                "OCR models not found in {:?}. Run `ocrs-cli` once to download them, \
                or download manually from https://github.com/robertknight/ocrs",
                cache_dir
            );
        }

        let detection_model = Model::load_file(&detection_path)
            .with_context(|| format!("Failed to load {:?}", detection_path))?;
        let recognition_model = Model::load_file(&recognition_path)
            .with_context(|| format!("Failed to load {:?}", recognition_path))?;

        let engine = OcrEngine::new(OcrEngineParams {
            detection_model: Some(detection_model),
            recognition_model: Some(recognition_model),
            ..Default::default()
        })?;

        Ok(Self { engine })
    }

    /// Extract text from a specific region of an image.
    pub fn extract_region(&self, image: &DynamicImage, region: &Region) -> Result<String> {
        // Crop to region
        let cropped = image.crop_imm(region.x, region.y, region.width, region.height);

        // Convert to format ocrs expects
        let rgb = cropped.to_rgb8();
        let dims = rgb.dimensions(); // (u32, u32)
        let source = ImageSource::from_bytes(rgb.as_raw(), dims)?;

        // Prepare and run OCR
        let input = self.engine.prepare_input(source)?;
        let word_rects = self.engine.detect_words(&input)?;
        let line_rects = self.engine.find_text_lines(&input, &word_rects);
        let text = self.engine.recognize_text(&input, &line_rects)?;

        // Join all recognized text
        let result: String = text
            .iter()
            .filter_map(|line| line.as_ref())
            .flat_map(|line| line.words())
            .map(|word| word.to_string())
            .collect::<Vec<_>>()
            .join(" ");

        Ok(result.trim().to_string())
    }

    /// Extract all calibrated regions and return structured data.
    pub fn extract_all(&self, image: &DynamicImage) -> ExtractedState {
        let mut state = ExtractedState::default();

        // Extract each region
        if let Ok(text) = self.extract_region(image, &regions::DATE) {
            state.date = Some(text);
        }
        if let Ok(text) = self.extract_region(image, &regions::TREASURY) {
            state.treasury = parse_number(&text);
        }
        if let Ok(text) = self.extract_region(image, &regions::MANPOWER) {
            state.manpower = parse_suffixed_int(&text);
        }
        if let Ok(text) = self.extract_region(image, &regions::SAILORS) {
            state.sailors = parse_suffixed_int(&text);
        }
        if let Ok(text) = self.extract_region(image, &regions::ADM_MANA) {
            state.adm_mana = parse_int(&text);
        }
        if let Ok(text) = self.extract_region(image, &regions::DIP_MANA) {
            state.dip_mana = parse_int(&text);
        }
        if let Ok(text) = self.extract_region(image, &regions::MIL_MANA) {
            state.mil_mana = parse_int(&text);
        }
        if let Ok(text) = self.extract_region(image, &regions::STABILITY) {
            state.stability = parse_stability(&text);
        }
        if let Ok(text) = self.extract_region(image, &regions::CORRUPTION) {
            state.corruption = parse_number(&text);
        }
        if let Ok(text) = self.extract_region(image, &regions::PRESTIGE) {
            state.prestige = parse_number(&text);
        }
        if let Ok(text) = self.extract_region(image, &regions::GOVT_STRENGTH) {
            state.govt_strength = parse_number(&text);
        }
        if let Ok(text) = self.extract_region(image, &regions::POWER_PROJ) {
            state.power_projection = parse_number(&text);
        }
        if let Ok(text) = self.extract_region(image, &regions::COUNTRY) {
            state.country = Some(text);
        }
        if let Ok(text) = self.extract_region(image, &regions::AGE) {
            state.age = Some(text);
        }

        state
    }
}

/// Parsed game state from OCR.
#[derive(Debug, Default)]
pub struct ExtractedState {
    pub date: Option<String>,
    pub treasury: Option<f32>,
    pub manpower: Option<i32>,
    pub sailors: Option<i32>,
    pub adm_mana: Option<i32>,
    pub dip_mana: Option<i32>,
    pub mil_mana: Option<i32>,
    pub stability: Option<i8>,
    pub corruption: Option<f32>,
    pub prestige: Option<f32>,
    pub govt_strength: Option<f32>,
    pub power_projection: Option<f32>,
    pub country: Option<String>,
    pub age: Option<String>,
}

impl std::fmt::Display for ExtractedState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn fmt_opt<T: std::fmt::Display>(v: &Option<T>) -> String {
            v.as_ref()
                .map(|x| x.to_string())
                .unwrap_or_else(|| "?".into())
        }
        fn fmt_opt_sign(v: &Option<i8>) -> String {
            v.map(|x| {
                if x >= 0 {
                    format!("+{}", x)
                } else {
                    x.to_string()
                }
            })
            .unwrap_or_else(|| "?".into())
        }

        writeln!(f, "Date:            {}", fmt_opt(&self.date))?;
        writeln!(f, "Country:         {}", fmt_opt(&self.country))?;
        writeln!(f, "Age:             {}", fmt_opt(&self.age))?;
        writeln!(f, "Treasury:        {}", fmt_opt(&self.treasury))?;
        writeln!(f, "Manpower:        {}", fmt_opt(&self.manpower))?;
        writeln!(f, "Sailors:         {}", fmt_opt(&self.sailors))?;
        writeln!(f, "ADM Mana:        {}", fmt_opt(&self.adm_mana))?;
        writeln!(f, "DIP Mana:        {}", fmt_opt(&self.dip_mana))?;
        writeln!(f, "MIL Mana:        {}", fmt_opt(&self.mil_mana))?;
        writeln!(f, "Stability:       {}", fmt_opt_sign(&self.stability))?;
        writeln!(f, "Corruption:      {}", fmt_opt(&self.corruption))?;
        writeln!(f, "Prestige:        {}", fmt_opt(&self.prestige))?;
        writeln!(f, "Govt Strength:   {}", fmt_opt(&self.govt_strength))?;
        writeln!(f, "Power Projection:{}", fmt_opt(&self.power_projection))?;
        Ok(())
    }
}

// ============================================================================
// Parsing helpers
// ============================================================================

/// Parse a plain integer from OCR text.
fn parse_int(s: &str) -> Option<i32> {
    let cleaned: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    cleaned.parse().ok()
}

/// Parse a floating point number from OCR text.
fn parse_number(s: &str) -> Option<f32> {
    let cleaned: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    cleaned.parse().ok()
}

/// Parse numbers with k/M suffix: "5.7k" -> 5700, "1.2M" -> 1200000
fn parse_suffixed_int(s: &str) -> Option<i32> {
    let s = s.trim().to_lowercase();

    if let Some(num_str) = s.strip_suffix('k') {
        let num: f32 = num_str
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '.')
            .collect::<String>()
            .parse()
            .ok()?;
        Some((num * 1000.0) as i32)
    } else if let Some(num_str) = s.strip_suffix('m') {
        let num: f32 = num_str
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '.')
            .collect::<String>()
            .parse()
            .ok()?;
        Some((num * 1_000_000.0) as i32)
    } else {
        parse_int(&s)
    }
}

/// Parse stability: "+2", "-1", "2" -> i8
fn parse_stability(s: &str) -> Option<i8> {
    let cleaned: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '-' || *c == '+')
        .collect();
    cleaned.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_int() {
        assert_eq!(parse_int("123"), Some(123));
        assert_eq!(parse_int("-5"), Some(-5));
        assert_eq!(parse_int("abc"), None);
    }

    #[test]
    fn test_parse_number() {
        assert_eq!(parse_number("123.45"), Some(123.45));
        assert_eq!(parse_number("-0.5"), Some(-0.5));
    }

    #[test]
    fn test_parse_suffixed_int() {
        assert_eq!(parse_suffixed_int("5.7k"), Some(5700));
        assert_eq!(parse_suffixed_int("29k"), Some(29000));
        assert_eq!(parse_suffixed_int("1.2M"), Some(1200000));
        assert_eq!(parse_suffixed_int("500"), Some(500));
    }

    #[test]
    fn test_parse_stability() {
        assert_eq!(parse_stability("+2"), Some(2));
        assert_eq!(parse_stability("-1"), Some(-1));
        assert_eq!(parse_stability("3"), Some(3));
    }
}
