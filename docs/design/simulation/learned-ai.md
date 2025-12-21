# Learned AI System

This document describes the architecture for training and deploying LLM-based AI players in eu4sim.

## Overview

The system uses small language models (2B parameters or less) fine-tuned to play EU4 through the existing `AiPlayer` trait interface. The approach follows an "Imitate, then Improve" strategy:

1. **Phase 1: Imitation Learning** - Train the model to make legal moves by observing gameplay
2. **Phase 2: Inference Integration** - Deploy the trained model in the game loop
3. **Phase 3: Reinforcement Learning** - Improve play quality through self-play and reward shaping

## Design Constraints

| Constraint | Choice | Rationale |
|------------|--------|-----------|
| **Inference Hardware** | CPU-only | Maximize accessibility; run multiple AIs on standard hardware |
| **Model Size** | ≤2B parameters | CPU inference speed; memory budget for multiple AIs |
| **Output Format** | Action index | Most reliable; no parsing errors; constrained output space |
| **Multi-AI Strategy** | Shared backbone | Single model serves all AI countries; minimal memory footprint |
| **Personalities** | LoRA adapters | Different reward functions produce different play styles |

## Architecture

### High-Level Data Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Training Pipeline (Python)                   │
├─────────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────────────┐ │
│  │ Rust Game    │────▶│ JSONL Data   │────▶│ HuggingFace trl/peft │ │
│  │ (data gen)   │     │ (state,action)│     │ (SFT + LoRA)         │ │
│  └──────────────┘     └──────────────┘     └──────────────────────┘ │
│                                                      │               │
│                                                      ▼               │
│                                            ┌─────────────────────┐  │
│                                            │ base.safetensors    │  │
│                                            │ + personality.lora  │  │
│                                            └─────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                                                       │
                                                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         Inference Pipeline (Rust)                    │
├─────────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────────────┐ │
│  │ WorldState   │────▶│ Prompt       │────▶│ Candle + LoRA        │ │
│  │ + Commands   │     │ Builder      │     │ (batched inference)  │ │
│  └──────────────┘     └──────────────┘     └──────────────────────┘ │
│                                                      │               │
│                                                      ▼               │
│                                            ┌─────────────────────┐  │
│                                            │ Action Index        │  │
│                                            │ (0, 1, 2, ...)      │  │
│                                            └─────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

### Prompt Format

The model receives a structured prompt and outputs a single token (action index):

```
<|country|>FRA<|/country|>
<|state|>
Date: 1445.3.15
Treasury: 523 ducats
Manpower: 45,000
Stability: +1
At war with: ENG, BUR
Armies: 3 (total 32,000 men)
  - Armee de France (12k) at Paris [id:1]
  - Armee du Nord (10k) at Picardy [id:2]
  - Armee du Midi (10k) at Provence [id:3]
<|/state|>
<|actions|>
0: Move Army 1 to Normandy
1: Move Army 2 to Artois
2: Move Army 3 to Dauphine
3: Offer white peace to ENG
4: Pass (do nothing)
<|/actions|>
<|choice|>
```

The model outputs: `0`, `1`, `2`, `3`, or `4`.

### Multi-Country Batching

When multiple AI countries need decisions in a single tick, batch their prompts:

```rust
pub struct LlmAiPlayer {
    model: Arc<Mutex<CandleModel>>,
    personality_lora: Option<PathBuf>,
    country: Tag,
}

impl AiPlayer for LlmAiPlayer {
    fn decide(
        &mut self,
        visible_state: &VisibleWorldState,
        available_commands: &AvailableCommands,
    ) -> Vec<Command> {
        let prompt = build_prompt(visible_state, available_commands);
        let action_index = self.model.lock().unwrap().generate(&prompt);

        match available_commands.get(action_index) {
            Some(cmd) => vec![cmd.clone()],
            None => vec![], // Invalid index = pass
        }
    }
}
```

For efficiency, the game loop can collect all AI prompts and run a single batched inference call.

## Training Pipeline

### Phase 1: Data Generation (Rust)

A Rust binary generates training data by running simulated games:

