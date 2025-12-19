use anyhow::Result;
use eu4sim_core::modifiers::TradegoodId;
use eu4sim_core::state::{
    Army, CountryState, Date, ProvinceState, Regiment, RegimentType, Terrain,
};
use eu4sim_core::{Fixed, WorldState};
use std::collections::HashMap;
use std::path::Path;

/// Parse terrain string to Terrain enum
fn parse_terrain(terrain_str: &str) -> Option<Terrain> {
    match terrain_str {
        "plains" | "grasslands" => Some(Terrain::Plains),
        "farmlands" => Some(Terrain::Farmlands),
        "hills" => Some(Terrain::Hills),
        "mountain" | "mountains" => Some(Terrain::Mountains),
        "forest" | "woods" => Some(Terrain::Forest),
        "marsh" | "wetlands" => Some(Terrain::Marsh),
        "jungle" => Some(Terrain::Jungle),
        "desert" | "drylands" => Some(Terrain::Desert),
        "ocean" | "sea" => Some(Terrain::Sea),
        _ => None,
    }
}

pub fn load_initial_state(
    game_path: &Path,
    start_date: Date,
    _rng_seed: u64,
) -> Result<(WorldState, eu4data::adjacency::AdjacencyGraph)> {
    // 0. Load Adjacency Graph (with Strict Cache Validation)
    log::info!("Loading adjacency graph (strict mode)...");
    let adjacency = eu4data::adjacency::load_adjacency_graph(
        game_path,
        eu4data::cache::CacheValidationMode::Strict,
    )
    .map_err(|e| anyhow::anyhow!("Failed to load adjacency graph: {}", e))?;

    // 1. Load Trade Goods
    log::info!("Loading trade goods from {:?}", game_path);
    let tradegoods = eu4data::tradegoods::load_tradegoods(game_path).unwrap_or_default();

    // Sort for deterministic ID assignment
    let mut sorted_goods: Vec<_> = tradegoods.iter().collect();
    sorted_goods.sort_by_key(|(k, _)| *k);

    let mut base_prices = HashMap::new();
    let mut name_to_id = HashMap::new();

    for (idx, (name, data)) in sorted_goods.iter().enumerate() {
        let id = TradegoodId(idx as u16);
        let price = Fixed::from_f32(data.base_price.unwrap_or(0.0));
        base_prices.insert(id, price);
        name_to_id.insert(name.to_string(), id);
        log::debug!("Tradegood {}: {} -> {}", id.0, name, price);
    }
    log::info!("Loaded {} trade goods", base_prices.len());

    // 2. Load Terrain
    log::info!("Loading terrain data...");
    let terrain_map = eu4data::terrain::load_terrain_overrides(game_path)
        .map_err(|e| anyhow::anyhow!("Failed to load terrain: {}", e))?;
    log::info!("Loaded {} terrain overrides", terrain_map.len());

    // 3. Load Provinces
    log::info!("Loading province history...");
    let (province_history, _) = eu4data::history::load_province_history(game_path)
        .map_err(|e| anyhow::anyhow!("Failed to load provinces: {}", e))?;

    let mut provinces = HashMap::new();
    let mut countries = HashMap::new();
    let mut country_total_manpower: HashMap<String, f32> = HashMap::new();
    let mut country_capitals: HashMap<String, u32> = HashMap::new();

    // First pass: Create provinces and identify countries
    for (id, hist) in province_history {
        // Map trade good
        let goods_id = hist
            .trade_goods
            .and_then(|name| name_to_id.get(&name))
            .copied();

        // Accumulate manpower logic
        if let Some(tag) = &hist.owner {
            let mp = hist.base_manpower.unwrap_or(0.0);
            *country_total_manpower.entry(tag.clone()).or_default() += mp;
            // Naive capital: First owned province
            country_capitals.entry(tag.clone()).or_insert(id);
        }

        // Create ProvinceState
        let p = ProvinceState {
            owner: hist.owner.clone(),
            controller: hist.owner.clone(),
            religion: hist.religion.clone(),
            culture: hist.culture.clone(),
            trade_goods_id: goods_id,
            base_tax: Fixed::from_f32(hist.base_tax.unwrap_or(0.0)),
            base_production: Fixed::from_f32(hist.base_production.unwrap_or(0.0)),
            base_manpower: Fixed::from_f32(hist.base_manpower.unwrap_or(0.0)),
            has_fort: hist.fort_15th.unwrap_or(false),
            is_sea: hist.base_tax.is_none()
                && hist.base_production.is_none()
                && hist.owner.is_none(),
            terrain: terrain_map.get(&id).and_then(|s| parse_terrain(s)),
        };
        provinces.insert(id, p.clone());

        // Init country if needed
        if let Some(tag) = p.owner {
            countries.entry(tag).or_insert_with(|| CountryState {
                treasury: Fixed::ZERO,
                manpower: Fixed::ZERO,
                prestige: Fixed::ZERO,
                stability: 0,
                adm_mana: Fixed::ZERO,
                dip_mana: Fixed::ZERO,
                mil_mana: Fixed::ZERO,
            });
        }
    }

    log::info!(
        "Loaded {} provinces, {} countries",
        provinces.len(),
        countries.len()
    );

    // 2b. Initialize Armies
    // Rule: 1 Infantry per 5 Manpower Dev
    let mut armies = HashMap::new();
    let mut next_army_id = 1;
    let reg_strength = Fixed::from_int(1000); // 1000 men

    for (tag, &total_mp) in &country_total_manpower {
        let reg_count = (total_mp / 5.0).floor() as usize;
        if reg_count == 0 {
            continue;
        }

        if let Some(&location) = country_capitals.get(tag) {
            let mut regiments = Vec::with_capacity(reg_count);
            for _ in 0..reg_count {
                regiments.push(Regiment {
                    type_: RegimentType::Infantry,
                    strength: reg_strength,
                });
            }

            let army_id = next_army_id;
            next_army_id += 1;

            armies.insert(
                army_id,
                Army {
                    id: army_id,
                    name: format!("{} Army", tag),
                    owner: tag.clone(),
                    location,
                    regiments,
                    movement: None,
                    embarked_on: None,
                },
            );
        }
    }

    log::info!("Initialized {} armies", armies.len());

    // 3. Assemble State
    Ok((
        WorldState {
            date: start_date,
            rng_seed: _rng_seed,
            rng_state: 0, // Initialize RNG state
            provinces,
            countries,
            base_goods_prices: base_prices,
            modifiers: Default::default(),
            diplomacy: Default::default(),
            global: Default::default(),
            armies,
            next_army_id,
            fleets: Default::default(),
            next_fleet_id: 1,
        },
        adjacency,
    ))
}
