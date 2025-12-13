
Write-Host "Updating snapshot tests..."
$env:UPDATE_SNAPSHOTS="1"
cargo test --bin eu4rs
if ($LASTEXITCODE -eq 0) {
    Write-Host "Snapshots updated successfully in tests/goldens/" -ForegroundColor Green
} else {
    Write-Host "Failed to update snapshots." -ForegroundColor Red
}
