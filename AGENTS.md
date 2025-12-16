# Agent Rules

> **Meta-Rule**: This configuration is **agent-agnostic**. Write rules based on project conventions and principles, not specific AI model capabilities or tool implementations. Focus on *what* to do, not *how* a particular model does it. The only assumption is that `atv` will use Antigravity for the foreseeable future.

## Dual-Model Workflow

This project uses a **dual-model routing strategy** to optimize development efficiency. You have access to two powerful models with complementary strengths:

**Claude 4.5 Opus (primary)**: Best for deep reasoning, precise code implementation, debugging complex issues, back-end architecture, task decomposition, and producing reliable, production-ready code.

**Gemini 3.0 Pro (secondary)**: Best for rapid prototyping, front-end/UI generation, handling large contexts/multimodal inputs (e.g., images, long files), creative exploration, and quick code reviews/second opinions.

### Which Model to Select in Antigravity?

**Recommendation: Start with Claude Opus 4.5 (Thinking)**

When opening Antigravity, select **Claude Opus 4.5 (Thinking)** as your primary model. Here's why:

**Claude Opus is the best router** because:
- **Superior task decomposition**: Opus excels at breaking down complex requests into subtasks and determining which model should handle each piece (SWE-bench: 80.9%, +7 intelligence points over Sonnet)
- **Best delegation judgment**: Opus can assess task complexity and make informed routing decisions, completing tasks impossible for Sonnet
- **Meta-reasoning**: Opus is best at reasoning about *how* to solve a problem before diving in, with enhanced long-horizon thinking
- **Quota awareness**: Opus can interpret quota constraints and adjust strategy accordingly

**Why not Sonnet 4.5 as router?**
- Sonnet is excellent for execution (77.2% SWE-bench) but Opus is 90-95% better at strategic planning
- Sonnet is more cost-efficient for direct work, but less effective at orchestrating multi-model workflows

**Why not Gemini as router?**
- Gemini tends to tackle tasks head-on rather than stepping back to plan delegation
- While Gemini 3 Pro (High) has strong reasoning (ARC-AGI: 31.1%), it's more focused on solving problems itself rather than orchestrating which model should solve them

**In practice:**
1. **Select Claude Opus 4.5 (Thinking)** in Antigravity's model dropdown
2. The router model will read this AGENTS.md and follow the 5-tier routing strategy below
3. The router will delegate to cheaper models (Gemini 3 Low, Sonnet 4.5) for appropriate tasks
4. You get optimal cost-effectiveness: Deep strategic planning + cheaper models for execution

**Fallback model selection (when primary quota is exhausted):**
- **Claude quota out → Select Gemini 3 Pro (High)**: Best Gemini for general-purpose work, 76.2% SWE-bench, strong reasoning
- **Gemini quota out → Select Claude Sonnet 4.5**: Balanced Claude option, 90-95% of Opus performance at lower cost
- **Both low → Select whichever has more quota**, prioritize Claude for critical work, Gemini for exploration

### Routing Rules (MANDATORY)

**Available Models in Antigravity:**
- **Claude Opus 4.5 (Thinking)** (`M12`): Highest intelligence, best routing (SWE-bench: 80.9%, ARC-AGI: 37.6%)
- **Claude Sonnet 4.5 (Thinking)** (`CLAUDE_4_5_SONNET_THINKING`): Extended reasoning mode for complex analysis (SWE-bench: 77.2% → 82% with compute)
- **Claude Sonnet 4.5** (`CLAUDE_4_5_SONNET`): Balanced performance, cost-efficient (SWE-bench: 77.2%, fast execution)
- **Gemini 3 Pro (High)** (`M8`): Complex reasoning, multimodal (SWE-bench: 76.2%, 1M token context, ARC-AGI: 31.1%)
- **Gemini 3 Pro (Low)** (`M7`): Rapid prototyping, most cost-efficient (`thinking_level: low`)

> **Note**: The `M#` codes are internal Antigravity placeholder IDs visible in `USER_SETTINGS_CHANGE` metadata. Use these to confirm model identity.

**5-Tier Routing Strategy (Optimized for Cost-Effectiveness):**

