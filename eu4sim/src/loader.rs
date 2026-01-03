use anyhow::Result;
use eu4sim_core::ideas::{IdeaGroupRegistry, RawIdea, RawIdeaGroup};
use eu4sim_core::modifiers::TradegoodId;
use eu4sim_core::state::{
    Army, CountryState, Date, Fleet, HashMap as ImHashMap, ProvinceState, Regiment, RegimentType,
    Ship, ShipType, SubjectRelationship, Terrain,
};
use eu4sim_core::subjects::{RawSubjectType, SubjectTypeRegistry};
use eu4sim_core::systems::ideas::{recalculate_idea_modifiers, ModifierStubTracker};
use eu4sim_core::trade::{CountryTradeState, TradeNodeId, TradeNodeState, TradeTopology};
use eu4sim_core::{Fixed, WorldState};
use std::collections::HashMap as StdHashMap;
use std::path::Path;

/// Convert from eu4data's RawIdeaGroup to eu4sim-core's RawIdeaGroup.
fn convert_raw_idea_group(raw: eu4data::ideas::RawIdeaGroup) -> RawIdeaGroup {
    RawIdeaGroup {
        name: raw.name,
        category: raw.category.map(|c| match c {
            eu4data::ideas::RawIdeaCategory::Adm => "ADM".to_string(),
            eu4data::ideas::RawIdeaCategory::Dip => "DIP".to_string(),
            eu4data::ideas::RawIdeaCategory::Mil => "MIL".to_string(),
        }),
        is_free: raw.is_free,
        required_tag: raw.required_tag,
        start_modifiers: raw
            .start_modifiers
            .into_iter()
            .map(|m| (m.key, m.value))
            .collect(),
        bonus_modifiers: raw
            .bonus_modifiers
            .into_iter()
            .map(|m| (m.key, m.value))
            .collect(),
        ideas: raw
            .ideas
            .into_iter()
            .enumerate()
            .map(|(i, idea)| RawIdea {
                name: idea.name,
                position: i as u8,
                modifiers: idea
                    .modifiers
                    .into_iter()
                    .map(|m| (m.key, m.value))
                    .collect(),
            })
            .collect(),
        ai_will_do_factor: raw.ai_will_do_factor,
    }
}

/// Convert from eu4data's RawPolicy to eu4sim-core's PolicyDef.
fn convert_raw_policy(
    raw: eu4data::policies::RawPolicy,
    id: eu4sim_core::systems::PolicyId,
) -> eu4sim_core::systems::PolicyDef {
    use eu4sim_core::ideas::ModifierEntry;
    use eu4sim_core::systems::{PolicyCategory, PolicyDef};

    let category = match raw.category {
        eu4data::policies::RawPolicyCategory::Adm => PolicyCategory::Administrative,
        eu4data::policies::RawPolicyCategory::Dip => PolicyCategory::Diplomatic,
        eu4data::policies::RawPolicyCategory::Mil => PolicyCategory::Military,
    };

    PolicyDef {
        id,
        name: raw.name,
        category,
        idea_group_1: raw.idea_group_1,
        idea_group_2: raw.idea_group_2,
        modifiers: raw
            .modifiers
            .into_iter()
            .map(|m| ModifierEntry {
                key: m.key,
                value: Fixed::from_f32(m.value),
            })
            .collect(),
    }
}

/// Convert from eu4data's RawSubjectType to eu4sim-core's RawSubjectType.
fn convert_raw_subject_type(raw: eu4data::subject_types::RawSubjectType) -> RawSubjectType {
    RawSubjectType {
        name: raw.name,
        copy_from: raw.copy_from,
        count: raw.count,
        joins_overlords_wars: raw.joins_overlords_wars,
        overlord_protects_external: raw.overlord_protects_external,
        can_fight_independence_war: raw.can_fight_independence_war,
        takes_diplo_slot: raw.takes_diplo_slot,
        can_be_integrated: raw.can_be_integrated,
        has_overlords_ruler: raw.has_overlords_ruler,
        is_voluntary: raw.is_voluntary,
        base_liberty_desire: raw.base_liberty_desire,
        liberty_desire_development_ratio: raw.liberty_desire_development_ratio,
        pays_overlord: raw.pays_overlord,
        forcelimit_to_overlord: raw.forcelimit_to_overlord,
    }
}

