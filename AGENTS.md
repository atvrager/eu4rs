# Agent Rules

> **Meta-Rule**: This configuration is **agent-agnostic**. Write rules based on project conventions and principles, not specific AI model capabilities or tool implementations. Focus on *what* to do, not *how* a particular model does it.

## Reference Documentation

Detailed guides live in [`docs/agents/`](docs/agents/README.md). Consult when needed:

| Topic | Doc | When to Read |
|-------|-----|--------------|
| Model routing | [routing.md](docs/agents/routing.md) | Choosing between Claude/Gemini, tier decisions |
| Claude Code | [claude-code.md](docs/agents/claude-code.md) | CLI/extension usage, handoffs, auto-approval |
| Calibration | [calibration.md](docs/agents/calibration.md) | Testing routing, security verification |
| Build tools | [build-tools.md](docs/agents/build-tools.md) | sccache, nextest, tokei setup |
| Persona | [persona.md](docs/agents/persona.md) | Session flavor, `/persona` skill |





## Batch Editing Protocol

**Default behavior: Make all edits autonomously, user reviews via git diff.**

When implementing multi-file changes:
1. **Announce the plan** upfront with file list
2. **Execute all changes** without stopping for per-edit approval
3. **Verify compilation** with `cargo check -p <crate>`
4. **Show summary** of changes at the end

**Exception - Ask before changes to:**
- Existing public APIs (breaking changes)
- Security-sensitive code (auth, secrets, credentials)
- Large refactors (>5 files or >200 lines changed)

## Documentation Requirements

After implementing features that add systems or complete roadmap items:

| File | When to Update |
|------|----------------|
| `docs/planning/roadmap.md` | Mark phase items complete |

Always update the `Last updated:` date. Use `/finalize-commit` for full checklist.

## Platform
- **Likely Windows**: Game modding project. Check your shell early in a session.
- **Shell detection**: May get PowerShell (default) or bash (advanced user).
- **Windows symlinks**: Require Developer Mode. See Windows Setup below.
- **PowerShell scripts**: Use `powershell -ExecutionPolicy Bypass -File script.ps1`.

## Working Directory
- **NEVER use `cd` commands**: Set the working directory per-command instead.
- **Track workspace root**: Default is `c:\Users\atv\Documents\src\eu4rs`.
- **Stay in context**: Specify appropriate working directory for each command.

## Windows Setup
To enable proper symlink support on Windows:
1. Enable **Developer Mode**: `Start-Process 'ms-settings:developers'` → toggle on
2. Configure git: `git config core.symlinks true`
3. For Git Bash: `MSYS=winsymlinks:nativestrict ln -s target linkname`
4. Re-checkout broken symlinks: `rm file.md && git checkout -- file.md`

## Line Endings
- **Enforce LF**: Unix-style line endings (`\n`), even on Windows.
- **Git Config**: Set `core.autocrlf` to `input` or `false` locally.
- **Normalization**: If you see "whole file diffs", run `git add --renormalize .`

## Documentation

- **Rust Code (Rustdoc)**:
  - Focus on the "Why" — explain design constraints and sharp edges
  - Skip the obvious — don't document trivial getters
  - Use standard Rustdoc formats

- **Project Documentation (`docs/`)**:
  - Document non-trivial systems, especially game domain specifics
  - Assume standard CS knowledge
  - Format: Markdown files

## Logging
- **ALWAYS** use `log!` macros (`info!`, `warn!`, `error!`, `debug!`) instead of `println!`
- **Exceptions**: Panics, early startup errors, CLI output intended for piping

## Code Quality
- **Zero Warnings**: Never accept compiler warnings. Fix existing warnings when touching a file.
- **Persona-Infused Comments**: Use persona style ONLY for new code or meaningfully refreshed logic.
- **Legacy Preservation**: Preserve original styling for minor fixes.
- **Clean up comments**: Remove "thinking comments" from final code.
- **Preserve comments**: Move comments with refactored code. Add comments for public APIs.
- **Remove allows**: Check for unnecessary `#[allow(...)]` attributes when refactoring.
- **Clippy Fixes**: Use `cargo clippy --fix` for simple lint resolution.

## No Magic Numbers or Strings

**CRITICAL**: Never hardcode values that come from data files.

- **Dimensions**: Texture sizes, sprite dimensions, UI element sizes → read from loaded assets
- **Paths**: File paths, sprite names → use constants or read from config/data files
- **Game values**: Stats, multipliers, thresholds → parse from game data files
- **Positions**: UI element positions, offsets → read from `.gui` files

**Why**: EU4 has extensive mod support. Hardcoded values break when:
- Game patches change values
- Mods override assets with different dimensions
- DLC adds new content with different specifications

