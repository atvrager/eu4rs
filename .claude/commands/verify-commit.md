---
description: Verify HEAD commit follows project conventions
---

# Verify Commit

Execute the post-commit verification workflow defined in `.agent/workflows/verify-commit.md`.

## Instructions

Use this to verify an existing commit follows good practices:

1. **Read workflow**: Follow all steps in `.agent/workflows/verify-commit.md`
2. **Run verification**: Execute `cargo xtask verify-commit` (if available)
3. **Check personality**: Ensure commit message has persona flavor
4. **Verify CI**: Run `cargo xtask ci` to ensure commit didn't break anything
5. **Report status**: Summarize findings (message format, docs, CI)

This is useful after another agent made a commit, or for quick fixes that bypassed the full workflow.