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
    println!("Checking Model Quota Status...\n");

    // Try Antigravity detection first (Windows only)
    #[cfg(target_os = "windows")]
    {
        match check_antigravity_quota() {
            Ok(quotas) if !quotas.is_empty() => {
                println!("📊 **Antigravity Model Quotas**\n");
                println!("| Model | Quota | Status |");
                println!("|-------|-------|--------|");
                for q in &quotas {
                    println!(
                        "| {} | {}% | {} |",
                        q.label,
                        q.percentage,
                        quota_status_label(q.percentage)
                    );
                }
                return Ok(());
            }
            Ok(_) => {
                println!("⚠️  Antigravity detected but no quota data available.\n");
            }
            Err(e) => {
                println!("ℹ️  Antigravity not detected: {}\n", e);
            }
        }
        println!("Falling back to API key validation...\n");
    }

    #[cfg(not(target_os = "windows"))]
    {
        println!("ℹ️  Antigravity detection not available on this platform.\n");
    }

    // Fallback to API key validation
    let client = Client::new();

    let anthropic_key = env::var("ANTHROPIC_API_KEY").ok();
    let claude_status = if let Some(key) = anthropic_key {
        match check_anthropic(&client, &key) {
            Ok(s) => s,
            Err(e) => format!("Error: {}", e),
        }
    } else {
        "Skipped (ANTHROPIC_API_KEY not set)".to_string()
    };

    let gemini_key = env::var("GEMINI_API_KEY").ok();
    let gemini_status = if let Some(key) = gemini_key {
        match check_gemini(&client, &key) {
            Ok(s) => s,
            Err(e) => format!("Error: {}", e),
        }
    } else {
        "Skipped (GEMINI_API_KEY not set)".to_string()
    };

    println!("📊 **API Key Validation** (no quota levels available)\n");
    println!("| Model Family | Status |");
    println!("|--------------|--------|");
    println!("| Claude | {} |", claude_status);
    println!("| Gemini | {} |", gemini_status);

    Ok(())
}

/// Returns a human-readable status based on quota percentage
#[cfg(target_os = "windows")]
fn quota_status_label(percentage: u8) -> &'static str {
    match percentage {
        0 => "🔴 Exhausted",
        1..=10 => "🔴 Critical",
        11..=50 => "🟡 Low",
        _ => "🟢 Healthy",
    }
}

// ============================================================================
// Windows-only Antigravity Detection
// ============================================================================

#[cfg(target_os = "windows")]
mod antigravity {
    use anyhow::{anyhow, Result};
    use regex::Regex;
    use reqwest::blocking::Client;
    use serde::Deserialize;
    use std::process::Command;
    use std::time::Duration;

    /// Quota info for a single model
    pub struct ModelQuota {
        pub label: String,
        pub percentage: u8,
    }

    /// Detect Antigravity language server and fetch quota data
    pub fn check_antigravity_quota() -> Result<Vec<ModelQuota>> {
        // Step 1: Find the language server process
        let (csrf_token, pid) = find_antigravity_process()?;

        // Step 2: Find listening ports for this PID
        let ports = find_listening_ports(pid)?;
        if ports.is_empty() {
            return Err(anyhow!("No listening ports found for PID {}", pid));
        }

        // Step 3: Try each port until we get a valid response
        let client = Client::builder()
            .danger_accept_invalid_certs(true) // localhost self-signed cert
            .timeout(Duration::from_secs(5))
            .build()?;

        for port in ports {
            if let Ok(quotas) = fetch_user_status(&client, port, &csrf_token) {
                return Ok(quotas);
            }
        }

        Err(anyhow!("Could not connect to Antigravity API"))
    }

