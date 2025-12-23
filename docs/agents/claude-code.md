# Claude Code Integration

> **Reference doc** — See [AGENTS.md](../../AGENTS.md) for core rules.

Claude Code is a parallel agent (VSCode extension / CLI) that can work alongside Antigravity. It uses the **Anthropic API directly** with its own rate limits (separate from Antigravity quota).

## Rate Limit Monitoring

Check Claude Code's rate limit status with:
```bash
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

## Model Selection in Claude Code

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

## Tier-Based Routing for Claude Code

Configure your tier in `.env` (`CLAUDE_CODE_TIER=max20`). Route tasks based on subscription:

| Tier | Routing Strategy |
|------|------------------|
| **Free** | Haiku only. Escalate all real work to Antigravity. Reserve for git ops and doc edits. |
| **Max 5 ($20)** | Haiku default, Sonnet for moderate tasks. Opus for critical only. Prefer Antigravity for large features. |
| **Max 20 ($100)** | Sonnet default, Opus for planning/debugging. Can run full features in Claude Code. |
| **Max 50 ($200)** | Opus freely. Use Claude Code as primary, Antigravity as backup. |

### Tier-Specific Guidance

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

## VS Code Extension Configuration

**Enable "Edit Automatically" for batch workflows:**

1. Open VS Code Command Palette (`Ctrl+Shift+P`)
2. Search: "Claude Code: Settings" or "Preferences: Open Settings (UI)"
3. Find: `Claude Code > Edit Automatically`
4. **Enable** the toggle

**Why this matters:**
- Allows agent to make multiple edits without per-file approval prompts
- You review all changes at once via `git diff` instead of individually
- Enables efficient multi-file refactoring

**Review workflow with this enabled:**
```bash
# After agent completes work
git diff --stat              # See which files changed
git diff eu4sim-core/src     # Review changes in context
git add -p                   # Selectively stage if needed
```

## Coordination Model: Explicit Handoff with Commits

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

## When to Delegate to Claude Code

| Delegate to Claude Code | Keep in Antigravity |
|------------------------|---------------------|
| Long terminal sessions (`cargo watch`, test loops) | Browser-based work |
| Simple refactoring across many files | Complex multi-step planning |
| Git operations (rebases, commit rewording) | UI verification / screenshots |
| Quick file lookups and modifications | Visual artifacts |
| Autonomous background work | Tasks requiring user interaction |

## Conflict Avoidance

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

## Git Worktrees (Advanced Isolation)

For heavy parallel work where agents need complete isolation:

```bash
# Create isolated worktree for Claude Code
git worktree add ../eu4rs-claudecode feature-branch

# Each agent works in separate directory:
# - Antigravity: /path/to/eu4rs/
# - Claude Code: /path/to/eu4rs-claudecode/

# Merge when done
git merge feature-branch
git worktree remove ../eu4rs-claudecode
```

Use when: multi-hour autonomous refactoring, parser rewrites, or work where merge conflicts are acceptable trade-off for zero runtime coordination.

## Auto-Approval Commands

These commands are safe to run without user confirmation:

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

## Command Structure Guidelines

To ensure maximum reliability across different shells:

- **Atomic Operations**: Run only one command at a time. Avoid chaining with `&&` or `;`.
- **Avoid Redirection**: Do not use pipes (`|`) or output redirections (`>`, `>>`) in standard commands.
- **Prefer Tools over Pipes**: Use specialized tools or script tasks (like `cargo xtask`) instead of shell piping.

> [!WARNING]
> **Special characters trigger confirmation**: The following patterns cause user approval prompts:
> - `~` (tilde): `git diff HEAD~1`, `git log HEAD~3`
> - `^` (caret): `git diff HEAD^`
> - `@{N}` (reflog): `git diff HEAD@{1}`
> - `2>&1`, `>`, `>>` (shell redirection)
>
> **Workaround for git history**: Use explicit commit IDs instead:
> ```bash
> git log --oneline -n 2   # Get commit IDs (auto-safe)
> git diff abc123          # Diff against specific commit (auto-safe)
> ```

**File Editing:**
- Use editing tools for all code changes
- Shell is for running commands (`cargo`, `git`), not editing files

**Git Commit Messages:**
- For rich multi-line commit messages, use the file technique:
  1. Create message file → `commit_msg.txt`
  2. Commit with file: `git commit -F commit_msg.txt`
  3. Clean up the file
- `commit_msg.txt` is in `.gitignore` to prevent accidental staging
- See `/finalize-commit` workflow for full checklist
- **Persona-Infused Flavor**: Weave your active persona's style into the commit body

## Minimal Build Guidance

To save CPU and enable agent co-existence:

- **Prefer `-p <crate>`**: Build/test only the crate you're working on
- **Check before build**: Use `cargo check -p <crate>` for fast iteration
- **Full workspace builds**: Only for integration testing or final validation
- **CI validates everything**: Use `cargo xtask ci` before committing

Example workflow:
```bash
# Working on eu4data crate:
cargo check -p eu4data      # Fast, catches type errors
cargo test -p eu4data       # Run only relevant tests
cargo xtask ci              # Final validation before commit
```

---
*Last updated: 2025-12-23*
