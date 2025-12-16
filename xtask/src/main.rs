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
    /// Clean auto-generated type files (preserves mod.rs)
    Clean,

    /// Analyze data coverage (local) or verify docs (CI)
    Coverage {
        /// Path to EU4 installation (auto-detected if not provided)
        #[arg(long)]
        eu4_path: Option<String>,
        /// Generate static docs/supported_fields.md (CI verification)
        #[arg(long)]
        doc_gen: bool,
        /// Discover schema from real game files
        #[arg(long)]
        discover: bool,
        /// Update eu4data/src/generated/categories.rs and schema.rs
        #[arg(long)]
        update: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Try ensuring .env is loaded if present
    let _ = dotenvy::dotenv();

    match cli.command {
        Commands::Ci => run_ci(),
        Commands::Snapshot => run_snapshot(),
        Commands::Quota => run_quota(),
        Commands::Clean => run_clean(),

        Commands::Coverage {
            eu4_path,
            doc_gen,
            discover,
            update,
        } => run_coverage(eu4_path, doc_gen, discover, update),
    }
}

fn run_ci() -> Result<()> {
    println!("Running Local CI Pipeline...");

    println!("\n[1/4] Checking Formatting...");
    run_command("cargo", &["fmt", "--", "--check"])?;

    println!("\n[2/4] Running Clippy...");
    run_command("cargo", &["clippy", "--", "-D", "warnings"])?;

    println!("\n[3/4] Running Tests...");
    // Use nextest for faster parallel test execution
    // Exclude xtask to avoid Windows file lock (xtask.exe is running)
    run_command(
        "cargo",
        &["nextest", "run", "--workspace", "--exclude", "xtask"],
    )?;

    println!("\n[4/4] Building Release...");
    run_command("cargo", &["build", "--release"])?;

    println!("\n[5/5] Verifying Documentation...");
    // Just run doc-gen in check mode effectively by re-generating and checking git status?
    // Actually, run_coverage with doc_gen will write the file.
    // CI should fail if the checked-in file differs.
    // simpler: run doc-gen, then check if git detects changes.
    run_coverage(Some("mock".to_string()), true, false, false)?;

    // Check for unstaged changes in docs/
    let status = Command::new("git")
        .args(["diff", "--exit-code", "docs/supported_fields.md"])
        .status()?;

    if !status.success() {
        anyhow::bail!("docs/supported_fields.md is out of date! Run `cargo xtask coverage --doc-gen` and commit the changes.");
    }

    println!("\nLocal CI Passed! 🚀");
    Ok(())
}

fn run_snapshot() -> Result<()> {
    println!("Updating snapshot tests...");

    // Set environment variable for the process
    env::set_var("UPDATE_SNAPSHOTS", "1");

    println!("Running snapshot tests with UPDATE_SNAPSHOTS=1...");
    println!("This will load world data once and generate all 5 maps.");

    // Run the snapshot tests directly - they will generate and save the images
    run_command("cargo", &["test", "-p", "eu4viz", "window::tests"])?;

    println!("\nSnapshot update complete.");
    Ok(())
}

fn run_clean() -> Result<()> {
    println!("Cleaning generated files...");
    let types_dir = std::path::Path::new("eu4data/src/generated/types");
    let module_list_path = types_dir.join("module_list.rs");

    // 1. Identify valid generated files from module_list.rs
    let mut known_files = std::collections::HashSet::new();
    known_files.insert("mod.rs".to_string());
    known_files.insert("module_list.rs".to_string());

    if module_list_path.exists() {
        let content = std::fs::read_to_string(&module_list_path)?;
        for line in content.lines() {
            if let Some(rest) = line.trim().strip_prefix("pub mod ") {
                if let Some(mod_name) = rest.strip_suffix(";") {
                    known_files.insert(format!("{}.rs", mod_name.trim()));
                }
            }
        }
    } else {
        println!("⚠️  module_list.rs not found. Assuming all files are unknown/safe to delete? No, strictly conservative.");
        // If module_list doesn't exist, we can't be sure what is generated.
        // But if mod.rs exists, maybe we can assume?
        // For safety, warn and exit? Or maybe just rely on the user having built once?
        // Let's proceed but warn.
    }

    if types_dir.exists() {
        let mut deleted_count = 0;
        for entry in std::fs::read_dir(types_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if known_files.contains(name) {
                        // It's a known generated file (or mod.rs/module_list.rs)
                        // Clean matching generated files, but preserve the metadata files
                        if name != "mod.rs" && name != "module_list.rs" {
                            std::fs::remove_file(&path)
                                .context(format!("Failed to remove {:?}", path))?;
                            deleted_count += 1;
                        }
                    } else {
                        // Unknown file!
                        println!("⚠️  Skipping unknown file: {:?}", name);
                    }
                }
            }
        }
        if deleted_count > 0 {
            println!(
                "✅ Removed {} generated type files (preserved mod.rs)",
                deleted_count
            );
            println!("Build script will detect changes in types/ directory and regenerate on next build.");
        } else {
            println!("Nothing to clean (or all files were unknown).");
        }
    } else {
        println!("Nothing to clean.");
    }

    Ok(())
}