/// Build estate registry from loaded raw data.
fn build_estate_registry(
    _raw_estates: StdHashMap<String, eu4data::estates::RawEstate>,
    _raw_privileges: Vec<eu4data::estates::RawPrivilege>,
) -> eu4sim_core::estates::EstateRegistry {
    use eu4sim_core::estates::{EstateRegistry, EstateTypeId};

    let registry = EstateRegistry::new(); // Start with hardcoded estates

    // Map estate names to IDs (using hardcoded mapping for now)
    let mut estate_name_to_id = StdHashMap::new();
    estate_name_to_id.insert("estate_nobles", EstateTypeId::NOBLES);
    estate_name_to_id.insert("estate_church", EstateTypeId::CLERGY);
    estate_name_to_id.insert("estate_burghers", EstateTypeId::BURGHERS);
    estate_name_to_id.insert("estate_dhimmi", EstateTypeId::DHIMMI);
    estate_name_to_id.insert("estate_cossacks", EstateTypeId::COSSACKS);
    estate_name_to_id.insert("estate_nomadic_tribes", EstateTypeId::TRIBES);
    estate_name_to_id.insert("estate_jains", EstateTypeId::JAINS);
    estate_name_to_id.insert("estate_maratha", EstateTypeId::MARATHAS);
    estate_name_to_id.insert("estate_rajput", EstateTypeId::RAJPUTS);
    estate_name_to_id.insert("estate_brahmins", EstateTypeId::BRAHMINS);
    estate_name_to_id.insert("estate_eunuchs", EstateTypeId::EUNUCHS);
    estate_name_to_id.insert("estate_janissaries", EstateTypeId::JANISSARIES);
    estate_name_to_id.insert("estate_qizilbash", EstateTypeId::QIZILBASH);
    estate_name_to_id.insert("estate_ghulams", EstateTypeId::GHULAMS);

    // For Phase 2, we're just loading the data but keeping the hardcoded registry
    // Phase 3+ will actually use the loaded modifier data
    log::debug!(
        "Estate registry populated with {} estates",
        registry.estate_count()
    );

    // Suppress unused warning - this mapping will be used in Phase 3
    let _ = estate_name_to_id;

    registry
}

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

    // 3. Load default map (for sea province detection)
    log::info!("Loading default map...");
    let default_map = eu4data::map::load_default_map(game_path)
        .map_err(|e| anyhow::anyhow!("Failed to load default map: {}", e))?;
    // Sea provinces include both sea_starts AND lakes (Caspian Sea, Aral Sea, etc.)
    let sea_provinces: std::collections::HashSet<u32> = default_map
        .sea_starts
        .iter()
        .chain(default_map.lakes.iter())
        .copied()
        .collect();

    // 3b. Load impassable (wasteland) provinces from climate.txt
    let wasteland_provinces = eu4data::climate::load_impassable_provinces(game_path)
        .map_err(|e| anyhow::anyhow!("Failed to load climate data: {}", e))?;
    log::info!(
        "Loaded {} sea provinces ({} seas + {} lakes), {} wastelands",
        sea_provinces.len(),
        default_map.sea_starts.len(),
        default_map.lakes.len(),
        wasteland_provinces.len()
    );

    // 4. Load Provinces
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
            // Note: hist.capital is the city NAME, not whether it's a country capital
            // Country capitals are defined in country history files as province IDs
            // TODO: Load country capitals and give them forts
            fort_level: if hist.fort_15th.unwrap_or(false) {
                1
            } else {
                0
            },
            is_capital: false, // TODO: Set from country history capital field
            is_mothballed: false,
            is_sea: sea_provinces.contains(&id),
            is_wasteland: wasteland_provinces.contains(&id),
            terrain: terrain_map.get(&id).and_then(|s| parse_terrain(s)),
            institution_presence: ImHashMap::default(),
            trade: Default::default(),
            cores,
            coring_progress: None,
            buildings: Default::default(),
            building_construction: None,
            has_port: false, // TODO: Detect from coastal + port buildings
            is_in_hre: hist.hre.unwrap_or(false),
            devastation: Fixed::ZERO,
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

    // 4b. Load Country History (government, religion, tech group, monarch)
    log::info!("Loading country history...");
    let (country_history, (ch_success, ch_fail)) =
        eu4data::history::load_country_history(game_path)
            .map_err(|e| anyhow::anyhow!("Failed to load country history: {}", e))?;
    log::info!(
        "Loaded {} country histories ({} failed)",
        ch_success,
        ch_fail
    );

    // Second pass: Set country data from history, manpower, and home trade nodes
    for (tag, country) in &mut countries {
        // Apply country history data if available
        if let Some(hist) = country_history.get(tag) {
            // Religion from country history (overrides capital-based fallback)
            if hist.religion.is_some() {
                country.religion = hist.religion.clone();
            }
            country.government_rank = hist.government_rank;
            country.technology_group = hist.technology_group.clone();

            // Monarch data
            if let Some(ref monarch) = hist.monarch {
                country.ruler_name = Some(monarch.name.clone());
                country.ruler_dynasty = monarch.dynasty.clone();
                country.ruler_adm = monarch.adm;
                country.ruler_dip = monarch.dip;
                country.ruler_mil = monarch.mil;
            }
        }
        // Calculate total development for this country
        let total_dev: f32 = provinces
            .values()
            .filter(|p| p.owner.as_deref() == Some(tag))
            .map(|p| (p.base_tax + p.base_production + p.base_manpower).to_f32())
            .sum();

        // Initialize manpower using EU4's actual formula:
        // Max pool = sum(base_manpower * 250 * (1 - autonomy)) + 10,000 base
        // We assume ~30% average autonomy since we don't parse autonomy yet
        const AUTONOMY_FACTOR: f32 = 0.70; // (1 - 0.30 average autonomy)
        const MEN_PER_BASE_MP: f32 = 250.0; // EU4's BASE_MP_TO_MANPOWER = 0.25 * 1000
        const BASE_MANPOWER_POOL: f32 = 10_000.0;
        const STARTING_MANPOWER_RATIO: f32 = 0.18; // ~18% of max at game start

        if let Some(&total_base_mp) = country_total_manpower.get(tag) {
            let province_contribution = total_base_mp * MEN_PER_BASE_MP * AUTONOMY_FACTOR;
            let max_pool = province_contribution + BASE_MANPOWER_POOL;
            let starting_pool = max_pool * STARTING_MANPOWER_RATIO;
            country.manpower = Fixed::from_f32(starting_pool);
            log::trace!(
                "{}: max_manpower={:.0}, starting={:.0}",
                tag,
                max_pool,
                starting_pool
            );
        }

        // Initialize starting treasury: ~0.12 ducats per development
        // Based on Ming having 146 ducats with 1220 dev at game start
        const TREASURY_PER_DEV: f32 = 0.12;
        country.treasury = Fixed::from_f32(total_dev * TREASURY_PER_DEV);

        if let Some(&capital_id) = country_capitals.get(tag) {
            // Fallback religion from capital (if not set from country history)
            if country.religion.is_none() {
                if let Some(capital) = provinces.get(&capital_id) {
                    country.religion = capital.religion.clone();
                }
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

    let fort_count = provinces.values().filter(|p| p.fort_level > 0).count();
    log::info!(
        "Loaded {} provinces ({} with forts), {} countries",
        provinces.len(),
        fort_count,
        countries.len()
    );

    // 2b. Initialize Armies
    // EU4 spawns armies at 75% of force limit, distributed across top-dev provinces
    // Army sizes are proportional to province development
    let mut armies = StdHashMap::new();
    let mut next_army_id = 1;
    let reg_strength = Fixed::from_int(1000); // 1000 men

    // Split armies for reasonable stack sizes
    const TARGET_REGIMENTS_PER_ARMY: usize = 25;
    const MAX_ARMIES_PER_COUNTRY: usize = 255;

    // Build mapping of country -> provinces sorted by development (for army distribution)
    let mut country_provinces_by_dev: StdHashMap<String, Vec<(u32, u32)>> = StdHashMap::new();
    for (&prov_id, prov) in &provinces {
        if let Some(ref owner) = prov.owner {
            // Sum development (convert from Fixed to u32 for sorting)
            let dev = (prov.base_tax + prov.base_production + prov.base_manpower).to_f32() as u32;
            country_provinces_by_dev
                .entry(owner.clone())
                .or_default()
                .push((prov_id, dev));
        }
    }
    // Sort each country's provinces by development (descending)
    for provs in country_provinces_by_dev.values_mut() {
        provs.sort_by(|a, b| b.1.cmp(&a.1));
    }

    // Calculate force limit for each country based on development
    // TODO: This is a simplified formula. EU4's actual force limit calculation includes:
    // - Base force limit from government type
    // - Modifiers from ideas, policies, buildings, advisors
    // - Subject contributions
    // - Trade company force limit
    // Verify accuracy via eu4sim-verify hydration tests
    // EU4 formula (simplified): base 6 + ~1 per 10 development
    let mut country_force_limit: StdHashMap<String, usize> = StdHashMap::new();
    for (tag, provs) in &country_provinces_by_dev {
        let total_dev: u32 = provs.iter().map(|(_, dev)| dev).sum();
        let force_limit = 6 + (total_dev as usize / 10);
        country_force_limit.insert(tag.clone(), force_limit);
    }

    for (tag, provs_by_dev) in &country_provinces_by_dev {
        let Some(&force_limit) = country_force_limit.get(tag) else {
            continue;
        };

        // EU4 starts countries at 75% of force limit
        let total_reg_count = (force_limit * 75 / 100).max(1);
        if total_reg_count == 0 {
            continue;
        }

        // Calculate number of armies needed
        let num_armies = total_reg_count
            .div_ceil(TARGET_REGIMENTS_PER_ARMY)
            .clamp(1, MAX_ARMIES_PER_COUNTRY);

        // Take top N provinces by dev for army locations
        let army_locations: Vec<(u32, u32)> =
            provs_by_dev.iter().take(num_armies).copied().collect();

        if army_locations.is_empty() {
            continue;
        }

        // Calculate total dev of army locations for proportional distribution
        let total_army_dev: u32 = army_locations.iter().map(|(_, dev)| dev).sum();

        // Distribute regiments proportionally to development
        let mut assigned_regs = 0usize;
        for (army_idx, &(location, dev)) in army_locations.iter().enumerate() {
            // Last army gets remainder to avoid rounding errors
            let army_reg_count = if army_idx == army_locations.len() - 1 {
                total_reg_count - assigned_regs
            } else {
                let proportion = dev as f64 / total_army_dev as f64;
                (total_reg_count as f64 * proportion).round() as usize
            };
            assigned_regs += army_reg_count;

            if army_reg_count == 0 {
                continue;
            }

            // Mix infantry and cavalry (roughly 15% cavalry)
            let cav_count = (army_reg_count * 15 / 100).max(if army_reg_count > 3 { 1 } else { 0 });
            let inf_count = army_reg_count - cav_count;

            let mut regiments = Vec::with_capacity(army_reg_count);
            for _ in 0..inf_count {
                regiments.push(Regiment {
                    type_: RegimentType::Infantry,
                    strength: reg_strength,
                    morale: Fixed::from_f32(eu4data::defines::combat::BASE_MORALE),
                });
            }
            for _ in 0..cav_count {
                regiments.push(Regiment {
                    type_: RegimentType::Cavalry,
                    strength: reg_strength,
                    morale: Fixed::from_f32(eu4data::defines::combat::BASE_MORALE),
                });
            }

            let army_id = next_army_id;
            next_army_id += 1;

            // Name armies with ordinal for larger countries
            let army_name = if num_armies > 1 {
                format!("{} {} Army", tag, army_idx + 1)
            } else {
                format!("{} Army", tag)
            };

            armies.insert(
                army_id,
                Army {
                    id: army_id,
                    name: army_name,
                    owner: tag.clone(),
                    location,
                    previous_location: None,
                    regiments,
                    movement: None,
                    embarked_on: None,
                    general: None,
                    in_battle: None,
                    infantry_count: inf_count as u32,
                    cavalry_count: cav_count as u32,
                    artillery_count: 0,
                },
            );
        }
    }

    log::info!("Initialized {} armies", armies.len());

    // 2c. Initialize Fleets
    // EU4 spawns fleets at 90% of naval force limit in coastal sea zones
    let mut fleets = StdHashMap::new();
    let mut next_fleet_id = 1u32;

    // Find coastal sea zones for each country (sea provinces adjacent to owned land)
    let mut country_sea_zones: StdHashMap<String, Vec<u32>> = StdHashMap::new();
    for (&prov_id, prov) in &provinces {
        if prov.is_sea {
            continue; // Skip sea provinces themselves
        }
        if let Some(ref owner) = prov.owner {
            // Check neighbors for sea zones
            for neighbor in adjacency.neighbors(prov_id) {
                if sea_provinces.contains(&neighbor) {
                    country_sea_zones
                        .entry(owner.clone())
                        .or_default()
                        .push(neighbor);
                }
            }
        }
    }
    // Deduplicate sea zones per country
    for zones in country_sea_zones.values_mut() {
        zones.sort();
        zones.dedup();
    }

    // Naval force limit: ~1 per 10 coastal development + base 6
    // TODO: Simplified formula, needs verification via hydration
    for (tag, provs_by_dev) in &country_provinces_by_dev {
        let Some(sea_zones) = country_sea_zones.get(tag) else {
            continue; // Landlocked country
        };
        if sea_zones.is_empty() {
            continue;
        }

        // Count coastal development (provinces adjacent to sea)
        let coastal_dev: u32 = provs_by_dev
            .iter()
            .filter(|(prov_id, _)| {
                adjacency
                    .neighbors(*prov_id)
                    .iter()
                    .any(|n| sea_provinces.contains(n))
            })
            .map(|(_, dev)| dev)
            .sum();

        let naval_force_limit = 6 + (coastal_dev as usize / 10);
        let total_ships = (naval_force_limit * 90 / 100).max(1);

        if total_ships == 0 {
            continue;
        }

        // Ship composition: ~40% galleys, ~30% light ships, ~30% transports
        // (No heavies at game start - they're expensive and late-game)
        let galley_count = total_ships * 40 / 100;
        let light_count = total_ships * 30 / 100;
        let transport_count = total_ships - galley_count - light_count;

        // Split into 1-2 fleets based on size
        let num_fleets = if total_ships > 20 { 2 } else { 1 };
        let ships_per_fleet = total_ships / num_fleets;

        // Find sea zones adjacent to highest-dev coastal provinces
        // (provs_by_dev is already sorted by development descending)
        let mut fleet_sea_zones: Vec<u32> = Vec::new();
        for (prov_id, _dev) in provs_by_dev.iter() {
            // Find sea zones adjacent to this province
            for neighbor in adjacency.neighbors(*prov_id) {
                if sea_provinces.contains(&neighbor) && !fleet_sea_zones.contains(&neighbor) {
                    fleet_sea_zones.push(neighbor);
                    if fleet_sea_zones.len() >= num_fleets {
                        break;
                    }
                }
            }
            if fleet_sea_zones.len() >= num_fleets {
                break;
            }
        }
        // Fallback to any sea zone if needed
        if fleet_sea_zones.is_empty() {
            fleet_sea_zones.push(sea_zones[0]);
        }

        for fleet_idx in 0..num_fleets {
            let location = fleet_sea_zones[fleet_idx % fleet_sea_zones.len()];

            // Distribute ships to this fleet
            let fleet_galley = if fleet_idx == 0 { galley_count } else { 0 };
            let fleet_light = if fleet_idx == 0 { 0 } else { light_count };
            let fleet_transport = if fleet_idx == 0 {
                transport_count
            } else {
                ships_per_fleet.saturating_sub(fleet_light)
            };

            let mut ships = Vec::new();
            for _ in 0..fleet_galley {
                ships.push(Ship {
                    type_: ShipType::Galley,
                    hull: Fixed::from_int(100),
                    durability: Fixed::from_int(100),
                });
            }
            for _ in 0..fleet_light {
                ships.push(Ship {
                    type_: ShipType::LightShip,
                    hull: Fixed::from_int(100),
                    durability: Fixed::from_int(100),
                });
            }
            for _ in 0..fleet_transport {
                ships.push(Ship {
                    type_: ShipType::Transport,
                    hull: Fixed::from_int(100),
                    durability: Fixed::from_int(100),
                });
            }

            if ships.is_empty() {
                continue;
            }

            let fleet_id = next_fleet_id;
            next_fleet_id += 1;

            let fleet_name = if num_fleets > 1 {
                format!("{} {} Fleet", tag, fleet_idx + 1)
            } else {
                format!("{} Fleet", tag)
            };

            fleets.insert(
                fleet_id,
                Fleet {
                    id: fleet_id,
                    name: fleet_name,
                    owner: tag.clone(),
                    location,
                    ships,
                    embarked_armies: Vec::new(),
                    movement: None,
                    admiral: None,
                    in_battle: None,
                },
            );
        }
    }

    log::info!("Initialized {} fleets", fleets.len());

    // 5. Load Subject Types
    log::info!("Loading subject types...");
    let raw_subject_types = eu4data::subject_types::load_subject_types(game_path)
        .map_err(|e| anyhow::anyhow!("Failed to load subject types: {}", e))?;
    let subject_types = SubjectTypeRegistry::from_raw(
        raw_subject_types
            .into_values()
            .map(convert_raw_subject_type),
    );
    log::info!(
        "Loaded {} subject types (vassal={:?}, tributary={:?})",
        subject_types.len(),
        subject_types.vassal_id,
        subject_types.tributary_id
    );

    // 5b. Load Idea Groups
    log::info!("Loading idea groups...");
    let raw_idea_groups = eu4data::ideas::load_idea_groups(game_path)
        .map_err(|e| anyhow::anyhow!("Failed to load idea groups: {}", e))?;
    let idea_groups =
        IdeaGroupRegistry::from_raw(raw_idea_groups.into_values().map(convert_raw_idea_group));
    let national_count = idea_groups.national_groups().count();
    let generic_count = idea_groups.generic_groups().count();
    log::info!(
        "Loaded {} idea groups ({} generic, {} national)",
        idea_groups.len(),
        generic_count,
        national_count
    );
    // Initialize national ideas for each country
    // Note: national_ideas_progress starts at 0 - it unlocks as you unlock ideas from picked groups
    for (tag, country) in &mut countries {
        if let Some(national) = idea_groups.national_ideas_for(tag) {
            country.ideas.national_ideas = Some(national.id);
            // Progress starts at 0 - start modifier is free, but individual ideas
            // unlock based on total ideas from picked generic groups
            country.ideas.national_ideas_progress = 0;
        }
    }
    let countries_with_national = countries
        .values()
        .filter(|c| c.ideas.national_ideas.is_some())
        .count();
    log::info!(
        "Initialized national ideas for {} countries",
        countries_with_national
    );

    // 5c. Load Policies
    log::info!("Loading policies...");
    let raw_policies = eu4data::policies::load_policies(game_path)
        .map_err(|e| anyhow::anyhow!("Failed to load policies: {}", e))?;
    let mut policy_registry = eu4sim_core::systems::PolicyRegistry::new();
    for (policy_id, raw_policy) in raw_policies.into_iter().enumerate() {
        let id = eu4sim_core::systems::PolicyId(policy_id as u16);
        let policy_def = convert_raw_policy(raw_policy.1, id);
        policy_registry.register(policy_def);
    }
    log::info!("Loaded {} policies", policy_registry.len());

    // 5c+. Load Event Modifiers
    log::info!("Loading event modifiers...");
    let event_modifiers =
        eu4data::event_modifiers::EventModifiersRegistry::load_from_game(game_path)
            .map_err(|e| anyhow::anyhow!("Failed to load event modifiers: {}", e))?;
    log::info!("Loaded {} event modifiers", event_modifiers.modifiers.len());

    // 5d. Load Estates
    log::info!("Loading estates...");
    let raw_estates = eu4data::estates::load_estates(game_path)
        .map_err(|e| anyhow::anyhow!("Failed to load estates: {}", e))?;
    let raw_privileges = eu4data::estates::load_privileges(game_path)
        .map_err(|e| anyhow::anyhow!("Failed to load privileges: {}", e))?;

    let estate_registry = build_estate_registry(raw_estates, raw_privileges);
    log::info!(
        "Loaded {} estates, {} privileges",
        estate_registry.estate_count(),
        estate_registry.privilege_count()
    );

    // Initialize estate state for each country based on government type
    for (tag, country) in &mut countries {
        country.estates = eu4sim_core::estates::CountryEstateState::new_for_country(
            country.government_type,
            country.religion.as_deref().unwrap_or("catholic"),
            &estate_registry,
        );
        log::debug!(
            "{}: {} estates available",
            tag,
            country.estates.available_estates.len()
        );
    }

    // 6. Load Diplomatic History (subjects, alliances, etc.)
    log::info!("Loading diplomatic history...");
    let diplomacy_entries = eu4data::diplomacy::load_diplomacy_history(game_path)
        .map_err(|e| anyhow::anyhow!("Failed to load diplomacy: {}", e))?;
    let date_str = format!(
        "{}.{}.{}",
        start_date.year, start_date.month, start_date.day
    );
    let active_diplomacy = eu4data::diplomacy::filter_active_at_date(&diplomacy_entries, &date_str);

    // Build subject relationships from diplomacy entries
    let mut subjects: StdHashMap<String, SubjectRelationship> = StdHashMap::new();
    for entry in &active_diplomacy {
        // Determine subject type from relationship
        let subject_type_name = match entry.relation_type.as_str() {
            "vassal" => "vassal",
            "march" => "march",
            "union" => "personal_union",
            "dependency" => entry.subject_type.as_deref().unwrap_or("vassal"),
            _ => continue, // Skip alliances, marriages, etc. for now
        };

        let Some(type_id) = subject_types.id_by_name(subject_type_name) else {
            log::warn!(
                "Unknown subject type '{}' for {} -> {}",
                subject_type_name,
                entry.first,
                entry.second
            );
            continue;
        };

        let relationship = SubjectRelationship {
            overlord: entry.first.clone(),
            subject: entry.second.clone(),
            subject_type: type_id,
            start_date,
            liberty_desire: 0,
            integration_progress: 0,
            integrating: false,
        };

        subjects.insert(entry.second.clone(), relationship);
    }

    log::info!("Loaded {} subject relationships", subjects.len());

    // 7. Apply idea modifiers for all countries
    log::info!("Applying idea modifiers...");
    let stub_tracker = ModifierStubTracker::new();
    let mut modifiers = eu4sim_core::modifiers::GameModifiers::default();

    for (tag, country) in &countries {
        recalculate_idea_modifiers(&mut modifiers, tag, country, &idea_groups, &stub_tracker);
    }

    // Report unimplemented modifiers (useful for roadmap)
    let unimplemented_count = stub_tracker.unimplemented_count();
    if unimplemented_count > 0 {
        log::debug!(
            "Idea modifiers: {} unique unimplemented (stub discovery)",
            unimplemented_count
        );
    }

    // 7b. Calculate policy slots and apply policy modifiers
    log::info!("Calculating policy slots and applying policy modifiers...");

    for (tag, country) in &mut countries {
        // Calculate available policy slots based on completed idea groups
        country.policy_slots = eu4sim_core::systems::calculate_policy_slots(&country.ideas);

        // Apply policy modifiers from enabled policies
        eu4sim_core::systems::apply_policy_modifiers(
            tag,
            &country.enabled_policies,
            &policy_registry,
            &mut modifiers,
        );
    }

    // 8. Assemble State
    Ok((
        WorldState {
            date: start_date,
            rng_seed: _rng_seed,
            rng_state: 0, // Initialize RNG state
            provinces: provinces.into(),
            countries: countries.into(),
            base_goods_prices: base_prices.into(),
            modifiers,
            diplomacy: eu4sim_core::state::DiplomacyState {
                subjects: subjects.into(),
                ..Default::default()
            },
            global: Default::default(),
            armies: armies.into(),
            next_army_id,
            fleets: fleets.into(),
            next_fleet_id,
            colonies: ImHashMap::default(),
            // Combat system
            generals: ImHashMap::default(),
            next_general_id: 1,
            admirals: ImHashMap::default(),
            next_admiral_id: 1,
            battles: ImHashMap::default(),
            next_battle_id: 1,
            naval_battles: ImHashMap::default(),
            next_naval_battle_id: 1,
            sieges: ImHashMap::default(),
            next_siege_id: 1,
            // Trade system
            trade_nodes: trade_nodes.into(),
            province_trade_node: province_trade_node.into(),
            trade_topology,
            // Building system
            building_name_to_id: ImHashMap::default(),
            building_defs: ImHashMap::default(),
            building_upgraded_by: ImHashMap::default(),
            // Subject type system
            subject_types,
            // Idea system
            idea_groups,
            // Policy system
            policies: policy_registry,
            // Event modifier system
            event_modifiers,
            // Government type system
            government_types: eu4sim_core::government::GovernmentRegistry::new(),
            // Estate system
            estates: estate_registry,
        },
        adjacency,
    ))
}
