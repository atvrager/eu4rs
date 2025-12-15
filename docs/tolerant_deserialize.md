# TolerantDeserialize Macro

## Overview
The `TolerantDeserialize` derive macro provides a custom deserialization implementation that handles duplicate keys in EU4 data files. This is necessary because EU4 uses a text format where the same key can appear multiple times (e.g., `add_core = FRA`, `add_core = ENG`), which Serde's default struct deserializer rejects as an error.

## When to Use
Use `#[derive(TolerantDeserialize)]` instead of `#[derive(Deserialize)]` when:
- The struct represents EU4 data that may contain duplicate keys
- You need explicit field definitions (for documentation/schema) but want crash-free parsing
- You have fields typed as `Option<Vec<T>>` that should accumulate duplicate values

## How It Works
The macro generates a custom `Deserialize` visitor that:
1. **Accumulates duplicate keys** into `Vec` fields (for `Option<Vec<T>>` types)
2. **Uses last-value-wins** for non-Vec fields
3. **Skips unknown fields** silently (no errors for unrecognized keys)

## Example
```rust
#[derive(TolerantDeserialize, SchemaType)]
pub struct ProvinceHistory {
    pub owner: Option<String>,  // last-wins if duplicated
    pub add_core: Option<Vec<IgnoredAny>>,  // accumulates all occurrences
    // ...
}
```

## IgnoredAny and Coverage
Fields typed as `IgnoredAny` or `Vec<IgnoredAny>` indicate **unimplemented features**:
- They are parsed (counted in coverage metrics) but not used
- They do NOT implement `Serialize`, so structs with these fields cannot be serialized
- **Goal**: Eliminate all `IgnoredAny` usage by implementing the fields properly

### Migration Path
1. **Identify** fields using `IgnoredAny` or `Vec<IgnoredAny>`
2. **Research** the field's structure in EU4 data files
3. **Define** a proper type (struct, enum, or primitive)
4. **Replace** `IgnoredAny` with the new type
5. **Remove** `TolerantDeserialize` if no duplicate keys remain (use standard `Deserialize`)

## Limitations
- Only works with structs that have named fields
- All `Vec` fields must be wrapped in `Option` (i.e., `Option<Vec<T>>`, not `Vec<T>`)
- Cannot serialize structs that use this macro (no `Serialize` implementation generated)

## See Also
- [`eu4data_derive/src/lib.rs`](file:///c:/Users/atv/Documents/src/eu4rs/eu4data_derive/src/lib.rs) - Macro implementation
- [`eu4data/src/history.rs`](file:///c:/Users/atv/Documents/src/eu4rs/eu4data/src/history.rs) - Example usage in `ProvinceHistory`
- [`docs/coverage.md`](file:///c:/Users/atv/Documents/src/eu4rs/docs/coverage.md) - Coverage metrics and goals
