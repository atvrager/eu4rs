use anyhow::Result;
use eu4sim_core::modifiers::TradegoodId;
use eu4sim_core::state::{
    Army, CountryState, Date, HashMap as ImHashMap, ProvinceState, Regiment, RegimentType, Terrain,
};
use eu4sim_core::trade::{CountryTradeState, TradeNodeId, TradeNodeState, TradeTopology};
use eu4sim_core::{Fixed, WorldState};
use std::collections::HashMap as StdHashMap;
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

    let mut base_prices = StdHashMap::new();
    let mut name_to_id = StdHashMap::new();

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

    // 2b. Load Trade Network
    log::info!("Loading trade network...");
    let trade_network = eu4data::tradenodes::load_trade_network(game_path)
        .map_err(|e| anyhow::anyhow!("Failed to load trade network: {}", e))?;
    log::info!(
        "Loaded {} trade nodes, {} province mappings",
        trade_network.nodes.len(),
        trade_network.province_to_node.len()
    );

    // Build trade node states and topology
    let mut trade_nodes = StdHashMap::new();
    let mut edges = StdHashMap::new();

    for node_def in &trade_network.nodes {
        // Convert eu4data::tradenodes::TradeNodeId to eu4sim_core::trade::TradeNodeId
        let node_id = TradeNodeId(node_def.id.0);
        trade_nodes.insert(node_id, TradeNodeState::default());

        // Store outgoing edges
        if !node_def.outgoing.is_empty() {
            let outgoing: Vec<TradeNodeId> = node_def
                .outgoing
                .iter()
                .map(|id| TradeNodeId(id.0))
                .collect();
            edges.insert(node_id, outgoing);
        }
    }

    let trade_topology = TradeTopology {
        order: trade_network
            .topological_order
            .iter()
            .map(|id| TradeNodeId(id.0))
            .collect(),
        end_nodes: trade_network
            .end_nodes
            .iter()
            .map(|id| TradeNodeId(id.0))
            .collect(),
        edges,
    };

    // Province to trade node mapping
    let province_trade_node: StdHashMap<u32, TradeNodeId> = trade_network
        .province_to_node
        .iter()
        .map(|(&prov_id, &node_id)| (prov_id, TradeNodeId(node_id.0)))
        .collect();

    // 3. Load Provinces
    log::info!("Loading province history...");
    let (province_history, _) = eu4data::history::load_province_history(game_path)
        .map_err(|e| anyhow::anyhow!("Failed to load provinces: {}", e))?;

    let mut provinces = StdHashMap::new();
    let mut countries = StdHashMap::new();
    let mut country_total_manpower: StdHashMap<String, f32> = StdHashMap::new();
    let mut country_capitals: StdHashMap<String, u32> = StdHashMap::new();

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

        // Create ProvinceState - load historical cores from province history
        // This includes owner cores AND reconquest claims (e.g., France on English Normandy)
        let mut cores = std::collections::HashSet::new();

        // Add all historical cores from game data
        if let Some(ref historical_cores) = hist.add_core {
            for tag in historical_cores {
                cores.insert(tag.clone());
            }
        }

        // Ensure owner always has a core (fallback if not in add_core)
        if let Some(tag) = &hist.owner {
            cores.insert(tag.clone());
        }

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
            institution_presence: ImHashMap::default(),
            trade: Default::default(),
            cores,
            coring_progress: None,
        };
        provinces.insert(id, p.clone());

        // Init country if needed
        if let Some(tag) = p.owner {
            countries.entry(tag).or_insert_with(|| CountryState {
                treasury: Fixed::ZERO,
                manpower: Fixed::ZERO,
                ..Default::default()
            });
        }
    }

    // Second pass: Set country religions and home trade nodes based on capital province
    // (Country history not yet loaded, so we use capital's religion as proxy)
    for (tag, country) in &mut countries {
        if let Some(&capital_id) = country_capitals.get(tag) {
            if let Some(capital) = provinces.get(&capital_id) {
                country.religion = capital.religion.clone();
            }

            // Set home trade node based on capital province
            if let Some(&home_node) = province_trade_node.get(&capital_id) {
                country.trade = CountryTradeState {
                    home_node: Some(home_node),
                    merchants_available: 2, // Starting merchants
                    merchants_total: 2,
                    ..Default::default()
                };
                log::debug!(
                    "{}: capital {} -> home trade node {:?}",
                    tag,
                    capital_id,
                    home_node
                );
            }
        }
    }

    log::info!(
        "Loaded {} provinces, {} countries",
        provinces.len(),
        countries.len()
    );

    // 2b. Initialize Armies
    let mut armies = StdHashMap::new();
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
                    morale: Fixed::from_f32(eu4data::defines::combat::BASE_MORALE),
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
                    general: None,
                    in_battle: None,
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
            provinces: provinces.into(),
            countries: countries.into(),
            base_goods_prices: base_prices.into(),
            modifiers: Default::default(),
            diplomacy: Default::default(),
            global: Default::default(),
            armies: armies.into(),
            next_army_id,
            fleets: ImHashMap::default(),
            next_fleet_id: 1,
            colonies: ImHashMap::default(),
            // Combat system
            generals: ImHashMap::default(),
            next_general_id: 1,
            battles: ImHashMap::default(),
            next_battle_id: 1,
            // Trade system
            trade_nodes: trade_nodes.into(),
            province_trade_node: province_trade_node.into(),
            trade_topology,
        },
        adjacency,
    ))
}
