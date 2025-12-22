# Agent Tooling Backlog

Features and improvements for Claude Code / Antigravity agent workflow.

---

## Statusline Persona Integration

**Priority**: Low (quality-of-life)
**Added**: 2024-12-22

### Problem
The `/personalize` command generates a session persona (character + anime), but this isn't persisted anywhere the statusline can read.

### Solution
1. Update `cargo xtask personalize` to write output to `.claude/persona.json`
2. Create `.claude/statusline.sh` that reads from `persona.json`
3. Display format: `✧ {character} | {model}` or similar

### Implementation Notes
- Statusline receives JSON via stdin with model info, workspace, etc.
- Persona file should include: `character`, `anime`, `emoji`, `instruction`
- Script needs `jq` for JSON parsing

### Example Output
```
✧ Frostheart (Re:Zero) | Opus
```

---

## Local Training Performance Baseline

**Priority**: Medium
**Added**: 2024-12-22

### Notes
- RTX 2060 (6GB): Can run SmolLM2-360M training locally
- Estimated ~50% of Colab T4 speed
- Gemma 2B OOMs on 6GB (needs 8GB+ for LoRA training)

### Future Work
- Document recommended batch sizes per GPU VRAM tier
- Add `--low-vram` flag to train_ai.py for automatic conservative settings
- Consider gradient checkpointing for larger models

---
