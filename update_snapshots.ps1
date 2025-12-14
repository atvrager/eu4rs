
Write-Host "Updating snapshot tests..."
$env:UPDATE_SNAPSHOTS="1"
cargo test --bin eu4rs

Write-Host "Generating root images..."
cargo run -- snapshot --output map_province.png --mode province
cargo run -- snapshot --output map_political.png --mode political
if ($LASTEXITCODE -eq 0) {
    Write-Host "Snapshots updated successfully in tests/goldens/" -ForegroundColor Green
} else {
    Write-Host "Failed to update snapshots." -ForegroundColor Red
}
