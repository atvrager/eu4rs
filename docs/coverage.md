# Data Coverage System

`eu4rs` uses a custom coverage system to track the completeness of its game data integration. Unlike standard code coverage (which measures which lines of code run), this system measures **how much of the EU4 game data is understood and used by the engine**.

## The Three Metrics

For every data field in every game file (e.g. `base_tax`, `manpower`, `color`), we track three increasing levels of support:

### 1. Parsed (`P`)
**"We know what this is."**
- The field is defined in a Rust struct (typically in `eu4data`).
- The parser reads the value from game files into memory.
- It is available for the engine to use.
- **Goal**: 100% for all categories. If we don't parse it, we can't use it.
- *Note*: Fields mapped to `IgnoredAny` or `HashMap<String, IgnoredAny>` count as Parsed, even if we discard the value. This acknowledges the field's existence.

### 2. Visualized (`V`)
**"The user can see this."**
- The data is rendered in the UI or on the Map.
- Examples: A province's `base_tax` shown in a tooltip, or `religion` color used in the Religion Map Mode.
- Annotated with `#[schema(visualized)]`.

### 3. Simulated (`S`)
**"The game logic uses this."**
- The data affects the simulation tick, AI decisions, or game rules.
- Examples: `base_tax` increasing income, `unrest` causing rebellions.
- Annotated with `#[schema(simulated)]`.

## Data Categories

EU4's game data is organized into directories, and we mirror this structure with **Data Categories**. Each category represents a logical grouping of related data files.

### Auto-Discovery

Categories are **automatically discovered** by scanning the EU4 installation:
- `common/*` directories (e.g., `common/religions`, `common/tradegoods`)
- `history/*` directories (e.g., `history/provinces`, `history/countries`)

The `DataCategory` enum and its methods are generated into `eu4data/src/generated/categories.rs` when you run `cargo xtask coverage --update`.

### Category Properties

Each category has:
- **Name**: Human-readable label (e.g., "Provinces History")
- **Path Suffix**: Relative path to the data directory (e.g., `history/provinces`)
- **Nesting**: Whether definitions are nested within groups (e.g., religions are inside religion groups)

### Current Categories

Run `cargo xtask coverage` to see all discovered categories with their coverage percentages. Examples include:

| Category | Path | Notes |
|----------|------|-------|
| Provinces History | `history/provinces/` | Province-level starting conditions |
| Countries History | `history/countries/` | Country-level starting conditions |
| Religions | `common/religions/` | Religion definitions (nested) |
| Cultures | `common/cultures/` | Culture definitions (nested) |
| Tradegoods | `common/tradegoods/` | Trade good definitions |
| Countries | `common/countries/` | Base country definitions |

## Usage

### The `SchemaType` Derive

We use a custom derive macro to automate coverage tracking. You don't need to manually register fields; just annotate your struct.

```rust
use eu4data_derive::SchemaType;  // or `use eu4data::coverage::SchemaType;` within eu4data
use serde::Deserialize;

#[derive(Deserialize, SchemaType)]
pub struct ProvinceHistory {
    // Parsed only (default - no annotation needed)
    pub extra_cost: Option<f32>,

    // Parsed + Visualized (shown on map or UI)
    #[schema(visualized)]
    pub trade_goods: Option<String>,

    // Parsed + Visualized (simulated is reserved for future game logic)
    #[schema(visualized)]
    pub base_tax: Option<f32>,
}
```

### The generic catch-all

To achieve 100% Parse coverage without implementing every single obscure field, use the `flatten` pattern with `IgnoredAny`. This tells the system "I accept any other fields here, but I don't care about their values yet."

```rust
#[serde(flatten)]
pub other: HashMap<String, IgnoredAny>,
```

## CLI Tool

The `xtask` tool manages the coverage database.

- **Check Coverage**:
  ```bash
  cargo xtask coverage
  ```
  Prints a summary ascii-bar report to the terminal.

- **Discovery Mode**:
  ```bash
  cargo xtask coverage --discover
  ```
  Scans your actual EU4 installation directory to find *every single field* present in the game files. It compares this "empirical truth" against your code to calculate percentages.

- **Update Documentation**:
  ```bash
  cargo xtask coverage --update
  ```
   Updates `docs/supported_fields.md` with the latest status of every field (✅/❌).
