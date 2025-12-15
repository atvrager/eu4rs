use anyhow::Result;
use clap::Parser;
use eu4sim_core::state::Date;
use eu4sim_core::{step_world, PlayerInputs, WorldState};
use std::collections::HashMap;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to game data
    #[arg(long, default_value = ".")]
    game_path: String, // Not really used yet but good to have

    /// Start year
    #[arg(long, default_value_t = 1444)]
    start_year: i32,

    /// Number of ticks to run
    #[arg(short, long, default_value_t = 10)]
    ticks: u32,

    /// Log level (error, warn, info, debug, trace)
    #[arg(long, default_value = "info")]
    log_level: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let level = std::str::FromStr::from_str(&args.log_level).unwrap_or(log::LevelFilter::Info);
    env_logger::Builder::new()
        .filter_level(level)
        .format_timestamp(None)
        .init();

    log::info!("Starting eu4sim...");

    // 1. Initialize State
    let mut state = WorldState {
        date: Date::new(args.start_year, 11, 11), // 1444.11.11
        rng_seed: 12345,
        provinces: HashMap::new(),
        countries: HashMap::new(),
        diplomacy: Default::default(),
        global: Default::default(),
    };

    log::info!("Initial State Date: {}", state.date);

    // 2. Game Loop
    for _ in 0..args.ticks {
        // Collect inputs (stub)
        let inputs = vec![PlayerInputs {
            country: "SWE".to_string(),
            commands: vec![], // No commands for now
        }];

        // Step
        state = step_world(&state, &inputs);

        log::info!("Tick: {}", state.date);
    }

    log::info!("Simulation finished at {}", state.date);

    Ok(())
}
