# Save File Validation

This document describes the save file validation framework (`eu4sim-verify`) for testing simulation accuracy against real EU4 save files.

## 1. Purpose

Validate that our simulation formulas match the game by:
1. **State Consistency** - Compare sim calculations to cached values in saves
2. **Next-Step Prediction** - Hydrate save → run sim → compare to future save
3. **Action Inference** - Diff sequential saves to detect player/AI actions

## 2. Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   Save File     │────▶│  parse.rs        │────▶│  ExtractedState │
│  (.eu4)         │     │  (eu4save/regex) │     │  (normalized)   │
└─────────────────┘     └──────────────────┘     └────────┬────────┘
                                                          │
        ┌─────────────────┬───────────────────────────────┼───────────────────┐
        ▼                 ▼                               ▼                   ▼
┌───────────────┐ ┌───────────────┐              ┌───────────────┐   ┌───────────────┐
│  verify.rs    │ │  hydrate.rs   │              │   diff.rs     │   │  extract.rs   │
│  (formulas)   │ │  (→WorldState)│              │  (actions)    │   │  (metrics)    │
└───────────────┘ └───────────────┘              └───────────────┘   └───────────────┘
```

### Modules

| Module | Purpose |
|--------|---------|
| `parse.rs` | Load save files (ZIP/text/binary), extract state |
| `hydrate.rs` | Convert ExtractedState → sim WorldState |
| `verify.rs` | Compare sim calculations to save values |
| `predict.rs` | Run sim forward, compare to future save |
| `diff.rs` | Infer actions by comparing sequential saves |
| `extract.rs` | Extract verification data from parsed state |
| `melt.rs` | Convert binary saves to text (debugging) |
| `report.rs` | Generate human-readable reports |

## 3. Data Model

### ExtractedState

Normalized representation of save file data:

```rust
pub struct ExtractedState {
    pub meta: SaveMeta,
    pub countries: HashMap<String, ExtractedCountry>,
    pub provinces: HashMap<u32, ExtractedProvince>,
}

pub struct ExtractedCountry {
    pub tag: String,
    pub treasury: Option<f64>,
    pub current_manpower: Option<f64>,
    pub max_manpower: Option<f64>,
    pub adm_power: Option<f64>,
    pub dip_power: Option<f64>,
    pub mil_power: Option<f64>,
    pub advisors: Vec<ExtractedAdvisor>,
    pub owned_province_ids: Vec<u32>,
}

pub struct ExtractedProvince {
    pub id: u32,
    pub owner: Option<String>,
    pub base_tax: Option<f64>,
    pub base_production: Option<f64>,
    pub base_manpower: Option<f64>,
    pub buildings: Vec<String>,
    pub local_autonomy: Option<f64>,
}
```

### InferredAction

Actions detected by diffing sequential saves:

```rust
pub enum InferredAction {
    BuildBuilding { province_id: u32, building: String, owner: String },
    DevelopProvince { province_id: u32, dev_type: DevType, from: f64, to: f64 },
    HireAdvisor { country: String, advisor_type: String, skill: u8 },
    DismissAdvisor { country: String, advisor_type: String, skill: u8 },
    SpendMana { country: String, mana_type: ManaType, amount: f64 },
    TreasuryChange { country: String, delta: f64, likely_cause: String },
}
```

## 4. CLI Commands

```bash
# Check save file metadata
eu4sim-verify info save.eu4

# Verify formulas against cached values
eu4sim-verify check save.eu4 --country FRA

# Predict future state from save
eu4sim-verify predict --from save_t0.eu4 --to save_t1.eu4 \
    --country KOR --game-path /path/to/eu4

# Diff two saves to infer actions
eu4sim-verify diff --before save_t0.eu4 --after save_t1.eu4

# Melt binary save to text (debugging)
eu4sim-verify melt save.eu4 --head 100
```

## 5. Save Format Support

| Format | Support | Notes |
|--------|---------|-------|
| Text (ZIP) | Full | Standard non-ironman saves |
| Text (plain) | Full | Uncompressed saves |
| Binary (Ironman) | Partial | Requires token file |

### Ironman Token Resolution

Binary saves use 16-bit token IDs instead of field names. Resolution options:

1. **Environment variable**: `EU4_IRONMAN_TOKENS=/path/to/eu4.txt`
2. **Local file**: `assets/tokens/eu4.txt`
3. **pdx-tools tokens**: Download from Rakaly project

Tokens change between game versions - must match save version.

## 6. Validation Phases

### Phase 1: State Consistency

Compare sim formula output to cached values in a single save:

```
Load save → Extract metrics → Run sim formulas → Compare
```

**Metrics**: max_manpower, monthly_income, trade_power, army_maintenance

### Phase 2: Next-Step Prediction

Validate simulation stepping by comparing predicted vs actual:

```
Save T → Hydrate → step_world() × N → Compare → Save T+N
```

**Approach**: Passive simulation (no AI actions) isolates formula bugs.

### Phase 3: Action Inference

Diff sequential saves to detect state changes:

```
Save T₀ ──┐
          ├──▶ Diff Engine ──▶ [Action List]
