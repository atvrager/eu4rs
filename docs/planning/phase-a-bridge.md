# Phase A: EU4 Bridge Proof of Concept

**Status:** Planning
**Goal:** AI makes ONE decision in real EU4 via screen reading (no save file)
**Dev Platform:** Linux (Arch) — prototyping and CI
**Target Platform:** Windows — primary gaming setup (AMD 7900 XTX)

> **Cross-platform from day one.** We develop on Linux but will play on Windows. All crate choices (`xcap`, `enigo`, `leptess`) are cross-platform. Avoid Linux-specific APIs.

---

## Success Criteria

1. Program captures EU4 window screenshot
2. Extracts date from top bar via OCR
3. Extracts treasury from top bar via OCR
4. Builds minimal `VisibleWorldState` (stub most fields)
5. Calls `eu4sim-ai` inference (in-process, CPU)
6. Logs: "AI chose action X for state Y"
7. **Bonus:** Sends spacebar to pause/unpause game

No command execution yet — just the observation → decision loop.

---

## Crate Structure

```
eu4-bridge/
├── Cargo.toml
├── src/
│   ├── main.rs           # CLI entry point
│   ├── capture.rs        # Window capture (xcap)
│   ├── ocr.rs            # Text extraction (tesseract)
│   ├── regions.rs        # UI region coordinates
│   ├── extraction.rs     # Screenshot → VisibleWorldState
│   └── input.rs          # Keyboard/mouse (enigo) [Phase A: spacebar only]
```

---

## Dependencies

```toml
[package]
name = "eu4-bridge"
version = "0.1.0"
edition = "2024"

[dependencies]
# Core sim types
eu4sim-core = { path = "../eu4sim-core" }
eu4sim-ai = { path = "../eu4sim-ai" }

# Window capture
xcap = "0.0.14"

# OCR - tesseract bindings
leptess = "0.14"  # or tesseract-rs

# Input automation
enigo = "0.2"

# Image processing
image = "0.25"

# CLI
clap = { version = "4", features = ["derive"] }

# Logging
log = "0.4"
env_logger = "0.11"

# Error handling
anyhow = "1.0"
```

### System Dependencies (Arch)

```bash
# OCR engine
sudo pacman -S tesseract tesseract-data-eng

# Screen capture (X11/Wayland)
# xcap handles this, but may need:
sudo pacman -S xdotool  # for window detection
```

---

## UI Regions (1920x1080)

From manual inspection of EU4 top bar:

| Field | Region (x, y, w, h) | Format | Example |
|-------|---------------------|--------|---------|
| Date | (880, 8, 160, 24) | `YYYY.MM.DD` | `1444.11.11` |
| Treasury | (108, 8, 80, 20) | digits | `100` |
| Manpower | (250, 8, 80, 20) | digits + `k` | `45k` |
| ADM mana | (420, 8, 40, 20) | digits | `100` |
| DIP mana | (490, 8, 40, 20) | digits | `50` |
| MIL mana | (560, 8, 40, 20) | digits | `50` |

**Note:** These are approximate. Phase A will include a calibration mode to fine-tune.

---

## Main Loop (Pseudocode)

```rust
fn main() -> Result<()> {
    env_logger::init();

    // 1. Find EU4 window
    let window = find_window("Europa Universalis IV")?;
    info!("Found EU4 window: {:?}", window);

    // 2. Load AI model (CPU, no adapter for now)
    let mut ai = LlmAi::with_base_model()?;
    info!("AI model loaded");

    // 3. Main loop
    loop {
        // Pause game
        send_key(Key::Space)?;
        sleep(Duration::from_millis(500));

        // Capture screenshot
        let screenshot = capture_window(&window)?;
        debug!("Captured {}x{} screenshot", screenshot.width(), screenshot.height());

        // Extract state via OCR
        let state = extract_state(&screenshot)?;
        info!("Extracted state: date={}, treasury={}", state.date, state.own_country.treasury);

        // Build available commands (stub for Phase A)
        let commands = vec![
            Command::Pass,
            Command::BuyTech { tech_type: TechType::Adm },
        ];

        // Call AI
        let chosen = ai.decide(&state, &commands);
        info!("AI chose: {:?}", chosen);

        // Phase A: Just log, don't execute
        // Phase C+: translate to UI actions and execute

        // Unpause
        send_key(Key::Space)?;

        // Wait for next tick
        sleep(Duration::from_secs(5));
    }
}
```

---

## OCR Strategy

### Why Tesseract?

