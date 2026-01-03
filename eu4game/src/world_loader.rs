//! World state and province data loading.
//!
//! This module handles loading game data from EU4 files:
//! - World state (provinces, countries, diplomacy)
//! - Province map and definitions
//! - Heightmap for terrain shading
//! - Province center calculations

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
/// Returns (world_state, playable_countries, country_colors).
#[allow(clippy::type_complexity)]
pub fn load_world_state() -> (
    eu4sim_core::WorldState,
    Vec<(String, String, i32)>,
    HashMap<String, [u8; 3]>,
) {
    use eu4sim_core::state::Date;

    // Try to load from EU4 game path
    if let Some(game_path) = eu4data::path::detect_game_path() {
        log::info!("Loading world state from: {}", game_path.display());
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
        HashMap::new(),
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
