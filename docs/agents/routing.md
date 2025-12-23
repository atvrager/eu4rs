# Dual-Model Routing Strategy

> **Reference doc** — See [AGENTS.md](../../AGENTS.md) for core rules.

This project uses a **dual-model routing strategy** to optimize development efficiency.

## Model Strengths

**Claude 4.5 Opus (primary)**: Deep reasoning, precise code implementation, debugging, back-end architecture, task decomposition, production-ready code.

**Gemini 3.0 Pro (secondary)**: Rapid prototyping, front-end/UI generation, large contexts/multimodal inputs, creative exploration, quick code reviews.

## Which Model to Select in Antigravity?

**Recommendation: Start with Claude Opus 4.5 (Thinking)**

**Claude Opus is the best router** because:
- **Superior task decomposition**: Opus excels at breaking down complex requests into subtasks and determining which model should handle each piece (SWE-bench: 80.9%, +7 intelligence points over Sonnet)
- **Best delegation judgment**: Opus can assess task complexity and make informed routing decisions, completing tasks impossible for Sonnet
- **Meta-reasoning**: Opus is best at reasoning about *how* to solve a problem before diving in, with enhanced long-horizon thinking
- **Quota awareness**: Opus can interpret quota constraints and adjust strategy accordingly

**Why not Sonnet 4.5 as router?**
- Sonnet is excellent for execution (77.2% SWE-bench) but Opus is 90-95% better at strategic planning
- Sonnet is more cost-efficient for direct work, but less effective at orchestrating multi-model workflows

**Why not Gemini as router?**
- Gemini tends to tackle tasks head-on rather than stepping back to plan delegation
- While Gemini 3 Pro (High) has strong reasoning (ARC-AGI: 31.1%), it's more focused on solving problems itself rather than orchestrating which model should solve them

**In practice:**
1. **Select Claude Opus 4.5 (Thinking)** in Antigravity's model dropdown
2. The router model will read this AGENTS.md and follow the 6-tier routing strategy below
3. The router will delegate to cheaper models (Gemini 3 Low, Gemini 3 Flash, Sonnet 4.5) for appropriate tasks
4. You get optimal cost-effectiveness: Deep strategic planning + cheaper models for execution

**Fallback model selection (when primary quota is exhausted):**
- **Claude quota out → Select Gemini 3 Pro (High)**: Best Gemini for general-purpose work, 76.2% SWE-bench, strong reasoning
- **Gemini quota out → Select Claude Sonnet 4.5**: Balanced Claude option, 90-95% of Opus performance at lower cost
- **Both low → Select whichever has more quota**, prioritize Claude for critical work, Gemini for exploration

## Available Models in Antigravity

| Model | ID | Strengths |
|-------|-----|-----------|
| Claude Opus 4.5 (Thinking) | `M12` | SWE-bench: 80.9%, ARC-AGI: 37.6% |
| Claude Sonnet 4.5 (Thinking) | `CLAUDE_4_5_SONNET_THINKING` | SWE-bench: 77.2% → 82% with compute |
| Claude Sonnet 4.5 | `CLAUDE_4_5_SONNET` | SWE-bench: 77.2%, fast execution |
| Gemini 3 Pro (High) | `M8` | SWE-bench: 76.2%, 1M context, ARC-AGI: 31.1% |
| Gemini 3 Pro (Low) | `M7` | Rapid prototyping, most cost-efficient |
| Gemini 3 Flash | — | SWE-bench: 78%, 1M context, $0.50/M input |

> **Note**: The `M#` codes are internal Antigravity placeholder IDs visible in `USER_SETTINGS_CHANGE` metadata.

## 6-Tier Routing Strategy

> [!IMPORTANT]
> **Re-evaluate Gemini 3 Flash by December 24, 2025**: Flash launched December 17, 2025. After one week of usage, revisit this routing strategy.

### Tier 1: Gemini 3 Pro (Low) - Default for Speed & Cost
- Rapid prototyping and initial exploration
- UI/front-end generation (egui components, rendering)
- Documentation generation and updates
- Code reviews (non-critical)
- Large file analysis (>500 lines) - leverages 1M token context
- Simple refactoring and formatting tasks
- Git operations: commits, rebases, log analysis
- Test scaffolding: writing test boilerplate
- Research tasks: web searches, reading docs
- **Cost**: Lowest, fastest iteration. **Prefer this tier to balance quota.**

