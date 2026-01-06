# ROCm Setup (AMD GPU on Windows)

ROCm 7.1.1 provides native AMD GPU support for PyTorch on Windows — no WSL needed!

## Prerequisites

- **GPU**: AMD Radeon RX 7000 series (gfx1100/gfx1101/gfx1102)
- **Driver**: Version 25.20.01.17 or newer (Dec 2025+)
- **Python**: 3.12

## Setup

```powershell
cd scripts

# Run the setup script
powershell -ExecutionPolicy Bypass -File setup_rocm.ps1
```

The script will:
1. Create a dedicated venv at `.venv-rocm`
2. Install ROCm SDK from AMD's repository
3. Install PyTorch 2.9.0 with ROCm support
4. Verify GPU detection

## Training

```powershell
# Activate the ROCm venv
.venv-rocm\Scripts\Activate.ps1

# Run training (ROCm will be auto-detected)
python train_ai.py --data ..\data\run_10yr_1.cpb.zip --max-steps 1000

# You should see: "Using device: ROCm (AMD Radeon RX 7900 XTX)"
```

## Inference (Bridge Mode)

Since Candle doesn't support ROCm yet, we use a Python bridge for inference:

**Terminal 1 - Start the inference server:**
```powershell
cd scripts
.venv-rocm\Scripts\Activate.ps1
python inference_server.py --adapter ../models/adapter

# You should see:
# Using device: ROCm (AMD Radeon RX 7900 XTX)
# Loading base model: google/gemma-2-2b-it
# Model ready for inference
# Inference server listening on 127.0.0.1:9876
```

**Terminal 2 - Run the simulation:**
```powershell
cargo run -p eu4sim --release -- --observer --llm-ai bridge --ticks 100
```

### Bridge Options

| Flag | Description |
|------|-------------|
| `--llm-ai bridge` | Use default server (127.0.0.1:9876) |
| `--llm-ai bridge:HOST:PORT` | Custom server address |
| `--llm-ai path/to/adapter` | Use Candle backend (CPU/CUDA) |

### Server Arguments

```powershell
python inference_server.py --help

  --base-model MODEL   HuggingFace model ID (default: google/gemma-2-2b-it)
  --adapter PATH       Path to LoRA adapter directory
  --host HOST          Host to bind (default: 127.0.0.1)
  --port PORT          Port to bind (default: 9876)
```

## Why a Separate Venv?

- ROCm wheels are self-contained from AMD's repository
- Different PyTorch build than the CUDA index
- Keeps main project clean for CUDA users

## Performance Notes

## Performance Notes

| Setting | Recommendation | Notes |
|---------|----------------|-------|
| `--batch-size` | **4** | Stable on 24GB VRAM. **8** caused instability/crashes in testing. |
| `--grad-accum` | **4** | Effective batch size = 4 * 4 = 16. |
| `--learning-rate` | **5e-5** | Lower LR verified stable for Gemma 3 fine-tuning. |
| `--lora-r` | **32** | Higher rank works well on 7900 XTX. `r=8` saves partial memory but didn't improve speed. |
| `--lora-dropout` | **0.05 - 0.1** | Use **0.1** for smaller datasets to prevent overfitting. |
| `--lr-scheduler` | **cosine** | Better convergence than linear for most fine-tuning. |
| `--weight-decay` | **0.01 - 0.05** | Standard regularization. |
| Mixed precision | fp16 | **Required** for ROCm on Windows (bf16 is unstable). |

## Advanced Tuning Arguments

You can now tune these via CLI:

| Argument | Default | Description |
| :--- | :--- | :--- |
| `--lora-r` | 16 | LoRA rank (dimension). Higher = more capacity. |
| `--lora-alpha` | 32 | Scaling factor. Usually 2x rank. |
| `--lora-dropout` | 0.05 | Dropout probability for adapter layers. |
| `--weight-decay` | 0.01 | Optimizer weight decay. |
| `--lr-scheduler` | linear | `linear`, `cosine`, `constant`. |

RX 7900 XTX has 24GB VRAM, but Windows display driver overhead limits usable VRAM. We firmly cap usage at ~16GB (0.7 fraction) to prevent system freezes.

## Authentication (Optional)

If you plan to train **gated models** (like `google/gemma-3-270m`), you must authenticate with HuggingFace:

**Option 1: `.env` file (Recommended)**
Add your token to the `.env` file in the project root:
```bash
HF_TOKEN=hf_...
```

**Option 2: CLI Login**
```powershell
.venv-rocm\Scripts\huggingface-cli login
```

## Troubleshooting

**"GPU not detected"**: Make sure driver >= 25.20.01.17 is installed.

**Import errors**: Ensure you're in the `.venv-rocm` venv, not the main one.

**Performance issues**: Check `rocm-smi` for GPU utilization. Try `--batch-size 8`.

## Switching Between Backends

```powershell
# For ROCm (AMD)
.venv-rocm\Scripts\Activate.ps1

# For DirectML (legacy AMD, slower)
.venv-dml\Scripts\Activate.ps1

# For CUDA (NVIDIA) - use main venv
uv run python train_ai.py ...
```