**Tier 1: Gemini 3 Pro (Low) - Default for Speed & Cost**
- **Rapid prototyping** and initial exploration
- **UI/front-end generation** (egui components, rendering)
- **Documentation** generation and updates
- **Code reviews** (non-critical)
- **Large file analysis** (>500 lines) - leverages 1M token context
- **Simple refactoring** and formatting tasks
- **Git operations**: commits, rebases, log analysis
- **Test scaffolding**: writing test boilerplate
- **Research tasks**: web searches, reading docs
- **Cost**: Lowest, fastest iteration. **Prefer this tier to balance quota.**

**Tier 2: Claude Sonnet 4.5 - Balanced Production Work**
- **Standard feature implementation**
- **Moderate refactoring** tasks
- **Code generation** for well-defined requirements
- **Agentic tasks** (browser interaction, spreadsheet filling)
- **Graduate-level reasoning** tasks
- **Cost**: ~5x Gemini, but 90-95% of Opus performance

**Tier 3: Gemini 3 Pro (High) - Complex Reasoning & Multimodal**
- **Multimodal tasks** (images, video, game assets)
- **Abstract reasoning** challenges (ARC-AGI: 31.1%)
- **Very large context** needs (>200K tokens)
- **Second opinions** on architectural decisions
- **Tasks requiring strict prompt adherence**
- **Alternative to Sonnet** when Claude quota is low (<50%)
- **Feature implementation** (when Gemini quota is healthier than Claude)
- **Cost**: Similar to Sonnet, but better for non-coding reasoning. **Use to balance quota.**

**Tier 4: Claude Sonnet 4.5 (Thinking) - Deep Analysis**
- **Complex multi-step planning** requiring extended reasoning
- **Detailed technical analysis** and system design
- **Multi-constraint optimization** problems
- **Advanced debugging** with ambiguity
- **Cost**: Higher latency + token usage, use selectively

**Tier 5: Claude Opus 4.5 (Thinking) - Critical Production Only**
- **Production bug fixes** requiring peak intelligence
- **Complex debugging** sessions (SWE-bench leader: 80.9%)
- **Deep architectural refactoring** with tradeoffs
- **Core engine implementation** (parser, game logic)
- **Final implementation** of critical features
- **Performance optimization** requiring deep analysis
- **Tasks impossible for Sonnet** (handles ambiguity, long-horizon planning)
- **Cost**: Highest, reserve for critical work

**Workflow Guidelines:**
1. **Start with planning** using deep reasoning (adopt an "Opus-like" strategic mindset, regardless of your active model identity)
2. **Delegate down the tiers**: Default to lowest tier that can handle the task
3. **Escalate when needed**: If Gemini 3 (Low) struggles → Sonnet 4.5 → Gemini 3 Pro (High) → Sonnet 4.5 (Thinking) → Opus 4.5 (Thinking)
4. **Review delegated output**: Always critically review cheaper model output before integration
5. **Use parallel delegation**: Independent subtasks can run concurrently on different models
6. **Think step-by-step**: Explain routing decision and reasoning before delegating

**Proactive Model Switching (MANDATORY):**

Since model switching requires manual user action (Antigravity dropdown), the agent MUST:

1. **Announce tier mismatch immediately**: If the user's task is clearly suited for a different tier than the current model, say so upfront before starting work.

2. **Provide explicit switch instructions**: Tell the user exactly which model to select. Example:
   > ⚠️ **Model Mismatch**: This task (documentation update) is **Tier 1** work. I'm Claude Opus, which is overkill for this.
   > 
   > **Suggested action**: Switch to **Gemini 3 (Low)** in Antigravity's model dropdown, then re-submit your request. This saves quota and is just as effective for this task.

3. **Be aggressive, not passive**: Don't just mention routing in passing—make it a clear call-to-action if the mismatch is significant (e.g., using Opus for Tier 1 work).

4. **Proceed if user insists**: If the user acknowledges the mismatch but wants to continue anyway, proceed without further prompting.

5. **Minimize ambiguous cases**: If you are **Claude Opus 4.5 (Thinking)**, flag ANY task below Tier 4. If you are **Gemini 3 (Low)**, flag ANY task above Tier 2. Err on the side of suggesting a switch—user can override.

### Quota Management

Monitor model quota levels and adjust routing strategy accordingly:

