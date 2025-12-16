# Code Statistics

This project uses **tokei** for lines-of-code counting and **tera-cli** for generating HTML reports.

## Installation

```powershell
cargo install tokei tera-cli
```

## Usage

```powershell
# Console output
tokei

# JSON output
tokei --output json > stats.json

# Generate HTML report (use cmd redirection on Windows)
cmd /c "tokei --output json > stats.json"
tera -f .github/stats_template.html --json stats.json > stats.html
```

## CI Integration

On pushes to `main`, the CI workflow automatically:
1. Runs `tokei` to generate `stats.json`
2. Renders `stats.html` using the template
3. Uploads both as artifacts (retained 90 days)

Download the latest from: **Actions → Latest main build → Artifacts → code-statistics**