```rust
// eu4sim-datagen/src/main.rs (future crate)

struct DataPoint {
    state: VisibleWorldState,
    available_commands: Vec<Command>,
    chosen_action: usize,
    outcome: GameOutcome,
}

fn generate_dataset(games: usize, bot_type: BotType) -> Vec<DataPoint> {
    // Run games with heuristic bot
    // Record (state, available_commands, action) tuples
    // Filter for winning games / good decisions
}
```

**Data Sources (in order of quality):**

| Source | Quality | Notes |
|--------|---------|-------|
| Random bot | Low | Baseline; many illegal/bad moves filtered out |
| Heuristic bot | Medium | Rule-based bot (attack weak neighbors, build economy) |
| Human replays | High | If replay format can be parsed; highest signal |
| Self-play (RL) | Best | Requires Phase 3 infrastructure |

### Phase 2: Supervised Fine-Tuning (Python)

```python
# scripts/train_ai.py

from transformers import AutoModelForCausalLM, AutoTokenizer
from peft import LoraConfig, get_peft_model
from trl import SFTTrainer

# Load base model (TinyGemma 2B or similar)
model = AutoModelForCausalLM.from_pretrained("google/gemma-2b")
tokenizer = AutoTokenizer.from_pretrained("google/gemma-2b")

# Configure LoRA (small adapter, fast training)
lora_config = LoraConfig(
    r=16,
    lora_alpha=32,
    target_modules=["q_proj", "v_proj"],
    lora_dropout=0.05,
)
model = get_peft_model(model, lora_config)

# Load Rust-generated data
dataset = load_dataset("json", data_files="training_data.jsonl")

# Train
trainer = SFTTrainer(
    model=model,
    train_dataset=dataset,
    # ... training arguments
)
trainer.train()

# Export
model.save_pretrained("models/eu4-balanced")
```

### Phase 3: Reinforcement Learning (Python + Rust)

Once the model plays legally (Phase 1-2), improve play quality with RL:

```python
# GRPO or PPO training loop

def get_reward(game_result: GameResult, country: str) -> float:
    """
    Reward function for score maximization.

    Customize this for different AI personalities.
    """
    score = game_result.final_scores[country]
    max_score = max(game_result.final_scores.values())

    return (
        + 10.0 * (score == max_score)           # Win bonus
        + 1.0 * score.provinces / 100           # Province control
        + 0.5 * score.development / 1000        # Development
        + 0.2 * score.prestige / 100            # Prestige
        - 5.0 * (score == 0)                    # Elimination penalty
    )
```

**Reward Variants for Different Personalities:**

| Personality | Reward Emphasis |
|-------------|-----------------|
| `balanced` | Standard score maximization |
| `aggressive` | +3x province conquest, -1x diplomatic penalties |
| `diplomatic` | +2x alliance count, +1x stability, -2x wars started |
| `economic` | +2x treasury, +2x development, -1x military spending |
| `survival` | +10x longevity bonus, +5x avoiding wars |

Each personality is a separate LoRA adapter trained with its reward function.

## Inference Pipeline (Rust)

### Crate Structure

```
eu4sim-ai/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── model.rs        # Candle model loading/inference
│   ├── prompt.rs       # State -> prompt conversion
│   ├── player.rs       # AiPlayer trait implementation
│   └── batch.rs        # Multi-country batching
```

### Dependencies

```toml
[dependencies]
candle-core = "0.8"
candle-nn = "0.8"
candle-transformers = "0.8"
tokenizers = "0.20"
safetensors = "0.4"
```

### Model Loading