fn run_quota() -> Result<()> {
    println!("Checking Model Quota Status...\n");

    // Try Antigravity detection first (Windows only)
    #[cfg(target_os = "windows")]
    let antigravity_success = {
        match check_antigravity_quota() {
            Ok(quotas) if !quotas.is_empty() => {
                println!("📊 **Antigravity Model Quotas**\n");
                println!("| Model | Quota | Status | Refreshes |");
                println!("|-------|-------|--------|----------|");
                for q in &quotas {
                    let refresh_str = q
                        .reset_time
                        .as_ref()
                        .map(format_refresh_time)
                        .unwrap_or_else(|| "Unknown".to_string());
                    println!(
                        "| {} | {}% | {} | {} |",
                        q.label,
                        q.percentage,
                        quota_status_label(q.percentage),
                        refresh_str
                    );
                }
                true
            }
            Ok(_) => {
                println!("⚠️  Antigravity detected but no quota data available.\n");
                false
            }
            Err(e) => {
                println!("ℹ️  Antigravity not detected: {}\n", e);
                false
            }
        }
    };

    #[cfg(not(target_os = "windows"))]
    let antigravity_success = {
        println!("ℹ️  Antigravity detection not available on this platform.\n");
        false
    };

    // Fallback to API key validation if Antigravity failed
    if !antigravity_success {
        println!("Falling back to API key validation...\n");

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
    }

    // Add geolocation triangulation (Windows only)
    #[cfg(target_os = "windows")]
    {
        println!("\n🌍 **Geolocation (Experimental)**\n");

        // Get timezone info
        if let Some(tz) = geolocation::get_timezone_info() {
            let offset_sign = if tz.utc_offset_hours >= 0 { "+" } else { "" };
            println!(
                "Timezone: {} (UTC{}{})",
                tz.name, offset_sign, tz.utc_offset_hours
            );
        } else {
            println!("Timezone: Could not detect");
        }

        // Get local time using chrono
        let now = chrono::Local::now();
        println!("Local time: {}\n", now.format("%Y-%m-%d %H:%M:%S"));

        // Ping triangulation
        println!("Ping triangulation (this may take a few seconds)...");
        let ping_results = geolocation::ping_targets();
        println!("{}", geolocation::format_ping_results(&ping_results));

        // Triangulate position
        let location = geolocation::triangulate(&ping_results);
        println!("Estimated location: {}", location.description);
        println!("Confidence: {}\n", location.confidence);

        // Sleep prediction
        let local_hour = now.format("%H").to_string().parse::<u32>().unwrap_or(12);
        let day_of_week = now.format("%w").to_string().parse::<u32>().unwrap_or(1);
        let month = now.format("%m").to_string().parse::<u32>().unwrap_or(1);

        let sleep_pred = geolocation::predict_sleep(
            geolocation::get_timezone_info().as_ref(),
            &location,
            local_hour,
            day_of_week,
            month,
        );

        if sleep_pred.should_burn_quota {
            println!("💤 **Sleep Prediction**: {}", sleep_pred.reason);
            println!("   → Consider burning through remaining quota before bed!");
        } else {
            println!("☀️  **Sleep Prediction**: {}", sleep_pred.reason);
        }
    }

    Ok(())
}

