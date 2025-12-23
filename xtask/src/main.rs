use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use regex::Regex;
use reqwest::blocking::Client;
use std::env;
use std::process::{Command, Stdio};

mod personalize;
mod train;

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

    /// Verify the HEAD commit follows project conventions (post-commit check)
    VerifyCommit,

    /// Authenticate with MyAnimeList (OAuth2 PKCE)
    MalLogin,

    /// Generate a persona based on MAL history
    Personalize,

    /// Train the AI using Python/UV pipeline
    Train {
        /// Path to training data (.cpb.zip or .jsonl)
        #[arg(long, default_value = "training_data.cpb.zip")]
        data: String,
        /// Base model to use
        #[arg(long, default_value = "google/gemma-2-2b-it")]
        model: String,
        /// Output directory for adapter
        #[arg(long, default_value = "models/adapter")]
        output: String,
        /// Number of training epochs (only for eager mode)
        #[arg(long, default_value_t = 1)]
        epochs: u32,
        /// Force eager loading (slower, allows full shuffle)
        #[arg(long)]
        eager: bool,
    },

    /// Inspect generated training data (.zip or .jsonl)
    Inspect {
        /// Path to file
        path: String,
    },

    /// Run a quick smoke test of the ML pipeline
    VerifyPipeline,

    /// Format Python scripts using ruff
    FormatPython {
        /// Check only, don't write changes
        #[arg(long)]
        check: bool,
    },

    /// Run full ML CI pipeline (Formatting + Smoke Test)
    MlCi,

    /// Compile Cap'n Proto schema for Rust and Python
    Schema {
        /// Verify schema compiles without generating (for CI)
        #[arg(long)]
        check: bool,
    },

    /// Generate batch training data (multiple simulations with different seeds)
    Datagen {
        /// Number of simulations to run
        #[arg(short = 'n', long, default_value_t = 10)]
        count: u32,

        /// Number of ticks per simulation
        #[arg(short, long, default_value_t = 365)]
        ticks: u32,

        /// Output pattern (use {seed} placeholder, e.g., "data/run_{seed}.cpb.zip")
        #[arg(short, long, default_value = "training_data/run_{seed}.cpb.zip")]
        output: String,

        /// Base seed (each simulation uses base_seed + run_index)
        #[arg(long, default_value_t = 1)]
        base_seed: u64,

        /// Number of top countries using GreedyAI in hybrid mode
        #[arg(long, default_value_t = 8)]
        greedy_count: usize,
    },

    /// Run LLM AI benchmark with optional adapter
    ///
    /// Examples:
    ///   cargo xtask llm smollm          # SmolLM2 base model
    ///   cargo xtask llm gemma3          # Gemma-3-270M base model
    ///   cargo xtask llm gemma3 run1     # Gemma3 + adapter matching *run1*
    Llm {
        /// Base model: "smollm" or "gemma3"
        base: String,

        /// Optional adapter name (fuzzy-matched against models/adapters/{base}*{name}*/)
        adapter: Option<String>,

        /// Number of ticks to simulate
        #[arg(short, long, default_value_t = 100)]
        ticks: u32,

        /// Disable CUDA (use CPU only)
        #[arg(long)]
        no_cuda: bool,
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

        Commands::VerifyCommit => run_verify_commit(),

        Commands::MalLogin => personalize::run_login(),
        Commands::Personalize => personalize::run_personalize(),
        Commands::Train {
            data,
            model,
            output,
            epochs,
            eager,
        } => train::run(&data, &model, &output, epochs, eager),
        Commands::Inspect { path } => train::inspect(&path),
        Commands::VerifyPipeline => train::verify_pipeline(),
        Commands::FormatPython { check } => train::format_python(check),
        Commands::MlCi => train::verify_pipeline(),
        Commands::Schema { check } => run_schema(check),
        Commands::Datagen {
            count,
            ticks,
            output,
            base_seed,
            greedy_count,
        } => run_datagen(count, ticks, &output, base_seed, greedy_count),
        Commands::Llm {
            base,
            adapter,
            ticks,
            no_cuda,
        } => run_llm(&base, adapter.as_deref(), ticks, no_cuda),
    }
}