```rust
pub struct Eu4AiModel {
    model: Gemma2Model,  // or equivalent from candle-transformers
    tokenizer: Tokenizer,
    lora_weights: Option<LoraWeights>,
    device: Device,  // CPU
}

impl Eu4AiModel {
    pub fn load(
        base_path: &Path,
        lora_path: Option<&Path>,
    ) -> Result<Self> {
        let device = Device::Cpu;

        // Load quantized base model (4-bit for speed)
        let model = load_quantized_gemma(base_path, &device)?;

        // Load LoRA if specified
        let lora_weights = lora_path.map(|p| load_lora(p, &device)).transpose()?;

        // Load tokenizer
        let tokenizer = Tokenizer::from_file(base_path.join("tokenizer.json"))?;

        Ok(Self { model, tokenizer, lora_weights, device })
    }

    pub fn choose_action(&self, prompt: &str) -> usize {
        // Tokenize
        let tokens = self.tokenizer.encode(prompt, true)?;

        // Run inference
        let logits = self.model.forward(&tokens)?;

        // Sample or argmax from digit tokens (0-9)
        let action_logits = extract_digit_logits(&logits);
        argmax(&action_logits)
    }
}
```

### Integration with Game Loop

```rust
// In eu4sim/src/main.rs or game loop

fn run_ai_tick(
    world: &WorldState,
    ai_countries: &[Tag],
    model: &Eu4AiModel,
) -> HashMap<Tag, Vec<Command>> {
    let mut all_inputs = Vec::new();

    // Collect all AI decisions needed
    for tag in ai_countries {
        let visible = compute_visible_state(world, tag);
        let available = compute_available_commands(world, tag);
        let prompt = build_prompt(&visible, &available);
        all_inputs.push((tag.clone(), prompt, available));
    }

    // Batch inference (if supported) or sequential
    let mut results = HashMap::new();
    for (tag, prompt, available) in all_inputs {
        let action_idx = model.choose_action(&prompt);
        let commands = available.get(action_idx)
            .map(|c| vec![c.clone()])
            .unwrap_or_default();
        results.insert(tag, commands);
    }

    results
}
```

## Model Selection

### Recommended Base Models

| Model | Params | License | Notes |
|-------|--------|---------|-------|
| **TinyGemma 2B** | 2B | Apache 2.0 | Google's smallest Gemma; good baseline |
| **Qwen2.5-0.5B** | 0.5B | Apache 2.0 | Very small; fastest inference |
| **Qwen2.5-1.5B** | 1.5B | Apache 2.0 | Good balance of size/capability |
| **Phi-3.5-mini** | 3.8B | MIT | Strong reasoning but may be too large |
| **SmolLM2-1.7B** | 1.7B | Apache 2.0 | HuggingFace's efficient small model |

**Recommendation**: Start with **TinyGemma 2B** (Gemma-2-2b-it). It is specifically designed for fine-tuning and distillation, making it ideal for creating distinct AI personalities ("flavor"). Use Qwen2.5 only if the 2B model proves too slow for the target hardware.

### Quantization

For CPU inference, use 4-bit quantization:

```rust
// Load quantized weights
let model = Gemma2Model::load_quantized(
    path,
    QuantizationConfig::Q4_0,  // 4-bit weights
    &Device::Cpu,
)?;
```

Expected inference speed on modern CPU:
- 0.5B model: ~50-100ms per action
- 1.5B model: ~200-400ms per action
- 2B model: ~300-500ms per action

## State Serialization

The `VisibleWorldState` must be converted to a prompt string. Key considerations:

### Information Hierarchy

```
Level 1 (always include):
  - Date
  - Own country core stats (treasury, manpower, stability, mana)
  - War status (who we're at war with)
  - Army summary (count, total strength)

Level 2 (include if relevant):
  - Individual army details (if <10 armies)
  - Active colonies
  - Pending diplomatic offers

Level 3 (summarize or omit):
  - Full province list (too many)
  - Complete diplomatic relations
  - Trade node details
```

### Token Budget

With ~2K context window (small model), allocate approximately:
- State description: 400-600 tokens
- Action list: 200-400 tokens
- Formatting overhead: 100 tokens
- Generation: 1-2 tokens

### Example Prompt Builder

