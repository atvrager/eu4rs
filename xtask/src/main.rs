use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use reqwest::blocking::Client;
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
    /// Check API quota availability for Claude and Gemini
    Quota,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Try ensuring .env is loaded if present
    let _ = dotenvy::dotenv();

    match cli.command {
        Commands::Ci => run_ci(),
        Commands::Snapshot => run_snapshot(),
        Commands::Quota => run_quota(),
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

fn run_quota() -> Result<()> {
    println!("Checking Model Quota Status...");
    let client = Client::new();

    // Check Claude
    let anthropic_key = env::var("ANTHROPIC_API_KEY").ok();
    let claude_status = if let Some(key) = anthropic_key {
        match check_anthropic(&client, &key) {
            Ok(s) => s,
            Err(e) => format!("Error: {}", e),
        }
    } else {
        "Skipped (ANTHROPIC_API_KEY not set)".to_string()
    };

    // Check Gemini
    let gemini_key = env::var("GEMINI_API_KEY").ok();
    let gemini_status = if let Some(key) = gemini_key {
        match check_gemini(&client, &key) {
            Ok(s) => s,
            Err(e) => format!("Error: {}", e),
        }
    } else {
        "Skipped (GEMINI_API_KEY not set)".to_string()
    };

    println!("\n| Model Family | Status | Details |");
    println!("|---|---|---|");
    println!("| **Claude** | {} |", claude_status);
    println!("| **Gemini** | {} |", gemini_status);

    Ok(())
}

fn check_anthropic(client: &Client, key: &str) -> Result<String> {
    // Make a minimal request to get headers
    // Using a dummy message request
    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "model": "claude-3-opus-20240229",
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "hi"}]
        }))
        .send()?;

    let headers = resp.headers();
    if let Some(remaining) = headers.get("anthropic-ratelimit-requests-remaining") {
        let count = remaining.to_str().unwrap_or("?").to_string();
        Ok(format!("Active (Requests Remaining: {})", count))
    } else if resp.status().is_success() {
        Ok("Active (No Rate Limit Headers)".to_string())
    } else {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        if text.contains("credit balance is too low") {
            Ok("Out of Credits (Balance too low)".to_string())
        } else {
            Ok(format!("Failed (HTTP {}): {}", status, text))
        }
    }
}

fn check_gemini(client: &Client, key: &str) -> Result<String> {
    // Check model validity by listing models (more robust than checking specific model)
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={}&pageSize=1",
        key
    );
    let resp = client.get(&url).send()?;

    if resp.status().is_success() {
        Ok("Active (API Reachable)".to_string())
    } else {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        Ok(format!("Failed (HTTP {}): {}", status, text))
    }
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
