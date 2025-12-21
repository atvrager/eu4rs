# DirectML Setup (AMD/Intel GPU on Windows)

DirectML lets you train on AMD or Intel GPUs on Windows. Due to torch version conflicts with the main project, it requires a separate virtual environment.

## Setup

```powershell
cd scripts

# Create DirectML-specific venv
python -m venv .venv-dml

# Activate it
.venv-dml\Scripts\activate

# Install compatible torch + directml
pip install torch==2.4.1 torch-directml

# Install other deps (compatible versions)
pip install transformers peft trl datasets pycapnp safetensors
```

## Usage

```powershell
# Make sure you're in the DirectML venv
.venv-dml\Scripts\activate

# Run training (DirectML will be auto-detected)
python train_ai.py --data ../data/run_10yr_1.cpb.zip --base-model HuggingFaceTB/SmolLM2-360M --max-steps 1000

# You should see: "Using device: DirectML (AMD/Intel GPU)"
```

## Switching Back

```powershell
# Deactivate DirectML venv
deactivate

# Use main project venv (managed by uv)
# Just use `uv run` commands as normal
```

## Why a Separate Venv?

- `torch-directml` requires `torch==2.4.1`
- Main project uses `torch>=2.9.1` for latest features
- Both work fine for training - LoRA/PEFT uses basic tensor ops
- Separate venvs avoid dependency conflicts

## Performance Tuning

**Optimal settings for DirectML** (tested with SmolLM2-360M on AMD GPU):

```powershell
python train_ai.py --data ../data/run_10yr_1.cpb.zip \
  --base-model HuggingFaceTB/SmolLM2-360M \
  --max-steps 1000 \
  --prefetch 1000 \
  --batch-size 1
```

| Setting | Value | Rationale |
|---------|-------|-----------|
| `--batch-size` | 1 | Larger batches add overhead without speedup on DirectML |
| `--prefetch` | 1000 | Background data loading overlaps with GPU training |
| `--workers` | 0 (default) | Multiprocessing fails on Windows (pickle error) |

**Benchmarks** (50 steps, SmolLM2-360M):

| Batch Size | s/it | samples/s | Notes |
|------------|------|-----------|-------|
| 1 | 5.6s | 0.71 | Best throughput |
| 2 | 12.5s | 0.64 | -10% throughput |
| 4 | 27.0s | 0.59 | -17% throughput |

DirectML shows ~15% GPU utilization even at larger batch sizes. Prefetch queue stays full (1000 items), confirming data loading is NOT the bottleneck. The limitation is DirectML's translation layer overhead.

**Training time estimate**: At 0.71 samples/s, a 2.5M sample dataset takes ~40 days. Consider using CUDA for large-scale training.

## Troubleshooting

**`ValueError: Your setup doesn't support bf16/gpu`**: DirectML doesn't support bf16 (bfloat16) or fp16 mixed precision. The training script automatically detects DirectML and uses fp32 instead. If you see this error, make sure you have the latest `train_ai.py`.

**`FutureWarning: torch.cpu.amp.autocast...`**: This is a PyTorch internal deprecation warning, not from our code. It's harmless and doesn't affect training.

**"DirectML available but failed"**: Check your GPU drivers are up to date.

**Slow performance**: DirectML is ~60-80% of native CUDA speed. Still much faster than CPU. Expect ~5-6 seconds per step for SmolLM2-360M.

**Out of memory**: Reduce batch size or use a smaller model (e.g., SmolLM2-135M).

**`AttributeError: Can't pickle local object`**: This occurs when using `--workers N` (N > 0). Windows uses spawn-based multiprocessing which requires pickling, but TRL's internal functions can't be pickled. Solution: Use `--workers 0` (default) with `--prefetch 1000` instead.
