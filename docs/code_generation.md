# Code Generation System

The usage of `eu4rs` now leverages an automated code generation system to create Rust types from EU4 data files. This reduces manual boilerplate and ensures our data structures match the actual game data schema.

## Overview

The system consists of three main components:
1. **Schema Discovery** (`eu4data/src/discovery.rs`): Scans `.txt` files to infer fields and types.
2. **Schema Definition** (`eu4data/src/generated/schema.rs`): A checked-in rust file acting as the "Gold Standard" schema.
3. **Type Generator** (`eu4data/src/codegen.rs`): Generates Rust structs from the schema.

## Workflow

### 1. Update Schema
To discover new fields or update type inference based on changed game data:

```bash
cargo xtask coverage --update
```

This commands:
- Scans your EU4 installation.
- Updates `eu4data/src/generated/categories.rs` (DataCategory enum).
- Updates `eu4data/src/generated/schema.rs` (Field definitions, frequencies, inferred types).

### 2. Generate Types
To generate Rust structs from the schema:

```bash
cargo xtask coverage --generate
```

This commands:
- Reads `schema.rs`.
- Generates `eu4data/src/generated/types/[category].rs` for every category.
- Updates `eu4data/src/generated/types/mod.rs`.

**Note:** Generated types are **NOT checked in** to git (ignored via `.gitignore`). They are generated on demand or during development.

## Hybrid Shim Strategy

We use a "Hybrid Shim" strategy to balance automation with manual control.

- **Generated Types** live in `eu4data::generated::types::*`.
- **Public API** lives in `eu4data::types::*`.

### Migrating a Type
To expose a generated type to the rest of the application:

1. Create a shim file in `eu4data/src/types/[name].rs`.
2. Add `pub mod [name];` to `eu4data/src/types.rs`.
3. In the shim, re-export the generated type:

```rust
// eu4data/src/types/technologies.rs
pub use crate::generated::types::technologies::*;
```

### Overriding a Type
If the generated code is insufficient (e.g., complex logic, custom serde, manual fixups):

1. Copy the generated code from `eu4data/src/generated/types/[name].rs`.
2. Paste it into your shim file `eu4data/src/types/[name].rs`.
3.  modify it as needed.
4. The public API `eu4data::types::[Name]` now uses your manual implementation.

## Type Inference

The system infers types based on sample values found in game files.

| Inferred Type | Rust Type | Notes |
|---------------|-----------|-------|
| `Integer` | `i32` | Preferred for determinism. |
| `Float` | `f32` | Flagged for possible fixed-point conversion. |
| `String` | `String` | Unquoted or quoted strings. |
| `Bool` | `bool` | yes/no |
| `IntList` | `Vec<i32>` | `{ 1 2 3 }` |
| `FloatList` | `Vec<f32>` | `{ 0.1 0.2 }` |
| `StringList` | `Vec<String>` | `{ "a" "b" }` |
| `Block` | `serde_json::Value` | Nested structures (placeholder). |
| `DynamicBlock` | `HashMap<String, serde_json::Value>` | Variable keys. |

### Multiplicity
If a field appears multiple times in the same block (e.g. `mercenary_companies` appearing multiple times in a history file), it is marked `appears_multiple: true` and wrapped in a `Vec<T>`.

## Customization

You can customize the generation logic in `eu4data/src/codegen.rs`.
- `sanitize_field_name`: Handles Rust keyword collisions (`type` -> `r#type`).
- `inferred_type_to_rust`: Maps inferred types to Rust types.
