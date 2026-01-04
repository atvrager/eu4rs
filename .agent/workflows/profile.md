---
description: Profile eu4game performance with Tracy to identify bottlenecks
---

# Profile Workflow

Use this workflow to capture real-time profiling data from eu4game and generate an analysis report that I can read and act upon.

## When to Use

- Investigating performance issues (frame drops, stutters)
- Before/after optimization comparisons
- Understanding where CPU time is spent
- Identifying rendering bottlenecks

---

## Prerequisites

### 1. Install Tracy Server (One-time Setup)

Download Tracy from: https://github.com/wolfpld/tracy/releases

**Windows:**
```powershell
# Extract Tracy.exe and tracy-csvexport.exe to a directory in PATH
# Or run from the extracted directory
```

**Linux:**
```bash
# Extract Tracy-release and tracy-csvexport
chmod +x Tracy-release tracy-csvexport
sudo mv Tracy-release /usr/local/bin/tracy
sudo mv tracy-csvexport /usr/local/bin/
```

**macOS:**
```bash
# Similar to Linux
```

### 2. Launch Tracy Server

Before profiling, start the Tracy server:

```bash
# Windows
.\Tracy.exe

# Linux/macOS
tracy
```

Leave this running - it will capture data when eu4game connects.

---

## Step 1: Run Profiling Session

```bash
cargo xtask profile --duration 60
```

**Options:**
- `--duration <seconds>`: How long to run (default: 60)
- `--output <dir>`: Custom output directory (default: `profiling/YYYYMMDD_HHMMSS`)

**What happens:**
1. Builds `eu4game` with Tracy instrumentation enabled
2. Runs the app for specified duration
3. Kills the app and saves `.tracy` capture file
4. Exports to CSV using `tracy-csvexport`
5. Generates markdown report using `scripts/analyze_tracy.py`

---

## Step 2: Review Report

The report is saved to `profiling/<timestamp>/report.md`:

```bash
# Read the latest report
cat profiling/latest/report.md  # Linux/macOS
type profiling\latest\report.md  # Windows (manual path)
```

**Or just tell me:**
> "Analyze the profiling report in profiling/20260104_153000"

And I'll read it directly.

---

## Step 3: Compare Runs (Optional)

For before/after comparisons:

```bash
# Before optimization
cargo xtask profile --output profiling/before_fix

# Make changes...

# After optimization
cargo xtask profile --output profiling/after_fix

# Compare manually or ask me to compare the reports
```

---

## Understanding the Report

The markdown report includes:

### Top 20 Hotspots
Functions/zones consuming the most total time. Look for:
- High % Total (>10%) - major bottlenecks
- High Max time - occasional slowdowns
- p95/p99 - tail latency issues

### Frame Statistics
If frame markers are present:
- Average FPS
- p95/p99 frame times (user experience)
- Worst frame identification

### Slowest Individual Calls
Single worst offenders - useful for finding one-off spikes.

---

## Instrumentation Guide

### Adding Zone Markers

To instrument new code for profiling:

```rust
// At the top of the file
#[cfg(feature = "tracy")]
use tracy_client;

// In a function
pub fn render_terrain(&mut self) {
    #[cfg(feature = "tracy")]
    let _zone = tracy_client::span!("render_terrain");

    // ... your code ...
}

// For frame boundaries (main loop)
loop {
    #[cfg(feature = "tracy")]
    tracy_client::frame_mark();

    update();
    render();
}
```

### Macro Helper

For convenience, add to a common module:

```rust
macro_rules! profile_zone {
    ($name:expr) => {
        #[cfg(feature = "tracy")]
        let _zone = tracy_client::span!($name);
    };
}
```

Then use: `profile_zone!("my_function");`

---

## Troubleshooting

### "No .tracy file found"
- Make sure Tracy server is running **before** starting the profile
- Check that eu4game connected (Tracy shows connection in UI)

### "tracy-csvexport not found"
- Install tracy-csvexport from Tracy releases
- Add to PATH or specify full path in `xtask/src/profile.rs`

### CSV export fails
- Make sure the `.tracy` file isn't corrupted
- Try opening it in Tracy GUI manually to verify

### Report generation fails
- Check that Python 3 is installed
- Verify `scripts/analyze_tracy.py` exists and is executable

---

## Output Files

Each profiling session creates:

```
profiling/
└── 20260104_153000/
    ├── capture.tracy   # Binary capture (DO NOT COMMIT)
    ├── profile.csv     # Exported data (DO NOT COMMIT)
    └── report.md       # Analysis report (CAN COMMIT if needed for comparison)
```

**Note:** `.gitignore` excludes `*.tracy` and `*.csv` files automatically.

---

## Best Practices

1. **Profile in Release Mode** - Always use `--release` builds (xtask does this automatically)
2. **Consistent Duration** - Use same duration for comparisons (60s default is good)
3. **Warm-up** - First few frames may be slower; 60s smooths this out
4. **Instrument Sparingly** - Too many zones add overhead; focus on major systems
5. **Frame Marks** - Always add `frame_mark()` in the main loop for FPS analysis

---

*See also: `docs/development/performance.md` for performance optimization guidelines.*
