# Golden Images for eu4game GUI Tests

This directory contains reference images for visual regression testing.

## How Goldens Work

1. **First run**: Tests save their output as new golden images
2. **Subsequent runs**: Tests compare output against goldens pixel-by-pixel
3. **On mismatch**: Test fails and saves `*_actual.png` for debugging

## Updating Goldens

When intentionally changing GUI rendering:

```bash
UPDATE_SNAPSHOTS=1 cargo test -p eu4game gui::tests
```

Or use the xtask command:

```bash
cargo xtask snapshot-gui
```

## Current Goldens

| Golden | Description |
|--------|-------------|
| `speed_controls.png` | Speed controls panel with date, indicator, buttons |
| `topbar.png` | Top bar with resource icons and backgrounds |

## Requirements

Tests require EU4 game assets to be available. Tests skip gracefully
(CI waiver) when:
- No GPU adapter is found
- EU4 game path is not detected

## Reviewing Changes

When a test fails, compare:
- `<name>.png` - expected golden
- `<name>_actual.png` - actual test output

Use an image diff tool to identify visual regressions.
