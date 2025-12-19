use anyhow::Result;
use clap::Parser;
use eu4sim_core::state::Date;
use eu4sim_core::{step_world, PlayerInputs, SimConfig};
use std::path::PathBuf;

mod loader;

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
}

use eu4sim_core::SimMetrics;

fn main() -> Result<()> {
    let args = Args::parse();

    if args.manifest {
        println!("{}", eu4data::manifest::GAME_MANIFEST.dump());
        return Ok(());
    }

    let level = std::str::FromStr::from_str(&args.log_level).unwrap_or(log::LevelFilter::Info);
    env_logger::Builder::new()
        .filter_level(level)
        .format_timestamp(None)
        .init();

    log::info!("Starting eu4sim...");

    // Resolve game path
    let game_path = PathBuf::from(args.game_path);

    // Initialize State
    let (mut state, adjacency) =
        loader::load_initial_state(&game_path, Date::new(args.start_year, 11, 11), 12345)?;

    log::info!("Initial State Date: {}", state.date);

    // Simulation config (monthly checksums)
    let config = SimConfig::default();

    let mut metrics = if args.benchmark {
        Some(SimMetrics::default())
    } else {
        None
    };

    // Game Loop
    for _ in 0..args.ticks {
        // Collect inputs (stub)
        let inputs = vec![PlayerInputs {
            country: "SWE".to_string(),
            commands: vec![], // No commands for now
        }];

        let prev_swe = state.countries.get("SWE").cloned();

        // Step
        state = step_world(&state, &inputs, Some(&adjacency), &config, metrics.as_mut());

        if let Some(swe) = state.countries.get("SWE") {
            use eu4sim_core::Fixed; // Import Fixed here or at top
            let prev_treasury = prev_swe.as_ref().map(|c| c.treasury).unwrap_or(Fixed::ZERO);
            let prev_manpower = prev_swe.as_ref().map(|c| c.manpower).unwrap_or(Fixed::ZERO);

            let delta_treasury = (swe.treasury - prev_treasury).to_f32();
            let delta_manpower = (swe.manpower - prev_manpower).to_f32();

            // Army composition
            let mut inf = 0;
            let mut cav = 0;
            let mut art = 0;

            for army in state.armies.values() {
                if army.owner == "SWE" {
                    for reg in &army.regiments {
                        match reg.type_ {
                            eu4sim_core::state::RegimentType::Infantry => inf += 1,
                            eu4sim_core::state::RegimentType::Cavalry => cav += 1,
                            eu4sim_core::state::RegimentType::Artillery => art += 1,
                        }
                    }
                }
            }

            // Fort count
            let mut forts = 0;
            for p in state.provinces.values() {
                if p.owner.as_deref() == Some("SWE") && p.has_fort {
                    forts += 1;
                }
            }

            // Colors
            let color_t = if delta_treasury > 0.0 {
                "\x1b[32m+"
            } else if delta_treasury < 0.0 {
                "\x1b[31m"
            } else {
                "\x1b[90m"
            };
            let color_m = if delta_manpower > 0.0 {
                "\x1b[32m+"
            } else if delta_manpower < 0.0 {
                "\x1b[31m"
            } else {
                "\x1b[90m"
            };
            let reset = "\x1b[0m";

            // Only log if something changed
            if delta_treasury.abs() > 0.001 || delta_manpower.abs() > 0.001 {
                log::info!(
                    "Tick: {} | SWE Treasury: {:.4} ({}{:.2}{}) | Manpower: {:.0} ({}{:.0}{}) | Army: I:{} C:{} A:{} | Forts: {}",
                    state.date,
                    swe.treasury.to_f32(),
                    color_t, delta_treasury, reset,
                    swe.manpower.to_f32(),
                    color_m, delta_manpower, reset,
                    inf, cav, art,
                    forts
                );
            }
        } else {
            log::info!("Tick: {}", state.date);
        }
    }

    log::info!("Simulation finished at {}", state.date);

    if let Some(m) = metrics {
        let years = (state.date.year - args.start_year) as f64;
        println!("\n=== Benchmark Results ===");
        println!(
            "Simulated: {} years in {:.2}s",
            years,
            m.total_time.as_secs_f64()
        );
        println!("Speed: {:.1} years/sec", m.years_per_second(years));
        println!("Tick avg: {:.3}ms", m.tick_avg_ms());
        println!("Breakdown:");
        println!(
            "  Movement:   {:>7.3}ms ({:4.1}%)",
            m.movement_time.as_secs_f64() * 1000.0 / m.total_ticks as f64,
            m.movement_time.as_secs_f64() / m.total_time.as_secs_f64() * 100.0
        );
        println!(
            "  Combat:     {:>7.3}ms ({:4.1}%)",
            m.combat_time.as_secs_f64() * 1000.0 / m.total_ticks as f64,
            m.combat_time.as_secs_f64() / m.total_time.as_secs_f64() * 100.0
        );
        println!(
            "  Occupation: {:>7.3}ms ({:4.1}%)",
            m.occupation_time.as_secs_f64() * 1000.0 / m.total_ticks as f64,
            m.occupation_time.as_secs_f64() / m.total_time.as_secs_f64() * 100.0
        );
        println!(
            "  Economy:    {:>7.3}ms ({:4.1}%)",
            m.economy_time.as_secs_f64() * 1000.0 / m.total_ticks as f64,
            m.economy_time.as_secs_f64() / m.total_time.as_secs_f64() * 100.0
        );
    }

    Ok(())
}
