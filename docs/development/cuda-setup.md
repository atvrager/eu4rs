# CUDA Setup for Local Development

This guide covers setting up CUDA for local GPU-accelerated ML inference with `eu4sim-ai`.

## Quick Start (Arch Linux)

```bash
# Install official CUDA from extra/
sudo pacman -S cuda

# Verify installation
nvcc --version  # Should show 13.x

# Test the build
cargo test -p eu4sim-ai --features cuda
```

## Requirements

| Component | Version | Notes |
|-----------|---------|-------|
| CUDA Toolkit | 13.0+ | From `extra/cuda` |
| NVIDIA Driver | 580+ | Check with `nvidia-smi` |
| cudarc | 0.18+ | Via candle git branch |

## Why CUDA 13?

We use CUDA 13 from Arch's official repos because:

1. **glibc Compatibility**: Arch uses bleeding-edge glibc (2.41+) which has new math functions (`cospi`, `sinpi`, `rsqrt`) that older CUDA versions can't compile against.

2. **cudarc Support**: The candle ML framework's `cudarc` bindings require 0.18+ for CUDA 13 support. We use candle from git (`main` branch) rather than crates.io 0.9.1.

3. **Simple Setup**: The official package integrates properly with system paths and doesn't require building GCC12 from AUR.

## GPU Architecture Notes

| GPU Generation | Compute Capability | BF16 Support |
|----------------|-------------------|--------------|
| Turing (RTX 20xx) | sm_75 | Limited |
| Ampere (RTX 30xx) | sm_80/86 | Full |
| Ada Lovelace (RTX 40xx) | sm_89 | Full |

The test suite gracefully skips BF16 tests on Turing and older GPUs.

## Environment Setup

The `cuda` package sets up `/etc/profile.d/cuda.sh` automatically. For additional configuration, add to `~/.commonsh/05_cuda` (or equivalent):

```bash
# CUDA environment setup for candle ML framework
# Official cuda package from extra/ sets up /etc/profile.d/cuda.sh
# This file adds CUDA_HOME alias expected by some Rust crates

if [ -d "/opt/cuda" ]; then
    export CUDA_HOME="/opt/cuda"
    # LD_LIBRARY_PATH for runtime linking
    export LD_LIBRARY_PATH="$CUDA_HOME/lib64${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi
```

## Troubleshooting

### "Unsupported cuda toolkit version"

**Cause**: cudarc version is too old for your CUDA toolkit.

**Fix**: Ensure candle is from git, not crates.io:
```toml
# eu4sim-ai/Cargo.toml
candle-core = { git = "https://github.com/huggingface/candle", branch = "main" }
```

### glibc header errors (`_Float32`, `cospi` undefined)

**Cause**: CUDA 12.x can't compile against glibc 2.41+ headers.

**Fix**: Use CUDA 13 from official repos:
```bash
sudo pacman -Rdd cuda12.0  # If you have the AUR version
sudo pacman -S cuda
```

### Build cache issues after switching CUDA versions

```bash
# Clear candle/cudarc build artifacts
rm -rf target/debug/build/candle-* target/debug/build/cudarc-*
cargo update -p candle-core -p candle-nn -p candle-transformers
```

## CI Notes

CI runs CPU-only tests. The `cuda` feature is for local development only:

```bash
cargo test -p eu4sim-ai              # CI-safe (CPU only)
cargo test -p eu4sim-ai --features cuda  # Local only (requires GPU)
```

## What NOT to Do

- **Don't install cuda12.0 from AUR**: Requires building GCC12, still has glibc issues
- **Don't use crates.io candle 0.9.1**: Uses cudarc 0.16.6 which doesn't support CUDA 13
- **Don't set NVCC_CCBIN to older GCC**: CUDA 13 is compatible with system GCC

## Version History

| Date | Change |
|------|--------|
| 2025-12-22 | Initial setup with CUDA 13 + candle git |