```rust
pub fn build_prompt(
    state: &VisibleWorldState,
    commands: &[Command],
) -> String {
    let mut prompt = String::with_capacity(2000);

    prompt.push_str("<|country|>");
    prompt.push_str(&state.observer);
    prompt.push_str("<|/country|>\n");

    prompt.push_str("<|state|>\n");
    prompt.push_str(&format!("Date: {}\n", state.date));
    prompt.push_str(&format!("Treasury: {} ducats\n",
        state.own_country.treasury.to_f64().round()));
    prompt.push_str(&format!("Manpower: {}\n",
        format_thousands(state.own_country.manpower.to_f64())));
    prompt.push_str(&format!("Stability: {:+}\n", state.own_country.stability));

    if state.at_war {
        prompt.push_str("Status: AT WAR\n");
    }

    // Add army summaries, etc.
    prompt.push_str("<|/state|>\n");

    prompt.push_str("<|actions|>\n");
    for (i, cmd) in commands.iter().enumerate() {
        prompt.push_str(&format!("{}: {}\n", i, format_command(cmd)));
    }
    prompt.push_str("<|/actions|>\n");

    prompt.push_str("<|choice|>");

    prompt
}
```

## Evaluation Metrics

### Legality Rate
Percentage of model outputs that correspond to valid action indices:
- Target: >99% after SFT Phase 1

### Win Rate vs Baselines
Head-to-head games against known baselines:
- Random bot: Should win >80% of games
- Heuristic bot: Should win >50% of games (after RL)

### Score Distribution
Track score percentile across many games:
- Mean score should exceed random baseline by 2x+
- Score variance should decrease with training

### Inference Latency
Time per action on target hardware:
- Target: <500ms per action on CPU
- Measure P50, P95, P99 latencies

## Future Extensions

### Multi-Turn Planning
Instead of single-action output, the model could output a plan:
```
Turn 1: Move Army 1 to X
Turn 2: Move Army 1 to Y
Turn 3: Attack enemy
```

This requires more complex training but could improve strategic play.

### Model Distillation
Train a larger model (7B+) to high skill, then distill to smaller model for deployment.

### Opponent Modeling
Include opponent behavior features in state:
```
ENG tends to: Attack coastal provinces, avoid land wars
BUR tends to: Ally with Austria, defensive posture
```

### Curriculum Learning
Progressively increase game complexity during training:
1. Single-province games (learn economy)
2. Small wars (learn military)
3. Full games (learn diplomacy/strategy)

## Action Space Design (Tradeoffs)

With ~30 command types and hundreds of possible targets (provinces, armies, countries), the action space can explode. This section compares approaches.

### Approach 1: Full Enumeration

**How it works**: List every legal action as a numbered option.

```
0: Move Army 1 to Province 45
1: Move Army 1 to Province 46
2: Move Army 1 to Province 47
...
847: Declare war on Burgundy
848: Pass
```

| Aspect | Rating | Notes |
|--------|--------|-------|
| **Training complexity** | Low | Simple supervised learning on action indices |
| **Inference speed** | Fast | Single forward pass, argmax over N logits |
| **Code complexity** | Low | Just enumerate and format |
| **Prompt size** | High | 500-2000 tokens for action list alone |
| **Scaling** | Poor | Context window limits (~2K for small models) |

**When to use**: Early prototyping, small action spaces (<100 actions).

### Approach 2: Top-K Filtering

**How it works**: Use heuristics to pre-filter to the K most "interesting" actions.

```
# Heuristic ranking:
# - Armies: only show moves toward enemies or objectives
# - Economy: only show builds in high-value provinces
# - Diplomacy: only show relevant targets (neighbors, rivals)

0: Move Army 1 to Normandy (enemy territory)
1: Move Army 2 to Artois (reinforcing front)
2: Offer peace to England (war score: 67%)
3: Build marketplace in Paris (ROI: 12%)
4: Pass
```

| Aspect | Rating | Notes |
|--------|--------|-------|
| **Training complexity** | Medium | Model learns from pre-filtered space |
| **Inference speed** | Fast | Still single forward pass |
| **Code complexity** | Medium | Need heuristic ranking logic |
| **Prompt size** | Low | Fixed K actions (e.g., 20-50) |
| **Scaling** | Good | Constant prompt size regardless of game state |

**Risk**: Heuristics might filter out the optimal action. Mitigate by:
- Always include "Pass" as an option
- Include random sampling of filtered-out actions during training
- Track "regret" metrics (did we filter out what a stronger model would choose?)

**When to use**: Production systems, larger games.

