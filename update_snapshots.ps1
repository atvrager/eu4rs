Write-Host "Updating snapshot tests..."
$env:UPDATE_SNAPSHOTS="1"

Write-Host "1. Rendering new snapshots..."
# Generate images first
cargo run -- snapshot --output map_province.png --mode province
cargo run -- snapshot --output map_political.png --mode political
cargo run -- snapshot --output map_tradegoods.png --mode trade-goods
cargo run -- snapshot --output map_religion.png --mode religion
cargo run -- snapshot --output map_culture.png --mode culture

if ($LASTEXITCODE -eq 0) {
    Write-Host "Snapshots rendered. Running tests to verify and commit..." -ForegroundColor Green
    # Now run tests, which should see the new images (if applicable) or pass if UPDATE_SNAPSHOTS is handled
    cargo test --bin eu4rs
} else {
    Write-Host "Failed to render snapshots." -ForegroundColor Red
    exit 1
}
