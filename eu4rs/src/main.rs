use clap::{Parser, Subcommand};
use eu4data::{
    Tradegood,
    history::ProvinceHistory,
    map::{DefaultMap, load_definitions},
};
use eu4txt::{DefaultEU4Txt, EU4Txt, from_node};
use image::{Rgb, RgbImage};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const PATH: &str =
    "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Europa Universalis IV\\common";

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Path to EU4 installation or directory to scan
    #[arg(long, default_value = PATH)]
    eu4_path: PathBuf,

    /// Pretty print parsed files
    #[arg(long)]
    pretty_print: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum, PartialEq)]
enum MapMode {
    TradeGoods,
    Political,
    All,
}

mod window;

#[derive(Subcommand)]
enum Commands {
    /// Dump tradegoods.txt to JSON
    DumpTradegoods,
    /// Render Map
    DrawMap {
        #[arg(long, default_value = "map_out.png")]
        output: PathBuf,
        #[arg(long, value_enum, default_value_t = MapMode::TradeGoods)]
        mode: MapMode,
    },
    /// Open the interactive map window
    /// Open the interactive map window
    DrawWindow {
        #[arg(long, default_value_t = true)] // Default to true for now as user likes it
        verbose: bool,
    },
    /// Render map to an image file (headless)
    Snapshot {
        /// Output path for the image
        #[arg(short, long, default_value = "snapshot.png")]
        output: String,
    },
}

