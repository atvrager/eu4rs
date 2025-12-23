# Agent Reference Documentation

This directory contains detailed reference documentation for AI agents working on the eu4rs project. The core rules live in [`AGENTS.md`](../../AGENTS.md) at the project root — these docs provide deep-dives on specific topics.

## Quick Reference

| Topic | When to Read |
|-------|--------------|
| [Routing Strategy](routing.md) | Model selection, tier decisions, quota management |
| [Claude Code Integration](claude-code.md) | Using Claude Code alongside Antigravity |
| [Calibration & Testing](calibration.md) | Verifying routing decisions, security protocols |
| [Build Tools](build-tools.md) | sccache, nextest, tokei setup |
| [Session Persona](persona.md) | Persona system, `/persona` skill, commit styling |

## Document Index

### [routing.md](routing.md)
**Dual-model routing strategy** — Complete guide to the 6-tier model selection system.
- Available models and their strengths
- When to use each tier
- Planning vs Fast mode
- Quota management and refresh-aware routing
- Proactive model switching protocol

### [claude-code.md](claude-code.md)
**Claude Code integration** — Working with Claude Code CLI/extension alongside Antigravity.
- Rate limit monitoring (`cargo xtask quota`)
- Model selection in Claude Code
- Tier-based routing by subscription
- Handoff protocols with commits
- Auto-approval commands
- Conflict avoidance between agents

### [calibration.md](calibration.md)
**Agent testing & calibration** — Verifying agent behavior.
- Routing calibration golden set
- Self-correction patterns for complex tasks
- Security & secrets paranoid verification protocol

### [build-tools.md](build-tools.md)
**Build performance tools** — Optional tools to speed up development.
- sccache (compiler cache)
- cargo-nextest (faster test runner)
- tokei (code statistics)

### [persona.md](persona.md)
**Session persona system** — Adding flavor to agent interactions.
- Persona file format and location
- Generating personas from MAL
- Adoption rules (what gets persona styling, what doesn't)
- Persona renewal triggers

## When to Consult These Docs

**As an agent, read these docs when:**
1. Making routing decisions between models → [routing.md](routing.md)
2. Handing off work to Claude Code → [claude-code.md](claude-code.md)
3. Unsure about tier assignment → [calibration.md](calibration.md)
4. Setting up a new dev environment → [build-tools.md](build-tools.md)
5. Persona feels faded or needs refresh → [persona.md](persona.md)

**The core rules in AGENTS.md are always loaded.** These reference docs are for when you need the full details.

---
*Last updated: 2025-12-23*
