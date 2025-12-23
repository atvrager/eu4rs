use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use eu4sim_core::observer::console::ConsoleObserver;
use eu4sim_core::observer::datagen::DataGenObserver;
use eu4sim_core::observer::event_log::EventLogObserver;
use eu4sim_core::state::Date;
use eu4sim_core::{step_world, ObserverRegistry, PlayerInputs, SimConfig, Snapshot, WorldState};
use rayon::prelude::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

mod loader;

/// Create minimal mock state for CI testing (no game files needed)
fn create_mock_state(seed: u64) -> (WorldState, eu4data::adjacency::AdjacencyGraph) {
    use eu4sim_core::state::{Army, CountryState, ProvinceState, Regiment, RegimentType, Terrain};
    use eu4sim_core::{BoundedFixed, BoundedInt, Fixed};

    // Create 3 mock countries with adjacent provinces
    let mut provinces = std::collections::HashMap::new();
    let mut countries = std::collections::HashMap::new();
    let mut armies = std::collections::HashMap::new();

    // Country AAA: Provinces 1, 2
    countries.insert(
        "AAA".to_string(),
        CountryState {
            treasury: Fixed::from_int(100),
            manpower: Fixed::from_int(10),
            stability: BoundedInt::new(1, -3, 3),
            prestige: BoundedFixed::new(
                Fixed::from_int(50),
                Fixed::from_int(-100),
                Fixed::from_int(100),
            ),
            adm_mana: Fixed::from_int(100),
            dip_mana: Fixed::from_int(100),
            mil_mana: Fixed::from_int(100),
            religion: Some("catholic".to_string()),
            ..Default::default()
        },
    );

    // Country BBB: Provinces 3, 4
    countries.insert(
        "BBB".to_string(),
        CountryState {
            treasury: Fixed::from_int(80),
            manpower: Fixed::from_int(8),
            stability: BoundedInt::new(0, -3, 3),
            prestige: BoundedFixed::new(
                Fixed::from_int(30),
                Fixed::from_int(-100),
                Fixed::from_int(100),
            ),
            adm_mana: Fixed::from_int(50),
            dip_mana: Fixed::from_int(50),
            mil_mana: Fixed::from_int(50),
            religion: Some("sunni".to_string()),
            ..Default::default()
        },
    );

    // Country CCC: Province 5
    countries.insert(
        "CCC".to_string(),
        CountryState {
            treasury: Fixed::from_int(50),
            manpower: Fixed::from_int(5),
            stability: BoundedInt::new(-1, -3, 3),
            prestige: BoundedFixed::new(
                Fixed::from_int(10),
                Fixed::from_int(-100),
                Fixed::from_int(100),
            ),
            adm_mana: Fixed::from_int(20),
            dip_mana: Fixed::from_int(20),
            mil_mana: Fixed::from_int(20),
            religion: Some("orthodox".to_string()),
            ..Default::default()
        },
    );

    // Create provinces (1-2: AAA, 3-4: BBB, 5: CCC)
    for id in 1..=5 {
        let owner = match id {
            1 | 2 => "AAA",
            3 | 4 => "BBB",
            5 => "CCC",
            _ => unreachable!(),
        };
        provinces.insert(
            id,
            ProvinceState {
                owner: Some(owner.to_string()),
                controller: Some(owner.to_string()),
                religion: countries[owner].religion.clone(),
                culture: Some("test_culture".to_string()),
                trade_goods_id: None,
                base_tax: Fixed::from_int(5),
                base_production: Fixed::from_int(5),
                base_manpower: Fixed::from_int(3),
                has_fort: id == 1 || id == 3,
                is_sea: false,
                terrain: Some(Terrain::Plains),
                institution_presence: Default::default(),
                trade: Default::default(),
            },
        );
    }

    // Create one army per country
    armies.insert(
        1,
        Army {
            id: 1,
            name: "AAA Army".to_string(),
            owner: "AAA".to_string(),
            location: 1,
            regiments: vec![Regiment {
                type_: RegimentType::Infantry,
                strength: Fixed::from_int(1000),
            }],
            movement: None,
            embarked_on: None,
        },
    );
    armies.insert(
        2,
        Army {
            id: 2,
            name: "BBB Army".to_string(),
            owner: "BBB".to_string(),
            location: 3,
            regiments: vec![Regiment {
                type_: RegimentType::Infantry,
                strength: Fixed::from_int(1000),
            }],
            movement: None,
            embarked_on: None,
        },
    );

    // Create adjacency: 1-2 (AAA internal), 2-3 (AAA-BBB border), 3-4 (BBB internal), 4-5 (BBB-CCC border)
    let mut adj = eu4data::adjacency::AdjacencyGraph::new();
    adj.add_adjacency(1, 2);
    adj.add_adjacency(2, 3);
    adj.add_adjacency(3, 4);
    adj.add_adjacency(4, 5);

    let state = WorldState {
        date: Date::new(1444, 11, 11),
        rng_seed: seed,
        rng_state: 0,
        provinces: provinces.into(),
        countries: countries.into(),
        base_goods_prices: Default::default(),
        modifiers: Default::default(),
        diplomacy: Default::default(),
        global: Default::default(),
        armies: armies.into(),
        next_army_id: 3,
        fleets: Default::default(),
        next_fleet_id: 1,
        colonies: Default::default(),
        // Trade system
        trade_nodes: Default::default(),
        province_trade_node: Default::default(),
        trade_topology: Default::default(),
    };

    (state, adj)
}

