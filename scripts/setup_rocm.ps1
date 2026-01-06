# ROCm 7.1.1 Setup Script for Windows
# Creates a dedicated venv with PyTorch + ROCm support for AMD GPUs
#
# Prerequisites:
#   - AMD Radeon RX 7000 series (or compatible)
#   - AMD driver version 25.20.01.17 or newer
#   - Python 3.12
#   - uv package manager

param(
    [switch]$SkipVerify,
    [switch]$Force
)

$ErrorActionPreference = "Stop"
$VenvPath = ".venv-rocm"
$RocmVersion = "rocm-rel-7.1.1"
$RocmRepo = "https://repo.radeon.com/rocm/windows/$RocmVersion"

Write-Host "=== ROCm 7.1.1 Setup for Windows ===" -ForegroundColor Cyan
Write-Host ""

# Check if uv is available
if (-not (Get-Command uv -ErrorAction SilentlyContinue)) {
    Write-Host "ERROR: 'uv' is not installed. Install it first:" -ForegroundColor Red
    Write-Host "  irm https://astral.sh/uv/install.ps1 | iex"
    exit 1
}

# Note: uv will auto-download Python 3.12 if not available
Write-Host "Note: ROCm wheels require Python 3.12 (uv will install it automatically)" -ForegroundColor Gray

# Create venv if needed
if (Test-Path $VenvPath) {
    if ($Force) {
        Write-Host "Removing existing venv..." -ForegroundColor Yellow
        Remove-Item -Recurse -Force $VenvPath
    } else {
        Write-Host "Venv already exists at $VenvPath" -ForegroundColor Yellow
        Write-Host "Use -Force to recreate it."
        exit 0
    }
}

Write-Host "Creating venv at $VenvPath..." -ForegroundColor Green
uv venv $VenvPath --python 3.12

# Build full path to venv python (for verification later)
$VenvDir = Join-Path $PSScriptRoot $VenvPath
$VenvPython = Join-Path $VenvDir "Scripts\python.exe"

Write-Host ""
Write-Host "Installing ROCm SDK packages..." -ForegroundColor Green
uv pip install --python $VenvPython --no-cache `
    "$RocmRepo/rocm_sdk_core-0.1.dev0-py3-none-win_amd64.whl" `
    "$RocmRepo/rocm_sdk_devel-0.1.dev0-py3-none-win_amd64.whl" `
    "$RocmRepo/rocm_sdk_libraries_custom-0.1.dev0-py3-none-win_amd64.whl" `
    "$RocmRepo/rocm-0.1.dev0.tar.gz"

Write-Host ""
Write-Host "Installing PyTorch with ROCm support..." -ForegroundColor Green
Write-Host "(This may take several minutes)"
uv pip install --python $VenvPython --no-cache `
    "$RocmRepo/torch-2.9.0+rocmsdk20251116-cp312-cp312-win_amd64.whl" `
    "$RocmRepo/torchaudio-2.9.0+rocmsdk20251116-cp312-cp312-win_amd64.whl" `
    "$RocmRepo/torchvision-0.24.0+rocmsdk20251116-cp312-cp312-win_amd64.whl"

Write-Host ""
Write-Host "Installing training dependencies..." -ForegroundColor Green
uv pip install --python $VenvPython transformers "peft==0.12.0" trl datasets pycapnp safetensors python-dotenv

Write-Host ""
Write-Host "=== Installation Complete ===" -ForegroundColor Cyan

if (-not $SkipVerify) {
    Write-Host ""
    Write-Host "Verifying installation..." -ForegroundColor Green
    
    # Use only discrete GPU (index 1) to avoid integrated GPU issues
    $env:CUDA_VISIBLE_DEVICES = "1"
    
    # Test torch import
    $torchTest = & $VenvPython -c "import torch; print('OK')" 2>&1
    if ($torchTest -eq "OK") {
        Write-Host "  [OK] PyTorch imported successfully" -ForegroundColor Green
    } else {
        Write-Host "  [FAIL] PyTorch import failed: $torchTest" -ForegroundColor Red
        exit 1
    }
    
    # Test GPU availability
    $gpuTest = & $VenvPython -c "import torch; print(torch.cuda.is_available())" 2>&1
    if ($gpuTest -eq "True") {
        Write-Host "  [OK] GPU is available" -ForegroundColor Green
    } else {
        Write-Host "  [FAIL] GPU not detected" -ForegroundColor Red
        Write-Host "  Make sure AMD driver 25.20.01.17+ is installed"
        exit 1
    }
    
    # Get GPU name
    $gpuName = & $VenvPython -c "import torch; print(torch.cuda.get_device_name(0))" 2>&1
    Write-Host "  [OK] GPU: $gpuName" -ForegroundColor Green
    
    # Check HIP version (confirms ROCm, not CUDA)
    $hipVersion = & $VenvPython -c "import torch; print(getattr(torch.version, 'hip', 'N/A'))" 2>&1
    if ($hipVersion -ne "N/A") {
        Write-Host "  [OK] HIP Runtime: $hipVersion" -ForegroundColor Green
    }
}

Write-Host ""
Write-Host "=== Setup Complete ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "To use ROCm for training:" -ForegroundColor White
Write-Host "  1. Activate the venv:" -ForegroundColor Gray
Write-Host "     $VenvPath\Scripts\Activate.ps1" -ForegroundColor Yellow
Write-Host ""
Write-Host "  2. Run training:" -ForegroundColor Gray
Write-Host "     python train_ai.py --data ..\data\run_10yr_1.cpb.zip --max-steps 100" -ForegroundColor Yellow
Write-Host ""
Write-Host "  You should see: 'Using device: ROCm (AMD Radeon RX 7900 XTX)'" -ForegroundColor Gray
