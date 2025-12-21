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
) -> bool {
    let new_greedy = calculate_top_countries(state, greedy_count);

    // Find current greedy tags
    let mut changes = Vec::new();

    for (tag, ai) in ais.iter() {
        let is_greedy = ai.name() == "GreedyAI";
        let should_be_greedy = new_greedy.contains(tag);

        if is_greedy != should_be_greedy {
            changes.push((tag.clone(), should_be_greedy));
        }
    }

    // Handle new countries that don't have an AI yet
    for tag in state.countries.keys() {
        if !ais.contains_key(tag) {
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

    if changes.is_empty() && dead_tags.is_empty() {
        return false;
    }

    // Apply changes
    for (tag, should_be_greedy) in changes {
        let ai: Box<dyn eu4sim_core::AiPlayer> = if should_be_greedy {
            Box::new(eu4sim_core::GreedyAI::new())
        } else {
            let base_seed = 12345u64;
            let tag_hash: u64 = tag.as_bytes().iter().map(|&b| b as u64).sum();
            let seed = base_seed.wrapping_add(tag_hash);
            Box::new(eu4sim_core::RandomAi::new(seed))
        };
        ais.insert(tag, ai);
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

    /// Write training data JSONL to file (use "-" for stdout)
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
}

use eu4sim_core::SimMetrics;

fn main() -> Result<()> {
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

    // Resolve game path
    let game_path = PathBuf::from(args.game_path);

    // Initialize State
    let (mut state, adjacency_raw) =
        loader::load_initial_state(&game_path, Date::new(args.start_year, 11, 11), 12345)?;
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

        // Determine which tags get GreedyAI
        let greedy_tags: HashSet<String> = match args.ai.as_str() {
            "greedy" => state.countries.keys().cloned().collect(),
            "hybrid" => {
                let top = calculate_top_countries(&state, args.greedy_count);
                eprintln!(
                    "Hybrid mode: {} countries use GreedyAI: {:?}",
                    top.len(),
                    top
                );
                top
            }
            _ => HashSet::new(), // random mode: no greedy
        };

        state
            .countries
            .keys()
            .map(|tag| {
                let ai: Box<dyn eu4sim_core::AiPlayer> = if greedy_tags.contains(tag) {
                    Box::new(eu4sim_core::GreedyAI::new())
                } else {
                    // Hash tag into seed for diversity
                    let base_seed = 12345u64;
                    let tag_hash: u64 = tag.as_bytes().iter().map(|&b| b as u64).sum();
                    let seed = base_seed.wrapping_add(tag_hash);
                    Box::new(eu4sim_core::RandomAi::new(seed))
                };
                (tag.clone(), ai)
            })
            .collect()
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

            // Pre-compute country strength once (O(armies), shared across all AIs)
            let known_country_strength: std::collections::HashMap<String, u32> = state
                .armies
                .values()
                .fold(std::collections::HashMap::new(), |mut acc, army| {
                    *acc.entry(army.owner.clone()).or_default() += army.regiments.len() as u32;
                    acc
                });

            // Generate AI commands for all countries (parallel)
            // Returns PlayerInputs for ALL countries so datagen can use precomputed available_commands
            inputs = ais
                .par_iter_mut()
                .map(|(tag, ai)| {
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
                            // Calculate relative war score (positive = winning, negative = losing)
                            let score = if is_attacker {
                                eu4sim_core::fixed::Fixed::from_int(war.attacker_score as i64)
                                    - eu4sim_core::fixed::Fixed::from_int(war.defender_score as i64)
                            } else {
                                eu4sim_core::fixed::Fixed::from_int(war.defender_score as i64)
                                    - eu4sim_core::fixed::Fixed::from_int(war.attacker_score as i64)
                            };
                            our_war_score.insert(war.id, score);

                            // Collect enemy provinces
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

                    // Build visible state with full intelligence
                    let visible_state = eu4sim_core::ai::VisibleWorldState {
                        date: state.date,
                        observer: tag.clone(),
                        own_country: state.countries.get(tag).cloned().unwrap_or_default(),
                        at_war,
                        known_countries: vec![], // Could populate with neighbors/contacts
                        enemy_provinces,
                        known_country_strength: known_country_strength.clone(),
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
            reassign_hybrid_ais(&mut ais, &state, args.greedy_count);
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
            "  AI:         {:>7.3}ms ({:4.1}%)",
            m.ai_time.as_secs_f64() * 1000.0 / total_ticks,
            m.ai_time.as_secs_f64() / cpu_time.as_secs_f64() * 100.0
        );

        let other_time = cpu_time
            .checked_sub(
                m.movement_time + m.combat_time + m.occupation_time + m.economy_time + m.ai_time,
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
        reassign_hybrid_ais(&mut ais, &state, 3);

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
        reassign_hybrid_ais(&mut ais, &state, 5);

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

        reassign_hybrid_ais(&mut ais, &state, 1);

        assert!(ais.contains_key("FRA"));
        assert!(!ais.contains_key("DEAD"), "Dead country should be removed");
    }
}