fn run_coverage(
    eu4_path: Option<String>,
    doc_gen: bool,
    discover: bool,
    update: bool,
) -> Result<()> {
    if doc_gen {
        let content = eu4data::coverage::generate_static_docs();
        let path = std::path::Path::new("docs/supported_fields.md");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        println!("✅ Generated docs/supported_fields.md");
        return Ok(());
    }

    let buf;
    let path = match eu4_path {
        Some(p) => {
            buf = std::path::PathBuf::from(p);
            buf.as_path()
        }
        None => {
            // Auto-detect
            match eu4data::path::detect_game_path() {
                Some(p) => {
                    println!("🔎 Auto-detected EU4 path: {:?}", p);
                    buf = p;
                    buf.as_path()
                }
                None => {
                    println!("⚠️  Could not detect EU4 installation.");
                    println!("Please provide a path via --eu4-path <PATH>");
                    return Ok(());
                }
            }
        }
    };

    if !path.exists() {
        println!("⚠️  EU4 path not found: {:?}", path);
        println!("Please provide a valid path via --eu4-path");
        return Ok(());
    }

    if discover || update {
        println!("🔎 Discovery mode enabled. This may take a minute...");
        if update {
            println!("💾 Will update eu4data/src/generated/categories.rs and schema.rs");
        }
    }

    if update {
        // Generate categories first (schema depends on this)
        let categories_content = eu4data::discovery::generate_categories_file(path)
            .context("Failed to generate categories")?;
        std::fs::write("eu4data/src/generated/categories.rs", categories_content)
            .context("Failed to write categories file")?;
        println!("✅ Updated eu4data/src/generated/categories.rs");

        // Then generate schema (uses the discovered categories)
        let schema_content =
            eu4data::discovery::generate_schema_file(path).context("Failed to generate schema")?;
        std::fs::write("eu4data/src/generated/schema.rs", schema_content)
            .context("Failed to write schema file")?;
        println!("✅ Updated eu4data/src/generated/schema.rs");
    }

    let report = eu4data::coverage::analyze_coverage(path, discover && !update)
        .context("Failed to analyze coverage")?;

    // Always print to terminal
    println!("{}", report.to_terminal());

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

/// Formats a reset time as a human-readable relative duration
#[cfg(target_os = "windows")]
fn format_refresh_time(reset_time: &chrono::DateTime<chrono::Utc>) -> String {
    use chrono::Utc;
    let now = Utc::now();

    if *reset_time <= now {
        return "Refreshed ✓".to_string();
    }

    let duration = *reset_time - now;
    let hours = duration.num_hours();
    let mins = duration.num_minutes() % 60;

    if hours > 24 {
        let days = hours / 24;
        format!("in {}d {}h", days, hours % 24)
    } else if hours > 0 {
        format!("in {}h {}m", hours, mins)
    } else if mins > 0 {
        format!("in {}m", mins)
    } else {
        "in <1m".to_string()
    }
}

// ============================================================================
// Windows-only Antigravity Detection
// ============================================================================

#[cfg(target_os = "windows")]
mod antigravity {
    use anyhow::{anyhow, Result};
    use chrono::{DateTime, Utc};
    use regex::Regex;
    use reqwest::blocking::Client;
    use serde::Deserialize;
    use std::process::Command;
    use std::time::Duration;

    /// Quota info for a single model
    pub struct ModelQuota {
        pub label: String,
        pub percentage: u8,
        pub reset_time: Option<DateTime<Utc>>,
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
                    let reset_time = quota_info
                        .reset_time
                        .as_ref()
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc));
                    quotas.push(ModelQuota {
                        label: config
                            .label
                            .clone()
                            .unwrap_or_else(|| "Unknown".to_string()),
                        percentage,
                        reset_time,
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
        reset_time: Option<String>,
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

// ============================================================================
// Geolocation Triangulation (Windows-only, for fun!)
// TODO: Map estimated lat/lon to nearest EU4 province for cheeky output like
//       "You appear to be in Province 224 (San Francisco)" or similar.
// ============================================================================

#[cfg(target_os = "windows")]
mod geolocation {
    use std::process::Command;

    /// Ping target with known location
    pub struct PingTarget {
        pub name: &'static str,
        pub host: &'static str,
        pub lat: f64,
        pub lon: f64,
    }

    /// Result of pinging a target
    pub struct PingResult {
        pub target: &'static PingTarget,
        pub rtt_ms: Option<f64>,
    }

    /// Guessed location
    #[allow(dead_code)]
    pub struct LocationGuess {
        pub description: String,
        pub lat: f64,
        pub lon: f64,
        pub confidence: &'static str,
    }

    /// Timezone information from the system
    #[allow(dead_code)]
    pub struct TimezoneInfo {
        pub name: String,
        pub utc_offset_hours: i32,
        pub utc_offset_minutes: i32,
    }

    /// Sleep prediction result
    pub struct SleepPrediction {
        pub should_burn_quota: bool,
        pub reason: String,
    }

    // Three reference points forming a nice global triangle:
    // Using AWS EC2 regional endpoints - they're unicast (fixed location),
    // respond reliably to ICMP, and have well-known geographic positions.
    // - Ireland (Europe)
    // - N. California (West Coast USA)
    // - Mumbai (South Asia)
    pub static PING_TARGETS: [PingTarget; 3] = [
        PingTarget {
            name: "Ireland",
            host: "ec2.eu-west-1.amazonaws.com", // AWS Dublin
            lat: 53.35,
            lon: -6.26,
        },
        PingTarget {
            name: "N. California",
            host: "ec2.us-west-1.amazonaws.com", // AWS N. California
            lat: 37.77,
            lon: -122.42,
        },
        PingTarget {
            name: "Singapore",
            host: "ec2.ap-southeast-1.amazonaws.com", // AWS Singapore
            lat: 1.35,
            lon: 103.82,
        },
    ];

    /// Get timezone info from Windows
    pub fn get_timezone_info() -> Option<TimezoneInfo> {
        // PowerShell: Get-TimeZone returns detailed timezone info
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                r#"$tz = Get-TimeZone; @{Name=$tz.DisplayName; Offset=$tz.BaseUtcOffset.TotalMinutes} | ConvertTo-Json"#,
            ])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value = serde_json::from_str(stdout.trim()).ok()?;

        let name = json["Name"].as_str()?.to_string();
        let offset_minutes = json["Offset"].as_f64()? as i32;

        Some(TimezoneInfo {
            name,
            utc_offset_hours: offset_minutes / 60,
            utc_offset_minutes: offset_minutes % 60,
        })
    }

    /// Ping a single host and get RTT in milliseconds
    fn ping_host(host: &str) -> Option<f64> {
        // Windows ping: ping -n 1 host
        // 1000ms timeout is plenty - even antipodal routes are under 400ms
        let output = Command::new("ping")
            .args(["-n", "1", "-w", "1000", host])
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse "time=XXms" or "time<1ms" from ping output
        // Windows format: "Reply from ... time=12ms TTL=..."
        for line in stdout.lines() {
            if line.contains("time=") || line.contains("time<") {
                // Extract time value
                if let Some(time_start) = line.find("time") {
                    let rest = &line[time_start..];
                    // Handle "time<1ms" case
                    if rest.starts_with("time<") {
                        return Some(0.5); // Estimate <1ms as 0.5ms
                    }
                    // Handle "time=XXms" case
                    if let Some(eq_pos) = rest.find('=') {
                        let after_eq = &rest[eq_pos + 1..];
                        let num_str: String = after_eq
                            .chars()
                            .take_while(|c| c.is_ascii_digit() || *c == '.')
                            .collect();
                        if let Ok(ms) = num_str.parse::<f64>() {
                            return Some(ms);
                        }
                    }
                }
            }
        }
        None
    }

    /// Ping all targets and collect results
    pub fn ping_targets() -> Vec<PingResult> {
        PING_TARGETS
            .iter()
            .map(|target| PingResult {
                target,
                rtt_ms: ping_host(target.host),
            })
            .collect()
    }

    /// Estimate distance from RTT using rough fiber speed-of-light
    /// Very rough: ~200km per ms RTT (accounting for routing overhead)
    fn rtt_to_km(rtt_ms: f64) -> f64 {
        // Round-trip, so divide by 2 for one-way, then apply speed factor
        // Speed of light in fiber: ~200,000 km/s = 200 km/ms
        // But real-world routing adds ~40% overhead, so ~140 km/ms effective
        // For round-trip: ~70 km per ms RTT
        rtt_ms * 70.0
    }

    /// Haversine distance between two lat/lon points in km
    #[allow(dead_code)]
    fn haversine(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
        let r = 6371.0; // Earth radius in km
        let d_lat = (lat2 - lat1).to_radians();
        let d_lon = (lon2 - lon1).to_radians();
        let lat1_rad = lat1.to_radians();
        let lat2_rad = lat2.to_radians();

        let a = (d_lat / 2.0).sin().powi(2)
            + lat1_rad.cos() * lat2_rad.cos() * (d_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().asin();

        r * c
    }

    /// Triangulate position from ping results using inverse-distance weighting
    pub fn triangulate(results: &[PingResult]) -> LocationGuess {
        // Collect successful pings
        let valid: Vec<_> = results
            .iter()
            .filter_map(|r| r.rtt_ms.map(|rtt| (r.target, rtt)))
            .collect();

        if valid.is_empty() {
            return LocationGuess {
                description: "Unknown (no ping responses)".to_string(),
                lat: 0.0,
                lon: 0.0,
                confidence: "None",
            };
        }

        // Find the closest target (minimum RTT)
        let (closest, min_rtt) = valid
            .iter()
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();

        // If one target is MUCH closer (<50ms), we're probably near it
        if *min_rtt < 50.0 {
            let region = match closest.name {
                "N. California" => "Near San Francisco Bay Area, California",
                "Ireland" => "Near Dublin, Ireland",
                "Singapore" => "Near Singapore, Southeast Asia",
                _ => "Unknown region",
            };
            return LocationGuess {
                description: region.to_string(),
                lat: closest.lat,
                lon: closest.lon,
                confidence: "High (very low latency to one target)",
            };
        }

        // Use inverse-RTT weighting for position estimation
        let total_inv_rtt: f64 = valid.iter().map(|(_, rtt)| 1.0 / rtt).sum();
        let estimated_lat: f64 = valid
            .iter()
            .map(|(t, rtt)| t.lat * (1.0 / rtt) / total_inv_rtt)
            .sum();
        let estimated_lon: f64 = valid
            .iter()
            .map(|(t, rtt)| t.lon * (1.0 / rtt) / total_inv_rtt)
            .sum();

        // Describe the region based on weighted center
        let description = describe_location(estimated_lat, estimated_lon);

        // Confidence based on how spread the RTTs are
        let max_rtt = valid.iter().map(|(_, rtt)| *rtt).fold(0.0f64, f64::max);
        let confidence = if max_rtt / min_rtt > 10.0 {
            "Medium (clear closest target)"
        } else {
            "Low (similar latencies, rough estimate)"
        };

        LocationGuess {
            description,
            lat: estimated_lat,
            lon: estimated_lon,
            confidence,
        }
    }

    /// Generate a human-readable location description from lat/lon
    fn describe_location(lat: f64, lon: f64) -> String {
        // Very rough geographic regions
        match (lat, lon) {
            // North America
            (l, lo) if l > 25.0 && l < 50.0 && lo < -60.0 && lo > -130.0 => {
                if lo < -100.0 {
                    "Western North America".to_string()
                } else {
                    "Eastern North America".to_string()
                }
            }
            // Europe
            (l, lo) if l > 35.0 && l < 70.0 && lo > -10.0 && lo < 40.0 => "Europe".to_string(),
            // South Asia
            (l, lo) if l > 5.0 && l < 40.0 && lo > 60.0 && lo < 100.0 => {
                "South Asia / India region".to_string()
            }
            // East Asia
            (l, lo) if l > 20.0 && l < 50.0 && lo > 100.0 && lo < 145.0 => "East Asia".to_string(),
            // Australia
            (l, lo) if l < -10.0 && l > -45.0 && lo > 110.0 && lo < 160.0 => {
                "Australia / Oceania".to_string()
            }
            // South America
            (l, lo) if l < 15.0 && l > -60.0 && lo < -30.0 && lo > -85.0 => {
                "South America".to_string()
            }
            _ => format!("Somewhere on Earth ({:.1}°, {:.1}°)", lat, lon),
        }
    }

    /// Predict if user should burn quota (going to sleep soon?)
    pub fn predict_sleep(
        _tz_info: Option<&TimezoneInfo>,
        _location: &LocationGuess,
        local_hour: u32,
        day_of_week: u32, // 0=Sunday, 6=Saturday
        month: u32,
    ) -> SleepPrediction {
        let is_weekend = day_of_week == 0 || day_of_week == 6;
        let is_winter = month == 12 || month == 1 || month == 2;

        // Base sleep threshold: 22:00 on weekdays, 23:30 on weekends
        let sleep_threshold = if is_weekend { 23 } else { 22 };
        let wake_threshold = 6;

        let reason = if local_hour >= sleep_threshold || local_hour < wake_threshold {
            let day_type = if is_weekend { "weekend" } else { "weekday" };
            let season = if is_winter { " in winter" } else { "" };
            format!(
                "Late {} ({:02}:00) on a {}{} — you're probably heading to bed soon!",
                if local_hour >= sleep_threshold {
                    "evening"
                } else {
                    "night"
                },
                local_hour,
                day_type,
                season
            )
        } else if local_hour >= 20 {
            "Evening hours — quota might not get used before bed.".to_string()
        } else {
            "Plenty of daytime left — no rush to burn quota.".to_string()
        };

        let should_burn = local_hour >= sleep_threshold || local_hour < wake_threshold;

        SleepPrediction {
            should_burn_quota: should_burn,
            reason,
        }
    }

    /// Format ping results as a nice table
    pub fn format_ping_results(results: &[PingResult]) -> String {
        let mut out = String::new();
        for r in results {
            let rtt_str = match r.rtt_ms {
                Some(ms) => {
                    let est_km = rtt_to_km(ms);
                    format!("~{:.0}ms (est. {:.0}km)", ms, est_km)
                }
                None => "timeout".to_string(),
            };
            out.push_str(&format!(
                "  {} ({}): {}\n",
                r.target.name, r.target.host, rtt_str
            ));
        }
        out
    }
}
