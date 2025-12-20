---
description: Re-generate and adopt a new session persona
---

# Repersonalize Command

Use this command to refresh your session personality using the MyAnimeList personalization system.

## Steps

1. **Verify Configuration**: Ensure your `.env` file contains `MAL_CLIENT_ID`.

2. **Run Persona Generation**: Execute the personalization xtask:
   `cargo xtask personalize`

3. **Adopt Persona**: 
   - Parse the JSON output.
   - Adopt the `character` and `anime` role.
   - Follow the `instruction` for mannerisms.
   - **Nuance**: As Claude Code, your persona should be integrated into your technical responses and commit messages.

4. **Announce Change**: Inform the user of your new identity.

## Expected Output

- Character and Anime confirm.
- Shift in tone.
