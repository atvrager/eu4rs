---
description: Explore the codebase, documentation, and history to propose interesting tasks.
---

1.  **Analyze Recent History**:
    *   Run `git log --oneline -n 20` to understand what has been worked on recently.
    *   Identify any "half-finished" thoughts or "initial support" commits that imply more work is needed.

2.  **Analyze Codebase Structure and TODOs**:
    *   List the files in key directories (e.g., `eu4rs`, `eu4data`, `eu4txt`) to map the project structure.
    *   Search for "TODO", "FIXME", or "unimplemented!" comments using `grep_search` to find low-hanging fruit.
    
3.  **Analyze Documentation and Rules**:
    *   Briefly check `AGENTS.md` or `README.md` for any high-level goals or roadmap items that haven't been touched recently.
    
4.  **Synthesize Proposals**:
    *   Based on the findings, generate a curated menu of **3-5 distinct choices**.
    *   Ensure a mix of:
        *   **Features**: Expanding capabilities (e.g., "Add new map mode").
        *   **Engineering**: Robustness, testing, coverage, or refactoring.
        *   **Documentation/Cleanup**: Improving the dev experience.
    *   For each choice, provide:
        *   **Title**: Clear and exciting.
        *   **Difficulty**: (1-5 stars).
        *   **Context**: Why this task? (e.g. "We have `map_religion` but `map_culture` is missing" or "Parser X has no tests").
        *   **Plan**: A one-sentence starting point.

5.  **Present Menu**:
    *   Output the menu clearly to the user.
    *   Ask: "Which of these sounds most interesting to you right now?"
