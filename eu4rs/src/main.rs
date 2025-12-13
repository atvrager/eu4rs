use eu4txt::{DefaultEU4Txt, EU4Txt};
use std::path::PathBuf;
use clap::Parser;

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
}

struct ScanStats {
    success: usize,
    failure: usize,
    tokens: usize,
    nodes: usize,
}

fn pretty_print_dir(dir: &std::path::Path, pretty_print: bool) -> Result<ScanStats, String> {
    let mut stats = ScanStats { success: 0, failure: 0, tokens: 0, nodes: 0 };
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
            } else {
                if path.extension().map_or(false, |ext| ext == "txt") {
                     // println!("{}", path.display());
                    let tokens = match DefaultEU4Txt::open_txt(path.to_str().unwrap()) {
                        Ok(t) => t,
                        Err(_) => {
                            // println!("Expected encoding error potentially");
                            continue;
                        }
                    };
                    
                    match DefaultEU4Txt::parse(tokens.clone()) { // Clone because parse consumers tokens (or we change parse sig)
                        // Actually parse takes Vec<Token>, opens_txt returns Vec<Token>.
                        // We need the count before move, or just count tokens.len()
                        Ok(ast) => {
                            stats.success += 1;
                            stats.tokens += tokens.len();
                            stats.nodes += ast.node_count();
                             if pretty_print {
                                DefaultEU4Txt::pretty_print(&ast, 0)?;
                            }
                        },
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
    }
    Ok(stats)
}
fn main() -> Result<(), String> {
    let args = Cli::parse();
    match pretty_print_dir(&args.eu4_path, args.pretty_print) {
        Ok(stats) => {
            println!("Done! Success: {}, Failure: {}", stats.success, stats.failure);
            println!("Total Tokens: {}, Total Nodes: {}", stats.tokens, stats.nodes);
        }
        Err(e) => {
            println!("pretty_print_dir critical failure: {}", e);
        }
    }

    Ok(())
}
