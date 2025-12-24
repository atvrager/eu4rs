use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use eu4sim_verify::{extract, melt, parse, report, verify};

#[derive(Parser)]
#[command(name = "eu4sim-verify")]
#[command(about = "Verify EU4 simulation accuracy against save files")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Verify a save file against simulation calculations
    Check {
        /// Path to the EU4 save file (.eu4)
        save_path: PathBuf,

        /// Tolerance for floating point comparisons (default: 0.01)
        #[arg(short, long, default_value = "0.01")]
        tolerance: f64,

        /// Filter to specific metrics (comma-separated: manpower,tax,trade,production)
        #[arg(short, long)]
        metrics: Option<String>,

        /// Filter to specific country tag
        #[arg(short, long)]
        country: Option<String>,

        /// Output report as JSON
        #[arg(long)]
        json: bool,

        /// Write report to file instead of stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Show save file metadata without verification
    Info {
        /// Path to the EU4 save file (.eu4)
        save_path: PathBuf,
    },

    /// Melt a binary save to text format (unknown tokens as hex)
    Melt {
        /// Path to the EU4 save file (.eu4)
        save_path: PathBuf,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Limit output to first N lines
        #[arg(long)]
        head: Option<usize>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    match cli.command {
        Commands::Check {
            save_path,
            tolerance,
            metrics: _metrics,
            country,
            json,
            output,
        } => {
            log::info!("Loading save file: {}", save_path.display());

            // Parse the save file
            let state = parse::load_save(&save_path)?;

            log::info!(
                "Loaded save: date={}, player={:?}",
                state.meta.date,
                state.meta.player
            );
            log::info!(
                "Found {} countries, {} provinces",
                state.countries.len(),
                state.provinces.len()
            );

            // Extract verification data
            let verify_data = extract::extract_for_verification(&state);

            // Run verification
            let mut summary = verify::verify_all(&verify_data, tolerance);

            // Filter by country if specified
            if let Some(ref tag) = country {
                let tag_upper = tag.to_uppercase();
                summary.results.retain(|r| {
                    // Check if the metric belongs to the specified country
                    let metric_str = format!("{}", r.metric);
                    metric_str.contains(&format!("({})", tag_upper))
                });
                // Recalculate counts
                summary.total = summary.results.len();
                summary.passed = summary
                    .results
                    .iter()
                    .filter(|r| r.status == eu4sim_verify::VerifyStatus::Pass)
                    .count();
                summary.failed = summary
                    .results
                    .iter()
                    .filter(|r| r.status == eu4sim_verify::VerifyStatus::Fail)
                    .count();
                summary.skipped = summary
                    .results
                    .iter()
                    .filter(|r| r.status == eu4sim_verify::VerifyStatus::Skip)
                    .count();
                log::info!(
                    "Filtered to {} country: {} results",
                    tag_upper,
                    summary.total
                );
            }

            // Generate report
            if json {
                let json_output = report::json_report(&summary)?;
                if let Some(path) = output {
                    std::fs::write(&path, &json_output)?;
                    log::info!("Report written to: {}", path.display());
                } else {
                    println!("{}", json_output);
                }
            } else {
                let mut writer: Box<dyn std::io::Write> = if let Some(path) = output {
                    Box::new(std::fs::File::create(&path)?)
                } else {
                    Box::new(std::io::stdout())
                };
                report::print_report(&summary, &mut writer)?;
            }

            // Exit with error code if there were failures
            if summary.failed > 0 {
                std::process::exit(1);
            }
        }

        Commands::Info { save_path } => {
            log::info!("Loading save file: {}", save_path.display());

            let state = parse::load_save(&save_path)?;

            println!("\n=== Save File Info ===");
            println!("Date: {}", state.meta.date);
            println!("Player: {:?}", state.meta.player);
            println!("Ironman: {}", state.meta.ironman);
            println!("Version: {:?}", state.meta.save_version);
            println!();
            println!("Countries: {}", state.countries.len());
            println!("Provinces: {}", state.provinces.len());

            if !state.countries.is_empty() {
                println!();
                println!("Sample countries:");
                for (tag, country) in state.countries.iter().take(5) {
                    println!("  {}: manpower={:?}", tag, country.current_manpower);
                }
                if state.countries.len() > 5 {
                    println!("  ... and {} more", state.countries.len() - 5);
                }
            }
        }

        Commands::Melt {
            save_path,
            output,
            head,
        } => {
            log::info!("Melting save file: {}", save_path.display());

            // Read the save file
            let file = std::fs::File::open(&save_path)?;
            let mut archive = zip::ZipArchive::new(file)?;

            // Read gamestate
            let mut gamestate = archive.by_name("gamestate")?;
            let mut data = Vec::new();
            std::io::Read::read_to_end(&mut gamestate, &mut data)?;

            log::info!("Read gamestate: {} bytes", data.len());

            // Check format
            if !data.starts_with(b"EU4bin") {
                println!("Save is already in text format!");
                // Just output the text
                let text = String::from_utf8_lossy(&data);
                if let Some(n) = head {
                    for line in text.lines().take(n) {
                        println!("{}", line);
                    }
                } else {
                    print!("{}", text);
                }
                return Ok(());
            }

            // Melt to text
            let mut melted = Vec::new();
            let stats = melt::melt_save(&data, &mut melted)?;

            log::info!(
                "Melted {} tokens ({} unknown)",
                stats.total_tokens,
                stats.unknown_tokens
            );

            // Output
            let text = String::from_utf8_lossy(&melted);
            if let Some(path) = output {
                std::fs::write(&path, &melted)?;
                println!("Melted save written to: {}", path.display());
            } else if let Some(n) = head {
                for line in text.lines().take(n) {
                    println!("{}", line);
                }
            } else {
                print!("{}", text);
            }
        }
    }

    Ok(())
}
