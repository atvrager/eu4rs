Write-Host "Running Local CI Pipeline..." -ForegroundColor Cyan

Write-Host "`n[1/4] Checking Formatting..." -ForegroundColor Yellow
cargo fmt -- --check
if ($LASTEXITCODE -ne 0) { Write-Error "Formatting failed!"; exit 1 }

Write-Host "`n[2/4] Running Clippy..." -ForegroundColor Yellow
cargo clippy -- -D warnings
if ($LASTEXITCODE -ne 0) { Write-Error "Clippy failed!"; exit 1 }

Write-Host "`n[3/4] Running Tests..." -ForegroundColor Yellow
cargo test
if ($LASTEXITCODE -ne 0) { Write-Error "Tests failed!"; exit 1 }

Write-Host "`n[4/4] Building Release..." -ForegroundColor Yellow
cargo build --release
if ($LASTEXITCODE -ne 0) { Write-Error "Build failed!"; exit 1 }

Write-Host "`nLocal CI Passed! 🚀" -ForegroundColor Green
