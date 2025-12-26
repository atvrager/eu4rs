use anyhow::Result;
use eu4sim_core::ideas::{IdeaGroupRegistry, RawIdea, RawIdeaGroup};
use eu4sim_core::modifiers::TradegoodId;
use eu4sim_core::state::{
    Army, CountryState, Date, HashMap as ImHashMap, ProvinceState, Regiment, RegimentType,
    SubjectRelationship, Terrain,
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

    let fort_count = provinces.values().filter(|p| p.fort_level > 0).count();
    log::info!(
        "Loaded {} provinces ({} with forts), {} countries",
        provinces.len(),
        fort_count,
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
                    previous_location: None,
                    regiments,
                    movement: None,
                    embarked_on: None,
                    general: None,
                    in_battle: None,
                    // All regiments are infantry at game start
                    infantry_count: reg_count as u32,
                    cavalry_count: 0,
                    artillery_count: 0,
                },
            );
        }
    }

    log::info!("Initialized {} armies", armies.len());

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
            fleets: ImHashMap::default(),
            next_fleet_id: 1,
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
        },
        adjacency,
    ))
}