/// Calculate top N countries by total development
fn calculate_top_countries(state: &WorldState, count: usize) -> HashSet<String> {
    let mut dev_by_country: HashMap<String, i64> = HashMap::new();
    for prov in state.provinces.values() {
        if let Some(owner) = &prov.owner {
            let dev = prov.base_tax.0 + prov.base_production.0 + prov.base_manpower.0;
            *dev_by_country.entry(owner.clone()).or_default() += dev;
        }
    }

    let mut ranked: Vec<_> = dev_by_country.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1));
    ranked.into_iter().take(count).map(|(t, _)| t).collect()
}

/// Reassign AIs based on current great power rankings
/// Returns true if any changes were made
fn reassign_hybrid_ais(
    ais: &mut BTreeMap<String, Box<dyn eu4sim_core::AiPlayer>>,
    state: &WorldState,
    greedy_count: usize,
    base_seed: u64,
) -> bool {
    let new_greedy = calculate_top_countries(state, greedy_count);

    // Extract LlmAi if present (we'll reassign it to a random GP)
    let llm_ai: Option<(String, Box<dyn eu4sim_core::AiPlayer>)> = {
        let llm_tag = ais
            .iter()
            .find(|(_, ai)| ai.name() == "LlmAi")
            .map(|(t, _)| t.clone());
        llm_tag.map(|tag| (tag.clone(), ais.remove(&tag).unwrap()))
    };

    // Pick a deterministic "random" GP for the LlmAi using seed + year
    let llm_target: Option<String> = if llm_ai.is_some() && !new_greedy.is_empty() {
        let greedy_vec: Vec<_> = new_greedy.iter().cloned().collect();
        let idx = (base_seed.wrapping_add(state.date.year as u64) as usize) % greedy_vec.len();
        Some(greedy_vec[idx].clone())
    } else {
        None
    };

    // Find current greedy tags (excluding LlmAi which we extracted)
    let mut changes = Vec::new();

    for (tag, ai) in ais.iter() {
        let is_greedy = ai.name() == "GreedyAI";
        // Should be greedy if in new_greedy AND not the LlmAi target
        let should_be_greedy = new_greedy.contains(tag) && llm_target.as_ref() != Some(tag);

        if is_greedy != should_be_greedy {
            changes.push((tag.clone(), should_be_greedy));
        }
    }

    // Handle new countries that don't have an AI yet
    for tag in state.countries.keys() {
        if !ais.contains_key(tag) && llm_target.as_ref() != Some(tag) {
            let should_be_greedy = new_greedy.contains(tag);
            changes.push((tag.clone(), should_be_greedy));
        }
    }

    // Remove dead countries
    let dead_tags: Vec<String> = ais
        .keys()
        .filter(|t| !state.countries.contains_key(*t))
        .cloned()
        .collect();
    for tag in &dead_tags {
        ais.remove(tag);
    }

    // Track if LlmAi needs reassignment
    let llm_changed = llm_ai
        .as_ref()
        .map(|(old_tag, _)| llm_target.as_ref() != Some(old_tag))
        .unwrap_or(false);

    if changes.is_empty() && dead_tags.is_empty() && !llm_changed {
        // Put LlmAi back if no changes needed
        if let Some((tag, ai)) = llm_ai {
            ais.insert(tag, ai);
        }
        return false;
    }

    // Apply changes for GreedyAI/RandomAi
    for (tag, should_be_greedy) in changes {
        let ai: Box<dyn eu4sim_core::AiPlayer> = if should_be_greedy {
            Box::new(eu4sim_core::GreedyAI::new())
        } else {
            let tag_hash: u64 = tag.as_bytes().iter().map(|&b| b as u64).sum();
            let seed = base_seed.wrapping_add(tag_hash);
            Box::new(eu4sim_core::RandomAi::new(seed))
        };
        ais.insert(tag, ai);
    }

    // Reassign LlmAi to the randomly chosen GP
    if let Some((old_tag, llm)) = llm_ai {
        if let Some(ref target_tag) = llm_target {
            ais.insert(target_tag.clone(), llm);
            if old_tag != *target_tag {
                eprintln!("LlmAi transferred: {} → {}", old_tag, target_tag);
            }
        } else {
            // No valid GP, put it back where it was
            ais.insert(old_tag, llm);
        }
    }

    eprintln!("AI pool updated: GreedyAI → {:?}", new_greedy);
    true
}