fn run_ci() -> Result<()> {
    println!("Running Local CI Pipeline...");

    println!("\n[1/4] Checking Formatting...");
    run_command("cargo", &["fmt", "--", "--check"])?;

    println!("\n[2/4] Running Clippy...");
    // Don't use --all-features: cuda/metal features are platform-specific
    // and should only be enabled for local GPU testing, not CI
    run_command(
        "cargo",
        &["clippy", "--all-targets", "--", "-D", "warnings"],
    )?;

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
        .args(["diff", "--exit-code", "docs/reference/supported-fields.md"])
        .status()?;

    if !status.success() {
        anyhow::bail!("docs/reference/supported-fields.md is out of date! Run `cargo xtask coverage --doc-gen` and commit the changes.");
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

    let client = Client::new();

    // =========================================================================
    // Claude Code Subscription (from .env config)
    // =========================================================================
    let claude_tier = env::var("CLAUDE_CODE_TIER").ok();
    if let Some(tier) = claude_tier {
        let (tier_name, multiplier) = match tier.to_lowercase().as_str() {
            "free" => ("Free", "1x"),
            "max5" => ("Max 5 ($20/mo)", "5x"),
            "max20" => ("Max 20 ($100/mo)", "20x"),
            "max50" => ("Max 50 ($200/mo)", "50x"),
            _ => (tier.as_str(), "?x"),
        };
        println!("📊 **Claude Code** (from CLAUDE_CODE_TIER in .env)\n");
        println!("Subscription: {} ({} Pro usage)", tier_name, multiplier);
        println!("   Note: Real-time availability not detectable programmatically.\n");
    } else {
        println!("📊 **Claude Code**\n");
        println!("   Set CLAUDE_CODE_TIER in .env (free/max5/max20/max50)\n");
    }

    // =========================================================================
    // Claude API (optional - for direct API access / scripts)
    // Note: Claude Code extension uses separate account-based auth, not API keys
    // =========================================================================
    let anthropic_key = env::var("ANTHROPIC_API_KEY").ok();
    if let Some(key) = anthropic_key {
        match check_anthropic(&client, &key) {
            ClaudeCheckResult::Valid(limits) => {
                if limits.has_data() {
                    println!("📊 **Claude API** (direct API key)\n");
                    println!("Plan: {}\n", limits.infer_tier());

                    println!("| Resource | Remaining | Limit | % | Status |");
                    println!("|----------|-----------|-------|---|--------|");

                    if let (Some(rem), Some(lim)) =
                        (limits.input_tokens_remaining, limits.input_tokens_limit)
                    {
                        let pct = limits.input_percentage().unwrap_or(0);
                        println!(
                            "| Input Tokens | {} | {} | {}% | {} |",
                            format_tokens(rem),
                            format_tokens(lim),
                            pct,
                            rate_limit_status(pct)
                        );
                    }

                    if let (Some(rem), Some(lim)) =
                        (limits.output_tokens_remaining, limits.output_tokens_limit)
                    {
                        let pct = limits.output_percentage().unwrap_or(0);
                        println!(
                            "| Output Tokens | {} | {} | {}% | {} |",
                            format_tokens(rem),
                            format_tokens(lim),
                            pct,
                            rate_limit_status(pct)
                        );
                    }

                    if let (Some(rem), Some(lim)) =
                        (limits.requests_remaining, limits.requests_limit)
                    {
                        let pct = limits.requests_percentage().unwrap_or(0);
                        println!(
                            "| Requests | {} | {} | {}% | {} |",
                            rem,
                            lim,
                            pct,
                            rate_limit_status(pct)
                        );
                    }

                    if let Some(reset) = &limits.reset_time {
                        println!("\nResets: {}", format_refresh_time_generic(reset));
                    }
                    println!();
                }
                // Don't show "active" message if no data - clutters output
            }
            ClaudeCheckResult::InvalidKey | ClaudeCheckResult::OutOfCredits => {
                // Don't show - not relevant to Claude Code extension
            }
            ClaudeCheckResult::Error(_) => {
                // Don't show - not relevant to Claude Code extension
            }
        }
    }

    // =========================================================================
    // Antigravity Model Quotas
    // =========================================================================
    println!("\n📊 **Antigravity Model Quotas**\n");
    match check_antigravity_quota() {
        Ok(quotas) if !quotas.is_empty() => {
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
        }
        Ok(_) => {
            println!("ℹ️  Antigravity detected but no quota data available.");
        }
        Err(e) => {
            println!("ℹ️  Antigravity not detected: {}", e);
        }
    }

    // =========================================================================
    // Gemini API (validation only - no rate limit API available)
    // =========================================================================
    println!("\n📊 **Gemini API**\n");
    let gemini_key = env::var("GEMINI_API_KEY").ok();
    if let Some(key) = gemini_key {
        match check_gemini(&client, &key) {
            Ok(s) => println!("{}", s),
            Err(e) => println!("Error: {}", e),
        }
    } else {
        println!("ℹ️  GEMINI_API_KEY not set (optional for Gemini routing)");
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
        let path = std::path::Path::new("docs/reference/supported-fields.md");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        println!("✅ Generated docs/reference/supported-fields.md");
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

fn run_verify_commit() -> Result<()> {
    println!("Verifying HEAD commit...\n");

    // 1. Get commit message
    let output = Command::new("git")
        .args(["log", "-1", "--format=%s%n%n%b"])
        .output()
        .context("Failed to get git log")?;
    let message = String::from_utf8_lossy(&output.stdout);

    // 2. Check conventions
    // Regex: ^(feat|fix|refactor|docs|test|chore|perf)(\(.+\))?: .+$
    let re = Regex::new(r"^(feat|fix|refactor|docs|test|chore|perf)(\(.+\))?: .+$").unwrap();
    let subject = message.lines().next().unwrap_or("");

    let mut passed = true;

    if re.is_match(subject) {
        println!("✅ Commit message format is correct.");
    } else {
        println!("❌ Commit message format invalid.");
        println!("   Expected: type(scope): description");
        println!("   Got:      {}", subject);
        passed = false;
    }

    // 3. Check docs updates
    let diff_output = Command::new("git")
        .args(["show", "--name-only", "--format=", "HEAD"])
        .output()
        .context("Failed to get git diff")?;
    let changed_files = String::from_utf8_lossy(&diff_output.stdout);

    let has_code_changes = changed_files.lines().any(|f| {
        f.starts_with("eu4sim-core/")
            || f.starts_with("eu4sim/")
            || f.starts_with("eu4data/")
            || f.starts_with("eu4viz/")
            || f.starts_with("xtask/")
    });
    let has_doc_updates = changed_files.lines().any(|f| f.starts_with("docs/"));
    let is_docs_commit = subject.starts_with("docs");

    if has_code_changes && !has_doc_updates && !is_docs_commit {
        println!("⚠️  Code changed but no docs updated.");
        println!("   If this added a feature or completed a roadmap item, update:");
        println!("   - docs/planning/mid-term-status.md");
        println!("   - docs/planning/roadmap.md");
    } else if has_doc_updates {
        println!("✅ Docs updated.");
    }

    if passed {
        println!("\nCommit Verified! 🚀");
        println!("Ready to push: git push");
    } else {
        anyhow::bail!("Verification failed. Please amend commit.");
    }

    Ok(())
}

fn run_datagen(
    count: u32,
    ticks: u32,
    output_pattern: &str,
    base_seed: u64,
    greedy_count: usize,
) -> Result<()> {
    println!("🎮 Batch Training Data Generation\n");
    println!("Simulations: {}", count);
    println!(
        "Ticks each:  {} (~{:.1} years)",
        ticks,
        ticks as f64 / 365.0
    );
    println!("Output:      {}", output_pattern);
    println!("Base seed:   {}", base_seed);
    println!("Greedy AIs:  {}\n", greedy_count);

    // Create output directory if needed
    if let Some(parent) = std::path::Path::new(output_pattern)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).context("Failed to create output directory")?;
    }

    // Build the release binary first (for consistent, fast runs)
    println!("Building eu4sim (release)...");
    run_command("cargo", &["build", "-p", "eu4sim", "--release"])?;
    println!();

    let start = std::time::Instant::now();
    let mut total_samples = 0u64;

    for i in 0..count {
        let seed = base_seed + i as u64;
        let output_path = output_pattern.replace("{seed}", &seed.to_string());

        println!("[{}/{}] Seed {} → {}", i + 1, count, seed, output_path);

        // Run simulation
        let status = Command::new("cargo")
            .args([
                "run",
                "-p",
                "eu4sim",
                "--release",
                "--",
                "--headless",
                "--observer",
                "--benchmark",
                "--ticks",
                &ticks.to_string(),
                "--seed",
                &seed.to_string(),
                "--greedy-count",
                &greedy_count.to_string(),
                "--datagen",
                &output_path,
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to run eu4sim")?;

        if !status.success() {
            anyhow::bail!("Simulation {} failed with seed {}", i + 1, seed);
        }

        // Get file size and estimate samples
        if let Ok(meta) = std::fs::metadata(&output_path) {
            let size_mb = meta.len() as f64 / (1024.0 * 1024.0);
            // Rough estimate: ~80 bytes per sample compressed
            let est_samples = meta.len() / 80;
            total_samples += est_samples;
            println!(
                "       Output: {:.1} MB (~{}k samples)\n",
                size_mb,
                est_samples / 1000
            );
        }
    }

    let elapsed = start.elapsed();
    let total_ticks = count as u64 * ticks as u64;
    let years = total_ticks as f64 / 365.0;

    println!("═══════════════════════════════════════════════════════");
    println!("✅ Batch complete!");
    println!("   Simulations: {}", count);
    println!("   Total ticks: {} ({:.1} sim-years)", total_ticks, years);
    println!(
        "   Time:        {:.1}s ({:.1} ticks/sec)",
        elapsed.as_secs_f64(),
        total_ticks as f64 / elapsed.as_secs_f64()
    );
    println!("   Est samples: ~{}k", total_samples / 1000);
    println!("═══════════════════════════════════════════════════════");

    Ok(())
}

fn run_schema(check: bool) -> Result<()> {
    println!("📐 Compiling Cap'n Proto schema...\n");

    let schema_path = std::path::Path::new("schemas/training.capnp");
    if !schema_path.exists() {
        anyhow::bail!("Schema not found: {:?}", schema_path);
    }

    // Check if capnp compiler is available
    let capnp_check = Command::new("capnp")
        .arg("--version")
        .output()
        .context("capnp compiler not found. Install with: choco install capnproto (Windows) or brew install capnp (macOS)")?;

    if !capnp_check.status.success() {
        anyhow::bail!("capnp compiler check failed");
    }

    let version = String::from_utf8_lossy(&capnp_check.stdout);
    println!("Using Cap'n Proto: {}", version.trim());

    if check {
        // Just verify the schema compiles
        println!("Verifying schema syntax...");
        let status = Command::new("capnp")
            .args(["compile", "-o-", "schemas/training.capnp"])
            .stdout(Stdio::null())
            .status()
            .context("Failed to run capnp compile")?;

        if status.success() {
            println!("✅ Schema is valid");
        } else {
            anyhow::bail!("Schema validation failed");
        }
    } else {
        // For Rust: Just trigger a cargo build (build.rs handles capnpc)
        println!("Compiling for Rust (via build.rs)...");
        run_command("cargo", &["build", "-p", "eu4sim-core"])?;
        println!("✅ Rust code generated in eu4sim-core target/");

        // For Python: Check if pycapnp can load the schema
        println!("\nVerifying Python schema loading...");
        let py_check = Command::new("uv")
            .current_dir("scripts")
            .args([
                "run",
                "python",
                "-c",
                "import capnp; capnp.load('../schemas/training.capnp'); print('OK')",
            ])
            .output();

        match py_check {
            Ok(output) if output.status.success() => {
                println!("✅ Python can load schema");
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!("⚠️  Python schema load failed: {}", stderr);
                println!("   Run: cd scripts && uv sync");
            }
            Err(e) => {
                println!("⚠️  Could not check Python: {}", e);
            }
        }
    }

    println!("\nSchema compilation complete! 🎉");
    Ok(())
}

/// Returns a human-readable status based on quota percentage
fn quota_status_label(percentage: u8) -> &'static str {
    match percentage {
        0 => "🔴 Exhausted",
        1..=10 => "🔴 Critical",
        11..=50 => "🟡 Low",
        _ => "🟢 Healthy",
    }
}

/// Returns a human-readable status for rate limits (same logic, different name for clarity)
fn rate_limit_status(percentage: u8) -> &'static str {
    match percentage {
        0 => "🔴 Exhausted",
        1..=10 => "🔴 Critical",
        11..=50 => "🟡 Low",
        _ => "🟢 Healthy",
    }
}

/// Format large token counts for readability (e.g., 450000 -> "450K")
fn format_tokens(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{}K", count / 1_000)
    } else {
        count.to_string()
    }
}

