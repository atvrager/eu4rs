use anyhow::{Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};

pub fn run(data_path: &str, model: &str, output_dir: &str, epochs: u32, eager: bool) -> Result<()> {
    println!("🚂 Cargo Orchestrator: Starting AI Training...");
    println!("   Data: {}", data_path);
    println!("   Base Model: {}", model);
    println!("   Output: {}", output_dir);

    // Check for uv
    let uv_check = Command::new("uv")
        .arg("--version")
        .output()
        .context("Failed to find 'uv'. Is it installed and in PATH?")?;

    if !uv_check.status.success() {
        anyhow::bail!("uv check failed. Please ensure uv is installed correctly.");
    }

    // Construct relative paths using OS separator
    let data_rel = Path::new("..").join(data_path);
    let output_rel = Path::new("..").join(output_dir);

    // Run training script via uv
    // This uses the virtualenv managed by uv in scripts/.venv
    let mut cmd = Command::new("uv");
    cmd.current_dir("scripts")
        .arg("run")
        .arg("train_ai.py")
        .arg("--data")
        .arg(data_rel)
        .arg("--base-model")
        .arg(model)
        .arg("--output")
        .arg(output_rel)
        .arg("--epochs")
        .arg(epochs.to_string());

    if eager {
        cmd.arg("--eager");
    }

    let status = cmd.status().context("Failed to execute training script")?;

    if status.success() {
        println!("✅ Training complete!");
        Ok(())
    } else {
        anyhow::bail!("Training script failed with status: {}", status);
    }
}

pub fn inspect(path: &str) -> Result<()> {
    println!("🔍 Inspecting data: {}", path);

    let path_rel = Path::new("..").join(path);

    // Run inspect_data.py via uv
    let status = Command::new("uv")
        .current_dir("scripts")
        .arg("run")
        .arg("inspect_data.py")
        .arg(path_rel)
        .status()
        .context("Failed to execute inspection script")?;

    if !status.success() {
        anyhow::bail!("Inspection script failed");
    }
    Ok(())
}

pub fn verify_pipeline() -> Result<()> {
    println!("🧪 Verifying ML Pipeline...");

    let binary_data_path = "verify_data.cpb.zip";

    // 1. Generate real training data via short simulation (using mock state for CI)
    println!("   Generating binary training data via simulation...");
    let sim_status = Command::new("cargo")
        .args([
            "run",
            "-p",
            "eu4sim",
            "--",
            "--ticks",
            "5",
            "--headless",
            "--observer",
            "--test-mode",
            "--datagen",
            binary_data_path,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("Failed to run eu4sim for datagen")?;

    if !sim_status.success() {
        anyhow::bail!("Simulation failed to generate training data");
    }

    if !std::path::Path::new(binary_data_path).exists() {
        anyhow::bail!("Training data file not created: {}", binary_data_path);
    }

    // 2. Verify Python can load the binary data
    println!("   Verifying Python can load Cap'n Proto data...");
    let py_load_check = Command::new("uv")
        .current_dir("scripts")
        .args([
            "run",
            "python",
            "-c",
            &format!(
                "from load_training_data import load_training_file; \
                 samples = load_training_file('../{}'); \
                 print(f'Loaded {{len(samples)}} samples')",
                binary_data_path
            ),
        ])
        .output()
        .context("Failed to run Python load check")?;

    if !py_load_check.status.success() {
        let stderr = String::from_utf8_lossy(&py_load_check.stderr);
        anyhow::bail!("Python failed to load binary data: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&py_load_check.stdout);
    println!("   {}", stdout.trim());

    // 3. Run Training with tiny model using binary data
    // Use eager mode for CI smoke test (tiny dataset, avoid streaming complexity)
    let model = "HuggingFaceTB/SmolLM2-135M";
    let output = "models/verify_adapter";
    println!("   Running training smoke test (1 epoch)...");
    run(binary_data_path, model, output, 1, true)?;

    // 4. Inspect output
    println!("   Inspecting output adapter...");
    if !std::path::Path::new(output)
        .join("adapter_model.safetensors")
        .exists()
    {
        anyhow::bail!("Adapter file not created!");
    }

    // 5. Cleanup
    println!("   Cleanup...");
    let _ = std::fs::remove_file(binary_data_path);
    let _ = std::fs::remove_dir_all(output);

    // 6. Check formatting
    println!("   Checking python formatting...");
    format_python(true)?;

    println!("✅ Pipeline Verified! (simulation → .cpb.zip → pycapnp → peft → adapter)");
    Ok(())
}

pub fn format_python(check: bool) -> Result<()> {
    println!("🐍 Formatting Python scripts (ruff)...");

    let mut cmd = Command::new("uv");
    cmd.current_dir("scripts")
        .arg("run")
        .arg("ruff")
        .arg("format");

    if check {
        cmd.arg("--check");
    }
    // format everything in scripts/
    cmd.arg(".");

    let status = cmd.status().context("Failed to execute ruff")?;

    if !status.success() {
        if check {
            anyhow::bail!("Python formatting check failed. Run 'cargo xtask fmt-py' to fix.");
        } else {
            anyhow::bail!("Python formatting failed");
        }
    }
    Ok(())
}
