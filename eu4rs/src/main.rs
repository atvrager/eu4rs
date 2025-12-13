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

fn pretty_print_dir(dir: &std::path::Path, pretty_print: bool) -> Result<(), String> {
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                println!("{}", path.display());
                pretty_print_dir(&path, pretty_print)?;
            } else {
                if path.extension().ok_or("no extension")? != "txt" {
                    continue; 
                }
                println!("{}", path.display());
                let tokens =
                    DefaultEU4Txt::open_txt(path.to_str().unwrap()).or(Err("failed to open file"))?;
                let _ast = match DefaultEU4Txt::parse(tokens) {
                    Ok(ast) => ast,
                    Err(e) => {
                        if e == "NoTokens" {
                            continue;
                        }
                        println!("{}", e);
                        return Err(e);
                    }
                };
                if pretty_print {
                    DefaultEU4Txt::pretty_print(&_ast, 0)?;
                }
            }
        }
    }
    Ok(())
}
fn main() -> Result<(), String> {
    let args = Cli::parse();
    match pretty_print_dir(&args.eu4_path, args.pretty_print) {
        Ok(()) => {
            println!("pretty_print_dir ok!");
        }
        Err(e) => {
            println!("pretty_print_dir failed: {}", e);
        }
    }

    Ok(())
}
