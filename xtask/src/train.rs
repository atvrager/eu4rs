use anyhow::{Context, Result};
use std::process::Command;

use std::path::Path;

pub fn run(data_path: &str, model: &str, output_dir: &str, epochs: u32) -> Result<()> {
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
    let status = Command::new("uv")
        .current_dir("scripts")
        .arg("run")
        .arg("train_ai.py")
        .arg("--data")
        .arg(data_rel)
        .arg("--base-model")
        .arg(model)
        .arg("--output")
        .arg(output_rel)
        .arg("--epochs")
        .arg(epochs.to_string())
        .status()
        .context("Failed to execute training script")?;

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

    // 1. Create dummy data
    println!("   Creating dummy data...");
    let dummy_path = "verify_data.jsonl";
    std::fs::write(
        dummy_path,
        r#"{"prompt": "Sit: War. Act:", "completion": " 1"}
{"prompt": "Sit: Peace. Act:", "completion": " 0"}
"#,
    )?;

    // 2. Run Training with tiny model
    let model = "HuggingFaceTB/SmolLM2-135M";
    let output = "models/verify_adapter";
    println!("   Running training smoke test (1 epoch)...");

    // Use existing run function, but maybe suppress some output? Or just let it print.
    run(dummy_path, model, output, 1)?;

    // 3. Inspect output
    println!("   Inspecting output adapter...");
    if !std::path::Path::new(output)
        .join("adapter_model.safetensors")
        .exists()
    {
        anyhow::bail!("Adapter file not created!");
    }

    // 4. Cleanup
    println!("   Cleanup...");
    let _ = std::fs::remove_file(dummy_path);
    let _ = std::fs::remove_dir_all(output); // Simplify cleanup

    // 5. Check formatting
    println!("   Checking python formatting...");
    format_python(true)?;

    println!("✅ Pipeline Verified! (uv -> python -> peft -> adapter)");
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