Save T₁ ──┘
```

**Detectable actions**: building, development, advisor changes, spending

## 7. Test Data

Korea saves (plaintext, compressed) for testing:

| File | Date | Days from Start |
|------|------|-----------------|
| Korea1444_11_11.eu4 | Nov 11, 1444 | 0 |
| Korea1444_12_01.eu4 | Dec 1, 1444 | 20 |
| Korea1445_01_01.eu4 | Jan 1, 1445 | 51 |
| Korea1445_02_01.eu4 | Feb 1, 1445 | 82 |

## 8. Coverage Tracking

Similar to `eu4data`'s schema coverage tool, track which save fields are parsed and validated.

### Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   Save File     │────▶│  Field Scanner   │────▶│  Field Registry │
│  (sample)       │     │  (all keys)      │     │  (discovered)   │
└─────────────────┘     └──────────────────┘     └────────┬────────┘
                                                          │
┌─────────────────┐                              ┌────────▼────────┐
│  ExtractedState │────▶  Intersection  ◀────────│  Field Matcher  │
│  (parsed)       │                              │  (coverage %)   │
└─────────────────┘                              └─────────────────┘
```

### Data Model

```rust
/// Registry of all fields found in save files
pub struct FieldRegistry {
    pub countries: FieldSet,
    pub provinces: FieldSet,
    pub trade_nodes: FieldSet,
    pub armies: FieldSet,
    pub diplomacy: FieldSet,
}

pub struct FieldSet {
    /// All fields discovered in sample saves
    pub discovered: HashSet<String>,
    /// Fields we currently extract
    pub extracted: HashSet<String>,
    /// Fields we validate against sim
    pub validated: HashSet<String>,
}

pub struct CoverageReport {
    pub category: String,
    pub discovered: usize,
    pub extracted: usize,
    pub validated: usize,
    pub missing: Vec<String>,
}
```

### CLI

```bash
# Scan save for all fields (discovery)
eu4sim-verify coverage --scan save.eu4

# Check current extraction coverage
eu4sim-verify coverage --check save.eu4

# Generate coverage report
eu4sim-verify coverage --report save.eu4 --output coverage.json
```

### Output Format

```
=== Save Field Coverage ===

Category         Discovered  Extracted  Validated  Coverage
─────────────────────────────────────────────────────────────
Countries              45         12          6       13%
Provinces              30          8          4       13%
Trade Nodes            15          0          0        0%
Armies                 20          0          0        0%
Diplomacy              25          0          0        0%
─────────────────────────────────────────────────────────────
TOTAL                 135         20         10       7%

Missing high-value fields:
  countries.army_tradition    (used in battle calculations)
  countries.prestige          (used in diplomacy)
  provinces.trade_good        (used in production income)
  provinces.culture           (used in unrest calculations)
```

### Priority Fields

Track which missing fields block specific validation goals:

| Goal | Required Fields | Status |
|------|-----------------|--------|
| Manpower validation | max_manpower, province base_manpower | Done |
| Treasury prediction | treasury, monthly_income ledger | Partial |
| Trade income | trade_nodes, merchants, trade_power | Missing |
| Army maintenance | army unit counts, maintenance modifiers | Missing |
| Battle outcomes | army_tradition, discipline, morale | Missing |

### Implementation

New module: `coverage.rs`

```rust
/// Scan a save file for all field names (discovery phase)
pub fn scan_fields(state: &ExtractedState) -> FieldRegistry {
    // Walk the parsed jomini structure to find all keys
    // Compare against ExtractedState fields
}

/// Generate coverage report
pub fn coverage_report(registry: &FieldRegistry) -> Vec<CoverageReport> {
    registry.categories().map(|cat| {
        CoverageReport {
            category: cat.name,
            discovered: cat.discovered.len(),
            extracted: cat.extracted.len(),
            validated: cat.validated.len(),
            missing: cat.discovered.difference(&cat.extracted).collect(),
        }
    }).collect()
}
```

## 9. Roadmap

### Action Cost Validation

Validate that detected actions have correct costs:

```
Action: Build marketplace in Seoul
  Expected cost: -100 ducats
  Actual delta:  -100 ducats
  Status: PASS
```

### Full Campaign Replay

Phase 4: Validate entire campaign by replaying all autosaves:

```
autosave_001.eu4 → autosave_002.eu4 → ... → current.eu4
     ↓                   ↓                      ↓
  Predict            Validate              Compare
```

## 10. Dependencies

```toml
[dependencies]
eu4save = "0.8"        # Binary save parsing
jomini = "0.34"        # Low-level Clausewitz parser
zip = "2.0"            # Save decompression
eu4sim-core = { path = "../eu4sim-core" }
eu4sim = { path = "../eu4sim" }
eu4data = { path = "../eu4data" }
```

## 11. References

- [eu4save docs](https://docs.rs/eu4save) - Binary save parsing
- [jomini](https://github.com/rakaly/jomini) - Clausewitz format parser
- [pdx-tools](https://github.com/pdx-tools/pdx-tools) - Token file source
- [Tour of Clausewitz Syntax](https://pdx.tools/blog/a-tour-of-pds-clausewitz-syntax)
