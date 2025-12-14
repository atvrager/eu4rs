---
description: Confirm model switch and resume work
---

# Switched Command

Use this command after the user has manually switched models in the Antigravity dropdown, following a model mismatch recommendation.

## Steps

1. **Verify the model switch**: Check the `USER_SETTINGS_CHANGE` metadata to confirm the user switched to the recommended model. Report the current model name.

2. **Acknowledge and proceed**: Confirm the switch was successful and immediately resume the task that was interrupted by the model mismatch warning.

## Expected Behavior

- Agent confirms: "✅ **Confirmed**: You're now on [Model Name] — perfect for [Tier X] work!"
- Agent immediately continues with the original task without requiring further user input
- No additional questions or delays

## Example Usage

**User**: "I need to update the README"
**Agent**: "⚠️ Model Mismatch: This is Tier 1 work. Switch to Gemini 3 (Low)."
**User**: `/switched` (after switching in dropdown)
**Agent**: "✅ Confirmed: You're now on Gemini 3 (Low) — perfect for Tier 1 work! [proceeds with README update]"