### Approach 3: Hierarchical Selection

**How it works**: Two-stage inference. First pick action *type*, then pick *target*.

```
# Stage 1: Action type
0: Move an army
1: Diplomatic action
2: Economic action
3: Pass

# (Model outputs: 0)

# Stage 2: Target selection (conditional on "Move an army")
0: Army 1 → Normandy
1: Army 1 → Picardy
2: Army 2 → Artois
3: Army 3 → Dauphine
```

| Aspect | Rating | Notes |
|--------|--------|-------|
| **Training complexity** | High | Need to train/coordinate two decision points |
| **Inference speed** | Slower | Two forward passes per decision |
| **Code complexity** | High | State machine for multi-stage prompting |
| **Prompt size** | Low | Each stage has small action space |
| **Scaling** | Excellent | Logarithmic in total action space |

**When to use**: Very large action spaces, when inference latency is acceptable.

### Approach 4: Hybrid (Recommended)

**How it works**: Combine Top-K filtering with soft hierarchical grouping in the prompt.

```
<|actions|>
## Military (your 3 armies)
0: Army "Armee de France" → Normandy (attack)
1: Army "Armee de France" → Picardy (defend)
2: Army "Armee du Nord" → Artois (attack)

## Diplomacy (active wars)
3: Offer white peace to England
4: Offer province cession to England

## Economy (top opportunities)
5: Build marketplace in Paris
6: Develop tax in Lyon

## Other
7: Pass (do nothing this tick)
<|/actions|>
```

| Aspect | Rating | Notes |
|--------|--------|-------|
| **Training complexity** | Medium | Standard action-index training |
| **Inference speed** | Fast | Single forward pass |
| **Code complexity** | Medium | Grouping + ranking logic |
| **Prompt size** | Moderate | 200-400 tokens |
| **Scaling** | Good | Adapts to game complexity |

**Advantages**:
- Grouped format helps model understand action semantics
- Top-K per category ensures coverage across action types
- Single inference pass keeps latency low
- Natural language hints (attack/defend) aid learning

### Recommendation

**Start with Approach 1 (Full Enumeration)** for initial prototyping:
- Simplest to implement
- Validates the pipeline end-to-end
- Acceptable for small test scenarios

**Graduate to Approach 4 (Hybrid)** for production:
- Implement as part of GreedyBot work (heuristics needed anyway)
- Reuse ranking logic for both GreedyBot decisions and prompt filtering
- Target 30-50 actions max in the prompt

### Action Space Metrics to Track

During development, log these metrics to guide iteration:

| Metric | Target | Red Flag |
|--------|--------|----------|
| Actions per tick (mean) | 20-50 | >200 (prompt overflow) |
| Actions per tick (P99) | <100 | >500 |
| Filtered optimal rate | >95% | <80% (heuristics too aggressive) |
| Prompt tokens (actions) | <400 | >800 |

## Open Questions

1. **Long-Horizon Credit Assignment**: EU4 games span 400+ years. How do we assign credit for decisions made centuries before game end?
   - Consider intermediate rewards (monthly/yearly checkpoints)
   - Use TD(λ) with shorter horizon

2. **Determinism**: The model's RNG (temperature sampling) could cause desyncs in multiplayer. Solutions:
   - Use argmax (deterministic) instead of sampling
   - Seed model RNG with game RNG state

3. **Model Updates**: How do we ship model updates?
   - LoRA adapters are small (~20MB), easy to distribute
   - Base model updates are large (~2GB), less frequent

## References

- [Candle](https://github.com/huggingface/candle) - Rust ML framework
- [trl](https://github.com/huggingface/trl) - Transformer Reinforcement Learning
- [peft](https://github.com/huggingface/peft) - Parameter-Efficient Fine-Tuning
- [GRPO Paper](https://arxiv.org/abs/2402.03300) - Group Relative Policy Optimization
- [AlphaZero](https://arxiv.org/abs/1712.01815) - For comparison with MCTS approach

## See Also

- [Learned AI Musings](learned-ai-musings.md) - Design notes and speculation from an LLM's perspective, including thoughts on playing real EU4