fn dump_tradegoods(base_path: &std::path::Path) -> Result<(), String> {
    let path = base_path.join("tradegoods/00_tradegoods.txt");
    println!("Loading {:?}", path);
    // dump_tradegoods logic here
    let tokens = DefaultEU4Txt::open_txt(path.to_str().unwrap()).map_err(|e| e.to_string())?;
    let ast = DefaultEU4Txt::parse(tokens)?;
    let goods: HashMap<String, Tradegood> = from_node(&ast)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&goods).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn draw_map(base_path: &Path, output_path: &Path, mode: MapMode) -> Result<(), String> {
    // 1. Load Definitions (ID -> Color, Color -> ID)
    let def_path = base_path.join("map/definition.csv");
    println!("Loading definitions from {:?}", def_path);
    let definitions = load_definitions(&def_path).map_err(|e| e.to_string())?;

    // Build reverse map (RGB -> ID)
    let mut color_to_id: HashMap<(u8, u8, u8), u32> = HashMap::new();
    for (id, def) in &definitions {
        color_to_id.insert((def.r, def.g, def.b), *id);
    }

    // 1b. Load Default Map (Sea/Lakes)
    let default_map_path = base_path.join("map/default.map");
    println!("Loading default map from {:?}", default_map_path);
    let dm_tokens =
        DefaultEU4Txt::open_txt(default_map_path.to_str().unwrap()).map_err(|e| e.to_string())?;
    let dm_ast = DefaultEU4Txt::parse(dm_tokens)?;
    let default_map: DefaultMap = from_node(&dm_ast)?;

    let mut water_ids: HashSet<u32> = HashSet::new();
    for id in default_map.sea_starts {
        water_ids.insert(id);
    }
    for id in default_map.lakes {
        water_ids.insert(id);
    }
    println!("Loaded {} water provinces (sea+lakes).", water_ids.len());

    // 2. Load Data based on Mode
    let mut goods: HashMap<String, Tradegood> = HashMap::new();
    let mut countries: HashMap<String, eu4data::countries::Country> = HashMap::new();

    match mode {
        MapMode::TradeGoods => {
            let goods_path = base_path.join("common/tradegoods/00_tradegoods.txt");
            println!("Loading trade goods from {:?}", goods_path);
            let tokens =
                DefaultEU4Txt::open_txt(goods_path.to_str().unwrap()).map_err(|e| e.to_string())?;
            let ast = DefaultEU4Txt::parse(tokens)?;
            goods = from_node(&ast)?;
        }
        MapMode::Political => {
            println!("Loading country tags...");
            let tags = eu4data::countries::load_tags(base_path).map_err(|e| e.to_string())?;
            println!("Loading {} country definitions...", tags.len());
            countries = eu4data::countries::load_country_map(base_path, &tags);
            println!("Loaded {} countries.", countries.len());
        }
        MapMode::All => unreachable!("MapMode::All should be handled by caller"),
    }

    // 3. Load Province History (ID -> Data)
    let history_path = base_path.join("history/provinces");
    println!("Loading history from {:?}", history_path);
    let mut province_history: HashMap<u32, ProvinceHistory> = HashMap::new();
    let mut stats_history = (0, 0); // (ok, err)

    if history_path.is_dir() {
        for entry in std::fs::read_dir(history_path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "txt") {
                // Parse ID from filename "123 - Name.txt"
                let stem = path.file_stem().unwrap().to_str().unwrap();
                let id_part = stem.split_whitespace().next().unwrap();
                if let Ok(id) = id_part.parse::<u32>() {
                    // Parse file content
                    let tokens = match DefaultEU4Txt::open_txt(path.to_str().unwrap()) {
                        Ok(t) => t,
                        Err(e) => {
                            // Open failed (IO error)
                            stats_history.1 += 1;
                            if stats_history.1 <= 5 {
                                println!("Failed to open {}: {}", path.display(), e);
                            }
                            continue;
                        }
                    };

                    match DefaultEU4Txt::parse(tokens) {
                        Ok(ast) => match from_node::<ProvinceHistory>(&ast) {
                            Ok(hist) => {
                                stats_history.0 += 1;
                                province_history.insert(id, hist);
                            }
                            Err(e) => {
                                stats_history.1 += 1;
                                if stats_history.1 <= 5 {
                                    println!("Failed to deserialize {}: {}", path.display(), e);
                                }
                            }
                        },
                        Err(e) => {
                            if e == "NoTokens" {
                                // Empty file, safe to ignore (no history data).
                            } else {
                                stats_history.1 += 1;
                                if stats_history.1 <= 5 {
                                    println!("Failed to parse {}: {}", path.display(), e);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    println!(
        "History Stats: Success={}, Failure={}",
        stats_history.0, stats_history.1
    );

    // 4. Render
    let map_path = base_path.join("map/provinces.bmp");
    println!("Loading map image from {:?}", map_path);
    let img = image::open(map_path).map_err(|e| e.to_string())?.to_rgb8();
    let (width, height) = img.dimensions();
    let mut out_img = RgbImage::new(width, height);

    println!("Rendering...");
    for (x, y, pixel) in img.enumerate_pixels() {
        let (r, g, b) = (pixel[0], pixel[1], pixel[2]);
        if let Some(id) = color_to_id.get(&(r, g, b)) {
            let mut out_color = Rgb([100, 100, 100]); // Default Grey

            if water_ids.contains(id) {
                out_color = Rgb([64, 164, 223]); // Water Blue
            } else if let Some(hist) = province_history.get(id) {
                match mode {
                    MapMode::TradeGoods => {
                        if let Some(good) = hist
                            .trade_goods
                            .as_ref()
                            .and_then(|name| goods.get(name))
                            .filter(|g| g.color.len() >= 3)
                        {
                            let fr = (good.color[0] * 255.0) as u8;
                            let fg = (good.color[1] * 255.0) as u8;
                            let fb = (good.color[2] * 255.0) as u8;
                            out_color = Rgb([fr, fg, fb]);
                        }
                    }
                    MapMode::Political => {
                        if let Some(country) = hist
                            .owner
                            .as_ref()
                            .and_then(|tag| countries.get(tag))
                            .filter(|c| c.color.len() >= 3)
                        {
                            out_color = Rgb([country.color[0], country.color[1], country.color[2]]);
                        }
                    }
                    MapMode::All => unreachable!(),
                }
            }
            out_img.put_pixel(x, y, out_color);
        } else {
            out_img.put_pixel(x, y, Rgb([0, 0, 0]));
        }
    }

    out_img.save(output_path).map_err(|e| e.to_string())?;
    println!("Saved {:?}", output_path);
    Ok(())
}

struct ScanStats {
    success: usize,
    failure: usize,
    tokens: usize,
    nodes: usize,
}

fn pretty_print_dir(dir: &std::path::Path, pretty_print: bool) -> Result<ScanStats, String> {
    let mut stats = ScanStats {
        success: 0,
        failure: 0,
        tokens: 0,
        nodes: 0,
    };
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                // println!("{}", path.display());
                let sub_stats = pretty_print_dir(&path, pretty_print)?;
                stats.success += sub_stats.success;
                stats.failure += sub_stats.failure;
                stats.tokens += sub_stats.tokens;
                stats.nodes += sub_stats.nodes;
            } else if path.extension().is_some_and(|ext| ext == "txt") {
                // println!("{}", path.display());
                let tokens = match DefaultEU4Txt::open_txt(path.to_str().unwrap()) {
                    Ok(t) => t,
                    Err(_) => {
                        // println!("Expected encoding error potentially");
                        continue;
                    }
                };

                match DefaultEU4Txt::parse(tokens.clone()) {
                    // Clone because parse consumers tokens (or we change parse sig)
                    // Actually parse takes Vec<Token>, opens_txt returns Vec<Token>.
                    // We need the count before move, or just count tokens.len()
                    Ok(ast) => {
                        stats.success += 1;
                        stats.tokens += tokens.len();
                        stats.nodes += ast.node_count();
                        if pretty_print {
                            DefaultEU4Txt::pretty_print(&ast, 0)?;
                        }
                    }
                    Err(e) => {
                        if e != "NoTokens" {
                            println!("Parse Fail: {} : {}", path.display(), e);
                            stats.failure += 1;
                        }
                    }
                }
            }
        }
    }
    Ok(stats)
}
fn main() -> Result<(), String> {
    let args = Cli::parse();

    if let Some(cmd) = &args.command {
        match cmd {
            Commands::DumpTradegoods => {
                dump_tradegoods(&args.eu4_path)?;
                return Ok(());
            }
            Commands::DrawMap { output, mode } => {
                let base = args.eu4_path.parent().unwrap();
                match mode {
                    MapMode::All => {
                        println!("=== Rendering Political Map ===");
                        draw_map(
                            base,
                            &PathBuf::from("map_political.png"),
                            MapMode::Political,
                        )?;

                        println!("\n=== Rendering Trade Goods Map ===");
                        draw_map(
                            base,
                            &PathBuf::from("map_tradegoods.png"),
                            MapMode::TradeGoods,
                        )?;
                    }
                    _ => {
                        draw_map(base, output, *mode)?;
                    }
                }
                return Ok(());
            }
            Commands::DrawWindow { verbose } => {
                pollster::block_on(window::run(*verbose));
                return Ok(());
            }
            Commands::Snapshot { output } => {
                let path = std::path::Path::new(output);
                pollster::block_on(window::snapshot(path));
                return Ok(());
            }
        }
    }

    // Default behavior handling:
    // If pretty_print flag is set, run the scanner.
    // Otherwise, default to GUI window.
    if args.pretty_print {
        match pretty_print_dir(&args.eu4_path, args.pretty_print) {
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
        pollster::block_on(window::run(true));
    }

    Ok(())
}
