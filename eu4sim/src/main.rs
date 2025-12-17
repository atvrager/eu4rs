use anyhow::Result;
use clap::Parser;
use eu4sim_core::state::Date;
use eu4sim_core::{step_world, PlayerInputs};
use std::path::PathBuf;

mod loader;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to game data
    #[arg(long, default_value = ".")]
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
}

fn main() -> Result<()> {
    let args = Args::parse();

    let level = std::str::FromStr::from_str(&args.log_level).unwrap_or(log::LevelFilter::Info);
    env_logger::Builder::new()
        .filter_level(level)
        .format_timestamp(None)
        .init();

    log::info!("Starting eu4sim...");

    // Resolve game path
    let game_path = if args.game_path == "." {
        eu4data::path::detect_game_path().unwrap_or_else(|| PathBuf::from("."))
    } else {
        PathBuf::from(args.game_path)
    };

    // Initialize State
    let mut state =
        loader::load_initial_state(&game_path, Date::new(args.start_year, 11, 11), 12345)?;

    log::info!("Initial State Date: {}", state.date);

    // Game Loop
    for _ in 0..args.ticks {
        // Collect inputs (stub)
        let inputs = vec![PlayerInputs {
            country: "SWE".to_string(),
            commands: vec![], // No commands for now
        }];

        // Step
        state = step_world(&state, &inputs);

        if let Some(swe) = state.countries.get("SWE") {
            log::info!(
                "Tick: {} | SWE Treasury: {:.4} | Manpower: {:.0}",
                state.date,
                swe.treasury.to_f32(),
                swe.manpower.to_f32()
            );
        } else {
            log::info!("Tick: {}", state.date);
        }
    }

    log::info!("Simulation finished at {}", state.date);

    Ok(())
}
