# Agent Rules

> **Meta-Rule**: This configuration is **agent-agnostic**. Write rules based on project conventions and principles, not specific AI model capabilities or tool implementations. Focus on *what* to do, not *how* a particular model does it. The only assumption is that `atv` will use Antigravity for the foreseeable future.

## Platform
- **Likely Windows**: This is a game modding project, so Windows is the primary platform. Check your shell early in a session.
- **Shell detection**: You may get PowerShell (default) or bash (advanced user). Run a quick check early to know what commands will work.
- **Windows symlinks**: Require Developer Mode. If symlinks appear as plain text files with just the target path, see Windows Setup below.
- **PowerShell scripts**: May not run directly due to execution policy. Use `powershell -ExecutionPolicy Bypass -File script.ps1` instead of `./script.ps1`.

## Working Directory
- **NEVER use `cd` commands**: Set the working directory per-command instead of navigating.
- **Track workspace root**: The active workspace is `c:\Users\atv\Documents\src\eu4rs` - use this as your default working directory for most commands.
- **Stay in context**: You don't need to navigate between directories. Just specify the appropriate working directory for each command.

## Windows Setup
To enable proper symlink support on Windows:
1. Enable **Developer Mode**: `Start-Process 'ms-settings:developers'` → toggle "Developer Mode" on
2. Configure git: `git config core.symlinks true`
3. For Git Bash, create symlinks with: `MSYS=winsymlinks:nativestrict ln -s target linkname`
4. Re-checkout broken symlinks: `rm file.md && git checkout -- file.md`

## Logging
- **ALWAYS** use the `log!` macros (e.g., `info!`, `warn!`, `error!`, `debug!`) instead of `println!` or `eprintln!`.
    - Exception: Panics, early startup errors before logger initialization, or CLI output intended for piping (e.g., `snapshot` text output if any).
    - Exception: `println!` may be used for interactive CLI prompts if strictly necessary, but prefer logging for status.

## Code Quality
- **Clean up comments**: Remove any "thinking comments" (e.g., "Wait, I should...", "Option A:...") from the final code. Comments should explain *why* code exists or *how* it works, not the history of how you wrote it.

## Common Commands
- `cargo xtask ci`: Run continuous integration tests. **Must pass before committing.**
- `cargo xtask snapshot`: Regenerate golden snapshots for tests. Use this when you've modified rendering pipelines and expect output changes. **Ask the user for manual validation of the new output.**

## Testing GUI Applications
- **Visual verification required**: GUI applications (like the main eu4rs app) cannot be effectively tested via automated command execution. Ask the user to run the program manually for visual verification.
- **Batch questions**: You can ask up to 3 questions at once when requesting testing feedback.

## Git Workflow
- **Commit reordering**: When you need to update an older commit, consider reordering commits via interactive rebase rather than using fixup/squash. Move the older commit to HEAD (or your new changes down to it), amend directly, then reorder back. This is simpler when there aren't many overlapping files between commits.
- **Non-interactive rebase**: Don't try to drive vim interactively. Use `GIT_SEQUENCE_EDITOR` and `GIT_EDITOR` with scripts or `sed`:
  ```bash
  # Reword a commit
  GIT_SEQUENCE_EDITOR="sed -i 's/pick abc123/reword abc123/'" \
  GIT_EDITOR="echo 'New message' >" \
  git rebase -i HEAD~3
  
  # Write a custom todo file
  echo "pick abc123 msg" > /tmp/todo.txt
  GIT_SEQUENCE_EDITOR="cp /tmp/todo.txt" git rebase -i origin/main
  ```

## Commit Messages
- **Focus on Deltas**: Write commit messages based ONLY on the actual code changes (diffs). Do not summarize the conversation history.
- **Format**: Use bulleted lists for details.
- **Content**: Be professional and technical. Do not mention "I ran CI" or "User requested this". Assume competence.