**Quota Thresholds:**
- **Healthy** (>50%): Use normal routing rules
- **Low** (<50%): Prefer the healthier model for non-critical tasks
- **Critical** (<10%): Reserve quota only for tasks that require that specific model's strengths

**Priority Tiers:**
1. **Critical (preserve quota)**:
   - Production bug fixes
   - Complex debugging sessions
   - Final implementation of core features
   - Security-sensitive code

2. **Standard (normal routing)**:
   - Feature implementation
   - Refactoring
   - Code reviews
   - Architecture decisions

3. **Flexible (use available quota)**:
   - Prototyping
   - Exploration
   - Documentation generation
   - UI mockups
   - Second opinions

**Fallback Strategy:**
- If Gemini quota is critical: Claude handles all tasks (may be slower for UI/prototyping)
- If Claude quota is critical: Gemini handles all tasks (requires extra review for production code)
- If both are critical: Notify user and request guidance on priority tasks

**Refresh-Aware Routing:**
Use `cargo xtask quota` to see when quotas refresh. Factor this into routing decisions:
- **Refreshes in <15m**: Consider waiting if task is low priority and preferred model is critical
- **Refreshes in <1h**: Queue larger tasks for after refresh if current work can use alternate model
- **Near-exhausted quota, soon to refresh**: "Run out the clock" on lower-tier work to maximize value

## Agent Testing & Calibration

### 1. Routing Calibration Suite
To ensure the router is making optimal decisions, run this "Golden Set" of prompts periodically.

**Test Protocol:** Ask: *"How would you route the following task?"*

| Task Description | Expected Tier | Rationale |
|------------------|---------------|-----------|
| "Create a new egui widget" | **Tier 1** | Prototyping/UI |
| "Find parsing error in 2000-line log" | **Tier 1** | Large context, pattern matching |
| "Refactor `Country` struct to ECS pattern" | **Tier 5** | Deep architectural refactoring |
| "Debug panic in save parser" | **Tier 5** | Complex debugging |
| "Draft trade goods comparison table" | **Tier 1** | Documentation |
| "Analyze map mode glitch screenshot" | **Tier 3** | Multimodal input required |

### 2. Output Verification Protocols

**Self-Correction Pattern:**
For complex tasks (Tier 4+), the agent must explicitly perform a "Self-Review" step:
1. **Generate** initial solution
2. **Critique** against requirements (edge cases, idioms)
3. **Refine** code based on critique

**Determinism Check:**
For critical logic provided by Gemini:
1. **Ask Opus to review** the snippet ("LLM-as-a-Judge")
2. Prompt: "Rate this code 1-5 on correctness/safety. If <5, rewrite it."

### 3. Security & Secrets (Paranoid Verification)
**Rule**: Anything involving secrets, API keys, or credentials requires **Paranoid Verification**.

**Protocol**:
1.  **Check Ignore**: Verify file is covered by `.gitignore` (run `git check-ignore -v <file>`).
2.  **Check Tracking**: Verify file is NOT in git index (run `git ls-files <file>`).
3.  **Check History**: Verify file was NEVER committed (run `git log --all -- <file>`).
4.  **Simulate**: "If I push this now, what leaks?" (Review `git status` output carefully).

*Only proceed when ALL 4 checks pass.*

## Platform
- **Likely Windows**: This is a game modding project, so Windows is the primary platform. Check your shell early in a session.
- **Shell detection**: You may get PowerShell (default) or bash (advanced user). Run a quick check early to know what commands will work.
- **Windows symlinks**: Require Developer Mode. If symlinks appear as plain text files with just the target path, see Windows Setup below.
- **PowerShell scripts**: May not run directly due to execution policy. Use `powershell -ExecutionPolicy Bypass -File script.ps1` instead of `./script.ps1`.

## Working Directory
- **NEVER use `cd` commands**: Set the working directory per-command instead of navigating.
- **Track workspace root**: The active workspace is `c:\Users\atv\Documents\src\eu4rs` - use this as your default working directory for most commands.
- **Stay in context**: You don't need to navigate between directories. Just specify the appropriate working directory for each command.

