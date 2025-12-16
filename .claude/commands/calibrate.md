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

## 3. Confirm Ready

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

**Status**: Calibrated and ready for autonomous work.
```
