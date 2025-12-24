# Save File Validation Guide

This document covers using EU4 save files to validate simulation correctness.

## Overview

EU4 save files contain cached values (manpower, income, etc.) that we can compare against our simulation calculations. This "state consistency" approach validates that our formulas match the game's.

## Token System

### The Problem

Ironman/binary EU4 saves use 16-bit token IDs instead of string field names. To parse them, you need a token mapping file.

```
0x2f3f player
0x284d countries
0x1234 some_field
```

### Token Sources

| Source | Notes |
|--------|-------|
| **pdx-tools** | Use [Rakaly CLI](https://github.com/rakaly/cli) with `--unknown-key stringify` |
| **PDX-Unlimiter** | Can extract from game executable |
| **eu4tokens** | Extracts strings but not proper IDs (WIP) |

### Why eu4tokens Doesn't Work (Yet)

Our `eu4tokens` tool finds field name strings in the game binary but can't recover the token ID mappings. The IDs are likely stored in:
- A hash table in .data/.bss sections
- Computed at runtime
- Embedded in code

Proper extraction requires deeper reverse engineering or runtime interception.

### Workaround: Rakaly CLI

```bash
# Install rakaly (from pdx-tools)
cargo install rakaly

# Melt with unknown tokens as hex
rakaly melt --unknown-key stringify save.eu4 > melted.txt

# The output will have unknown tokens as 0x1234=value
```

This produces text output that can be parsed with regex.

## Using eu4sim-verify

### With Proper Tokens
```bash
# Set token file path
export EU4_IRONMAN_TOKENS=/path/to/eu4.txt

# Run verification
cargo run -p eu4sim-verify -- check save.eu4
```

### With Text Saves (Non-Ironman)
```bash
# Text saves parse without tokens
cargo run -p eu4sim-verify -- info save.eu4
cargo run -p eu4sim-verify -- check save.eu4
```

### Output
```
=== Verification Report ===

Total: 10 | Passed: 7 | Failed: 2 | Skipped: 1
Pass Rate: 77.8%

--- FAILURES ---
[FAIL] MaxManpower(FRA): expected=50000, actual=48000, delta=2000
       Delta 2000 exceeds tolerance 500

--- PASSES ---
[PASS] MonthlyTax(FRA): expected=100.0, actual=99.5

--- SKIPPED ---
[SKIP] MonthlyTrade(FRA): Trade verification not yet implemented
```

## Metrics Verified

| Metric | Status | Notes |
|--------|--------|-------|
| Max Manpower | ✅ Basic | Province-based, no modifiers |
| Monthly Tax | ✅ Basic | Province-based, no modifiers |
| Monthly Trade | ⏳ Skip | Needs trade node data |
| Monthly Production | ⏳ Skip | Needs goods prices |
| Institution Spread | ⏳ Skip | Complex calculation |

## Save File Format

### Structure
```
save.eu4 (ZIP archive)
├── meta       # Game metadata, version
├── gamestate  # Main game state (binary or text)
└── ai         # AI data
```

### Binary vs Text
- **EU4bin**: Ironman saves, uses token IDs
- **EU4txt**: Normal saves, human-readable

### Version Compatibility

Token IDs change between game versions. Check save version:
```bash
unzip -p save.eu4 meta | xxd | head
# Look for version string like "1.37.x.x"
```

## Future Work

1. **Token ID extraction**: Reverse engineer proper ID mappings from game binary
2. **Runtime hooking**: Use LD_PRELOAD to intercept token table at runtime
3. **Community tokens**: Collaborate with pdx-tools/ironmelt communities
4. **More metrics**: Trade, diplomacy, military calculations