struct RawModeGuard;

impl RawModeGuard {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to game data
    #[arg(long, default_value_t = eu4data::path::detect_game_path()
        .and_then(|p| p.to_str().map(String::from))
        .unwrap_or_else(|| ".".to_string()))]
    game_path: String,

    /// Start year
    #[arg(long, default_value_t = 1444)]
    start_year: i32,

    /// Number of ticks to run
    #[arg(short, long, default_value_t = 10)]
    ticks: u32,

    /// Log level (error, warn, info, debug, trace)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Dump game data manifest and exit
    #[arg(long)]
    manifest: bool,

    /// Print timing summary at end
    #[arg(long, help = "Print timing summary at end")]
    benchmark: bool,

    /// Comma-separated list of country tags to observe (default: "SWE")
    #[arg(long, default_value = "SWE")]
    tags: String,

    /// Run in observer mode (AI controls all countries)
    #[arg(long)]
    observer: bool,

    /// Initial simulation speed (1-5)
    #[arg(long, default_value_t = 5)]
    speed: u64,

    /// Write event log JSONL to file (use "-" for stdout)
    #[arg(long)]
    event_log: Option<String>,

    /// Write training data to file (requires --observer). Use ".cpb.zip" for binary (recommended), ".zip" for JSON archive, or ".jsonl" for streaming
    #[arg(long)]
    datagen: Option<String>,

    /// AI type: "random", "greedy", or "hybrid" (default)
    #[arg(long, default_value = "hybrid")]
    ai: String,

    /// Number of top countries (by development) to use GreedyAI in hybrid mode
    #[arg(long, default_value_t = 8)]
    greedy_count: usize,

    /// Headless mode: disable TUI, keyboard input, and console observer
    #[arg(long)]
    headless: bool,

    /// Random seed for simulation reproducibility
    #[arg(long, default_value_t = 12345)]
    seed: u64,

    /// Test mode: use minimal mock state instead of loading game files (for CI)
    #[arg(long)]
    test_mode: bool,

    /// Use LLM AI for the top Great Power. Provide path to LoRA adapter directory.
    /// Downloads base model from HuggingFace on first run (~700MB).
    #[arg(long, value_name = "ADAPTER_PATH")]
    llm_ai: Option<PathBuf>,

    /// Base model for LLM AI (default: SmolLM2-360M).
    /// Options: "smollm" (HuggingFaceTB/SmolLM2-360M), "gemma3" (google/gemma-3-270m)
    #[arg(long, value_name = "MODEL")]
    llm_ai_base: Option<String>,
}

use eu4sim_core::SimMetrics;

