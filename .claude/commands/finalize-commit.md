---
description: Complete feature implementation with CI, docs, and commit
---

# Finalize Commit

Execute the post-implementation checklist defined in `.agent/workflows/finalize-commit.md`.

## Instructions

Use this after implementing any feature, fix, or refactor:

1. **Read workflow**: Follow all steps in `.agent/workflows/finalize-commit.md`
2. **Verify CI**: Run `cargo xtask ci` - must pass
3. **Update docs**: If applicable, update roadmap/status files
4. **Create commit**: Use conventional commits with persona flavor
5. **Verify state**: Ensure clean git status
6. **Handoff**: Report ready to push

This ensures commits follow project conventions and nothing is forgotten.