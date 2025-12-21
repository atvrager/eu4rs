# Training Data Schema

This directory contains [Cap'n Proto](https://capnproto.org/) schema definitions for ML training data.

## Files

- `training.capnp` - Training sample format (state, actions, choices)

## Why Cap'n Proto?

- **Zero-copy reads**: Fast deserialization for large training datasets
- **Schema-first**: Single source of truth for Rust and Python types
- **Evolution-safe**: New fields can be added without breaking old data

## Building

### Prerequisites

Install the Cap'n Proto compiler:

```bash
# Linux/macOS
brew install capnp   # or apt-get install capnproto

# Windows
choco install capnproto
# Or download from https://capnproto.org/install.html
```

### Compile for Rust

```bash
cargo xtask schema
```

This generates Rust code in `eu4sim-core/src/generated/capnp/`.

### Compile for Python

```bash
cd scripts
capnp compile -I ../schemas -opython:schema ../schemas/training.capnp
```

This generates `schema/training_capnp.py`.

## Schema Evolution Rules

1. **New fields**: Add at the end with next ordinal number
2. **Never reuse ordinals**: Deleted fields leave gaps
3. **Never reorder**: Ordinal numbers are wire format
4. **Deprecate safely**: Rename to `deprecatedFieldName`

Example:
```capnp
struct Foo {
  name @0 :Text;
  deprecatedAge @1 :UInt8;  # Was: age
  email @2 :Text;           # Added later
  phone @3 :Text;           # Added even later
}
```

## Usage

### Rust

```rust
use crate::generated::capnp::training_capnp;

let sample = training_capnp::training_sample::Reader::new(&data);
println!("Tick: {}", sample.get_tick());
```

### Python

```python
import capnp
capnp.remove_import_hook()
import training_capnp

with open("training.bin", "rb") as f:
    batch = training_capnp.TrainingBatch.read(f)
    for sample in batch.samples:
        print(f"Tick {sample.tick}: {sample.country}")
```

## See Also

- [docs/design/data/training-data-format.md](../docs/design/data/training-data-format.md) - Full documentation
- [Cap'n Proto Language Guide](https://capnproto.org/language.html)
