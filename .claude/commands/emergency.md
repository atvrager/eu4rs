---
description: Emergency quota-exhaustion handoff workflow
---

# Emergency Handoff

Execute the emergency handoff workflow defined in `.agent/workflows/emergency.md`.

## Instructions

When quota is nearly exhausted and you need to preserve context for the next session:

1. **Read workflow**: Follow all steps in `.agent/workflows/emergency.md`
2. **Save pending work** to `docs/planning/handoff.md`
3. **Commit WIP changes** (even incomplete work)
4. **Notify user** with summary of what's saved and where

This ensures no context is lost between sessions.