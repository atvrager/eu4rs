---
description: Confirm model switch and resume work
---

# Switched (Claude Code)

The user has just switched models using `/model <alias>`. Confirm and resume.

## Steps

1. **Verify switch**: Check your system context for the new model ID
2. **Report**: State the confirmed model and tier
3. **Resume**: Continue the interrupted task immediately without further prompting

## Expected Output

```
Confirmed: Now on [Model Name] (Tier X) - ready to continue.
```

Then immediately resume the task that was interrupted by the model mismatch warning.