**Pattern**: Load data → store dimensions/values → use stored values at render/compute time.

## Code Coverage
- **Goal**: >75% in all categories (lines, functions, branches) is a MUST.
- **No Regressions**: New commits cannot regress below 75% without explicit acknowledgement.
- **Tools**:
  - `cargo llvm-cov --summary-only`: Quick stats
  - `cargo llvm-cov --open`: Detailed HTML report
  - `cargo llvm-cov --lcov --output-path lcov.info`: LCOV for CI/IDE

## Common Commands
- `cargo xtask ci`: Run CI tests. **Must pass before committing.** Run proactively.
- `cargo xtask ml-ci`: ML pipeline checks. Run if modifying `scripts/` or ML logic.
- `cargo xtask snapshot`: Regenerate golden snapshots. Ask user for validation.
- `cargo xtask coverage --update`: Refresh schema from game files.
- `cargo xtask coverage --generate`: Generate Rust types from schema.

## Testing GUI Applications
- **Visual verification required**: Ask user to run GUI apps manually.
- **Batch questions**: Up to 3 questions when requesting testing feedback.

## Snapshot Testing
- **When to use**: Visual output or complex deterministic data structures.
- **How to use**: `crate::testing::assert_snapshot(&image, "snapshot_name")`
- **Location**: [`eu4viz/tests/goldens/`](eu4viz/tests/goldens/README.md)
- **Updating**: Delete `.png` and run test, or `cargo xtask snapshot` for batch.

## Test File Organization

**Pattern**: Separate test code into `foo_tests.rs` files alongside `foo.rs`.

```rust
// In foo.rs - at the end of the file
#[cfg(test)]
#[path = "foo_tests.rs"]
mod tests;

// In foo_tests.rs - the test file
use super::*;

#[test]
fn test_something() {
    // ...
}
```

**Benefits**:
- Keeps source files smaller (better for token limits in AI tools)
- Tests remain unit tests with access to private members via `use super::*`
- Clear separation between production and test code

**When to extract**:
- Test module exceeds ~100 lines
- Source file is already large (>500 lines)
- New modules should start with separate test files

**Naming**: `foo.rs` → `foo_tests.rs` (snake_case, `_tests` suffix)

## Git Workflow
- **Commit reordering**: Consider reordering via interactive rebase rather than fixup/squash.
- **Non-interactive rebase**: Use `GIT_SEQUENCE_EDITOR` and `GIT_EDITOR` with scripts:
  ```bash
  GIT_SEQUENCE_EDITOR="sed -i 's/pick abc123/reword abc123/'" \
  GIT_EDITOR="echo 'New message' >" \
  git rebase -i HEAD~3
  ```

## Commit Messages
- **Focus on Deltas**: Write based on actual diffs, not conversation history.
- **Format**: Use bulleted lists for details.
- **Content**: Professional and technical. Don't mention "I ran CI" or "User requested this".
- **PowerShell Warning**: Avoid backticks in `git commit -m` strings. Use `git commit -F file.txt` instead.
- **Watch out for**: `` `t `` (tab), `` `n `` (newline) which corrupt messages.

## Communication Standards
- **Backticks for Code**: Always wrap code expressions, function names, and file paths in backticks.
  - **Good**: "The `process_input` function returns `true`."
  - **Bad**: "The process_input function returns true."

## Type Inference Guidelines

When generating types from EU4 data:

1. **Distinguish integers from floats**: `"100"` → Integer, `"0.1"` → Float
2. **Prefer specific types**: `"yes"/"no"` → `bool`, colors → `[i32; 3]`
3. **Use stable, wide types**: `i32` over `i16`, `Option<T>` for all fields
4. **Document conversion intent**: Comment that `f32` may become fixed-point in sim layer
5. **Flag ambiguous types**: Use `InferredType::Unknown` for human review

See [`docs/type_system.md`](docs/type_system.md) for full architecture.

## EU4 Domain Knowledge

### Country Tags

EU4 uses 3-letter tags that don't match modern conventions:

| Tag | Country | Why Non-Obvious |
|-----|---------|-----------------|
| **TUR** | Ottomans | Not OTT |
| **HAB** | Austria | Habsburg dynasty, not AUS |
| **BRA** | Brandenburg | Not Brazil |
| **MOS** | Muscovy | RUS is formed Russia |
| **CAS** | Castile | SPA is formed Spain |
| **ENG** | England | GBR is formed Great Britain |
| **BUR** | Burgundy | Not BRG |
| **HOL** | Holland | NED is formed Netherlands |
| **MCH** | Manchu | QNG is formed Qing |
| **TIM** | Timurids | MUG is formed Mughals |

**Pattern:** Tags use dynasty names (HAB = Habsburg) or historical names (TUR = Türk).
