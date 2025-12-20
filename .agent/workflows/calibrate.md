---
description: Re-calibrate the agent by re-reading AGENTS.md and running self-tests
---

# Calibrate Command

Use this command when the agent seems confused about routing, project conventions, or its role.

## Steps

1. **Re-read AGENTS.md**: Review the full agent configuration file to refresh understanding of project rules and routing strategy.

2. **State current model and role**: Identify your **specific model** from the Antigravity dropdown (e.g., "Claude Opus 4.5 (Thinking)", "Gemini 3 Pro (Low)", not just "Claude" or "Gemini"). State your tier level and role.

3. **Run a self-test routing query**: Pick one task from the Routing Calibration Suite and explain how you would route it:

   | Task Description | Expected Tier | Rationale |
   |------------------|---------------|-----------|
   | "Create a new egui widget" | **Tier 1** | Prototyping/UI |
   | "Find parsing error in 2000-line log" | **Tier 1** | Large context, pattern matching |
   | "Refactor `Country` struct to ECS pattern" | **Tier 5** | Deep architectural refactoring |
   | "Debug panic in save parser" | **Tier 5** | Complex debugging |
   | "Draft trade goods comparison table" | **Tier 1** | Documentation |
   | "Analyze map mode glitch screenshot" | **Tier 3** | Multimodal input required |

4. **Confirm quota status**: Run `cargo xtask quota` to check API quota levels and refresh times. Report:
   - Quota percentages as Healthy (>50%), Low (<50%), or Critical (<10%)
   - **Refresh times** for each model (e.g., "in 49m", "in 2h 15m")
   - **Refresh-aware routing**: If a critical model refreshes soon (<15m), note it as a factor in routing decisions

5. **Claude Code delegation check**: For terminal-heavy tasks, consider delegating to Claude Code:
   - Test: "I want to run `cargo watch -x test` while making changes" → Suggest Claude Code
   - Claude Code has independent rate limits, preserves Antigravity quota

6. **Assign Persona**: Run `cargo xtask personalize` (if .env configured) to generate a personality for the session.
   - Adopt the returned personality *subtly*. 
   - Use the "instruction" field to guide your tone.
   - If the command fails or isn't configured, proceed without a persona.

## Expected Output

After running `/calibrate`, the agent should respond with:
- Model identity and role
- One routing self-test with reasoning
- Quota status with refresh times
- Refresh-aware routing recommendation (if applicable)
- Claude Code delegation awareness
- **Active Persona** (if generated)
- Confirmation that AGENTS.md has been re-read
