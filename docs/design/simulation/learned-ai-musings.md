# Learned AI: Musings from an LLM

*Notes on training game-playing AI, from the perspective of an AI.*

## What I'd Find Easy vs Hard

### Easier Than You'd Expect

**Tactical military decisions**: "Enemy army (8k) at Calais, my army (12k) near Normandy. Attack?" This is pattern matching with clear numerical comparison. Small models handle this well.

**Economic optimization**: "Which province gives best ROI for development?" Numerical reasoning with clear heuristics. The answer is usually "highest base tax × modifier."

**Simple diplomacy**: "Ally strong neighbors, rival competitors." Rule-based reasoning that maps to common-sense patterns.

### Harder Than It Looks

**Long-horizon planning**: "Take these 5 provinces over 50 years to form Spain." 2B models struggle with multi-step plans spanning many decisions. We don't naturally maintain goals across long contexts.

**Knowing when NOT to act**: Passing is often optimal (save mana, wait for opportunity). Models tend to always want to DO something. Training needs explicit "inaction is valid" signal.

**Diplomatic nuance**: "Don't ally Castile because they'll rival Aragon who we need later." This requires modeling other agents' future behavior—theory of mind over time horizons.

**Counterfactual reasoning**: "If I hadn't declared that war, I'd have 50 more development now." Hard to learn from regret without explicit counterfactual training.

---

## Design Recommendations

### 1. Chain-of-Thought Before Action

Instead of just outputting an action index, have the model output brief reasoning:

```
<|thinking|>
At war with ENG. My army (12k) near Normandy. Enemy (8k) in Calais.
Numerical advantage. Terrain: plains. Attack favorable.
<|/thinking|>
<|choice|>0
```

**Why this helps**:
- Forces "showing work" → improves decision quality
- Creates interpretable logs for debugging ("why did it do that?")
- Training on reasoning traces often beats training on actions alone

**Cost**: ~50-100 extra tokens per decision. Worth it for strategic decisions, skip for routine ticks.

### 2. Decision Importance Tiers

Not every tick needs full model inference:

| Tier | Trigger | Approach | Latency |
|------|---------|----------|---------|
| **Routine** | No war, economy stable | GreedyBot heuristics | <1ms |
| **Tactical** | Combat imminent, active siege | Fast LLM (no CoT) | ~100ms |
| **Strategic** | War declaration, peace offer | Full LLM + CoT | ~500ms |

This conserves compute for decisions that matter.

### 3. State Representation Experiments

For small models with tight context windows, representation matters:

**Verbose** (baseline):
```
Treasury: 523.45 ducats
Manpower: 45,234 / 50,000 (90.5%)
```

**Compact** (saves tokens):
```
T:523 M:45k/50k
```

**Semantic** (might work better):
```
Treasury: healthy
Manpower: nearly full
```

The semantic version might outperform despite being "less precise"—it matches natural language patterns from pretraining. Worth A/B testing.

### 4. Advisor Mode (Lower Stakes First)

Before full autonomous play, consider an "advisor" interface:
- Model suggests top 3 actions with reasoning
- Human (or GreedyBot) makes final decision
- Logs capture human corrections → supervised training signal