- Open source, well-maintained
- Works on Linux without cloud APIs
- Can be trained on EU4 fonts if needed

### Preprocessing Pipeline

```rust
fn extract_text(screenshot: &Image, region: Region) -> Result<String> {
    // 1. Crop to region
    let crop = screenshot.crop(region);

    // 2. Convert to grayscale
    let gray = crop.to_luma8();

    // 3. Threshold (EU4 uses light text on dark background)
    let binary = threshold(&gray, 128);

    // 4. Invert (Tesseract prefers dark text on light background)
    let inverted = invert(&binary);

    // 5. Scale up 2x (helps OCR accuracy)
    let scaled = resize(&inverted, 2.0);

    // 6. Run Tesseract
    let text = tesseract::ocr(&scaled, "eng")?;

    // 7. Clean up (remove whitespace, fix common errors)
    Ok(clean_ocr_text(&text))
}

fn clean_ocr_text(raw: &str) -> String {
    raw.trim()
       .replace('l', "1")  // Common OCR error
       .replace('O', "0")  // Common OCR error
       .replace('S', "5")  // Common OCR error
}
```

### Date Parsing

```rust
fn parse_date(text: &str) -> Result<Date> {
    // Expected: "1444.11.11" or "1444. 11. 11" (OCR spacing)
    let parts: Vec<&str> = text.split('.').collect();
    if parts.len() != 3 {
        bail!("Invalid date format: {}", text);
    }

    let year = parts[0].trim().parse()?;
    let month = parts[1].trim().parse()?;
    let day = parts[2].trim().parse()?;

    Ok(Date::new(year, month, day))
}
```

---

## Calibration Mode

For fine-tuning region coordinates:

```bash
# Capture a screenshot and highlight regions
eu4-bridge --calibrate

# Outputs: regions.png with colored boxes
# User adjusts config.toml, re-runs
```

```rust
fn calibrate(screenshot: &Image) {
    let mut debug_img = screenshot.clone();

    // Draw rectangles for each region
    draw_rect(&mut debug_img, REGIONS.date, Color::RED);
    draw_rect(&mut debug_img, REGIONS.treasury, Color::GREEN);
    draw_rect(&mut debug_img, REGIONS.manpower, Color::BLUE);
    // ...

    debug_img.save("regions.png")?;
    info!("Saved calibration image to regions.png");

    // Also dump OCR results
    for (name, region) in REGIONS.iter() {
        let text = extract_text(screenshot, region)?;
        println!("{}: \"{}\"", name, text);
    }
}
```

---

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_date_parsing() {
    assert_eq!(parse_date("1444.11.11").unwrap(), Date::new(1444, 11, 11));
    assert_eq!(parse_date("1444. 11. 11").unwrap(), Date::new(1444, 11, 11)); // OCR spacing
}

#[test]
fn test_treasury_parsing() {
    assert_eq!(parse_treasury("100").unwrap(), Fixed::from_int(100));
    assert_eq!(parse_treasury("1,234").unwrap(), Fixed::from_int(1234));
    assert_eq!(parse_treasury("l00").unwrap(), Fixed::from_int(100)); // OCR error
}
```

### Integration Test

```bash
# Requires EU4 running at 1920x1080
eu4-bridge --test-capture
# Saves: test_capture.png, test_ocr.txt
```

---

## Open Questions

1. **X11 vs Wayland:** `xcap` supports both, but Wayland capture may need permissions. Test on your setup.

2. **Window focus:** Does EU4 need to be focused for capture? (Probably not for screenshot, yes for input)

3. **Resolution:** Start with 1920x1080 hardcoded. Generalize later.

4. **Font training:** If OCR accuracy is <90%, may need to train Tesseract on EU4 fonts.

5. **Proton vs Native:** EU4 has a native Linux build. Does it work? Or using Proton?

---

## Deliverables

- [ ] `eu4-bridge` crate with basic structure
- [ ] Window capture working
- [ ] OCR extracting date and treasury (>80% accuracy)
- [ ] `VisibleWorldState` built from OCR (stubbed fields)
- [ ] AI inference called (logs decision)
- [ ] Spacebar pause/unpause working
- [ ] Calibration mode for region tuning

---

## Next Steps (Phase B+)

- **Phase B:** Extract more fields (manpower, mana, war status)
- **Phase C:** Execute one command (`DevelopProvince` via clicks)
- **Phase D:** Support 5-10 command types
- **Phase E:** Full campaign automation

---

*Created: 2025-12-21*