    /// Find the Antigravity language_server process and extract CSRF token
    fn find_antigravity_process() -> Result<(String, u32)> {
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                r#"Get-CimInstance Win32_Process -Filter "name='language_server_windows_x64.exe'" | Select-Object ProcessId,CommandLine | ConvertTo-Json"#,
            ])
            .output()?;

        if !output.status.success() {
            return Err(anyhow!("PowerShell command failed"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value = serde_json::from_str(stdout.trim())
            .map_err(|_| anyhow!("No language_server process found"))?;

        // Handle both single object and array responses
        let processes: Vec<&serde_json::Value> = if json.is_array() {
            json.as_array().unwrap().iter().collect()
        } else {
            vec![&json]
        };

        // Find Antigravity process (has --app_data_dir antigravity in command line)
        let token_re = Regex::new(r"--csrf_token[=\s]+([a-f0-9\-]+)").unwrap();
        for proc in processes {
            let cmd_line = proc["CommandLine"].as_str().unwrap_or("");
            let pid = proc["ProcessId"].as_u64().unwrap_or(0) as u32;

            // Check if this is Antigravity (not Codeium or other)
            if cmd_line.to_lowercase().contains("antigravity") {
                // Extract CSRF token
                if let Some(caps) = token_re.captures(cmd_line) {
                    let token = caps.get(1).unwrap().as_str().to_string();
                    return Ok((token, pid));
                }
            }
        }

        Err(anyhow!("Antigravity language_server not found"))
    }

    /// Find listening TCP ports for a given PID
    fn find_listening_ports(pid: u32) -> Result<Vec<u16>> {
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "Get-NetTCPConnection -OwningProcess {} -State Listen -ErrorAction SilentlyContinue | Select-Object -ExpandProperty LocalPort | ConvertTo-Json",
                    pid
                ),
            ])
            .output()?;

        if !output.status.success() {
            return Ok(vec![]);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let trimmed = stdout.trim();

        if trimmed.is_empty() {
            return Ok(vec![]);
        }

        // Parse JSON (can be single number or array)
        let json: serde_json::Value =
            serde_json::from_str(trimmed).unwrap_or(serde_json::json!([]));

        let mut ports = Vec::new();
        if let Some(arr) = json.as_array() {
            for v in arr {
                if let Some(p) = v.as_u64() {
                    ports.push(p as u16);
                }
            }
        } else if let Some(p) = json.as_u64() {
            ports.push(p as u16);
        }

        // Sort descending - higher ports are more likely to be the API
        ports.sort_by(|a, b| b.cmp(a));
        Ok(ports)
    }

    /// Fetch user status from Antigravity API
    fn fetch_user_status(client: &Client, port: u16, csrf_token: &str) -> Result<Vec<ModelQuota>> {
        let url = format!(
            "https://127.0.0.1:{}/exa.language_server_pb.LanguageServerService/GetUserStatus",
            port
        );

        let body = serde_json::json!({
            "metadata": {
                "ideName": "antigravity",
                "extensionName": "antigravity",
                "locale": "en"
            }
        });

        let resp = client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Connect-Protocol-Version", "1")
            .header("X-Codeium-Csrf-Token", csrf_token)
            .json(&body)
            .send()?;

        if !resp.status().is_success() {
            return Err(anyhow!("API returned {}", resp.status()));
        }

        let data: UserStatusResponse = resp.json()?;
        let mut quotas = Vec::new();

        if let Some(configs) = data
            .user_status
            .cascade_model_config_data
            .as_ref()
            .and_then(|d| d.client_model_configs.as_ref())
        {
            for config in configs {
                if let Some(quota_info) = &config.quota_info {
                    let percentage = (quota_info.remaining_fraction.unwrap_or(0.0) * 100.0) as u8;
                    quotas.push(ModelQuota {
                        label: config
                            .label
                            .clone()
                            .unwrap_or_else(|| "Unknown".to_string()),
                        percentage,
                    });
                }
            }
        }

        Ok(quotas)
    }

    // Response types for GetUserStatus API
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct UserStatusResponse {
        user_status: UserStatus,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct UserStatus {
        cascade_model_config_data: Option<CascadeModelConfigData>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CascadeModelConfigData {
        client_model_configs: Option<Vec<ClientModelConfig>>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ClientModelConfig {
        label: Option<String>,
        quota_info: Option<QuotaInfo>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct QuotaInfo {
        remaining_fraction: Option<f64>,
    }
}

#[cfg(target_os = "windows")]
use antigravity::{check_antigravity_quota, ModelQuota};

#[cfg(target_os = "windows")]
#[allow(dead_code)]
type QuotaResult = Vec<ModelQuota>;

fn check_anthropic(client: &Client, key: &str) -> Result<String> {
    let resp = client
        .get("https://api.anthropic.com/v1/models")
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .send()?;

    if resp.status().is_success() {
        Ok("Active (API Key Valid)".to_string())
    } else {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        if status.as_u16() == 401 {
            Ok("Invalid API Key".to_string())
        } else if text.contains("credit balance is too low") {
            Ok("Out of Credits (Balance too low)".to_string())
        } else {
            Ok(format!("Failed (HTTP {}): {}", status, text))
        }
    }
}

fn check_gemini(client: &Client, key: &str) -> Result<String> {
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
