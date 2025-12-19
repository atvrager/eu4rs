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

    /// Run in observer mode (AI controls all countries)
    #[arg(long)]
    observer: bool,
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

    // Initialize AI if in observer mode
    let mut ais: std::collections::HashMap<String, eu4sim_core::ai::RandomAi> = if args.observer {
        state
            .countries
            .keys()
            .map(|tag| (tag.clone(), eu4sim_core::ai::RandomAi::new(12345)))
            .collect()
    } else {
        std::collections::HashMap::new()
    };

    // Pre-allocate buffers for AI (reused each tick)
    let mut available: Vec<eu4sim_core::Command> = Vec::with_capacity(1024);

    // Game Loop
    for _ in 0..args.ticks {
        let mut inputs = Vec::new();

        if args.observer {
            let ai_start = std::time::Instant::now();
            // Generate AI commands for all countries
            for (tag, ai) in &mut ais {
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

                // Reuse available buffer
                available.clear();

                // Find armies and generate valid move commands
                for (id, army) in &state.armies {
                    if &army.owner == tag {
                        let neighbors = adjacency.neighbors(army.location);
                        for dest in neighbors {
                            // Check if we can move to this destination
                            let can_move = if let Some(prov) = state.provinces.get(&dest) {
                                match &prov.owner {
                                    None => true,                        // Uncolonized
                                    Some(owner) if owner == tag => true, // Own territory
                                    Some(owner) => {
                                        // Need military access OR be at war
                                        state.diplomacy.has_military_access(tag, owner)
                                            || state.diplomacy.are_at_war(tag, owner)
                                    }
                                }
                            } else {
                                true // Province not in state, assume OK
                            };

                            if can_move {
                                available.push(eu4sim_core::Command::Move {
                                    army_id: *id,
                                    destination: dest,
                                });
                            }
                        }
                    }
                }

                let cmds = eu4sim_core::ai::AiPlayer::decide(ai, &visible_state, &available);
                if !cmds.is_empty() {
                    inputs.push(PlayerInputs {
                        country: tag.clone(),
                        commands: cmds,
                    });
                }
            }
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
