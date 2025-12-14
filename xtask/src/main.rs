use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::env;
use std::process::{Command, Stdio};

#[derive(Parser)]
#[command(name = "xtask")]
#[command(about = "Development automation scripts", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Local CI pipeline: fmt, clippy, test, build
    Ci,
    /// Update snapshot tests: renders images and runs tests
    Snapshot,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Ci => run_ci(),
        Commands::Snapshot => run_snapshot(),
    }
}

fn run_ci() -> Result<()> {
    println!("Running Local CI Pipeline...");

    println!("\n[1/4] Checking Formatting...");
    run_command("cargo", &["fmt", "--", "--check"])?;

    println!("\n[2/4] Running Clippy...");
    run_command("cargo", &["clippy", "--", "-D", "warnings"])?;

    println!("\n[3/4] Running Tests...");
    run_command("cargo", &["test"])?;

    println!("\n[4/4] Building Release...");
    run_command("cargo", &["build", "--release"])?;

    println!("\nLocal CI Passed! 🚀");
    Ok(())
}

fn run_snapshot() -> Result<()> {
    println!("Updating snapshot tests...");

    // Set environment variable for the process
    env::set_var("UPDATE_SNAPSHOTS", "1");

    println!("1. Rendering new snapshots...");

    // Generate images
    let modes = [
        ("map_province.png", "province"),
        ("map_political.png", "political"),
        ("map_tradegoods.png", "trade-goods"),
        ("map_religion.png", "religion"),
        ("map_culture.png", "culture"),
    ];

    for (output, mode) in modes {
        println!("  Rendering {}...", output);
        run_command(
            "cargo",
            &[
                "run", "--bin", "eu4rs", "--", "snapshot", "--output", output, "--mode", mode,
            ],
        )?;
    }

    println!("Snapshots rendered. Running tests to verify and commit...");
    run_command("cargo", &["test", "--bin", "eu4rs"])?;

    println!("Snapshot update complete.");
    Ok(())
}

fn run_command(cmd: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(cmd)
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("Failed to execute {}", cmd))?;

    if !status.success() {
        anyhow::bail!("Command failed: {} {}", cmd, args.join(" "));
    }
    Ok(())
}
