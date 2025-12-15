---
description: Find a low-coverage data type and bring it to 100% parsed coverage
---

# Workflow: Improve Data Coverage

This workflow guides you through identifying a data category with low parsing coverage and implementing the missing fields to reach 100% "Parsed" status.

## 1. Identify a Candidate

Run the coverage tool to see the current status of all data categories.

```bash
cargo xtask coverage
```

Look for categories with low "Parsed" percentages (e.g., < 20%) but a manageable number of known fields.
*   **Ignore** "Provinces History" (or any target explicitly excluded by the user).
*   **Prefer** categories with < 100 fields for a focused task.
*   **Target**: A category where we can realistically define all fields.

**Decision Point**: Select one `DataCategory` (e.g., `Tradegoods`, `Religions`, `Cultures`).

## 2. Inspect Missing Fields

Check `docs/supported_fields.md` to see exactly which fields are missing (marked with ❌) for your chosen category.

```bash
# Bash
grep -A 100 "## [Your Category Name]" docs/supported_fields.md

# PowerShell
Select-String -Pattern "## [Your Category Name]" -Context 0,100 docs/supported_fields.md
```


## 3. Implement Fields

Locate the corresponding Rust struct in `eu4data/src/`.

```bash
# Bash
grep -r "struct [StructName]" eu4data/src/

# PowerShell
Get-ChildItem -Recurse eu4data/src/ | Select-String -Pattern "struct [StructName]"
```

### Case A: Struct Exists
Modify the struct to add the missing fields. Ensure it derives `SchemaType`.

### Case B: Struct Does Not Exist (0% Coverage)
If there is no Rust type for this category yet:

1.  **Create a new module**: Create `eu4data/src/[name].rs` (e.g., `diplomacy.rs`).
2.  **Define the struct**:
    ```rust
    use serde::{Deserialize, Serialize};
    // Use TolerantDeserialize to handle duplicate keys and skip unknown fields
    use eu4data_derive::{SchemaType, TolerantDeserialize}; 

    #[derive(Debug, Clone, TolerantDeserialize, SchemaType)]
    pub struct MyNewType {
        // Add fields here based on docs/supported_fields.md
        pub color: Option<Vec<u8>>,
    }
    ```
3.  **Register module**: Add `pub mod [name];` to `eu4data/src/lib.rs`.
4.  **Register in coverage**: Update `eu4data/src/coverage.rs` in `get_manual_annotations`:
    ```rust
    DataCategory::MyCategory => {
        // Auto-load from Struct
        for f in crate::[name]::MyNewType::fields() {
            map.insert(
                f.name,
                ManualAnnotation {
                    parsed: true,
                    visualized: f.visualized,
                    simulated: f.simulated,
                    notes: None,
                },
            );
        }
    }
    ```

### Implementation Guidelines
### Implementation Guidelines
*   **Robust Parsing**: Use `#[derive(TolerantDeserialize)]` instead of `Deserialize` for EU4 data. It handles duplicate keys (e.g., multiple `add_core`) and safely ignores unknown fields.
*   **Strong Typing**: Always prefer specific types (`f32`, `String`, `bool`) over generic containers.
*   **Lists**: Use `Vec<String>` or `Vec<f32>` for lists. For `TolerantDeserialize`, wrap in Option: `Option<Vec<T>>`.
*   **Dynamic Blocks**: Use `HashMap<String, ...>` for blocks where keys are dynamic (like modifiers).
*   **Catch-Alls**: If you must use a catch-all (e.g. `other`), use `IgnoredAny` but:
    *   Mark it `#[serde(skip_serializing)]` (IgnoredAny doesn't serialize).
    *   Document *why* it's inclusive (e.g. "Future proofing", "Too many random modifiers").
    *   Ideally, list the known fields you are intentionally ignoring in a comment.

**Example:**
```rust
#[derive(Debug, TolerantDeserialize, SchemaType)]
pub struct MyData {
    // Existing fields...
    pub color: Option<Vec<u8>>,

    // New fields
    pub cost: Option<f32>,
    pub modifier: Option<HashMap<String, f32>>,
}
```

## 4. Verify Improvements

Re-run the coverage update command. This will re-scan the code and update the documentation.

```bash
# Update schema and regenerate documentation
cargo xtask coverage --update --doc-gen
```

Check the output. Did the coverage for your category jump to 100%?
*   If **Yes**: Great!
*   If **No**: Check `docs/supported_fields.md` again to see what is still missing. You might have missed a field or named it slightly differently.

## 5. Final Verification

Ensure that the changes didn't break compilation or tests.

```bash
cargo xtask ci
```

## 6. Commit

Create a commit summarizing the coverage improvement.

```text
feat: achieve 100% parsing coverage for [Category Name]

- Added missing fields to [Struct Name] struct
- Updated [filename].rs
- Coverage improved from [X]% to 100% for [Category Name]
```