/// Formats a reset time as a human-readable relative duration (non-Windows version)
fn format_refresh_time_generic(reset_time: &chrono::DateTime<chrono::Utc>) -> String {
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

/// Formats a reset time as a human-readable relative duration
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
// Cross-platform Antigravity Detection
// ============================================================================

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

    // =========================================================================
    // Platform-specific process discovery
    // =========================================================================

    #[cfg(target_os = "windows")]
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

    #[cfg(target_os = "linux")]
    fn find_antigravity_process() -> Result<(String, u32)> {
        // Use ps to find language_server_linux_x64 with antigravity in command line
        let output = Command::new("ps")
            .args(["aux"])
            .output()?;

        if !output.status.success() {
            return Err(anyhow!("ps command failed"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let token_re = Regex::new(r"--csrf_token\s+([a-f0-9\-]+)").unwrap();

        for line in stdout.lines() {
            // Look for the language server binary with antigravity app_data_dir
            if line.contains("language_server_linux_x64") && line.contains("--app_data_dir antigravity") {
                // Extract PID (second column in ps aux output)
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(pid) = parts[1].parse::<u32>() {
                        // Extract CSRF token from command line
                        if let Some(caps) = token_re.captures(line) {
                            let token = caps.get(1).unwrap().as_str().to_string();
                            return Ok((token, pid));
                        }
                    }
                }
            }
        }

        Err(anyhow!("Antigravity language_server not found"))
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    fn find_antigravity_process() -> Result<(String, u32)> {
        Err(anyhow!("Platform not supported"))
    }

    #[cfg(target_os = "windows")]
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

    #[cfg(target_os = "linux")]
    fn find_listening_ports(pid: u32) -> Result<Vec<u16>> {
        // Use ss (socket statistics) to find listening ports by PID
        let output = Command::new("ss")
            .args(["-tlnp"])
            .output()?;

        if !output.status.success() {
            return Ok(vec![]);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let pid_pattern = format!("pid={}", pid);
        let port_re = Regex::new(r"127\.0\.0\.1:(\d+)").unwrap();

        let mut ports = Vec::new();
        for line in stdout.lines() {
            if line.contains(&pid_pattern) {
                // Extract port from address like "127.0.0.1:46277"
                if let Some(caps) = port_re.captures(line) {
                    if let Ok(port) = caps.get(1).unwrap().as_str().parse::<u16>() {
                        ports.push(port);
                    }
                }
            }
        }

        // Sort descending - higher ports are more likely to be the API
        ports.sort_by(|a, b| b.cmp(a));
        Ok(ports)
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    fn find_listening_ports(_pid: u32) -> Result<Vec<u16>> {
        Ok(vec![])
    }

    // =========================================================================
    // Shared API interaction logic
    // =========================================================================

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

use antigravity::{check_antigravity_quota, ModelQuota};

#[allow(dead_code)]
type QuotaResult = Vec<ModelQuota>;

/// Rate limit info from Claude API response headers
struct ClaudeRateLimits {
    input_tokens_remaining: Option<u64>,
    input_tokens_limit: Option<u64>,
    output_tokens_remaining: Option<u64>,
    output_tokens_limit: Option<u64>,
    requests_remaining: Option<u64>,
    requests_limit: Option<u64>,
    reset_time: Option<chrono::DateTime<chrono::Utc>>,
}

impl ClaudeRateLimits {
    /// Parse rate limit headers from an HTTP response
    fn from_headers(headers: &reqwest::header::HeaderMap) -> Self {
        let get_u64 = |name: &str| -> Option<u64> {
            headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok())
        };

        let reset_time = headers
            .get("anthropic-ratelimit-input-tokens-reset")
            .or_else(|| headers.get("anthropic-ratelimit-requests-reset"))
            .and_then(|v| v.to_str().ok())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        Self {
            input_tokens_remaining: get_u64("anthropic-ratelimit-input-tokens-remaining"),
            input_tokens_limit: get_u64("anthropic-ratelimit-input-tokens-limit"),
            output_tokens_remaining: get_u64("anthropic-ratelimit-output-tokens-remaining"),
            output_tokens_limit: get_u64("anthropic-ratelimit-output-tokens-limit"),
            requests_remaining: get_u64("anthropic-ratelimit-requests-remaining"),
            requests_limit: get_u64("anthropic-ratelimit-requests-limit"),
            reset_time,
        }
    }

    /// Calculate percentage remaining for a limit type
    fn percentage(&self, remaining: Option<u64>, limit: Option<u64>) -> Option<u8> {
        match (remaining, limit) {
            (Some(r), Some(l)) if l > 0 => Some(((r as f64 / l as f64) * 100.0) as u8),
            _ => None,
        }
    }

    /// Get input tokens percentage remaining
    fn input_percentage(&self) -> Option<u8> {
        self.percentage(self.input_tokens_remaining, self.input_tokens_limit)
    }

    /// Get output tokens percentage remaining
    fn output_percentage(&self) -> Option<u8> {
        self.percentage(self.output_tokens_remaining, self.output_tokens_limit)
    }

    /// Get requests percentage remaining
    fn requests_percentage(&self) -> Option<u8> {
        self.percentage(self.requests_remaining, self.requests_limit)
    }

    /// Check if we got any rate limit data
    fn has_data(&self) -> bool {
        self.input_tokens_limit.is_some()
            || self.output_tokens_limit.is_some()
            || self.requests_limit.is_some()
    }

    /// Infer the API tier/plan based on rate limits
    /// Reference: https://docs.anthropic.com/en/api/rate-limits
    fn infer_tier(&self) -> &'static str {
        // Anthropic tiers (as of late 2024):
        // Tier 1 (Free/$5):    40K input/min,   8K output/min,   50 req/min
        // Tier 2 ($40+):      80K input/min,  16K output/min,  1000 req/min
        // Tier 3 ($200+):    160K input/min,  32K output/min,  2000 req/min
        // Tier 4 ($400+):    400K input/min,  80K output/min,  4000 req/min
        // Scale (custom):     Higher limits, custom pricing
        match self.requests_limit {
            Some(r) if r >= 4000 => "Tier 4+ (Scale) 🚀",
            Some(r) if r >= 2000 => "Tier 3 ($200+)",
            Some(r) if r >= 1000 => "Tier 2 ($40+)",
            Some(r) if r >= 50 => "Tier 1 (Free/$5)",
            _ => match self.input_tokens_limit {
                Some(t) if t >= 400_000 => "Tier 4+ (Scale) 🚀",
                Some(t) if t >= 160_000 => "Tier 3 ($200+)",
                Some(t) if t >= 80_000 => "Tier 2 ($40+)",
                Some(t) if t >= 40_000 => "Tier 1 (Free/$5)",
                _ => "Unknown tier",
            },
        }
    }
}

/// Result of checking Claude API - either just validation or full rate limits
#[allow(dead_code)] // Error variant used for completeness, may be logged in future
enum ClaudeCheckResult {
    Valid(ClaudeRateLimits),
    InvalidKey,
    OutOfCredits,
    Error(String),
}

/// Models to try for token counting (in order of preference)
/// Using stable aliases where possible - these should remain valid longer
const TOKEN_COUNT_MODELS: &[&str] = &[
    "claude-sonnet-4-5-20250514", // Current Sonnet 4.5
    "claude-3-5-sonnet-latest",   // Alias that should track latest 3.5
    "claude-3-haiku-20240307",    // Stable older model as fallback
];

fn check_anthropic(client: &Client, key: &str) -> ClaudeCheckResult {
    // Try each model until one works - the rate limit headers are account-wide
    for model in TOKEN_COUNT_MODELS {
        let body = format!(
            r#"{{"model":"{}","messages":[{{"role":"user","content":"x"}}]}}"#,
            model
        );

        let resp = client
            .post("https://api.anthropic.com/v1/messages/count_tokens")
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .body(body)
            .send();

        match resp {
            Ok(response) => {
                let status = response.status();
                let headers = response.headers().clone();

                if status.is_success() {
                    return ClaudeCheckResult::Valid(ClaudeRateLimits::from_headers(&headers));
                } else if status.as_u16() == 401 {
                    return ClaudeCheckResult::InvalidKey;
                } else {
                    let text = response.text().unwrap_or_default();
                    if text.contains("credit balance is too low") {
                        return ClaudeCheckResult::OutOfCredits;
                    }
                    // Model might be invalid - try next one
                    if text.contains("model")
                        || text.contains("not found")
                        || text.contains("invalid")
                    {
                        continue;
                    }
                    return ClaudeCheckResult::Error(format!("HTTP {}: {}", status, text));
                }
            }
            Err(e) => return ClaudeCheckResult::Error(format!("Request failed: {}", e)),
        }
    }

    ClaudeCheckResult::Error("All model fallbacks failed".to_string())
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

/// Run LLM AI benchmark with optional LoRA adapter.
fn run_llm(base: &str, adapter: Option<&str>, ticks: u32, no_cuda: bool) -> Result<()> {
    // Validate base model
    let base_flag = match base.to_lowercase().as_str() {
        "smollm" | "smollm2" => "smollm",
        "gemma3" | "gemma" => "gemma3",
        other => anyhow::bail!("Unknown base model '{}'. Use 'smollm' or 'gemma3'.", other),
    };

    // Find adapter path if specified
    let adapter_path = if let Some(adapter_name) = adapter {
        let adapters_dir = std::path::Path::new("models/adapters");
        if !adapters_dir.exists() {
            anyhow::bail!("models/adapters/ directory not found");
        }

        // Fuzzy match: look for directories matching {base}*{adapter_name}*
        let pattern = format!("{}*{}*", base_flag, adapter_name);
        let matches: Vec<_> = std::fs::read_dir(adapters_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_lowercase();
                name.contains(&base_flag.to_lowercase())
                    && name.contains(&adapter_name.to_lowercase())
            })
            .collect();

        match matches.len() {
            0 => anyhow::bail!(
                "No adapter found matching pattern '{}' in models/adapters/",
                pattern
            ),
            1 => {
                let path = matches[0].path();
                println!("Using adapter: {}", path.display());
                Some(path)
            }
            _ => {
                let names: Vec<_> = matches
                    .iter()
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect();
                anyhow::bail!(
                    "Multiple adapters match '{}': {:?}. Be more specific.",
                    pattern,
                    names
                );
            }
        }
    } else {
        None
    };

    // Build cargo command
    let mut args = vec!["run", "--release", "-p", "eu4sim"];

    if !no_cuda {
        args.extend(["--features", "eu4sim-ai/cuda"]);
    }

    args.push("--");

    // Add model flags
    args.extend(["--llm-ai-base", base_flag]);

    // Add adapter if specified
    let adapter_str;
    if let Some(path) = &adapter_path {
        adapter_str = path.to_string_lossy().to_string();
        args.extend(["--llm-ai", &adapter_str]);
    }

    // Add common benchmark flags
    let ticks_str = ticks.to_string();
    args.extend([
        "--ticks",
        &ticks_str,
        "--benchmark",
        "--observer",
        "--headless",
        "--log-level",
        "warn",
    ]);

    println!(
        "Running: cargo {}",
        args.iter()
            .map(|s| {
                if s.contains(' ') {
                    format!("\"{}\"", s)
                } else {
                    s.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    );

    run_command("cargo", &args)
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