fn main() -> Result<()> {
    // Load .env file for HF_TOKEN and other config
    let _ = dotenvy::dotenv();

    let args = Args::parse();

    if args.manifest {
        println!("{}", eu4data::manifest::GAME_MANIFEST.dump());
        return Ok(());
    }

    let log_level = if (args.observer || args.benchmark) && args.log_level == "info" {
        "warn"
    } else {
        &args.log_level
    };

    let level = std::str::FromStr::from_str(log_level).unwrap_or(log::LevelFilter::Info);
    env_logger::Builder::new()
        .filter_level(level)
        .format_timestamp(None)
        .init();

    log::info!("Starting eu4sim...");

    // Initialize State (either from game files or mock data for CI)
    let (mut state, adjacency_raw) = if args.test_mode {
        log::info!("Test mode: using mock state");
        create_mock_state(args.seed)
    } else {
        let game_path = PathBuf::from(&args.game_path);
        loader::load_initial_state(&game_path, Date::new(args.start_year, 11, 11), args.seed)?
    };
    let adjacency = Arc::new(adjacency_raw);

    log::info!("Initial State Date: {}", state.date);

    // Simulation config (monthly checksums)
    let config = SimConfig::default();

    let mut metrics = if args.benchmark {
        Some(SimMetrics::default())
    } else {
        None
    };

    // Initialize AI if in observer mode
    // Use BTreeMap for deterministic iteration order
    let mut ais: BTreeMap<String, Box<dyn eu4sim_core::AiPlayer>> = if args.observer {
        log::info!("Using AI: {}", args.ai);

        // In hybrid mode, include 1 LLM AI unless datagen is enabled (training data purity)
        let use_llm = (args.ai == "hybrid" || args.llm_ai.is_some() || args.llm_ai_base.is_some())
            && args.datagen.is_none(); // Skip LLM when generating training data

        // When LLM is enabled: LLM gets #1 country, GreedyAI gets next greedy_count
        // When LLM is disabled: GreedyAI gets top greedy_count countries
        // This ensures greedy_count specifies exactly how many GreedyAIs we get
        let llm_tag: Option<String> = if use_llm {
            let top = calculate_top_countries(&state, 1);
            top.into_iter().next()
        } else {
            None
        };

        // Determine which tags get GreedyAI (excluding the LLM tag if present)
        let greedy_tags: HashSet<String> = match args.ai.as_str() {
            "greedy" => state.countries.keys().cloned().collect(),
            "hybrid" => {
                // Get top (greedy_count + 1 if LLM) countries, then exclude LLM tag
                let extra = if use_llm { 1 } else { 0 };
                let mut top = calculate_top_countries(&state, args.greedy_count + extra);
                if let Some(ref llm) = llm_tag {
                    top.remove(llm);
                }
                eprintln!(
                    "Hybrid mode: {} GreedyAI + {} LLM = {} smart AIs: {:?}",
                    top.len(),
                    if llm_tag.is_some() { 1 } else { 0 },
                    top.len() + if llm_tag.is_some() { 1 } else { 0 },
                    top
                );
                top
            }
            _ => HashSet::new(), // random mode: no greedy
        };

        // Initialize LLM AI (in hybrid mode or if explicitly requested, but NOT for datagen)
        let llm_ai: Option<Box<dyn eu4sim_core::AiPlayer>> = if use_llm {
            // Resolve base model name to HuggingFace repo
            let base_model = match args.llm_ai_base.as_deref() {
                Some("gemma3") | Some("gemma-3") => "google/gemma-3-270m",
                Some("smollm") | Some("smollm2") | None => "HuggingFaceTB/SmolLM2-360M",
                Some(other) => other, // Allow full repo IDs
            };

            let result = if let Some(adapter_path) = &args.llm_ai {
                eprintln!(
                    "Loading LLM AI with adapter: {:?} (base: {})",
                    adapter_path, base_model
                );
                eu4sim_ai::LlmAi::new(base_model, Some(adapter_path.clone()))
            } else {
                eprintln!("Loading LLM AI with base model: {}", base_model);
                eu4sim_ai::LlmAi::new(base_model, None)
            };

            match result {
                Ok(ai) => {
                    eprintln!("LLM AI loaded successfully for: {:?}", llm_tag);
                    Some(Box::new(ai))
                }
                Err(e) => {
                    eprintln!("Failed to load LLM AI: {}. Falling back to GreedyAI.", e);
                    None
                }
            }
        } else {
            None
        };

        // Build AI map
        let mut ai_map: BTreeMap<String, Box<dyn eu4sim_core::AiPlayer>> = BTreeMap::new();
        let mut llm_ai = llm_ai; // Make it mutable so we can take() from it

        for tag in state.countries.keys() {
            let ai: Box<dyn eu4sim_core::AiPlayer> =
                if llm_tag.as_ref() == Some(tag) && llm_ai.is_some() {
                    // Use LLM AI for the top GP
                    llm_ai.take().unwrap()
                } else if greedy_tags.contains(tag) {
                    Box::new(eu4sim_core::GreedyAI::new())
                } else {
                    // Hash tag into seed for diversity
                    let tag_hash: u64 = tag.as_bytes().iter().map(|&b| b as u64).sum();
                    let seed = args.seed.wrapping_add(tag_hash);
                    Box::new(eu4sim_core::RandomAi::new(seed))
                };
            ai_map.insert(tag.clone(), ai);
        }
        ai_map
    } else {
        BTreeMap::new()
    };

    // Track last year for hybrid AI reassignment
    let mut last_reassign_year = args.start_year;

    // Note: available commands buffer is now allocated per-AI in parallel loop

    // Only enable TUI in interactive mode
    let _guard = if args.headless {
        None
    } else {
        Some(RawModeGuard::new()?)
    };
    let mut speed = args.speed.clamp(1, 5) as usize;
    // Map speed 1..5 to delay in ms
    let delays = [1000, 500, 200, 50, 0];

    use std::io::Write;

    // Clear screen and show controls only in interactive mode
    if !args.headless {
        print!("\x1b[2J\x1b[1;1H");
        print!("Controls: 1-5 to set speed, +/- to adjust, q to quit\r\n");
    }

    let tags: Vec<String> = args
        .tags
        .split(',')
        .map(|s| s.trim().to_uppercase())
        .collect();

    // Initialize observer registry (console observer only in interactive mode)
    let mut observers = ObserverRegistry::new();
    if !args.headless {
        let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();
        observers.register(Box::new(ConsoleObserver::new(&tag_refs)));
    }

    // Register event log observer if requested
    if let Some(ref path) = args.event_log {
        let observer = if path == "-" {
            EventLogObserver::stdout()
        } else {
            EventLogObserver::file(path)?
        };
        observers.register(Box::new(observer));
    }

    // Register datagen observer if requested
    // Note: available_commands are now passed via PlayerInputs (precomputed in AI loop)
    if let Some(ref path) = args.datagen {
        let observer = if path == "-" {
            DataGenObserver::stdout()
        } else {
            DataGenObserver::file(path)?
        };
        observers.register(Box::new(observer));
    }

    let mut tick: u64 = 0;
    let mut paused = false;
    let mut header_printed = false;

    // Wall clock timer for total runtime
    let wall_start = std::time::Instant::now();

    // Game Loop
    while tick < args.ticks as u64 {
        // Poll input (only in interactive mode)
        if !args.headless {
            while event::poll(std::time::Duration::ZERO)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == event::KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('1') => speed = 1,
                            KeyCode::Char('2') => speed = 2,
                            KeyCode::Char('3') => speed = 3,
                            KeyCode::Char('4') => speed = 4,
                            KeyCode::Char('5') => speed = 5,
                            KeyCode::Char('+') | KeyCode::Char('=') => speed = (speed + 1).min(5),
                            KeyCode::Char('-') => speed = speed.saturating_sub(1).max(1),
                            KeyCode::Char(' ') => paused = !paused,
                            KeyCode::Char('q') => return Ok(()),
                            _ => {}
                        }
                    }
                }
            }

            // Render header with speed/pause status (separate from observer)
            if header_printed {
                // Move cursor up to overwrite header + observer lines
                // Observer prints: tags.len() country lines
                // Main prints: 1 header line
                // Total: tags.len() + 1
                print!("\x1b[{}A", tags.len() + 1);
            }
            header_printed = true;

            let status_suffix = if paused { " [PAUSED]" } else { "" };
            let progress_pct = (tick as f64 / args.ticks as f64) * 100.0;
            print!(
                "[{}] Speed: {} | Tick {}/{} ({:.1}%){}                    \r\n",
                state.date, speed, tick, args.ticks, progress_pct, status_suffix
            );
            std::io::stdout().flush().unwrap();

            // Pause Logic (render state but don't advance)
            if paused {
                // Still notify observers so display updates
                let snapshot = Snapshot::new(state.clone(), tick, 0);
                observers.notify(&snapshot);
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
        }

        // Logic Step - generate AI inputs
        let mut inputs: Vec<PlayerInputs> = Vec::new();

        if args.observer {
            let ai_start = std::time::Instant::now();

            // Pre-compute global army strength (O(armies), shared across all AIs)
            let global_strength: HashMap<String, u32> =
                state.armies.values().fold(HashMap::new(), |mut acc, army| {
                    *acc.entry(army.owner.clone()).or_default() += army.regiments.len() as u32;
                    acc
                });

            // Pre-compute neighbor countries for each country (fog of war)
            // A country "knows" another if they share a province border
            let neighbor_countries: HashMap<String, HashSet<String>> = {
                // First, get owned provinces per country
                let mut country_provinces: HashMap<String, Vec<u32>> = HashMap::new();
                for (prov_id, prov) in &state.provinces {
                    if let Some(owner) = &prov.owner {
                        country_provinces
                            .entry(owner.clone())
                            .or_default()
                            .push(*prov_id);
                    }
                }

                // Then find neighbors via adjacency graph
                let mut neighbors: HashMap<String, HashSet<String>> = HashMap::new();
                for (tag, provs) in &country_provinces {
                    let mut known = HashSet::new();
                    for &prov_id in provs {
                        for neighbor_id in adjacency.neighbors(prov_id) {
                            if let Some(neighbor_prov) = state.provinces.get(&neighbor_id) {
                                if let Some(neighbor_owner) = &neighbor_prov.owner {
                                    if neighbor_owner != tag {
                                        known.insert(neighbor_owner.clone());
                                    }
                                }
                            }
                        }
                    }
                    neighbors.insert(tag.clone(), known);
                }
                neighbors
            };

            // Generate AI commands for all countries (parallel)
            // Returns PlayerInputs for ALL countries so datagen can use precomputed available_commands
            inputs = ais
                .par_iter_mut()
                .map(|(tag, ai)| {
                    // Start with neighbor countries (fog of war baseline)
                    let mut known_countries: HashSet<String> =
                        neighbor_countries.get(tag).cloned().unwrap_or_default();

                    // Check if this country is at war with anyone
                    let at_war = state
                        .diplomacy
                        .wars
                        .values()
                        .any(|war| war.attackers.contains(tag) || war.defenders.contains(tag));

                    // Calculate war scores and enemy provinces for this country
                    let mut our_war_score = std::collections::HashMap::new();
                    let mut enemy_provinces = std::collections::HashSet::new();
                    for war in state.diplomacy.wars.values() {
                        let is_attacker = war.attackers.contains(tag);
                        let is_defender = war.defenders.contains(tag);

                        if is_attacker || is_defender {
                            // Add all war participants to known countries
                            for participant in war.attackers.iter().chain(war.defenders.iter()) {
                                known_countries.insert(participant.clone());
                            }

                            // Calculate relative war score (positive = winning, negative = losing)
                            let score = if is_attacker {
                                eu4sim_core::fixed::Fixed::from_int(war.attacker_score as i64)
                                    - eu4sim_core::fixed::Fixed::from_int(war.defender_score as i64)
                            } else {
                                eu4sim_core::fixed::Fixed::from_int(war.defender_score as i64)
                                    - eu4sim_core::fixed::Fixed::from_int(war.attacker_score as i64)
                            };
                            our_war_score.insert(war.id, score);

                            // Collect enemy provinces (only for known enemies)
                            let enemy_tags: Vec<&String> = if is_attacker {
                                war.defenders.iter().collect()
                            } else {
                                war.attackers.iter().collect()
                            };

                            for (prov_id, prov) in &state.provinces {
                                if let Some(owner) = &prov.owner {
                                    if enemy_tags.contains(&owner) {
                                        enemy_provinces.insert(*prov_id);
                                    }
                                }
                            }
                        }
                    }

                    // Filter strength to only known countries (fog of war)
                    let known_country_strength: HashMap<String, u32> = global_strength
                        .iter()
                        .filter(|(country, _)| known_countries.contains(*country))
                        .map(|(k, v)| (k.clone(), *v))
                        .collect();

                    // Build visible state with fog-of-war filtered intelligence
                    let visible_state = eu4sim_core::ai::VisibleWorldState {
                        date: state.date,
                        observer: tag.clone(),
                        own_country: state.countries.get(tag).cloned().unwrap_or_default(),
                        at_war,
                        known_countries: known_countries.into_iter().collect(),
                        enemy_provinces,
                        known_country_strength,
                        our_war_score,
                    };

                    // Compute available commands once - reused by AI and datagen
                    let available = state.available_commands(tag, Some(&*adjacency));
                    let cmds = ai.decide(&visible_state, &available);

                    PlayerInputs {
                        country: tag.clone(),
                        commands: cmds,
                        available_commands: available,
                        visible_state: Some(visible_state),
                    }
                })
                .collect();
            if let Some(m) = metrics.as_mut() {
                m.ai_time += ai_start.elapsed();
            }
        } else {
            // Default stub inputs for non-observer mode
            inputs.push(PlayerInputs {
                country: "SWE".to_string(),
                commands: vec![],
                available_commands: vec![], // Not computed in non-observer mode
                visible_state: None,
            });
        }

        // Step
        state = step_world(
            &state,
            &inputs,
            Some(&*adjacency),
            &config,
            metrics.as_mut(),
        );
        tick += 1;

        // Yearly AI pool reassignment in hybrid mode
        if args.ai == "hybrid" && state.date.year > last_reassign_year {
            last_reassign_year = state.date.year;
            reassign_hybrid_ais(&mut ais, &state, args.greedy_count, args.seed);
        }

        // Notify observers with post-step state and inputs that were processed
        let snapshot = Snapshot::new(state.clone(), tick, 0);
        observers.notify_with_inputs(&snapshot, &inputs);

        // Speed control delay
        if speed < 5 {
            std::thread::sleep(std::time::Duration::from_millis(delays[speed - 1]));
        }
    }

    log::info!("Simulation finished at {}", state.date);

    // Capture wall time before any cleanup/printing
    let wall_time = wall_start.elapsed();

    if let Some(m) = metrics {
        let years = (state.date.year - args.start_year) as f64;
        let cpu_time = m.total_time + m.ai_time; // Time tracked in hot loops

        println!("\n=== Benchmark Results ===");
        println!("Simulated: {} years ({} ticks)", years, m.total_ticks);
        println!(
            "Wall time: {:.2}s | CPU time: {:.2}s",
            wall_time.as_secs_f64(),
            cpu_time.as_secs_f64()
        );

        // I/O overhead is the gap between wall time and tracked CPU time
        let io_overhead = wall_time.saturating_sub(cpu_time);
        if io_overhead.as_secs_f64() > 0.1 {
            println!(
                "I/O overhead: {:.2}s ({:.1}% of wall time)",
                io_overhead.as_secs_f64(),
                io_overhead.as_secs_f64() / wall_time.as_secs_f64() * 100.0
            );
        }

        let years_per_sec = if wall_time.as_secs_f64() > 0.0 {
            years / wall_time.as_secs_f64()
        } else {
            0.0
        };
        println!("Speed: {:.2} years/sec (wall)", years_per_sec);

        let total_ticks = m.total_ticks.max(1) as f64;
        println!(
            "Tick avg: {:.3}ms (wall) / {:.3}ms (cpu)",
            wall_time.as_secs_f64() * 1000.0 / total_ticks,
            cpu_time.as_secs_f64() * 1000.0 / total_ticks
        );

        println!("Breakdown (of CPU time):");
        println!(
            "  Movement:   {:>7.3}ms ({:4.1}%)",
            m.movement_time.as_secs_f64() * 1000.0 / total_ticks,
            m.movement_time.as_secs_f64() / cpu_time.as_secs_f64() * 100.0
        );
        println!(
            "  Combat:     {:>7.3}ms ({:4.1}%)",
            m.combat_time.as_secs_f64() * 1000.0 / total_ticks,
            m.combat_time.as_secs_f64() / cpu_time.as_secs_f64() * 100.0
        );
        println!(
            "  Occupation: {:>7.3}ms ({:4.1}%)",
            m.occupation_time.as_secs_f64() * 1000.0 / total_ticks,
            m.occupation_time.as_secs_f64() / cpu_time.as_secs_f64() * 100.0
        );
        println!(
            "  Economy:    {:>7.3}ms ({:4.1}%)",
            m.economy_time.as_secs_f64() * 1000.0 / total_ticks,
            m.economy_time.as_secs_f64() / cpu_time.as_secs_f64() * 100.0
        );
        println!(
            "  Trade:      {:>7.3}ms ({:4.1}%)",
            m.trade_time.as_secs_f64() * 1000.0 / total_ticks,
            m.trade_time.as_secs_f64() / cpu_time.as_secs_f64() * 100.0
        );
        println!(
            "  AI:         {:>7.3}ms ({:4.1}%)",
            m.ai_time.as_secs_f64() * 1000.0 / total_ticks,
            m.ai_time.as_secs_f64() / cpu_time.as_secs_f64() * 100.0
        );

        let other_time = cpu_time
            .checked_sub(
                m.movement_time
                    + m.combat_time
                    + m.occupation_time
                    + m.economy_time
                    + m.trade_time
                    + m.ai_time,
            )
            .unwrap_or_default();

        println!(
            "  Other:      {:>7.3}ms ({:4.1}%)",
            other_time.as_secs_f64() * 1000.0 / total_ticks,
            other_time.as_secs_f64() / cpu_time.as_secs_f64() * 100.0
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use eu4sim_core::state::{CountryState, Date, ProvinceState};
    use eu4sim_core::Fixed;

    /// Create a minimal WorldState with N countries and provinces
    #[allow(clippy::field_reassign_with_default)]
    fn make_test_world(country_devs: &[(String, i64)]) -> WorldState {
        let mut state = WorldState::default();
        state.date = Date::new(1444, 1, 1);

        for (i, (tag, dev)) in country_devs.iter().enumerate() {
            state.countries.insert(tag.clone(), CountryState::default());

            // Create a province with this development
            let prov = ProvinceState {
                owner: Some(tag.clone()),
                base_tax: Fixed::from_int(*dev / 3),
                base_production: Fixed::from_int(*dev / 3),
                base_manpower: Fixed::from_int(*dev / 3),
                ..Default::default()
            };
            state.provinces.insert((i + 1) as u32, prov);
        }

        state
    }

    #[test]
    fn test_reassign_hybrid_ais_count_invariant() {
        // Property: After reassignment, exactly min(greedy_count, num_countries)
        // countries should have GreedyAI
        let countries = vec![
            ("FRA".to_string(), 100),
            ("SPA".to_string(), 90),
            ("ENG".to_string(), 80),
            ("AUS".to_string(), 70),
            ("TUR".to_string(), 60),
        ];

        let state = make_test_world(&countries);
        let mut ais: BTreeMap<String, Box<dyn eu4sim_core::AiPlayer>> = BTreeMap::new();

        // Start with all RandomAI
        for (tag, _) in &countries {
            ais.insert(tag.clone(), Box::new(eu4sim_core::RandomAi::new(12345)));
        }

        // Reassign with greedy_count = 3
        reassign_hybrid_ais(&mut ais, &state, 3, 12345);

        // Count GreedyAIs
        let greedy_count = ais.values().filter(|ai| ai.name() == "GreedyAI").count();
        assert_eq!(greedy_count, 3, "Expected 3 GreedyAIs after reassignment");

        // Verify it's the top 3 by development
        let top_3: HashSet<_> = calculate_top_countries(&state, 3);
        assert!(top_3.contains("FRA"));
        assert!(top_3.contains("SPA"));
        assert!(top_3.contains("ENG"));
    }

    #[test]
    fn test_reassign_hybrid_ais_handles_fewer_countries() {
        // Property: If greedy_count > num_countries, all countries get GreedyAI
        let countries = vec![("FRA".to_string(), 100), ("SPA".to_string(), 90)];

        let state = make_test_world(&countries);
        let mut ais: BTreeMap<String, Box<dyn eu4sim_core::AiPlayer>> = BTreeMap::new();

        for (tag, _) in &countries {
            ais.insert(tag.clone(), Box::new(eu4sim_core::RandomAi::new(12345)));
        }

        // Reassign with greedy_count = 5 (more than available)
        reassign_hybrid_ais(&mut ais, &state, 5, 12345);

        let greedy_count = ais.values().filter(|ai| ai.name() == "GreedyAI").count();
        assert_eq!(
            greedy_count, 2,
            "All countries should be GreedyAI when count exceeds available"
        );
    }

    #[test]
    fn test_reassign_hybrid_ais_removes_dead_countries() {
        // Property: Dead countries (not in state.countries) should be removed from ais
        let countries = vec![("FRA".to_string(), 100)];
        let state = make_test_world(&countries);

        let mut ais: BTreeMap<String, Box<dyn eu4sim_core::AiPlayer>> = BTreeMap::new();
        ais.insert(
            "FRA".to_string(),
            Box::new(eu4sim_core::RandomAi::new(12345)),
        );
        ais.insert(
            "DEAD".to_string(), // Country not in state
            Box::new(eu4sim_core::RandomAi::new(12345)),
        );

        reassign_hybrid_ais(&mut ais, &state, 1, 12345);

        assert!(ais.contains_key("FRA"));
        assert!(!ais.contains_key("DEAD"), "Dead country should be removed");
    }
}
