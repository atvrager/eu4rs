# Agent Testing & Calibration

> **Reference doc** — See [AGENTS.md](../../AGENTS.md) for core rules.

## 1. Routing Calibration Suite

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

## 2. Output Verification Protocols

**Self-Correction Pattern:**
For complex tasks (Tier 4+), the agent must explicitly perform a "Self-Review" step:
1. **Generate** initial solution
2. **Critique** against requirements (edge cases, idioms)
3. **Refine** code based on critique

**Determinism Check:**
For critical logic provided by Gemini:
1. **Ask Opus to review** the snippet ("LLM-as-a-Judge")
2. Prompt: "Rate this code 1-5 on correctness/safety. If <5, rewrite it."

## 3. Security & Secrets (Paranoid Verification)

**Rule**: Anything involving secrets, API keys, or credentials requires **Paranoid Verification**.

**Protocol**:
1.  **Check Ignore**: Verify file is covered by `.gitignore` (run `git check-ignore -v <file>`).
2.  **Check Tracking**: Verify file is NOT in git index (run `git ls-files <file>`).
3.  **Check History**: Verify file was NEVER committed (run `git log --all -- <file>`).
4.  **Simulate**: "If I push this now, what leaks?" (Review `git status` output carefully).

*Only proceed when ALL 4 checks pass.*

---
*Last updated: 2025-12-23*