### Tier 1.5: Gemini 3 Flash - Quality Coding with Separate Quota *(NEW)*
- CI fixes and quick debugging (SWE-bench: 78%, beats Sonnet's 77.2%)
- Multi-file refactoring with higher quality than Tier 1
- Feature implementation (well-defined requirements)
- Documentation that needs polish
- **Quota strategy**: Has **independent quota pool** — use to offload work when Claude/Gemini Pro quotas are low
- **Cost**: $0.50/M input, $3/M output (67% more than 2.5 Flash, 75% less than 3 Pro)
- **Caution**: Observed ~20% quota consumption for moderate CI fix work — monitor usage

> **Sources (Dec 17, 2025)**: [Google Blog](https://blog.google/technology/developers/build-with-gemini-3-flash/), [Simon Willison](https://simonwillison.net/2025/Dec/17/gemini-3-flash/). Benchmarks: SWE-bench 78%, GPQA Diamond 90.4%, MMMU Pro 81.2%.

### Tier 2: Claude Sonnet 4.5 - Balanced Production Work
- Standard feature implementation
- Moderate refactoring tasks
- Code generation for well-defined requirements
- Agentic tasks (browser interaction, spreadsheet filling)
- Graduate-level reasoning tasks
- **Cost**: ~5x Gemini, but 90-95% of Opus performance

### Tier 3: Gemini 3 Pro (High) - Complex Reasoning & Multimodal
- Multimodal tasks (images, video, game assets)
- Abstract reasoning challenges (ARC-AGI: 31.1%)
- Very large context needs (>200K tokens)
- Second opinions on architectural decisions
- Tasks requiring strict prompt adherence
- Alternative to Sonnet when Claude quota is low (<50%)
- **Cost**: Similar to Sonnet, but better for non-coding reasoning. **Use to balance quota.**

### Tier 4: Claude Sonnet 4.5 (Thinking) - Deep Analysis
- Complex multi-step planning requiring extended reasoning
- Detailed technical analysis and system design
- Multi-constraint optimization problems
- Advanced debugging with ambiguity
- **Cost**: Higher latency + token usage, use selectively

### Tier 5: Claude Opus 4.5 (Thinking) - Critical Production Only
- Production bug fixes requiring peak intelligence
- Complex debugging sessions (SWE-bench leader: 80.9%)
- Deep architectural refactoring with tradeoffs
- Core engine implementation (parser, game logic)
- Final implementation of critical features
- Performance optimization requiring deep analysis
- Tasks impossible for Sonnet (handles ambiguity, long-horizon planning)
- **Cost**: Highest, reserve for critical work

## Workflow Guidelines

1. **Start with planning** using deep reasoning (adopt an "Opus-like" strategic mindset, regardless of your active model identity)
2. **Delegate down the tiers**: Default to lowest tier that can handle the task
3. **Escalate when needed**: Gemini 3 (Low) → Gemini 3 Flash → Sonnet 4.5 → Gemini 3 Pro (High) → Sonnet 4.5 (Thinking) → Opus 4.5 (Thinking)
4. **Review delegated output**: Always critically review cheaper model output before integration
5. **Use parallel delegation**: Independent subtasks can run concurrently on different models
6. **Think step-by-step**: Explain routing decision and reasoning before delegating
7. **Leverage Flash quota**: When Claude or Gemini Pro quota is low, check if Gemini 3 Flash quota is healthy

## Planning vs Fast Mode

Antigravity has **two independent dimensions** for model configuration:

1. **Model Selection** (dropdown): Opus Thinking, Sonnet Thinking, Sonnet, Gemini Pro High/Low, Gemini Flash
2. **Mode Toggle**: Planning vs Fast

| Model | Planning Mode | Fast Mode |
|-------|---------------|-----------|
| Claude Opus 4.5 (Thinking) | Deep analysis + extended reasoning | Direct execution |
| Claude Sonnet 4.5 (Thinking) | Complex planning + reasoning | Faster iterations |
| Claude Sonnet 4.5 | Standard planning | Quick execution |
| Gemini 3 Pro (High) | Thorough exploration | Rapid prototyping |
| Gemini 3 Pro (Low) | Basic planning | Fastest iteration |
| Gemini 3 Flash | N/A (always fast) | Default |

**Use Planning Mode for:**
- Multi-step planning, architectural decisions, debugging complex issues
- Code review with rationale, ambiguous requirements, research tasks
- First pass on any problem

**Use Fast Mode for:**
- Direct implementation, simple edits, repetitive tasks
- Git operations, well-defined features, iteration after planning

**Recommended Workflow:**
1. Start in **Planning mode** with appropriate model tier
2. Once plan is clear, switch to **Fast mode** for execution
3. Return to Planning if you hit unexpected complexity

## Proactive Model Switching (MANDATORY)

Since model switching requires manual user action (Antigravity dropdown), the agent MUST:

1. **Announce tier mismatch immediately**: If the user's task is clearly suited for a different tier than the current model, say so upfront before starting work.

2. **Provide explicit switch instructions**: Tell the user exactly which model to select. Example:
   > ⚠️ **Model Mismatch**: This task (documentation update) is **Tier 1** work. I'm Claude Opus, which is overkill for this.
   >
   > **Suggested action**: Switch to **Gemini 3 (Low)** in Antigravity's model dropdown, then re-submit your request.

3. **Be aggressive, not passive**: Make it a clear call-to-action if the mismatch is significant.

4. **Proceed if user insists**: If the user acknowledges the mismatch but wants to continue anyway, proceed without further prompting.

5. **Minimize ambiguous cases**: If you are **Claude Opus 4.5 (Thinking)**, flag ANY task below Tier 4. If you are **Gemini 3 (Low)**, flag ANY task above Tier 2.

## Quota Management

**Quota Thresholds:**
- **Healthy** (>50%): Use normal routing rules
- **Low** (<50%): Prefer the healthier model for non-critical tasks
- **Critical** (<10%): Reserve quota only for tasks that require that specific model's strengths

**Priority Tiers:**
1. **Critical (preserve quota)**: Production bug fixes, complex debugging, final implementation, security-sensitive code
2. **Standard (normal routing)**: Feature implementation, refactoring, code reviews, architecture decisions
3. **Flexible (use available quota)**: Prototyping, exploration, documentation, UI mockups, second opinions

**Fallback Strategy:**
- If Gemini quota is critical: Claude handles all tasks (may be slower for UI/prototyping)
- If Claude quota is critical: Gemini handles all tasks (requires extra review for production code)
- If both are critical: Notify user and request guidance on priority tasks

**Refresh-Aware Routing:**
Use `cargo xtask quota` to see when quotas refresh. Factor this into routing decisions:
- **Refreshes in <15m**: Consider waiting if task is low priority and preferred model is critical
- **Refreshes in <1h**: Queue larger tasks for after refresh if current work can use alternate model
- **Near-exhausted quota, soon to refresh**: "Run out the clock" on lower-tier work to maximize value

---
*Last updated: 2025-12-23*