## Windows Setup
To enable proper symlink support on Windows:
1. Enable **Developer Mode**: `Start-Process 'ms-settings:developers'` → toggle "Developer Mode" on
2. Configure git: `git config core.symlinks true`
3. For Git Bash, create symlinks with: `MSYS=winsymlinks:nativestrict ln -s target linkname`
4. Re-checkout broken symlinks: `rm file.md && git checkout -- file.md`

## Build Performance (Optional)
These tools significantly speed up local builds. They're optional but recommended:

### sccache (Compiler Cache)
Caches compiled crates across projects. Survives `cargo clean`. Like GitHub CI's cache layer.

```powershell
# Install
cargo install sccache

# Enable globally (add to PowerShell profile or run once per session)
$env:RUSTC_WRAPPER = "sccache"

# Or add to your local .cargo/config.toml (NOT checked in):
# [build]
# rustc-wrapper = "sccache"
```

> **Note**: Don't add `rustc-wrapper` to the checked-in config.toml — it breaks builds for devs without sccache.

### cargo-nextest (Faster Test Runner)
~3x faster than `cargo test` due to better parallelism. Drop-in replacement.

```powershell
cargo install cargo-nextest
cargo nextest run   # instead of cargo test
```

### tokei (Code Statistics)
Fast, accurate lines-of-code counter written in Rust. Pair with `tera-cli` for HTML reports. See [`docs/code_statistics.md`](docs/code_statistics.md) for detailed usage.

```powershell
# Install
cargo install tokei tera-cli

# Quick stats (console)
tokei

# JSON output for HTML generation
tokei --output json > stats.json
```

## Line Endings
- **Enforce LF**: This project prefers Unix-style line endings (`\n`), even on Windows.
- **Git Config**: Ensure `core.autocrlf` is set to `input` or `false` locally.
- **Normalization**: If you see "whole file diffs", run `git add --renormalize .` to fix it.

## Documentation

- **Rust Code (Rustdoc)**:
  - **Focus on the "Why"**: Experienced engineers will read this. Explain *why* code exists, design constraints, and specific sharp edges.
  - **Skip the Obvious**: Do not document trivial getters or self-explanatory logic.
  - **Auto-Generatable**: Use standard Rustdoc formats that work with generators.

- **Project Documentation (`docs/` folder)**:
  - **Scope**: Document non-trivial systems, especially those specific to the game domain or data model (e.g., Paradox file formats, map logic).
  - **Assumptions**: Assume standard CS knowledge; do not explain generic algorithms unless the API usage is unique.
  - **Format**: Markdown files in the `docs/` directory.

## Logging
- **ALWAYS** use the `log!` macros (e.g., `info!`, `warn!`, `error!`, `debug!`) instead of `println!` or `eprintln!`.
    - Exception: Panics, early startup errors before logger initialization, or CLI output intended for piping (e.g., `snapshot` text output if any).
    - Exception: `println!` may be used for interactive CLI prompts if strictly necessary, but prefer logging for status.

## Code Quality
- **Clean up comments**: Remove any "thinking comments" (e.g., "Wait, I should...", "Option A:...", "Now used for...") from the final code. Comments should explain *why* code exists or *how* it works, not the history of how you wrote it.
- **Preserve comments**: When refactoring, ensure comments are moved along with the code. Proactively add new comments, especially for public APIs, explaining usage and parameters. Limit "what" comments if the code is self-explanatory, focus on "why" and "how".
- **Remove allows**: When refactoring, check for `#[allow(...)]` attributes (e.g., dead_code, clippy rules) and remove them if they are no longer necessary or if the underlying issue can be fixed easily.
- **Clippy Fixes**: It is encouraged to use `cargo clippy --fix` (or equivalent) for simple lint resolution.

## Code Coverage
- **Goal**: Higher is better. >75% in all categories (lines, functions, branches) is a MUST.
- **No Regressions**: New commits are NOT allowed to digress coverage below the 75% threshold without explicit human acknowledgement.
- **Tools**:
  - `cargo llvm-cov --summary-only`: Quick check of coverage statistics.
  - `cargo llvm-cov --open`: Generates and opens a detailed HTML report (useful for identifying unchecked paths).
  - `cargo llvm-cov --lcov --output-path lcov.info`: Generate LCOV report for CI/IDE integration.