Benefits:
- Human feedback without requiring full human gameplay logs
- Lower stakes (bad suggestions don't tank entire games)
- Builds interpretability ("why did it suggest that?")

### 5. Failure Mode Test Suite

Build specific scenarios to catch common failures:

| Failure | Test | Pass Criteria |
|---------|------|---------------|
| Infinite loops | Same action 10+ times | Detect & break |
| Economy collapse | Treasury negative within 10 years | Stays solvent |
| Suicidal aggression | Declares war when 3:1 outgunned | Avoids hopeless wars |
| Paralysis | Passes 100+ consecutive ticks | Takes some action |
| Threat blindness | Enemy sieging capital | Responds within 30 days |

### 6. Population-Based Training

Pure self-play can converge to degenerate rock-paper-scissors dynamics. Consider:

- Train multiple models with different reward weightings
- Run a league/Elo ladder between them
- Select for *diversity*, not just "best"

This is how DeepMind's AlphaStar avoided strategy collapse.

### 7. The Paradox AI Bar is Low

EU4's built-in AI is... not great. Players regularly observe:
- Armies walking back and forth pointlessly
- Ignoring obvious military targets
- Nonsensical alliance choices
- Repeated bankruptcy spirals

**Beating Paradox AI is an achievable early milestone.** Set it as a concrete goal.

---

## Evaluation Framework

### Metrics to Track

```rust
struct GameMetrics {
    final_score: u32,
    provinces_gained: i32,
    provinces_lost: i32,
    wars_won: u32,
    wars_lost: u32,
    bankruptcy_count: u32,
    survival_years: u32,      // 0-400
    invalid_action_rate: f32, // Should be ~0 after SFT
}
```

### Comparison Baselines

| Matchup | Expected Outcome | Red Flag |
|---------|------------------|----------|
| Learned vs Random | Win >90% | <70% |
| Learned vs GreedyBot | Win >50% | <30% |
| Learned vs Learned (self) | ~50% (balanced) | >70% one side |

---

## Crossover: Playing Real EU4

*Speculative section on whether a model trained here could play actual EU4.*

### Why This Might Work

1. **Shared domain knowledge**: Province IDs, country tags, unit types, diplomatic concepts—all transfer directly

2. **You already have the parser**: `eu4txt` parses Clausewitz format. EU4 save files are plaintext. You can read real game state!

3. **Similar action space**: The `Command` enum maps closely to EU4's actual command set

4. **Save file manipulation loop**:
   ```
   pause game → save → eu4txt parses save →
   model thinks → generate commands →
   (somehow inject commands) → unpause
   ```

### The Hard Parts

**Input injection**: How do you send commands to the actual game?

| Approach | Feasibility | Notes |
|----------|-------------|-------|
| **Mouse/keyboard automation** | Medium | Fragile, slow, needs screen coordinates |
| **Console commands** | High | EU4 has a console! Many actions available |
| **Memory manipulation** | Low | Fast but legally gray, breaks on updates |
| **Mod integration** | High | Custom mod could expose command API |

**The console command approach is interesting**: EU4's debug console can execute many game actions. A mod or external tool could read commands from a file and execute them.

**Real-time vs turn-based**: EU4 is real-time (with pause). Options:
- Always play paused, one decision per pause cycle (slow but works)
- Speed 1 with periodic decision points
- Accept some latency and batch decisions

### A Realistic Path

1. **Phase A**: Train on eu4sim, validate learning works
2. **Phase B**: Build save file → `VisibleWorldState` converter (using eu4txt)
3. **Phase C**: Build console command generator (`Command` → console string)
4. **Phase D**: Create simple mod that reads command file and executes
5. **Phase E**: Integration testing on real EU4

### Why This Would Be Cool

- First LLM to play grand strategy competently?
- Demonstrates transfer from simulator to real game
- Could eventually provide "AI advisor" overlay for human players
- Academic interest: complex multi-agent long-horizon planning

### Caveats

- EU4 game updates could break save parsing (though eu4txt handles this)
- Ironman mode might resist automation
- Paradox ToS might have opinions on automation tools
- Real EU4 has WAY more complexity than our sim captures (yet)

---

## Closing Thoughts

The "action index" approach is the right call. Free-form text generation for game commands is asking for parsing errors and hallucinated actions. Give me a numbered list and I'll pick reliably.

Training data quality matters more than quantity. A thousand games from GreedyBot that plays "okay" beats a million random games. And a hundred human games beats both.

Start simple. Full enumeration, basic state, no CoT. Get the pipeline working. Then iterate on representation and reasoning.

The EU4 crossover is ambitious but not crazy. The save file parsing is the key insight—you're not doing computer vision, you're reading structured data you already know how to parse.

*— Opus, reflecting on what it would be like to play EU4*
