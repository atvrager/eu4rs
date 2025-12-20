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
use eu4sim_core::{step_world, ObserverRegistry, PlayerInputs, SimConfig, Snapshot};
use rayon::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;

mod loader;

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
    let mut ais: std::collections::BTreeMap<String, eu4sim_core::ai::RandomAi> = if args.observer {
        state
            .countries
            .keys()
            .map(|tag| {
                // Hash tag into seed for diversity
                let base_seed = 12345u64;
                let tag_hash: u64 = tag.as_bytes().iter().map(|&b| b as u64).sum();
                let seed = base_seed.wrapping_add(tag_hash);
                (tag.clone(), eu4sim_core::ai::RandomAi::new(seed))
            })
            .collect()
    } else {
        std::collections::BTreeMap::new()
    };

    // Note: available commands buffer is now allocated per-AI in parallel loop

    let _guard = RawModeGuard::new()?;
    let mut speed = args.speed.clamp(1, 5) as usize;
    // Map speed 1..5 to delay in ms
    let delays = [1000, 500, 200, 50, 0];

    use std::io::Write;

    // Clear screen firmly to remove compilation artifacts
    print!("\x1b[2J\x1b[1;1H");
    print!("Controls: 1-5 to set speed, +/- to adjust, q to quit\r\n");

    let tags: Vec<String> = args
        .tags
        .split(',')
        .map(|s| s.trim().to_uppercase())
        .collect();

    // Initialize observer registry with console observer
    let mut observers = ObserverRegistry::new();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();
    observers.register(Box::new(ConsoleObserver::new(&tag_refs)));

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
    if let Some(ref path) = args.datagen {
        let observer = if path == "-" {
            DataGenObserver::stdout(Some(Arc::clone(&adjacency)))
        } else {
            DataGenObserver::file(path, Some(Arc::clone(&adjacency)))?
        };
        observers.register(Box::new(observer));
    }

    let mut tick: u64 = 0;
    let mut paused = false;
    let mut header_printed = false;

    // Game Loop
    while tick < args.ticks as u64 {
        // Poll input
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
        print!(
            "[{}] Speed: {}{}                                   \r\n",
            state.date, speed, status_suffix
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

        // Logic Step - generate AI inputs
        let mut inputs: Vec<PlayerInputs> = Vec::new();

        if args.observer {
            let ai_start = std::time::Instant::now();
            // Generate AI commands for all countries (parallel)
            inputs = ais
                .par_iter_mut()
                .filter_map(|(tag, ai)| {
                    // Check if this country is at war with anyone
                    let at_war = state
                        .diplomacy
                        .wars
                        .values()
                        .any(|war| war.attackers.contains(tag) || war.defenders.contains(tag));

                    // Minimal omniscient state for now
                    let visible_state = eu4sim_core::ai::VisibleWorldState {
                        date: state.date,
                        observer: tag.clone(),
                        own_country: state.countries.get(tag).cloned().unwrap_or_default(),
                        at_war,
                        known_countries: vec![], // Unused for RandomAi, skip allocation
                    };

                    // Allocate available commands buffer per-AI
                    let available = state.available_commands(tag, Some(&*adjacency));

                    let cmds = eu4sim_core::ai::AiPlayer::decide(ai, &visible_state, &available);
                    if !cmds.is_empty() {
                        Some(PlayerInputs {
                            country: tag.clone(),
                            commands: cmds,
                        })
                    } else {
                        None
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

        // Notify observers with post-step state and inputs that were processed
        let snapshot = Snapshot::new(state.clone(), tick, 0);
        observers.notify_with_inputs(&snapshot, &inputs);

        // Speed control delay
        if speed < 5 {
            std::thread::sleep(std::time::Duration::from_millis(delays[speed - 1]));
        }
    }

    log::info!("Simulation finished at {}", state.date);

    if let Some(m) = metrics {
        let years = (state.date.year - args.start_year) as f64;
        let real_total_time = m.total_time + m.ai_time;

        println!("\n=== Benchmark Results ===");
        println!(
            "Simulated: {} years in {:.2}s",
            years,
            real_total_time.as_secs_f64()
        );
        let years_per_sec = if real_total_time.as_secs_f64() > 0.0 {
            years / real_total_time.as_secs_f64()
        } else {
            0.0
        };
        println!("Speed: {:.1} years/sec", years_per_sec);

        let total_ticks = m.total_ticks.max(1) as f64;
        println!(
            "Tick avg: {:.3}ms",
            real_total_time.as_secs_f64() * 1000.0 / total_ticks
        );

        println!("Breakdown:");
        println!(
            "  Movement:   {:>7.3}ms ({:4.1}%)",
            m.movement_time.as_secs_f64() * 1000.0 / total_ticks,
            m.movement_time.as_secs_f64() / real_total_time.as_secs_f64() * 100.0
        );
        println!(
            "  Combat:     {:>7.3}ms ({:4.1}%)",
            m.combat_time.as_secs_f64() * 1000.0 / total_ticks,
            m.combat_time.as_secs_f64() / real_total_time.as_secs_f64() * 100.0
        );
        println!(
            "  Occupation: {:>7.3}ms ({:4.1}%)",
            m.occupation_time.as_secs_f64() * 1000.0 / total_ticks,
            m.occupation_time.as_secs_f64() / real_total_time.as_secs_f64() * 100.0
        );
        println!(
            "  Economy:    {:>7.3}ms ({:4.1}%)",
            m.economy_time.as_secs_f64() * 1000.0 / total_ticks,
            m.economy_time.as_secs_f64() / real_total_time.as_secs_f64() * 100.0
        );
        println!(
            "  AI:         {:>7.3}ms ({:4.1}%)",
            m.ai_time.as_secs_f64() * 1000.0 / total_ticks,
            m.ai_time.as_secs_f64() / real_total_time.as_secs_f64() * 100.0
        );

        let other_time = real_total_time
            .checked_sub(
                m.movement_time + m.combat_time + m.occupation_time + m.economy_time + m.ai_time,
            )
            .unwrap_or_default();

        println!(
            "  Other:      {:>7.3}ms ({:4.1}%)",
            other_time.as_secs_f64() * 1000.0 / total_ticks,
            other_time.as_secs_f64() / real_total_time.as_secs_f64() * 100.0
        );
    }

    Ok(())
}
