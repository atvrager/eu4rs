---
description: Quick quota-exhaustion handoff to save context before session ends
---

# Emergency Handoff Workflow

When quota is nearly exhausted and you need to preserve context for the next session:

## Steps

1. **Save pending work** to `docs/planning/handoff.md`:
   - Current task state
   - Blocking issues
   - Code review feedback
   - Specific fixes needed with file locations

2. **Create handoff prompt** if delegating to another model:
   - Include persona instructions
   - List concrete fixes with line numbers
   - Specify validation command (`cargo xtask ci`)

3. **Commit any uncommitted work** (even WIP):
   ```powershell
   git add .
   git commit -m "wip: [context]"
   ```

4. **Notify user** with summary of what's saved and where

## Example Handoff File Location
`docs/planning/handoff.md` - gitignored, safe for temporary context
