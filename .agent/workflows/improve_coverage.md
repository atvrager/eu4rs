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

### 1. Generate Types
Run the generator to create Rust types from the schema:

```bash
cargo xtask coverage --generate
```

This creates files in `eu4data/src/generated/types/`.

### 2. Expose the Type
Create a public API shim for your new type.

1.  **Create file**: `eu4data/src/types/[name].rs` (e.g. `diplomacy.rs`).
2.  **Content**:
    *   **Simple Case**: Re-export generated type.
        ```rust
        pub use crate::generated::types::diplomacy::*;
        ```
    *   **Complex Case**: Copy code from `generated/types/diplomacy.rs` and modify it manually.
3.  **Register module**: Add `pub mod [name];` to `eu4data/src/types.rs`.

### 3. Register for Coverage
Update `eu4data/src/coverage.rs` to track your new type.

1.  Find `get_manual_annotations`.
2.  Add a generic walker for your category:
    ```rust
    DataCategory::MyCategory => {
        // Auto-load from Struct
        for f in crate::types::MyType::fields() {
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
