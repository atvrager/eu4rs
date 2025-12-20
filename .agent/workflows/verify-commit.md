---
description: Verify the HEAD commit follows good practices (post-commit check)
---

# Verify Commit

Use this workflow when a commit already exists and you want to verify it follows project conventions. This is a post-hoc sanity check.

## When to Use

- After another agent (e.g., Gemini) made a commit
- After a quick fix that bypassed the full workflow
- When the user asks "did we forget anything?"

---

## Step 1: Run Verification

// turbo
```powershell
cargo xtask verify-commit
```

This automates:
1. Conventional commit message format check
2. Doc update verification (if code changed)

---

## Step 2: Verify CI Still Passes

// turbo
```powershell
cargo xtask ci
```

If CI fails, the commit introduced a problem. Fix it:
```powershell
# Fix the issue, then:
git add .
git commit --amend --no-edit
```

---

## Step 3: Report Status

Summarize findings:

```
✅ Commit verified:
- Messages & Docs: [output of xtask verify-commit]
- CI: [passing/failing]

Ready to push when you are:
  git push
```

---

## Quick Fixes

### Bad commit message
```powershell
git commit --amend -m "feat(scope): proper description"
```

> [!CAUTION]
> **Amend safety**: Only use `--amend` if HEAD was created in this session AND has not been pushed to remote.

### Forgot to update docs
```powershell
# Make doc changes, then:
git add docs/
git commit -m "docs: update roadmap for [feature]"
```
