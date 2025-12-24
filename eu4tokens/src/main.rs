use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod elf;
mod pe;

#[derive(Parser)]
#[command(name = "eu4tokens")]
#[command(about = "Extract binary token mappings from EU4 game files")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract tokens from EU4 game binary
    Derive {
        /// Path to EU4 game directory or executable
        #[arg(value_name = "PATH")]
        path: PathBuf,

        /// Output file for tokens (default: assets/tokens/eu4.txt)
        #[arg(short, long, default_value = "assets/tokens/eu4.txt")]
        output: PathBuf,
    },

    /// Use an existing tokens file (copy/validate)
    Use {
        /// Path to existing tokens.txt file
        #[arg(value_name = "TOKENS_FILE")]
        path: PathBuf,

        /// Output file (default: assets/tokens/eu4.txt)
        #[arg(short, long, default_value = "assets/tokens/eu4.txt")]
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Derive { path, output } => {
            derive_tokens(&path, &output)?;
        }
        Commands::Use { path, output } => {
            use_existing_tokens(&path, &output)?;
        }
    }

    Ok(())
}

fn derive_tokens(path: &std::path::Path, output: &std::path::Path) -> Result<()> {
    log::info!("Deriving tokens from: {}", path.display());

    // Find the EU4 binary
    let binary_path = find_eu4_binary(path)?;
    log::info!("Found binary: {}", binary_path.display());

    // Detect binary format and extract tokens
    let tokens = extract_tokens(&binary_path)?;
    log::info!("Extracted {} tokens", tokens.len());

    // Write output
    write_tokens(&tokens, output)?;
    log::info!("Written to: {}", output.display());

    println!(
        "Successfully extracted {} tokens to {}",
        tokens.len(),
        output.display()
    );
    Ok(())
}

fn use_existing_tokens(path: &std::path::Path, output: &std::path::Path) -> Result<()> {
    log::info!("Using existing tokens from: {}", path.display());

    // Read and validate the tokens file
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read tokens file: {}", path.display()))?;

    let tokens = parse_tokens_file(&content)?;
    log::info!("Loaded {} tokens", tokens.len());

    // Ensure output directory exists
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create output directory: {}", parent.display()))?;
    }

    // Copy to output location
    std::fs::write(output, &content)
        .with_context(|| format!("Failed to write tokens to: {}", output.display()))?;

    println!(
        "Successfully copied {} tokens to {}",
        tokens.len(),
        output.display()
    );
    Ok(())
}

fn find_eu4_binary(path: &std::path::Path) -> Result<PathBuf> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }

    // Look for common binary names in directory
    let candidates = ["eu4", "eu4.exe", "europa4", "europa4.exe"];
    for candidate in candidates {
        let binary_path = path.join(candidate);
        if binary_path.exists() {
            return Ok(binary_path);
        }
    }

    anyhow::bail!(
        "Could not find EU4 binary in {}. Expected one of: {:?}",
        path.display(),
        candidates
    )
}

fn extract_tokens(binary_path: &std::path::Path) -> Result<Vec<(u16, String)>> {
    let file = std::fs::File::open(binary_path)
        .with_context(|| format!("Failed to open binary: {}", binary_path.display()))?;

    let mmap =
        unsafe { memmap2::Mmap::map(&file) }.with_context(|| "Failed to memory-map binary")?;

    // Detect format using magic bytes
    if mmap.len() < 4 {
        anyhow::bail!("Binary file too small");
    }

    match &mmap[0..4] {
        // ELF magic: 0x7f 'E' 'L' 'F'
        [0x7f, b'E', b'L', b'F'] => {
            log::info!("Detected ELF binary (Linux)");
            elf::extract_tokens(&mmap)
        }
        // PE magic: 'M' 'Z'
        [b'M', b'Z', ..] => {
            log::info!("Detected PE binary (Windows)");
            pe::extract_tokens(&mmap)
        }
        magic => {
            anyhow::bail!(
                "Unknown binary format. Magic bytes: {:02x} {:02x} {:02x} {:02x}",
                magic[0],
                magic[1],
                magic[2],
                magic[3]
            )
        }
    }
}

fn write_tokens(tokens: &[(u16, String)], output: &std::path::Path) -> Result<()> {
    // Ensure output directory exists
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create output directory: {}", parent.display()))?;
    }

    let mut content = String::new();
    for (id, name) in tokens {
        content.push_str(&format!("0x{:04x} {}\n", id, name));
    }

    std::fs::write(output, &content)
        .with_context(|| format!("Failed to write tokens to: {}", output.display()))?;

    Ok(())
}

fn parse_tokens_file(content: &str) -> Result<Vec<(u16, String)>> {
    let mut tokens = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid token line {}: expected 'ID NAME'", line_num + 1);
        }

        let id_str = parts[0].trim_start_matches("0x").trim_start_matches("0X");
        let id = u16::from_str_radix(id_str, 16)
            .with_context(|| format!("Invalid token ID on line {}: {}", line_num + 1, parts[0]))?;

        tokens.push((id, parts[1].to_string()));
    }

    Ok(tokens)
}
