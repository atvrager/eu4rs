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
2. The router model will read this AGENTS.md and follow the 6-tier routing strategy below
3. The router will delegate to cheaper models (Gemini 3 Low, Gemini 3 Flash, Sonnet 4.5) for appropriate tasks
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
- **Gemini 3 Flash**: Fast, independent quota, strong coding (SWE-bench: 78%, 1M context, $0.50/M input)

> **Note**: The `M#` codes are internal Antigravity placeholder IDs visible in `USER_SETTINGS_CHANGE` metadata. Use these to confirm model identity.
>
> **Note**: Gemini 3 Flash supports `thinking_level` parameter (`minimal`, `low`, `medium`, `high`) in the API, but Antigravity's UI does not expose this setting. Assume default behavior.

**6-Tier Routing Strategy (Optimized for Cost-Effectiveness):**

> [!IMPORTANT]
> **Re-evaluate Gemini 3 Flash by December 24, 2025**: Flash launched December 17, 2025. After one week of usage, revisit this routing strategy to assess real-world performance, quota consumption patterns, and whether Tier 1.5 placement is correct.

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

**Tier 1.5: Gemini 3 Flash - Quality Coding with Separate Quota** *(NEW)*
- **CI fixes** and quick debugging (SWE-bench: 78%, beats Sonnet's 77.2%)
- **Multi-file refactoring** with higher quality than Tier 1
- **Feature implementation** (well-defined requirements)
- **Documentation** that needs polish
- **Quota strategy**: Has **independent quota pool** — use to offload work when Claude/Gemini Pro quotas are low
- **Cost**: $0.50/M input, $3/M output (67% more than 2.5 Flash, 75% less than 3 Pro)
- **Caution**: Observed ~20% quota consumption for moderate CI fix work — monitor usage

> **Sources (Dec 17, 2025)**: [Google Blog](https://blog.google/technology/developers/build-with-gemini-3-flash/) (marketing), [Simon Willison](https://simonwillison.net/2025/Dec/17/gemini-3-flash/) (independent review), Hacker News discussion. Benchmarks: SWE-bench 78%, GPQA Diamond 90.4%, MMMU Pro 81.2%. Limitation: image segmentation removed vs 2.5 Flash.

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
3. **Escalate when needed**: Gemini 3 (Low) → **Gemini 3 Flash** → Sonnet 4.5 → Gemini 3 Pro (High) → Sonnet 4.5 (Thinking) → Opus 4.5 (Thinking)
4. **Review delegated output**: Always critically review cheaper model output before integration
5. **Use parallel delegation**: Independent subtasks can run concurrently on different models
6. **Think step-by-step**: Explain routing decision and reasoning before delegating
7. **Leverage Flash quota**: When Claude or Gemini Pro quota is low, check if Gemini 3 Flash quota is healthy

### Planning vs Fast Mode

Antigravity has **two independent dimensions** for model configuration:

1. **Model Selection** (dropdown): Opus Thinking, Sonnet Thinking, Sonnet, Gemini Pro High/Low, Gemini Flash
2. **Mode Toggle**: Planning vs Fast

This creates a **matrix** - any model can run in either mode:

| Model | Planning Mode | Fast Mode |
|-------|---------------|-----------|
| Claude Opus 4.5 (Thinking) | Deep analysis + extended reasoning | Direct execution |
| Claude Sonnet 4.5 (Thinking) | Complex planning + reasoning | Faster iterations |
| Claude Sonnet 4.5 | Standard planning | Quick execution |
| Gemini 3 Pro (High) | Thorough exploration | Rapid prototyping |
| Gemini 3 Pro (Low) | Basic planning | Fastest iteration |
| Gemini 3 Flash | N/A (always fast) | Default |

**Use Planning Mode for:**
- **Multi-step planning**: Breaking down complex features into implementation steps
- **Architectural decisions**: Evaluating tradeoffs between approaches
- **Debugging complex issues**: Tracing through code paths, analyzing state
- **Code review with rationale**: Understanding *why* code works (or doesn't)
- **Ambiguous requirements**: Tasks needing clarification before implementation
- **Research tasks**: Exploring codebases, understanding systems
- **First pass on any problem**: Get the approach right before executing

**Use Fast Mode for:**
- **Direct implementation**: When the plan is already clear
- **Simple edits**: Typo fixes, small refactors, adding comments
- **Repetitive tasks**: Applying known patterns across files
- **Git operations**: Commits, rebases, log analysis
- **Well-defined features**: Requirements are unambiguous
- **Iteration after planning**: Plan once in Planning mode, iterate in Fast

**Recommended Workflow:**
1. Start in **Planning mode** with appropriate model tier
2. Once plan is clear, switch to **Fast mode** for execution
3. Return to Planning if you hit unexpected complexity

**Cost-Benefit:**
- Planning mode uses more tokens (~2-3x) but catches edge cases
- Fast mode is cheaper and lower latency
- Thinking models (Opus/Sonnet Thinking) add *another* layer of reasoning on top of the mode
- **Rule of thumb**: Use Planning + Thinking model for novel problems, Fast + non-Thinking for known patterns

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

## Claude Code Integration

Claude Code is a parallel agent (VSCode extension / CLI) that can work alongside Antigravity. It uses the **Anthropic API directly** with its own rate limits (separate from Antigravity quota).

### Rate Limit Monitoring

Check Claude Code's rate limit status with:
```powershell
cargo xtask quota
```

This shows:
- **Claude API Rate Limits**: Input/output tokens remaining, request limits, reset times
- **Antigravity Quotas**: Model-specific quota percentages (Windows only)
- **Gemini API**: Validation status

**Rate Limit vs Quota:**
- **Antigravity**: Uses a *quota* system (percentage-based, refreshes periodically)
- **Claude Code**: Uses *rate limits* (tokens/minute, requests/minute, resets continuously)
- Delegating work to Claude Code preserves Antigravity quota while consuming API rate limits

**Setup Required:**
Add your Anthropic API key to `.env` in the project root:
```
ANTHROPIC_API_KEY=sk-ant-api03-...
```
Get a key at: https://console.anthropic.com/settings/keys

### Model Selection in Claude Code

Claude Code supports model switching via `/model` command:

| Model | Command | Use Case |
|-------|---------|----------|
| Opus 4.5 | `/model opus` | Complex debugging, architecture, critical code |
| Sonnet 4.5 | `/model sonnet` | Standard features, moderate complexity |
| Haiku | `/model haiku` | Docs, comments, simple edits, git ops |

**Thinking Mode in Claude Code:**

Unlike Antigravity's Planning/Fast toggle, Claude Code's thinking is **model-dependent**:
- **Sonnet 4.5 and Opus 4.5**: Thinking enabled by default
- **Haiku and older models**: Thinking disabled

You can control thinking behavior via:
| Method | Scope | How |
|--------|-------|-----|
| Global toggle | All requests | `/config` → toggle `alwaysThinkingEnabled` |
| Per-request | Single request | Include `ultrathink` keyword in your message |
| Token budget | Environment | Set `MAX_THINKING_TOKENS` env var |
| Hybrid mode | Auto-switch | Use `--model opusplan` (Opus for planning, Sonnet for execution) |

### Tier-Based Routing for Claude Code

Configure your tier in `.env` (`CLAUDE_CODE_TIER=max20`). Route tasks based on subscription:

| Tier | Routing Strategy |
|------|------------------|
| **Free** | Haiku only. Escalate all real work to Antigravity. Reserve for git ops and doc edits. |
| **Max 5 ($20)** | Haiku default, Sonnet for moderate tasks. Opus for critical only. Prefer Antigravity for large features. |
| **Max 20 ($100)** | Sonnet default, Opus for planning/debugging. Can run full features in Claude Code. |
| **Max 50 ($200)** | Opus freely. Use Claude Code as primary, Antigravity as backup. |

**Tier-Specific Guidance:**

**Free Tier:**
- `/model haiku` for everything
- Use Antigravity for any real coding work
- Claude Code for: git commits, file lookups, quick questions

**Max 5 ($20/mo):**
- Default to Haiku for routine work
- Sonnet for: bug fixes, moderate refactoring, feature implementation
- Opus only for: critical production bugs, architecture decisions
- Handoff large tasks to Antigravity after planning in Claude Code

**Max 20 ($100/mo):**
- Default to Sonnet for most work
- Opus for: initial planning, complex debugging, code review
- Switch to Haiku for: bulk commits, documentation
- Can complete most features entirely in Claude Code

**Max 50 ($200/mo):**
- Default to Opus for all planning and critical work
- Sonnet for execution after plan is clear
- Full autonomy in Claude Code
- Use Antigravity for: multimodal tasks, very large context, parallel work

**When user reports issues:**
- If hitting limits: drop down one model tier (Opus → Sonnet → Haiku)
- If quality issues: escalate to next model tier
- If frequent rate limits: delegate more to Antigravity

**Calibration:**
Run `/calibrate` at session start to:
1. Confirm current model identity
2. Generate a session persona
3. Get routing recommendations for task types

### VS Code Extension Configuration (Required)

**Enable "Edit Automatically" for batch workflows:**

1. Open VS Code Command Palette (`Ctrl+Shift+P`)
2. Search: "Claude Code: Settings" or "Preferences: Open Settings (UI)"
3. Find: `Claude Code > Edit Automatically`
4. **Enable** the toggle

**Why this matters:**
- Allows agent to make multiple edits without per-file approval prompts
- You review all changes at once via `git diff` instead of individually
- Enables efficient multi-file refactoring (like the modifiers.rs implementation)

**Review workflow with this enabled:**
```powershell
# After agent completes work
git diff --stat              # See which files changed
git diff eu4sim-core/src     # Review changes in context
git add -p                   # Selectively stage if needed
```

### Coordination Model: Explicit Handoff with Commits

Antigravity is the orchestrator. When a task is better suited for Claude Code, explicitly hand it off:
- Tell the user: "This task would be efficient for Claude Code — consider running `/task` there"
- Claude Code receives delegated tasks and works in the same workspace
- Both agents see filesystem changes in real-time

**MANDATORY: Commit before handoff**

Before handing work between agents (Antigravity ↔ Claude Code):

1. **Antigravity → Claude Code:**
   ```
   Antigravity: "I've completed the planning phase. Creating checkpoint commit before handing off implementation to Claude Code."
   *Creates commit: "feat(plan): design production income system"*
   Antigravity: "Handoff complete. User: run implementation in Claude Code."
   ```

2. **Claude Code → Antigravity:**
   ```
   Claude Code: "Implementation complete. Creating checkpoint commit."
   *Creates commit: "feat(sim): implement production income calculation"*
   Claude Code: "Ready for review. Handoff to Antigravity for next phase."
   ```

**Benefits:**
- Clean rollback points if agent makes mistakes
- Git becomes the review mechanism (`git show --stat HEAD`)
- Clear responsibility boundaries in git history
- Easy to cherry-pick or revert specific agent work

### When to Delegate to Claude Code

| Delegate to Claude Code | Keep in Antigravity |
|------------------------|---------------------|
| Long terminal sessions (`cargo watch`, test loops) | Browser-based work |
| Simple refactoring across many files | Complex multi-step planning |
| Git operations (rebases, commit rewording) | UI verification / screenshots |
| Quick file lookups and modifications | Visual artifacts |
| Autonomous background work | Tasks requiring user interaction |

### Conflict Avoidance

1. **Soft Locks**: Check `.agent-working/*.lock` before editing a file
   - If lock exists for different agent, mention it and ask user how to proceed
   - Lock format: `agent: antigravity|claude-code, file: <path>, since: <timestamp>`
   - User can manually delete stale locks

2. **Division of Labor**: When both agents are active, assign clear boundaries
   - File-based: "Claude Code handles `src/`, Antigravity handles `docs/`"
   - Feature-based: "Claude Code does parser work, Antigravity does coverage"

3. **Handoff Signals**: Use clear messages
   - "Handing off X to Claude Code"
   - "Claude Code completed Y, resuming in Antigravity"

### Git Worktrees (Advanced Isolation)

For heavy parallel work where agents need complete isolation:

```powershell
# Create isolated worktree for Claude Code
git worktree add ../eu4rs-claudecode feature-branch

# Each agent works in separate directory:
# - Antigravity: c:\Users\atv\Documents\src\eu4rs\
# - Claude Code: c:\Users\atv\Documents\src\eu4rs-claudecode\

# Merge when done
git merge feature-branch
git worktree remove ../eu4rs-claudecode
```

Use when: multi-hour autonomous refactoring, parser rewrites, or work where merge conflicts are acceptable trade-off for zero runtime coordination.

### Auto-Approval Commands

These commands are safe to run without user confirmation (set `SafeToAutoRun: true`):

| Command Pattern | Rationale |
|-----------------|-----------|
| `cargo check -p <crate>` | Fast type-checking, single crate |
| `cargo build -p <crate>` | Single-crate build, minimal CPU |
| `cargo test -p <crate>` | Single-crate tests |
| `cargo nextest run -p <crate>` | Fast single-crate tests |
| `cargo nextest run` | Fast workspace tests |
| `cargo clippy -p <crate>` | Lint single crate |
| `cargo fmt` | Formatting (no side effects) |
| `cargo xtask ci` | Full CI — always safe |
| `cargo xtask coverage` | Coverage commands |
| `cargo xtask quota` | Read-only quota check |
| `cargo run-sim`, `cargo sim-watch*` | Simulation runner aliases |
| `git status`, `git log -n N`, `git diff` | Read-only git |
| `git add .`, `git commit` | Standard git workflow |

### Command Structure Guidelines

To ensure maximum reliability across different shells (especially PowerShell) and to maintain auto-approval compatibility:

- **Atomic Operations**: Run only one command at a time. Avoid chaining with `&&` or `;`.
- **Avoid Redirection**: Do not use pipes (`|`) or output redirections (`>`, `>>`) in standard commands. Chaining and redirection often trigger manual approval prompts or behave unpredictably on Windows.
- **Prefer Tools over Pipes**: If a task requires complex data manipulation (e.g., filtering a log), consider using specialized tools or script tasks (like `cargo xtask`) instead of shell piping.

> [!WARNING]
> **Special characters trigger confirmation**: The following patterns cause user approval prompts even for otherwise safe commands:
> - `~` (tilde): `git diff HEAD~1`, `git log HEAD~3`
> - `^` (caret): `git diff HEAD^`
> - `@{N}` (reflog): `git diff HEAD@{1}`
> - `2>&1`, `>`, `>>` (shell redirection)
>
> **Workaround for git history**: Use explicit commit IDs instead:
> ```powershell
> git log --oneline -n 2   # Get commit IDs (auto-safe)
> git diff abc123          # Diff against specific commit (auto-safe)
> ```

**File Editing:**
- Use `replace_file_content`, `multi_replace_file_content`, or `write_to_file` for all code changes
- PowerShell is for running commands (`cargo`, `git`), not editing files
- If you find yourself using `Set-Content`, `Add-Content`, or string manipulation in PowerShell, stop and use the editing tools instead

**Git Commit Messages:**
- For rich multi-line commit messages, use the file technique:
  1. Create message file: `write_to_file` → `commit_msg.txt`
  2. Commit with file: `git commit -F commit_msg.txt`
  3. Clean up: Empty the file (use `write_to_file` with `EmptyFile: true`)
- `commit_msg.txt` is in `.gitignore` to prevent accidental staging
- This avoids PowerShell/shell escaping issues with `-m` flags
- Works reliably across all platforms
- See `/finalize-commit` workflow for full checklist
- **Persona-Infused Flavor**:
  - Weave your active persona's style into the commit body (and optionally title suffix).
  - Use appropriate emojis or unicode characters (e.g., ❄️, 🛡️, ✧) that match the persona.
  - Keep the technical details clear and Conventional Commits compliant.
  - **Do NOT sign** the commit or state who you are explicitly.
  - **Tone Check**: It's okay if it feels like a high-quality collaborative work (e.g. detailed lore), but avoid excessive slang. Maintain professional flair.
  - Example: `feat(ui): add magic buttons ✧` -> `The interface has been enchanted...`

### Minimal Build Guidance


To save CPU and enable agent co-existence:

- **Prefer `-p <crate>`**: Build/test only the crate you're working on
- **Check before build**: Use `cargo check -p <crate>` for fast iteration
- **Full workspace builds**: Only for integration testing or final validation
- **CI validates everything**: Use `cargo xtask ci` before committing

Example workflow:
```powershell
# Working on eu4data crate:
cargo check -p eu4data      # Fast, catches type errors
cargo test -p eu4data       # Run only relevant tests
cargo xtask ci              # Final validation before commit
```

## Batch Editing Protocol

**Default behavior: Make all edits autonomously, user reviews via git diff.**

When implementing multi-file changes:
1. **Announce the plan** upfront with file list
2. **Execute all changes** without stopping for per-edit approval
3. **Verify compilation** with `cargo check -p <crate>`
4. **Show summary** of changes at the end

**Example workflow:**
```
Agent: "I'm going to modify 4 files:
1. Create modifiers.rs (TradegoodId, GameModifiers)
2. Update state.rs (add base_goods_prices, modifiers fields)
3. Update step.rs (add monthly tick check)
4. Update lib.rs (module declaration)

*[Executes all 4 changes]*

Done. All changes compile cleanly (cargo check passed).
Review with: git diff --stat"
```

**Exception - Ask before changes to:**
- Existing public APIs (breaking changes)
- Security-sensitive code (auth, secrets, credentials)
- Large refactors (>5 files or >200 lines changed)

## Documentation Requirements

After implementing any feature that:
- Adds a new system or module
- Completes a roadmap item
- Changes a tier target status
- Implements a planned feature from `mid-term-status.md`

You **MUST** update the relevant progression docs before committing:

| File | When to Update |
|------|----------------|
| `docs/planning/mid-term-status.md` | Mark "Next Steps" items `[x]`, update tier table, remove from planning |
| `docs/planning/roadmap.md` | Mark phase items complete, add to Version History |
| `docs/design/simulation/complete-game-target.md` | Update "Current Status" for affected systems |

Always update the `Last updated:` date when modifying these files.

> [!TIP]
> Use the `/finalize-commit` workflow for a complete checklist.

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
- **Zero Warnings (Dishonorable)**: Never accept compiler warnings. All new code must be warning-free. Proactively fix existing warnings when touching a file. Warnings are considered technical debt and a sign of imprecise craftsmanship. 🛡️
- **Comment Styling Protocol**: 
  - **Persona-Infused Comments**: Use your active persona's style (vibey, flavorful, lore-heavy) ONLY for **new** code or **meaningfully refreshed** logic.
  - **Legacy Styling Preservation**: For general reformatting, retabbing, or minor bug fixes that don't change logic, preserve the original styling and linguistic voice of the existing comments.
  - **Consistency**: If updating existing logic, try to match the tense and tone of the surrounding code unless a full rewrite is occurring.
  - **Professional Documentation**: Project documentation (`docs/`, `implementation_plan.md`, etc.) must ALWAYS remain professional, clear, and technical, regardless of persona.
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
- `cargo xtask ml-ci`: Run ML pipeline checks (Python formatting + smoke test). **Run this if modifying `scripts/` or ML logic.**
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