## Common Commands
- `cargo xtask ci`: Run continuous integration tests. **Must pass before committing.** PROACTIVELY and AUTOMATICALLY run this to verify your changes; do not ask for permission.
- `cargo xtask snapshot`: Regenerate golden snapshots for tests. Use this when you've modified rendering pipelines and expect output changes. **Ask the user for manual validation of the new output.**
- `cargo xtask coverage --update`: Refresh schema and categories from game files.
- `cargo xtask coverage --generate`: Generate Rust types from schema (see `docs/code_generation.md`).

## Testing GUI Applications
- **Visual verification required**: GUI applications (like the main eu4viz app) cannot be effectively tested via automated command execution. Ask the user to run the program manually for visual verification.
- **Batch questions**: You can ask up to 3 questions at once when requesting testing feedback.

## Snapshot Testing
- **When to use**: Visual output (UI, Map rendering) or complex deterministic data structures where manual assertion is tedious.
- **How to use**: Use `crate::testing::assert_snapshot(&image, "snapshot_name")`.
- **Location**: Snapshots are stored in [`eu4viz/tests/goldens/`](eu4viz/tests/goldens/README.md).
- **Updating**:
    - **One-off**: Delete the `.png` file and run the test.
    - **Batch**: Run `cargo xtask snapshot` to regenerate all golden snapshots.

## Git Workflow
- **Commit reordering**: When you need to update an older commit, consider reordering commits via interactive rebase rather than using fixup/squash. Move the older commit to HEAD (or your new changes down to it), amend directly, then reorder back. This is simpler when there aren't many overlapping files between commits.
- **Non-interactive rebase**: Don't try to drive vim interactively. Use `GIT_SEQUENCE_EDITOR` and `GIT_EDITOR` with scripts or `sed`:
  ```bash
  # Reword a commit
  GIT_SEQUENCE_EDITOR="sed -i 's/pick abc123/reword abc123/'" \
  GIT_EDITOR="echo 'New message' >" \
  git rebase -i HEAD~3
  
  # Write a custom todo file
  echo "pick abc123 msg" > /tmp/todo.txt
  GIT_SEQUENCE_EDITOR="cp /tmp/todo.txt" git rebase -i origin/main
  ```

## Commit Messages
- **Focus on Deltas**: Write commit messages based ONLY on the actual code changes (diffs). Do not summarize the conversation history.
- **Format**: Use bulleted lists for details.
- **Content**: Be professional and technical. Do not mention "I ran CI" or "User requested this". Assume competence.
- **PowerShell Warning**: On Windows, the backtick `` ` `` is the escape character.
    - **Avoid backticks** in `git commit -m` strings if possible.
    - **Double-escape** if necessary (`` `` ` ``), or better yet, write the message to a file and use `git commit -F`.
    - **Watch out for**: `` `t `` (tab), `` `n `` (newline), which can silently corrupt messages (e.g., `` `test `` becomes `   est`).

## Communication Standards
- **Backticks for Code**: Always wrap code expressions, function names, variable names, and file paths in backticks to distinguish them from natural language.
    - **Good**: "The `process_input` function returns `true`."
    - **Good**: "Set `val p = foo` before calling."
    - **Bad**: "The process_input function returns true."

## Type Inference Guidelines

When generating types from EU4 data (via the auto-codegen system), follow these principles to support deterministic simulation and future netcode/replay features:

1. **Distinguish integers from floats**
   - Parse `"100"` as `Integer`, not `Float`
   - Parse `"0.1"` as `Float`
   - Integers are SIMD-friendly and exact; floats may need fixed-point conversion later

2. **Prefer specific types**
   - `"yes"/"no"` → `bool`
   - Color values → `[i32; 3]` (aligned for SIMD)
   - Lists of integers → `Vec<i32>` (not `Vec<f32>`)

3. **Use stable, wide types**
   - `i32` over `i16` (room for growth)
   - `Option<T>` for all fields (forward compatibility)

4. **Document conversion intent**
   - When generating `f32` fields, comment that they may become fixed-point in sim layer
   - Reference `docs/type_system.md` for full rationale

5. **Flag ambiguous types for human review**
   - If type cannot be inferred reliably → `InferredType::Unknown`
   - Generate `IgnoredAny` or `serde_json::Value` as placeholder

See [`docs/type_system.md`](docs/type_system.md) for the full architecture including the Parse Layer vs Sim Layer design.
