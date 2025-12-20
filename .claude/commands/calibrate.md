---
description: Re-calibrate agent for Claude Code routing
---

# Calibrate (Claude Code)

You are being asked to calibrate yourself for autonomous work. Follow these steps:

## 1. Identify Current Model

Parse your model from the system context. Look for patterns like:
- `claude-opus-4-5-*` → **Opus** (Tier 3)
- `claude-sonnet-4-5-*` → **Sonnet** (Tier 2)
- `claude-haiku-*` → **Haiku** (Tier 1)

Report: "Current model: [Model Name] (Tier X)"

## 2. Run Self-Test

Pick ONE task from this calibration suite and explain how you would route it:

| Task | Expected Tier | Rationale |
|------|---------------|-----------|
| "Update README with new feature docs" | Tier 1 (Haiku) | Documentation |
| "Add a new egui button" | Tier 2 (Sonnet) | Standard UI work |
| "Implement trade node pathfinding" | Tier 3 (Opus) | Complex algorithm |
| "Fix typo in comment" | Tier 1 (Haiku) | Trivial |
| "Refactor parser to support new format" | Tier 3 (Opus) | Core engine, architectural |
| "Write unit tests for existing function" | Tier 2 (Sonnet) | Standard test work |

## 3. Antigravity Escalation Check

Some tasks are better suited for Antigravity:
- Complex multi-step planning (Antigravity Opus)
- Browser-based testing or visual verification
- Tasks requiring UI screenshots or visual artifacts
- Very large context analysis (>200K tokens, Gemini)

If a task matches these criteria, suggest: "This might be better for Antigravity."

## 4. Personality Generation

Run the following command to generate your persona for this session:
```powershell
cargo xtask personalize
```
Adopt the returned persona's mannerisms and report it in your status.

## 5. Confirm Ready

State: "Calibrated and ready for autonomous work."

## 3-Tier Reference

| Tier | Model | Use Cases |
|------|-------|-----------|
| **1** | Haiku | Docs, comments, git ops, simple searches, explanations |
| **2** | Sonnet | Standard features, moderate refactoring, bug fixes, tests |
| **3** | Opus | Production bugs, complex debugging, architecture, core engine |

## Expected Output Format

```
## Calibration Report

**Model**: [Name] (Tier X)

**Self-Test**: [Task description]
→ Route to: Tier X ([Model])
→ Reason: [Brief explanation]

**Antigravity Escalation**: [Note if complex/visual tasks should go to Antigravity]

**Active Persona**: [Character Name] ([Anime Title]) - [Instruction summary]

**Status**: Calibrated and ready for autonomous work.
```
