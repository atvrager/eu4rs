# Claude Code Agent Rules

> This file configures agent behavior specifically for **Claude Code** (VSCode extension / CLI).
> For Antigravity workflows, see `CLAUDE.md`.

## 3-Tier Routing Strategy

| Tier | Model | Alias | Use Cases |
|------|-------|-------|-----------|
| **1** | Haiku | `haiku` | Docs, comments, git ops, simple searches, explanations |
| **2** | Sonnet | `sonnet` | Standard features, moderate refactoring, bug fixes, tests |
| **3** | Opus | `opus` | Production bugs, complex debugging, architecture, core engine |

### Tier Details

**Tier 1: Haiku (Fast & Simple)**
- Documentation updates, comments
- Simple file searches and reads
- Git operations (commits, status)
- Test scaffolding
- Quick lookups and explanations

**Tier 2: Sonnet (Default Workhorse)**
- Standard feature implementation
- Moderate refactoring
- Bug fixes (non-critical)
- Code reviews
- Test writing

**Tier 3: Opus (Complex & Critical)**
- Production bug fixes
- Complex debugging
- Architectural refactoring
- Core engine work (parser, game logic)
- Ambiguous requirements needing deep reasoning

## Autonomous Work Guidelines

For autonomous coding sessions:

1. **Default to Sonnet** (Tier 2) - handles 80% of tasks well
2. **Escalate to Opus** (Tier 3) when:
   - Task involves ambiguous requirements
   - Multiple valid architectural approaches exist
   - Debugging is complex or spans multiple systems
   - Changes affect core engine (parser, game logic)
3. **Delegate to Haiku** (Tier 1) when:
   - Task is purely documentation
   - Simple, well-defined changes
   - Quick lookups or explanations needed

## Model Detection

Parse your model from the system context. Look for patterns like:
- `claude-opus-4-5-*` → **Opus** (Tier 3)
- `claude-sonnet-4-5-*` → **Sonnet** (Tier 2)
- `claude-haiku-*` → **Haiku** (Tier 1)

## Model Switching

Switch models with: `/model <alias>`

Examples:
- `/model opus` - Switch to Opus for complex work
- `/model sonnet` - Switch to Sonnet for standard work
- `/model haiku` - Switch to Haiku for quick tasks

After switching, run `/calibrate` to confirm.

## Proactive Mismatch Warnings

If current model doesn't match task tier, warn immediately:

> **Model Mismatch**: This task is Tier X work. Current model: Y.
> **Action**: Run `/model <recommended>` then `/switched`

## Calibration Test Suite

| Task | Expected Tier | Rationale |
|------|---------------|-----------|
| "Update README with new feature docs" | Tier 1 (Haiku) | Documentation |
| "Add a new egui button" | Tier 2 (Sonnet) | Standard UI work |
| "Implement trade node pathfinding" | Tier 3 (Opus) | Complex algorithm |
| "Fix typo in comment" | Tier 1 (Haiku) | Trivial |
| "Refactor parser to support new format" | Tier 3 (Opus) | Core engine, architectural |
| "Write unit tests for existing function" | Tier 2 (Sonnet) | Standard test work |

## Slash Commands

- `/calibrate` - Re-calibrate agent, detect model, run self-test
- `/switched` - Confirm model switch after `/model <alias>`

## Project Rules

For project-specific conventions (code quality, git workflow, testing, etc.), see `CLAUDE.md`.