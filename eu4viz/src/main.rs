use clap::Parser;
use std::path::PathBuf;

mod args;
mod camera;
mod ops;
mod window;

pub mod logger;
mod renderer;
mod state;
#[cfg(test)]
mod testing;
mod text;
mod ui;

use args::{Cli, Commands, MapMode};

fn run(mut args: Cli) -> Result<(), String> {
    // Resolve EU4 Path
    let eu4_path = if let Some(p) = args.eu4_path {
        p
    } else {
        match eu4data::path::detect_game_path() {
            Some(p) => {
                log::info!("Auto-detected EU4 path: {:?}", p);
                p
            }
            None => {
                return Err(
                    "Could not detect EU4 installation. Please provide --eu4-path.".to_string(),
                );
            }
        }
    };
    // Update args with resolved path so subsequent code uses it
    args.eu4_path = Some(eu4_path.clone());

    if let Some(cmd) = &args.command {
        match cmd {
            Commands::DumpTradegoods => {
                ops::dump_tradegoods(&eu4_path)?;
                return Ok(());
            }
            Commands::Manifest => {
                println!("{}", eu4data::manifest::GAME_MANIFEST.dump());
                return Ok(());
            }
            Commands::DrawMap { output, mode } => {
                let base = eu4_path;
                match mode {
                    MapMode::All => {
                        println!("=== Rendering Political Map ===");
                        ops::draw_map(
                            &base,
                            &PathBuf::from("map_political.png"),
                            MapMode::Political,
                        )?;

                        println!("\n=== Rendering Trade Goods Map ===");
                        ops::draw_map(
                            &base,
                            &PathBuf::from("map_tradegoods.png"),
                            MapMode::TradeGoods,
                        )?;
                    }
                    _ => {
                        ops::draw_map(&base, output, *mode)?;
                    }
                }
                return Ok(());
            }
            Commands::DrawWindow { verbose: _ } => {
                let base = eu4_path;
                let level =
                    std::str::FromStr::from_str(&args.log_level).unwrap_or(log::LevelFilter::Info);
                window::run(level, &base);
                return Ok(());
            }
            Commands::Snapshot { output, mode } => {
                let base = eu4_path;
                let path = std::path::Path::new(output);
                let level =
                    std::str::FromStr::from_str(&args.log_level).unwrap_or(log::LevelFilter::Info);
                pollster::block_on(window::snapshot(&base, path, *mode, level))?;
                return Ok(());
            }
            Commands::Lookup { key } => {
                let loc_path = eu4_path.join("localisation");
                let mut loc = eu4data::localisation::Localisation::new();
                println!(
                    "Loading localisation from {:?} ({})...",
                    loc_path, args.language
                );
                match loc.load_from_dir(&loc_path, &args.language) {
                    Ok(n) => println!("Loaded {} keys.", n),
                    Err(e) => println!("Warning: Failed to load localisation: {}", e),
                }

                match loc.get(key) {
                    Some(val) => println!("{} -> {}", key, val),
                    None => println!("{} -> [NOT_FOUND]", key),
                }
                return Ok(());
            }
            Commands::Languages => {
                let loc_path = eu4_path.join("localisation");
                match eu4data::localisation::Localisation::list_languages(&loc_path) {
                    Ok(langs) => {
                        println!("Available languages:");
                        for lang in langs {
                            println!("- {}", lang);
                        }
                    }
                    Err(e) => println!("Error scanning languages: {}", e),
                }
                return Ok(());
            }
        }
    }

    // Default behavior handling:
    if args.pretty_print {
        match ops::pretty_print_dir(&eu4_path, args.pretty_print) {
            Ok(stats) => {
                println!(
                    "Done! Success: {}, Failure: {}",
                    stats.success, stats.failure
                );
                println!(
                    "Total Tokens: {}, Total Nodes: {}",
                    stats.tokens, stats.nodes
                );
            }
            Err(e) => {
                println!("pretty_print_dir critical failure: {}", e);
            }
        }
    } else {
        // Default to Source Port GUI
        let base = eu4_path;
        let level = std::str::FromStr::from_str(&args.log_level).unwrap_or(log::LevelFilter::Info);
        window::run(level, &base);
    }

    Ok(())
}

fn main() -> Result<(), String> {
    run(Cli::parse())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_pretty_print_dir_logic() {
        let dir = tempdir().unwrap();
        let valid_path = dir.path().join("valid.txt");
        let mut valid_file = File::create(&valid_path).unwrap();
        writeln!(valid_file, "good_key = value").unwrap();

        let invalid_path = dir.path().join("invalid.txt");
        let mut invalid_file = File::create(&invalid_path).unwrap();
        writeln!(invalid_file, "key =").unwrap();

        let stats = ops::pretty_print_dir(dir.path(), false).expect("pretty_print_dir failed");

        assert_eq!(stats.success, 1);
        assert_eq!(stats.failure, 1);
        assert!(stats.tokens > 0);
        assert!(stats.nodes > 0);
    }

    #[test]
    fn test_dump_tradegoods_dispatch() {
        let dir = tempdir().unwrap();
        // The implementation expects `path/tradegoods/00_tradegoods.txt`
        // OR `path/common/tradegoods/00_tradegoods.txt` depending on logic.
        // `ops::dump_tradegoods` does `base_path.join("tradegoods/00_tradegoods.txt")`.

        let tradegoods_dir = dir.path().join("common/tradegoods");
        std::fs::create_dir_all(&tradegoods_dir).unwrap();

        let tg_file = tradegoods_dir.join("00_tradegoods.txt");
        let mut f = File::create(&tg_file).unwrap();
        writeln!(f, r#"grain = {{ color = {{ 1 1 1 }} }}"#).unwrap();

        let args = Cli {
            eu4_path: Some(dir.path().to_path_buf()),
            pretty_print: false,
            language: "english".to_string(),
            log_level: "info".to_string(),
            command: Some(Commands::DumpTradegoods),
        };

        let result = run(args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_lookup_missing_path() {
        let dir = tempdir().unwrap();
        // Missing localisation dir
        let args = Cli {
            eu4_path: Some(dir.path().to_path_buf()),
            pretty_print: false,
            language: "english".to_string(),
            log_level: "info".to_string(),
            command: Some(Commands::Lookup {
                key: "TEST".to_string(),
            }),
        };
        // It prints warning but returns Ok(()) usually in current impl
        // Let's verify it doesn't panic.
        assert!(run(args).is_ok());
    }

    #[test]
    fn test_pretty_print_missing_dir() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("missing");
        let args = Cli {
            eu4_path: Some(missing),
            pretty_print: true,
            language: "english".to_string(),
            log_level: "info".to_string(),
            command: None,
        };
        // Should print error but return Ok
        assert!(run(args).is_ok());
    }
}
