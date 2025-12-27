# Feature Probing System

> **Status**: Design Complete (2025-12-24)

## Goal

Automatically track which features are implemented across the crate stack using compile-time metadata.

**Problem**: How do we know when a feature (like buildings) is fully implemented across data/sim/verify/AI/viz?

**Solution**: Use derive macros to generate static metadata, then cross-reference at compile time.

## Current Stack Flow

```
eu4data          →  eu4sim-verify  →  eu4sim-core     →  eu4sim-ai    →  eu4viz
(game files)         (save parsing)    (simulation)      (decisions)     (display)

Buildings: 60 fields   extraction?      no state field    no commands     no render
Trade:     100 fields  partial          TradeState        SendMerchant    nodes shown
War:       25 fields   partial          War struct        DeclareWar      armies shown
```

## Existing Infrastructure

| Component | Location | Reusable? |
|-----------|----------|-----------|
| `SchemaType` derive | `eu4data_derive/src/lib.rs:237` | Yes - extend for features |
| `SchemaFields` trait | `eu4data/src/coverage.rs` | Yes - field introspection |
| `available_commands()` | `eu4sim-core/src/step.rs:538` | Yes - runtime enumeration |
| `VisibleWorldState` | `eu4sim-core/src/ai/mod.rs:97` | Yes - AI visibility |

## Implementation

### Phase 1: Command Categorization

```rust
// eu4sim-core/src/input.rs
#[derive(FeatureProbe)]
pub enum Command {
    #[probe(category = "warfare", tier = 1)]
    DeclareWar { target: Tag, cb: Option<String> },

    #[probe(category = "diplomacy", tier = 2, stub = true)]
    OfferAlliance { target: Tag },

    #[probe(category = "economy", tier = 1)]
    BuildInProvince { province_id: ProvinceId, building: String },
}
```

Generated:
```rust
impl Command {
    pub fn categories() -> &'static [(&'static str, &'static [&'static str])] {
        &[
            ("warfare", &["DeclareWar", "OfferPeace", ...]),
            ("diplomacy", &["OfferAlliance", ...]),
            ("economy", &["BuildInProvince", "DevelopProvince", ...]),
        ]
    }

    pub fn tier(&self) -> u8 { ... }
    pub fn is_stub(&self) -> bool { ... }
}
```

### Phase 2: State Field Tracking

```rust
// eu4sim-core/src/state.rs
#[derive(FeatureProbe)]
pub struct ProvinceState {
    #[probe(feature = "economy")]
    pub base_production: Fixed,

    #[probe(feature = "military")]
    pub fort_level: u8,

    #[probe(feature = "institutions")]
    pub institution_presence: HashMap<InstitutionId, f32>,
}
```

Generated:
```rust
impl ProvinceState {
    pub fn feature_fields() -> HashMap<&'static str, Vec<&'static str>> {
        hashmap! {
            "economy" => vec!["base_production", "base_tax", "base_manpower"],
            "military" => vec!["fort_level"],
            "institutions" => vec!["institution_presence"],
        }
    }
}
```

### Phase 3: Cross-Layer Registry

```rust
// eu4sim-core/src/probe.rs

pub struct FeatureStatus {
    pub name: &'static str,
    pub data_fields: usize,      // Count from eu4data SchemaType
    pub sim_fields: usize,       // Count from FeatureProbe
    pub commands: (usize, usize), // (implemented, total)
    pub tier: u8,
}

pub fn feature_matrix() -> Vec<FeatureStatus> {
    // Cross-reference all derive-generated metadata
}
```

### Phase 4: CLI

```bash
cargo run -p eu4sim-verify -- features

# Output:
Feature        Data   Sim    Commands  Status
────────────────────────────────────────────
warfare        ✅ 12  ✅ 8   ✅ 6/6    Complete
diplomacy      ✅ 8   ⚠️ 3   ⚠️ 2/8    Partial (stubs)
buildings      ✅ 60  ❌ 0   ❌ 0/2    Data only
institutions   ✅ 4   ✅ 2   ✅ 2/2    Complete
```

## Files to Create/Modify

| File | Action |
|------|--------|
| `eu4data_derive/src/lib.rs` | Add `FeatureProbe` derive macro |
| `eu4sim-core/src/input.rs` | Add `#[probe(...)]` to Command |
| `eu4sim-core/src/state.rs` | Add `#[probe(...)]` to state types |
| `eu4sim-core/src/probe.rs` | New: feature matrix registry |
| `eu4sim-verify/src/main.rs` | Add `features` subcommand |

## Success Criteria

- [ ] `#[derive(FeatureProbe)]` generates static metadata
- [ ] `Command::categories()` returns categorized commands
- [ ] `feature_matrix()` cross-references all layers
- [ ] CLI shows completeness per feature
- [ ] Updates automatically when code changes

## Design Decisions

| Decision | Rationale |
|----------|-----------|
| Compile-time metadata | Zero runtime cost, always accurate |
| Extend existing derive macro infrastructure | Reuse `eu4data_derive` patterns |
| Feature categories match game concepts | warfare, diplomacy, economy, trade, etc. |
| Tier annotation | Track implementation depth (stub vs complete) |

## Dependencies

No new crates required - reuses existing proc-macro infrastructure.
