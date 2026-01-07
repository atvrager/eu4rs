//! World state and province data loading.
//!
//! This module handles loading game data from EU4 files:
//! - World state (provinces, countries, diplomacy)
//! - Province map and definitions
//! - Heightmap for terrain shading
//! - Province center calculations

use crate::dds::load_dds;
use crate::gui;
use std::collections::HashMap;

/// Extracts GUI-displayable country resources from the simulation state.
pub fn extract_country_resources(
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

/// Computes the center point of each province for marker placement.
pub fn compute_province_centers(
    province_map: &image::RgbaImage,
    province_lookup: &Option<eu4data::map::ProvinceLookup>,
) -> HashMap<u32, (u32, u32)> {
    let Some(lookup) = province_lookup else {
        return HashMap::new();
    };

    // Accumulate pixel positions for each province
    let mut sums: HashMap<u32, (u64, u64, u64)> = HashMap::new();

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
/// Returns (world_state, playable_countries, country_colors, localisation).
#[allow(clippy::type_complexity)]
pub fn load_world_state() -> (
    eu4sim_core::WorldState,
    Vec<(String, String, i32)>,
    HashMap<String, [u8; 3]>,
    eu4data::localisation::Localisation,
) {
    use eu4sim_core::state::Date;

    // Try to load from EU4 game path
    if let Some(game_path) = eu4data::path::detect_game_path() {
        log::info!("Loading world state from: {}", game_path.display());

        // Load localization for English
        let mut localisation = eu4data::localisation::Localisation::new();
        let loc_path = game_path.join("localisation");
        if loc_path.exists() {
            match localisation.load_from_dir(&loc_path, "english") {
                Ok(count) => log::info!("Loaded {} localization entries", count),
                Err(e) => log::warn!("Failed to load localization: {}", e),
            }
        }
        let start_date = Date::new(1444, 11, 11);

        // Load country colors from game data
        let country_colors: HashMap<String, [u8; 3]> =
            match eu4data::countries::load_tags(&game_path) {
                Ok(tags) => {
                    let colors: HashMap<String, [u8; 3]> =
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
                    HashMap::new()
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
                let mut country_dev: HashMap<String, i32> = HashMap::new();
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

                return (world, playable, country_colors, localisation);
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
        HashMap::new(),
        eu4data::localisation::Localisation::new(),
    )
}

/// Loads province map, lookup table, and heightmap from EU4 game files.
pub fn load_province_data() -> (
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

/// Loads terrain texture from EU4 game files.
///
/// This loads terrain.bmp (palette texture) and converts it to RGB.
/// The palette colors represent terrain types (green for forest, tan for desert, etc.)
#[allow(dead_code)] // Will be used for RealTerrain map mode
pub fn load_terrain_texture() -> Option<image::RgbaImage> {
    let game_path = eu4data::path::detect_game_path()?;
    let terrain_path = game_path.join("map/terrain.bmp");

    if !terrain_path.exists() {
        log::warn!("Terrain texture not found at: {}", terrain_path.display());
        return None;
    }

    log::info!("Loading terrain texture from: {}", terrain_path.display());
    match image::open(&terrain_path) {
        Ok(img) => {
            // Convert from palette to RGBA
            let rgba = img.to_rgba8();
            log::info!(
                "Loaded terrain texture ({}x{})",
                rgba.width(),
                rgba.height()
            );
            Some(rgba)
        }
        Err(e) => {
            log::warn!("Failed to load terrain texture: {}", e);
            None
        }
    }
}

/// Loads normal map from EU4 game files.
pub fn load_normal_map() -> Option<image::RgbaImage> {
    let game_path = eu4data::path::detect_game_path()?;
    let normal_path = game_path.join("map/world_normal.bmp");

    if !normal_path.exists() {
        log::warn!("Normal map not found at: {}", normal_path.display());
        return None;
    }

    log::info!("Loading normal map from: {}", normal_path.display());
    image::open(&normal_path).ok().map(|img| img.to_rgba8())
}

/// Loads water colormap from EU4 game files.
pub fn load_water_colormap() -> Option<image::RgbaImage> {
    let game_path = eu4data::path::detect_game_path()?;
    let water_path = game_path.join("map/terrain/colormap_water.dds");

    if !water_path.exists() {
        log::warn!("Water colormap not found at: {}", water_path.display());
        return None;
    }

    log::info!("Loading water colormap from: {}", water_path.display());
    load_dds(&water_path).ok()
}

/// Loads seasonal colormap from EU4 game files.
pub fn load_seasonal_colormap(season: &str) -> Option<image::RgbaImage> {
    let game_path = eu4data::path::detect_game_path()?;
    let path = game_path.join(format!("map/terrain/colormap_{}.dds", season));

    if !path.exists() {
        log::warn!("Seasonal colormap not found at: {}", path.display());
        return None;
    }

    log::info!("Loading {} colormap from: {}", season, path.display());
    load_dds(&path).ok()
}

/// Loads terrain indices (raw palette indices) from terrain.bmp.
pub fn load_terrain_indices() -> Option<image::GrayImage> {
    let game_path = eu4data::path::detect_game_path()?;
    let terrain_path = game_path.join("map/terrain.bmp");

    let bytes = std::fs::read(&terrain_path).ok()?;

    // Quick BMP header check
    if bytes.len() < 54 || &bytes[0..2] != b"BM" {
        return None;
    }
    let pixel_offset = u32::from_le_bytes(bytes[10..14].try_into().ok()?) as usize;
    let width = u32::from_le_bytes(bytes[18..22].try_into().ok()?) as u32;
    let height = u32::from_le_bytes(bytes[22..26].try_into().ok()?) as u32;
    let bit_count = u16::from_le_bytes(bytes[28..30].try_into().ok()?);

    if bit_count != 8 {
        log::warn!("terrain.bmp is not 8-bit ({} bits)", bit_count);
        return None;
    }

    let pixel_data = &bytes[pixel_offset..];
    let mut indices = Vec::with_capacity((width * height) as usize);
    let row_size = (width + 3) & !3; // BMP rows are 4-byte aligned

    for y in (0..height).rev() {
        let row_start = (y * row_size) as usize;
        if row_start + width as usize <= pixel_data.len() {
            indices.extend_from_slice(&pixel_data[row_start..row_start + width as usize]);
        }
    }

    if indices.len() != (width * height) as usize {
        log::warn!(
            "terrain.bmp size mismatch: expected {}, got {}",
            width * height,
            indices.len()
        );
        return None;
    }

    image::GrayImage::from_raw(width, height, indices)
}

/// Loads the terrain atlas.
pub fn load_terrain_atlas() -> Option<image::RgbaImage> {
    let game_path = eu4data::path::detect_game_path()?;
    let path = game_path.join("map/terrain/atlas0.dds");
    if !path.exists() {
        return None;
    }
    load_dds(&path).ok()
}

/// Loads the terrain atlas normals.
pub fn load_terrain_atlas_normal() -> Option<image::RgbaImage> {
    let game_path = eu4data::path::detect_game_path()?;
    let path = game_path.join("map/terrain/atlas_normal0.dds");
    if !path.exists() {
        return None;
    }
    load_dds(&path).ok()
}

/// Loads atlas tile mapping.
pub fn load_atlas_tile_mapping() -> [u32; 256] {
    let mut mapping = [0u32; 256];

    // Try to load from terrain.txt if available
    if let Some(graphical_terrain) = eu4data::path::detect_game_path()
        .and_then(|p| eu4data::terrain::load_graphical_terrain(&p).ok())
    {
        log::info!(
            "Loaded {} graphical terrain definitions for atlas mapping",
            graphical_terrain.len()
        );
        for (color_index, name) in graphical_terrain {
            // Heuristic: map common names to known indices in vanilla atlas
            let atlas_index = match name.as_str() {
                "plains" => 0,
                "forest" => 1,
                "hills" => 2,
                "mountain" => 3,
                "woods" => 4,
                "marsh" => 5,
                "desert" => 6,
                "jungle" => 7,
                "steppes" => 8,
                "tundra" => 9,
                "savannah" => 10,
                "tropical_wood" => 1, // Fallback to forest
                "farmland" => 0,      // Fallback to plains
                _ => (color_index % 16) as u32,
            };
            mapping[color_index as usize] = atlas_index;
        }
        return mapping;
    }

    // Default mapping: index maps directly to tile index 0-15
    for (i, item) in mapping.iter_mut().enumerate() {
        *item = (i % 16) as u32;
    }
    mapping
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a minimal WorldState for testing country resource extraction.
    #[allow(clippy::field_reassign_with_default)] // Clearer for test setup
    fn create_test_world_state() -> eu4sim_core::WorldState {
        use eu4sim_core::state::{CountryState, Date, ProvinceState};
        use eu4sim_core::{Fixed, Mod32};

        let mut world = eu4sim_core::WorldState::default();
        world.date = Date::new(1444, 11, 11);

        // Add a test country with some resources
        let mut country = CountryState::default();
        country.treasury = Fixed::from_f32(500.0);
        country.manpower = Fixed::from_f32(25000.0);
        country.stability.set(1);
        country.prestige.set(Fixed::from_f32(50.0));
        country.adm_mana = Fixed::from_f32(100.0);
        country.dip_mana = Fixed::from_f32(75.0);
        country.mil_mana = Fixed::from_f32(150.0);
        // Set income breakdown
        country.income.taxation = Fixed::from_f32(10.0);
        country.income.trade = Fixed::from_f32(5.0);
        country.income.production = Fixed::from_f32(8.0);
        country.income.expenses = Fixed::from_f32(3.0);
        world.countries.insert("TST".to_string(), country);

        // Add a test province owned by TST
        let mut province = ProvinceState::default();
        province.owner = Some("TST".to_string());
        province.base_tax = Mod32::from_f32(3.0);
        province.base_production = Mod32::from_f32(3.0);
        province.base_manpower = Mod32::from_f32(2.0);
        world.provinces.insert(1, province);

        // Add another province for TST
        let mut province2 = ProvinceState::default();
        province2.owner = Some("TST".to_string());
        province2.base_tax = Mod32::from_f32(5.0);
        province2.base_production = Mod32::from_f32(4.0);
        province2.base_manpower = Mod32::from_f32(3.0);
        world.provinces.insert(2, province2);

        world
    }

    #[test]
    fn test_extract_country_resources_found() {
        let world = create_test_world_state();

        let resources = extract_country_resources(&world, "TST");
        assert!(resources.is_some());

        let res = resources.unwrap();
        assert_eq!(res.treasury as i32, 500);
        assert_eq!(res.manpower, 25000);
        assert_eq!(res.stability, 1);
        assert_eq!(res.prestige as i32, 50);
        assert_eq!(res.adm_power, 100);
        assert_eq!(res.dip_power, 75);
        assert_eq!(res.mil_power, 150);

        // Net income = taxation + trade + production - expenses = 10 + 5 + 8 - 3 = 20
        assert_eq!(res.income as i32, 20);

        // Max manpower = sum of province base_manpower * 250
        // Province 1: 2 * 250 = 500
        // Province 2: 3 * 250 = 750
        // Total: 1250
        assert_eq!(res.max_manpower, 1250);
    }

    #[test]
    fn test_extract_country_resources_not_found() {
        let world = create_test_world_state();

        let resources = extract_country_resources(&world, "XXX");
        assert!(resources.is_none());
    }

    #[test]
    fn test_compute_province_centers_empty_lookup() {
        let img = image::RgbaImage::new(100, 100);
        let centers = compute_province_centers(&img, &None);
        assert!(centers.is_empty());
    }

    #[test]
    fn test_compute_province_centers_single_province() {
        // Create a 10x10 image with a single color representing province 1
        let mut img = image::RgbaImage::new(10, 10);
        for pixel in img.pixels_mut() {
            *pixel = image::Rgba([100, 50, 25, 255]);
        }

        // Create lookup with this color -> province 1
        let lookup = eu4data::map::ProvinceLookup {
            by_color: {
                let mut map = std::collections::HashMap::new();
                map.insert((100, 50, 25), 1);
                map
            },
            by_id: {
                let mut map = std::collections::HashMap::new();
                map.insert(
                    1,
                    eu4data::map::ProvinceDefinition {
                        id: 1,
                        r: 100,
                        g: 50,
                        b: 25,
                        name: "Test".to_string(),
                        x: String::new(),
                    },
                );
                map
            },
        };

        let centers = compute_province_centers(&img, &Some(lookup));
        assert_eq!(centers.len(), 1);
        // Center of 10x10 image is (4.5, 4.5) -> (4, 4) when truncated
        let center = centers.get(&1).unwrap();
        assert_eq!(*center, (4, 4));
    }

    #[test]
    fn test_compute_province_centers_multiple_provinces() {
        // Create a 20x10 image: left half is province 1, right half is province 2
        let mut img = image::RgbaImage::new(20, 10);
        for (x, _y, pixel) in img.enumerate_pixels_mut() {
            if x < 10 {
                *pixel = image::Rgba([255, 0, 0, 255]); // Province 1 (red)
            } else {
                *pixel = image::Rgba([0, 255, 0, 255]); // Province 2 (green)
            }
        }

        let lookup = eu4data::map::ProvinceLookup {
            by_color: {
                let mut map = std::collections::HashMap::new();
                map.insert((255, 0, 0), 1);
                map.insert((0, 255, 0), 2);
                map
            },
            by_id: {
                let mut map = std::collections::HashMap::new();
                map.insert(
                    1,
                    eu4data::map::ProvinceDefinition {
                        id: 1,
                        r: 255,
                        g: 0,
                        b: 0,
                        name: "Red".to_string(),
                        x: String::new(),
                    },
                );
                map.insert(
                    2,
                    eu4data::map::ProvinceDefinition {
                        id: 2,
                        r: 0,
                        g: 255,
                        b: 0,
                        name: "Green".to_string(),
                        x: String::new(),
                    },
                );
                map
            },
        };

        let centers = compute_province_centers(&img, &Some(lookup));
        assert_eq!(centers.len(), 2);

        // Province 1: x in [0,9], y in [0,9] -> center (4, 4)
        let center1 = centers.get(&1).unwrap();
        assert_eq!(*center1, (4, 4));

        // Province 2: x in [10,19], y in [0,9] -> center (14, 4)
        let center2 = centers.get(&2).unwrap();
        assert_eq!(*center2, (14, 4));
    }

    #[test]
    fn test_compute_province_centers_unrecognized_colors() {
        // Create image with colors not in the lookup
        let mut img = image::RgbaImage::new(10, 10);
        for pixel in img.pixels_mut() {
            *pixel = image::Rgba([123, 45, 67, 255]);
        }

        // Empty lookup
        let lookup = eu4data::map::ProvinceLookup {
            by_color: std::collections::HashMap::new(),
            by_id: std::collections::HashMap::new(),
        };

        let centers = compute_province_centers(&img, &Some(lookup));
        assert!(centers.is_empty());
    }

    /// Test that load_world_state returns playable countries when game files exist.
    ///
    /// This is an integration test that requires EU4 game files.
    #[test]
    fn test_load_world_state_returns_playable_countries() {
        // Skip if no game path
        if eu4data::path::detect_game_path().is_none() {
            eprintln!(
                "Skipping test_load_world_state_returns_playable_countries: EU4 game files not found"
            );
            return;
        }

        let (world_state, playable_countries, country_colors, localisation) = load_world_state();

        // Verify world state has content
        assert!(
            !world_state.provinces.is_empty(),
            "World state should have provinces"
        );
        assert!(
            !world_state.countries.is_empty(),
            "World state should have countries"
        );

        // Verify playable countries exist
        assert!(
            !playable_countries.is_empty(),
            "Should have playable countries, got 0. World has {} provinces and {} countries.",
            world_state.provinces.len(),
            world_state.countries.len()
        );

        // Verify some expected major nations exist
        let tags: Vec<&str> = playable_countries
            .iter()
            .map(|(t, _, _)| t.as_str())
            .collect();
        assert!(
            tags.contains(&"TUR"),
            "Ottoman Empire (TUR) should be playable. Available: {:?}",
            &tags[..tags.len().min(20)]
        );
        assert!(tags.contains(&"FRA"), "France (FRA) should be playable");
        assert!(tags.contains(&"HAB"), "Austria (HAB) should be playable");

        // Verify country colors loaded
        assert!(!country_colors.is_empty(), "Should have country colors");

        // Verify localization loaded
        assert!(
            localisation.get("HAB_ideas").is_some(),
            "Localization should include HAB_ideas"
        );

        eprintln!(
            "load_world_state verified: {} provinces, {} countries, {} playable, {} colors, loc loaded={}",
            world_state.provinces.len(),
            world_state.countries.len(),
            playable_countries.len(),
            country_colors.len(),
            localisation.get("HAB_ideas").is_some()
        );
    }
}
