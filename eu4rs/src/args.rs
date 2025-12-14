use clap::{Parser, Subcommand};
use std::path::PathBuf;

const PATH: &str =
    "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Europa Universalis IV\\common";

#[derive(Parser, Debug, PartialEq)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Path to EU4 installation or directory to scan
    #[arg(long, default_value = PATH)]
    pub eu4_path: PathBuf,

    /// Pretty print parsed files
    #[arg(long)]
    pub pretty_print: bool,

    /// Language to load (e.g. "english", "spanish")
    #[arg(long, default_value = "english")]
    pub language: String,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum, PartialEq, Eq, Hash)]
pub enum MapMode {
    Province,
    TradeGoods,
    Political,
    Religion,
    Culture,
    All,
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum Commands {
    /// Dump `tradegoods.txt` to JSON format.
    DumpTradegoods,

    /// Render a map of the world.
    DrawMap {
        /// Output path for the image (default: "map_out.png").
        #[arg(long, default_value = "map_out.png")]
        output: PathBuf,
        /// The map mode to render (e.g., TradeGoods, Political).
        #[arg(long, value_enum, default_value_t = MapMode::TradeGoods)]
        mode: MapMode,
    },

    /// Open the interactive map window (default behavior).
    DrawWindow {
        /// Enable verbose logging.
        #[arg(long, default_value_t = true)]
        verbose: bool,
    },

    /// Render map to an image file (headless mode).
    Snapshot {
        /// Output path for the image.
        #[arg(short, long, default_value = "snapshot.png")]
        output: String,

        /// The map mode to render (e.g., TradeGoods, Political).
        #[arg(long, value_enum, default_value_t = MapMode::Province)]
        mode: MapMode,
    },

    /// Lookup a localisation key.
    ///
    /// Example: `lookup PROV1` -> "Stockholm"
    Lookup {
        /// The key to look up (e.g. PROV1, trade_efficiency).
        key: String,
    },

    /// List all available languages found in the localisation directory.
    Languages,
}
