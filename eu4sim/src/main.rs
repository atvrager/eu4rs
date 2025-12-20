use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use eu4sim_core::state::Date;
use eu4sim_core::{step_world, PlayerInputs, SimConfig};
use std::path::PathBuf;

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
            .map(|tag| {
                // Hash tag into seed for diversity
                let base_seed = 12345u64;
                let tag_hash: u64 = tag.as_bytes().iter().map(|&b| b as u64).sum();
                let seed = base_seed.wrapping_add(tag_hash);
                (tag.clone(), eu4sim_core::ai::RandomAi::new(seed))
            })
            .collect()
    } else {
        std::collections::HashMap::new()
    };

    // Pre-allocate buffers for AI (reused each tick)
    let mut available: Vec<eu4sim_core::Command> = Vec::with_capacity(1024);

    let _guard = RawModeGuard::new()?;
    let mut speed = args.speed.clamp(1, 5) as usize;
    // Map speed 1..5 to delay in ms
    let delays = [1000, 500, 200, 50, 0];

    use std::io::Write;
    print!("Controls: 1-5 to set speed, +/- to adjust, q to quit\r\n");

    let tags: Vec<String> = args
        .tags
        .split(',')
        .map(|s| s.trim().to_uppercase())
        .collect();
    // Track state at the start of the current month
    let mut month_start_states = std::collections::HashMap::new();
    // Track deltas from the previous completed month
    let mut last_month_deltas = std::collections::HashMap::new();

    for tag in &tags {
        if let Some(c) = state.countries.get(tag) {
            month_start_states.insert(tag.clone(), c.clone());
            last_month_deltas.insert(tag.clone(), (0.0, 0.0));
        }
    }

    let mut tick = 0;
    let mut paused = false;
    let mut first_print = true;

    // Game Loop
    while tick < args.ticks {
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

        // Move cursor up if multi-line
        // Use first_print flag instead of tick count to handle pause refreshes
        if !first_print {
            // +1 for header line
            print!("\x1b[{}A", tags.len() + 1);
        }
        first_print = false;

        // Render Status Header
        let status_suffix = if paused { " [PAUSED]" } else { "" };
        print!(
            "[{}] Speed: {}{}                                   \r\n",
            state.date, speed, status_suffix
        );

        for tag in tags.iter() {
            if let Some(country) = state.countries.get(tag) {
                // Use persistent last_month_deltas
                let (delta_treasury, delta_manpower) =
                    last_month_deltas.get(tag).copied().unwrap_or((0.0, 0.0));

                // Army composition
                let mut inf = 0;
                let mut cav = 0;
                let mut art = 0;

                for army in state.armies.values() {
                    if &army.owner == tag {
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
                    if p.owner.as_ref() == Some(tag) && p.has_fort {
                        forts += 1;
                    }
                }

                // Colors
                let color_t = if delta_treasury > 0.0 {
                    "\x1b[32m"
                } else if delta_treasury < 0.0 {
                    "\x1b[31m"
                } else {
                    "\x1b[90m"
                };
                let color_m = if delta_manpower > 0.0 {
                    "\x1b[32m"
                } else if delta_manpower < 0.0 {
                    "\x1b[31m"
                } else {
                    "\x1b[90m"
                };
                let reset = "\x1b[0m";

                let output = format!(
                    " {}: 💰{:>7.1}({}{:>+6.1}{}) 👥{:>6.0}({}{:>+5.0}{}) ⚔️{:>3.0}/{:>3.0}/{:>3.0} | Army:{:>3}/{:>3}/{:>3} Forts:{:>2}    ",
                    tag,
                    country.treasury.to_f32(),
                    color_t,
                    delta_treasury,
                    reset,
                    country.manpower.to_f32(),
                    color_m,
                    delta_manpower,
                    reset,
                    country.adm_mana.to_f32(),
                    country.dip_mana.to_f32(),
                    country.mil_mana.to_f32(),
                    inf,
                    cav,
                    art,
                    forts
                );

                print!("{}\r\n", output);
            } else {
                let output = format!(
                    " {}: \x1b[31m[ELIMINATED]\x1b[0m                                                                            ",
                    tag
                );
                print!("{}\r\n", output);
            }
        }
        std::io::stdout().flush().unwrap();

        // Pause Logic
        if paused {
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }

        // Logic Step
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

        // Update month start states on the 1st of the month
        if state.date.day == 1 {
            for tag in &tags {
                if let Some(c) = state.countries.get(tag) {
                    if let Some(prev) = month_start_states.insert(tag.clone(), c.clone()) {
                        let dt = (c.treasury - prev.treasury).to_f32();
                        let dm = (c.manpower - prev.manpower).to_f32();
                        last_month_deltas.insert(tag.clone(), (dt, dm));
                    }
                }
            }
        }

        // Step
        state = step_world(&state, &inputs, Some(&adjacency), &config, metrics.as_mut());
        tick += 1;

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
