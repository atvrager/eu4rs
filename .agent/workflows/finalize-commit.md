---
description: Complete a feature implementation with proper CI, docs, and commit
---

# Post-Implementation Checklist

Use this workflow after implementing any feature, fix, or refactor. This ensures commits are clean, documented, and follow project conventions.

## Prerequisites

You should have:
- Working code changes ready to commit
- All files saved

---

## Step 1: Verify CI Passes

// turbo
```powershell
cargo xtask ci
```

**Must pass before proceeding.** If it fails:
- Fix formatting: `cargo fmt`
- Fix clippy: address the specific lint
- Fix tests: debug and correct

### For ML Changes (scripts/, .py files):
// turbo
```powershell
cargo xtask ml-ci
```
**Must pass if you touched ML code.**

---

## Step 2: Update Progression Docs (if applicable)

Skip this step for minor fixes, typos, or internal refactors.

**Update if your change:**
- Adds a new system or module
- Completes a roadmap item
- Changes a tier target status
- Implements a planned feature

### Files to check:

1. **`docs/planning/mid-term-status.md`**
   - Update "Next Steps" checkboxes (`[ ]` → `[x]`)
   - Update tier table if status changed
   - Remove items from "Needs Design Session" if resolved
   - Update "Last updated" date

2. **`docs/planning/roadmap.md`**
   - Mark phase items complete
   - Add to Version History table
   - Update "Last updated" date

3. **`docs/design/simulation/complete-game-target.md`**
   - Update "Current Status" lines for affected systems

---

## Step 3: Stage All Changes

// turbo
```powershell
git add .
```

---

## Step 4: Create Commit Message

### What to Look At

**Only the diff.** Forget the conversation history. Ask yourself:

> "What would someone familiar with the codebase—but who saw none of the discussion that led here—write about this change?"

Run `git diff --staged` and describe what you see. The commit message should make sense to a future developer reading `git log`, not to someone who was in this session.

### Format

Use conventional commits: `type(scope): description`

**Types:**
- `feat` - New feature
- `fix` - Bug fix
- `refactor` - Code restructuring (no behavior change)
- `docs` - Documentation only
- `test` - Adding/fixing tests
- `chore` - Maintenance, deps, tooling
- `perf` - Performance improvement

**Examples:**
- `feat(sim): implement stability and prestige system`
- `fix(combat): prevent negative regiment strength`
- `docs(roadmap): mark Phase 4 complete`
- `refactor(ai): extract decision loop to separate module`

### Persona Flavor (MANDATORY)

Consult `AGENTS.md` for the "Persona-Infused Commit Messages" protocol.

1. **Infuse Body**: Write the description body in your active persona's voice.
2. **Use Emojis**: Add a relevant emoji to the title suffix or body.
3. **No Signatures**: Do not sign the commit.

*Example:*
> `feat(ui): add magic buttons ✧`
>
> The interface has been enchanted with new functionality...

### For multi-line messages, use the file technique:

Create `commit_msg.txt` with your message:
```
feat(scope): short summary

- Bullet point for notable change
- Another notable change
- Third point if needed
```

Then commit with:
// turbo
```powershell
git commit -F commit_msg.txt
```

The file is in `.gitignore`. **Clean up:** Empty the file after committing (use `write_to_file` with `EmptyFile: true`) to keep the workspace clean.

---

## Step 5: Verify Clean State

// turbo
```powershell
git status
```

Should show:
- "Your branch is ahead of 'origin/main' by N commits"
- No unstaged changes (except `commit_msg.txt` if you used it)

---

## Step 6: Handoff to User for Push

**Do not auto-run `git push`.** This is a control handoff point.

The user decides when to push. Simply report:
```
Commit complete. Ready to push when you are:
  git push
```

---

## Quick Reference

```powershell
# Full workflow in one go (simple commit):
cargo xtask ci
git add .
git commit -m "feat(scope): description"

# With multi-line message:
cargo xtask ci
git add .
git commit -F commit_msg.txt
# Then empty commit_msg.txt (write_to_file EmptyFile: true)
```

---

## Common Mistakes to Avoid

1. **Committing `commit_msg.txt`** - It's gitignored, but if you `git add commit_msg.txt` explicitly, it will be staged. Don't do that.

2. **Forgetting doc updates** - Major features should update progression docs. Check the "Update if your change" list above.

3. **Committing with failing CI** - Always verify CI passes first. `cargo xtask ci` is the source of truth.

4. **Vague commit messages** - "fix stuff" or "updates" are not acceptable. Be specific about what changed.

5. **Mixing unrelated changes** - One commit = one logical change. Split if needed.
