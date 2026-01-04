# Reinforcement Learning for EU4 AI

**Status**: Phase 3 - Not Started (as of 2026-01-04)
**Related**: See [`learned-ai.md`](./learned-ai.md) for overall architecture

---

## Overview

This document analyzes what infrastructure exists for reinforcement learning (RL) in the EU4 AI system and what's needed to implement Phase 3 (RL training). It covers both supervised learning (what we have) and various RL approaches.

### Quick Summary

**Current State**: ✅ **Supervised Learning (Phase 1-2) Complete**
- Behavior cloning from `GreedyAI` heuristic
- SFT (Supervised Fine-Tuning) working end-to-end
- Models can play EU4 at ~80% of GreedyAI skill

**Missing for RL**: ❌ **Game Outcome Tracking, Reward Functions, Episode Data**
- No notion of "winning" or "losing"
- No trajectory sequences (state → action → reward → next state)
- No policy gradient training infrastructure

---

## Table of Contents

1. [Current Infrastructure (Phase 1-2)](#current-infrastructure-phase-1-2)
2. [Missing Components for RL](#missing-components-for-rl)
3. [Supervised vs. RL Approaches](#supervised-vs-rl-approaches)
4. [Recommended Path: Offline RL](#recommended-path-offline-rl)
5. [Implementation Roadmap](#implementation-roadmap)
6. [Technical Specifications](#technical-specifications)
7. [References](#references)

---

## Current Infrastructure (Phase 1-2)

### ✅ What Exists

#### 1. Data Collection

**Location**: `eu4sim-core/src/observer/datagen.rs` (500+ lines)

- **TrainingSample** struct captures:
  - `tick`, `country`, `state` (VisibleWorldState)
  - `available_commands`, `chosen_actions` (indices), `chosen_commands`
  - Multi-command support (AI can submit multiple actions per tick)

- **Output Formats**:
  - Binary (`*.cpb.zip`): Cap'n Proto, ~10x faster than JSON
  - JSON Archive (`*.zip`): Per-year deflate-compressed JSONL
  - Streaming (`*.jsonl`): Legacy format

- **CLI Integration**:
  ```bash
  cargo xtask datagen --count 10 --ticks 365 --greedy-count 8
  # Generates training_data/run_{seed}.cpb.zip files
  ```

- **Schema**: `schemas/training.capnp` (228 lines)
  - Defines `TrainingSample`, `TrainingBatch`, `TrainingFile`
  - VisibleWorldState with 15+ fields
  - Command union type (~40 command variants)

**Performance**: 2.2-3.0 years/sec simulation speed (1444 start, observer mode)

---

#### 2. Observation Space

**Location**: `eu4sim-core/src/ai/mod.rs` (lines 50-179)

**VisibleWorldState** (15+ fields):
- Basic: `date`, `observer` (country tag), `own_country` (CountryState), `at_war`
- Enemies: `known_countries`, `enemy_provinces`, `known_country_strength`
- War: `our_war_score`, `current_war_enemy_strength`
- Military: `own_generals`, `armies_without_general`, `own_fleets`, `army_locations`
- Diplomatic: `own_ae` (Aggressive Expansion), `coalition_against_us`, `pending_call_to_arms`
- Siege: `fort_provinces`, `active_sieges`
- Infrastructure: `our_army_sizes`, `our_army_provinces`, `staging_provinces`

**CountryState** (44+ fields):
- Economics: `treasury`, `manpower`, `prestige`, `armyTradition`
- Mana: `adm_mana`, `dip_mana`, `mil_mana`
- Tech: `admTech`, `dipTech`, `milTech` (0-32)
- Status: `stability` (-3 to +3), `religion`, `embracedInstitutions`

**Serializable**: Everything is `Serialize/Deserialize` for training data.

---

#### 3. Action Space

**Location**: `eu4sim-core/src/input.rs` (lines 47-200+)

**Command** enum (~40 variants):
- **Military**: Move, MoveFleet, Embark, Disembark, MergeArmies, SplitArmy, RecruitGeneral, AssignGeneral
- **Diplomatic**: DeclareWar, OfferPeace, AcceptPeace, RejectPeace, JoinWar, CallAllyToWar, OfferAlliance, BreakAlliance, SetRival, RemoveRival, OfferRoyalMarriage
- **Economic**: BuildInProvince, CancelConstruction, DemolishBuilding, DevelopProvince, BuyTech, EmbraceInstitution
- **Colonization**: StartColony, AbandonColony
- **Ideas**: PickIdeaGroup, UnlockIdea
- **Estates**: GrantPrivilege, RevokePrivilege, SeizeLand
- **Religion**: AssignMissionary, RecallMissionary, ConvertCountryReligion, MoveCapital
- **Other**: Pass, Quit

**Action Selection**:
- `AvailableCommands` = Vec<Command> (computed per country per tick)
- Model outputs index into this vector
- Multi-action support: 1 diplomatic + N military + 1 economic + N trade per tick

---

#### 4. Model Loading & Inference

**Location**: `eu4sim-ai/src/`

- **llm_ai.rs** (431 lines): `LlmAi` struct implementing `AiPlayer` trait
  - Loads SmolLM2-360M, Gemma-2-2b, Gemma-3-270M from HuggingFace Hub
  - Merges LoRA adapters at load time (160 weight pairs)
  - Runs inference via Candle ML framework
  - CPU: 600-1000ms per decision, GPU: ~100-200ms

- **model.rs** (800+ lines): Model loading, tokenization, text generation
  - Supports multiple architectures: SmolLM2, Gemma2, Gemma3
  - SafeTensors weight loading
  - LoRA adapter merging
  - Up to 50 tokens for action outputs

- **prompt.rs** (347 lines): Prompt building
  - Converts `VisibleWorldState` → structured text prompt
  - Multi-action format with category grouping
  - Index mapping for parsing model outputs back to Commands
  - Top-K filtering per category (max 10 to prevent prompt explosion)

- **device.rs** (97 lines): CUDA/Metal/CPU device selection

**CLI Integration**: `eu4sim/src/main.rs`
```bash
cargo xtask llm gemma3 run1  # Fuzzy-match adapter in models/adapters/
```

---

#### 5. Supervised Training Pipeline

**Location**: `scripts/`

- **train_ai.py** (300+ lines): SFT using HuggingFace `trl.SFTTrainer`
  - Supports: Gemma-2-2b-it, Qwen2.5, SmolLM2-360M
  - LoRA config (r=16, alpha=32, target q_proj/v_proj)
  - Device detection (CUDA > DirectML > CPU)
  - Epoch-based training, learning rate scheduling

- **load_training_data.py** (400+ lines): Multi-format data loader
  - Cap'n Proto binary (native zero-copy)
  - Eager mode (full shuffle), chunked, streaming
  - Prefetch queue for CPU/GPU pipelining
  - Converts to HuggingFace Dataset

- **Colab Notebooks**:
  - `scripts/eu4_training_smollm.ipynb`: SmolLM2 fine-tuning
  - `scripts/eu4_training_gemma3.ipynb`: Gemma-3 fine-tuning

**Performance**:
- Model load: ~1.0s
- LoRA merge: 160 weight pairs, <1s
- Training speed: ~100-500 samples/sec (depends on GPU)

---

### Summary Table: What Exists

| Component | Status | Location | Notes |
|-----------|--------|----------|-------|
| **Base Model Loading** | ✅ Done | `eu4sim-ai/model.rs` | SmolLM2, Gemma-2/3 via HuggingFace |
| **LoRA Adapter Merging** | ✅ Done | `eu4sim-ai/model.rs:200+` | 160 weight pairs, CPU/GPU aware |
| **Inference Pipeline** | ✅ Done | `eu4sim-ai/llm_ai.rs` | Candle-based, 600-1000ms/decision |
| **Prompt Builder** | ✅ Done | `eu4sim-ai/prompt.rs` | Structured format, multi-action |
| **AiPlayer Integration** | ✅ Done | `eu4sim-core/ai/mod.rs` | Trait-based, deterministic |
| **Observation Space** | ✅ Designed | `eu4sim-core/ai/mod.rs:98-179` | VisibleWorldState (15+ fields) |
| **Action Space** | ✅ Designed | `eu4sim-core/input.rs` | 40+ command types |
| **Data Generation** | ✅ Done | `eu4sim-core/observer/datagen.rs` | Binary + JSON, streaming |
| **SFT Training** | ✅ Done | `scripts/train_ai.py` | HuggingFace trl/peft |
| **CLI Integration** | ✅ Done | `eu4sim/main.rs` | `--llm-ai` flag, TUI display |

**What This Gives You**: Behavior cloning from `GreedyAI`. The model learns to imitate the heuristic player.

---

## Missing Components for RL

### ❌ Critical Gaps

#### 1. Game Outcome Tracking

**Problem**: Simulations run, but no one tracks who won or what the final state was.

**What's Missing**:
```rust
// In eu4sim-core/src/state.rs or new scoring.rs
pub struct GameResult {
    pub winner: Option<Tag>,           // "FRA", "TUR", etc.
    pub country_scores: HashMap<Tag, GameScore>,
    pub total_ticks: u32,
    pub end_date: Date,
}

pub struct GameScore {
    pub provinces_owned: u32,
    pub total_development: Fixed,     // Sum of dev across provinces
    pub prestige: Fixed,
    pub mil_tech: u8,
    pub adm_tech: u8,
    pub dip_tech: u8,
    pub survived: bool,               // Not eliminated
    pub rank: u8,                     // 1 = winner, 2 = second, etc.
}
```

**Where to Add**:
- `eu4sim-core/src/scoring.rs` (new file): Implement scoring logic
- `eu4sim-core/src/step.rs`: Call `compute_game_result()` at end of simulation
- Store in `Episode` struct (see below)

**Why Needed**: RL algorithms optimize for reward, which comes from game outcomes.

---

#### 2. Reward Functions

**Problem**: No notion of "good" or "bad" outcomes. AI doesn't know if it's winning.

**What's Missing**:
```rust
// In eu4sim-core/src/reward.rs (new file)
pub trait RewardFunction {
    fn compute_reward(&self, game_result: &GameResult, country: &Tag) -> f32;
}

pub struct StandardReward;
impl RewardFunction for StandardReward {
    fn compute_reward(&self, result: &GameResult, country: &Tag) -> f32 {
        let score = &result.country_scores[country];

        let mut reward = 0.0;

        // Win bonus
        if result.winner == Some(*country) {
            reward += 100.0;
        }

        // Province control (0-50 points)
        reward += (score.provinces_owned as f32 / 100.0) * 50.0;

        // Development (0-30 points)
        reward += score.total_development.to_f32().min(3000.0) / 100.0;

        // Prestige (0-10 points)
        reward += score.prestige.to_f32().clamp(-100.0, 100.0) / 10.0;

        // Tech bonus (0-10 points)
        let avg_tech = (score.adm_tech + score.dip_tech + score.mil_tech) as f32 / 3.0;
        reward += avg_tech / 3.2;  // Max ~10 points at tech 32

        // Survival bonus
        if score.survived {
            reward += 20.0;
        } else {
            reward -= 50.0;  // Heavy penalty for elimination
        }

        reward
    }
}

// Personality variants (for multi-objective RL)
pub struct AggressiveReward;  // +bonus for provinces conquered via war
pub struct DiplomaticReward;  // +bonus for alliances and peaceful expansion
pub struct EconomicReward;    // +bonus for development and tech
pub struct SurvivalReward;    // Heavily penalize elimination, ignore everything else
```

**Design Considerations**:
- **Dense vs Sparse**: Standard reward is sparse (only at end). Could add per-tick shaping.
- **Personality Types**: Different reward functions → different playstyles
- **Reward Hacking**: Avoid exploitable metrics (e.g., don't reward mana spending directly)

#### 2.1 The Horizon Problem (Critical)

**Challenge**: A full game is ~146,000 ticks. With standard discount factor $\gamma=0.99$, rewards from the end of the game vanish before reaching the early game ($0.99^{1000} \approx 4 \times 10^{-5}$).
**Solution**:
- **Intermediate Rewards**: Compute rewards *yearly* or *monthly* based on delta-score.
- **Potential Based Shaping**: $R_t = \gamma \Phi(S_{t+1}) - \Phi(S_t)$ where $\Phi$ is the estimated value (e.g., Game Score).
- **Truncated Episodes**: Train on 10-50 year chunks rather than full games, using value function to bootstrap the end state.

---

#### 3. Episode/Trajectory Data

**Problem**: Current `TrainingSample` = single (state, action) pair. RL needs sequences.

**What's Missing**:
```rust
// In eu4sim-core/src/observer/datagen.rs
pub struct Episode {
    pub country: Tag,
    pub trajectory: Vec<TrajectoryStep>,
    pub final_reward: f32,
    pub game_result: GameResult,
}

pub struct TrajectoryStep {
    pub tick: u32,
    pub state: VisibleWorldState,
    pub actions_taken: Vec<Command>,
    pub next_state: VisibleWorldState,  // NEW: t+1 state
    pub reward: f32,                    // NEW: Could be 0 until end (sparse)
    pub done: bool,                     // Episode termination flag
}
```

**Cap'n Proto Schema** (`schemas/training.capnp`):
```capnp
struct Episode {
  country @0 :Text;
  trajectory @1 :List(TrajectoryStep);
  finalReward @2 :Float32;
  gameResult @3 :GameResult;
}

struct TrajectoryStep {
  tick @0 :UInt32;
  state @1 :VisibleWorldState;
  actionsTaken @2 :List(Command);
  nextState @3 :VisibleWorldState;  # NEW
  reward @4 :Float32;               # NEW
  done @5 :Bool;                    # NEW
}

struct GameResult {
  winner @0 :Text;  # Country tag or null
  countryScores @1 :List(CountryScore);
  totalTicks @2 :UInt32;
  endDate @3 :Date;
}

struct CountryScore {
  tag @0 :Text;
  provincesOwned @1 :UInt32;
  totalDevelopment @2 :Float32;
  prestige @3 :Float32;
  # ... etc
}
```

**Why Needed**: Policy gradient algorithms (PPO, GRPO) require full trajectories to compute:
- Returns: Discounted sum of rewards `G_t = r_t + γr_{t+1} + γ²r_{t+2} + ...`
- Advantages: `A_t = Q(s_t, a_t) - V(s_t)` (how much better than average)

---

#### 4. Policy Gradient Training Infrastructure

**Problem**: Only supervised learning exists. No RL training loop.

**What's Missing**: `scripts/train_rl.py` (new file)

```python
from trl import PPOTrainer, PPOConfig, AutoModelForCausalLMWithValueHead
from transformers import AutoTokenizer
import torch

class EU4RLTrainer:
    def __init__(self, base_model_name, reward_fn):
        self.tokenizer = AutoTokenizer.from_pretrained(base_model_name)

        # Actor-Critic: Policy network + Value head
        self.model = AutoModelForCausalLMWithValueHead.from_pretrained(base_model_name)

        self.reward_fn = reward_fn

        self.ppo_config = PPOConfig(
            learning_rate=1e-5,
            batch_size=32,
            mini_batch_size=4,
            gradient_accumulation_steps=8,
            ppo_epochs=4,
            max_grad_norm=0.5,
        )

        self.trainer = PPOTrainer(
            config=self.ppo_config,
            model=self.model,
            tokenizer=self.tokenizer,
        )

    def train_rl(self, episodes):
        """Train using PPO on collected episodes."""
        for episode in episodes:
            # 1. Extract trajectory
            states = [step.state for step in episode.trajectory]
            actions = [step.actions_taken for step in episode.trajectory]

            # 2. Compute returns (discounted rewards)
            returns = self._compute_returns(episode, gamma=0.99)

            # 3. Convert to model inputs
            queries = [self._state_to_prompt(s) for s in states]
            responses = [self._actions_to_text(a) for a in actions]

            # 4. PPO update
            stats = self.trainer.step(queries, responses, returns)

        return stats

    def _compute_returns(self, episode, gamma):
        """Compute discounted returns G_t."""
        returns = []
        G = 0
        for step in reversed(episode.trajectory):
            G = step.reward + gamma * G
            returns.insert(0, G)
        return torch.tensor(returns)
```

**HuggingFace TRL Library** provides:
- `PPOTrainer`: Proximal Policy Optimization
- `AutoModelForCausalLMWithValueHead`: LLM + value network
- `RewardTrainer`: Train separate reward model (optional)

**What You Need to Provide**:
- Episode data loader
- Reward function integration
- Prompt/response conversion functions
- **Action Masking**: Logic to mask invalid actions during generation (Critical for convergence).

---

#### 5. Self-Play Infrastructure (Optional but Recommended)

**Problem**: Training only against `GreedyAI` leads to overfitting to its weaknesses.

**What's Missing**:
- **Tournament System**: LLM vs LLM games, multiple model versions compete
- **ELO Ranking**: Track skill progression across model versions
- **Population-Based Training**: Multiple agents with different personalities

**Example Architecture**:
```rust
// In eu4sim/src/tournament.rs (new file)
pub struct Tournament {
    pub models: Vec<ModelVersion>,
    pub games_per_matchup: u32,
    pub elo_ratings: HashMap<String, f64>,
}

pub struct ModelVersion {
    pub name: String,           // "gemma3_rl_v1", "gemma3_rl_v2", etc.
    pub adapter_path: PathBuf,
    pub elo: f64,
}

impl Tournament {
    pub fn run_matchup(&mut self, model_a: &str, model_b: &str) -> Vec<GameResult> {
        // Run N games with random country assignments
        // Update ELO ratings based on results
    }

    pub fn run_tournament(&mut self) -> TournamentReport {
        // Round-robin or Swiss-system tournament
        // Generate rankings and statistics
    }
}
```

**Why It Helps**:
- Prevents overfitting to specific opponents
- Enables co-evolution (models improve together)
- Diverse training data (different strategies emerge)

---

### Summary: Missing Components

| Component | Supervised | Offline RL | Online RL |
|-----------|-----------|-----------|-----------|
| SFT Training | ✅ Have | ✅ Have | ❌ Skip |
| Game Scoring | ❌ N/A | ❌ **NEED** | ❌ **NEED** |
| Reward Function | ❌ N/A | ❌ **NEED** | ❌ **NEED** |
| Episode Tracking | ❌ N/A | ❌ **NEED** | ❌ **NEED** |
| Policy Gradient (PPO) | ❌ N/A | ❌ **NEED** | ❌ **NEED** |
| Self-Play | ❌ N/A | ⚠️ Optional | ❌ **NEED** |

---

## Supervised vs. RL Approaches

### Approach 1: Supervised Learning (Current)

```
GreedyAI plays games → Generate (state, action) pairs → SFT train LLM
```

**Pros**:
- ✅ Works today (fully implemented)
- ✅ Fast to train (just SFT, no environment interaction)
- ✅ No reward engineering needed
- ✅ Stable training (no exploration noise)

**Cons**:
- ❌ **Ceiling = GreedyAI skill** (can't surpass teacher)
- ❌ No exploration (only imitates known strategies)
- ❌ Brittle (fails on states teacher never encountered)
- ❌ No credit assignment (can't distinguish good/bad decisions)

**When to Use**: Bootstrapping. Get a baseline working model quickly.

---

### Approach 2: Offline RL (Hybrid)

```
1. SFT train on GreedyAI data (bootstrap)
2. Generate trajectories with SFT model
3. Label with game outcomes (win/loss/score)
4. RL fine-tune to maximize rewards
```

**Pros**:
- ✅ Starts from good baseline (SFT model)
- ✅ Can surpass teacher (optimizes for wins, not imitation)
- ✅ Uses existing datagen infrastructure
- ✅ Safer than online RL (less exploration risk)

**Cons**:
- ❌ Needs reward function (outcome labeling)
- ❌ Needs episode tracking (trajectories)
- ❌ Limited by offline data distribution (can't explore much)

**When to Use**: **Recommended first step** after SFT. Incremental improvement.

**Algorithm**: PPO or GRPO (Group Relative Policy Optimization)

---

### Approach 3: Online RL (Pure Unsupervised)

```
1. LLM plays games against itself
2. Compute rewards from outcomes
3. Update policy via PPO/GRPO
4. Repeat (no human data, self-play)
```

**Pros**:
- ✅ No teacher needed (fully autonomous)
- ✅ Self-improves via exploration
- ✅ Discovers novel strategies (beyond human knowledge)
- ✅ Unlimited data (generate as needed)

**Cons**:
- ❌ **Very slow** (100K+ games to converge)
- ❌ Needs self-play infrastructure (tournament system)
- ❌ **Unstable training** (reward hacking, mode collapse)
- ❌ Expensive (lots of GPU hours)

**When to Use**: After offline RL plateaus. For discovering superhuman strategies.

**Algorithm**: PPO + self-play (like AlphaGo/OpenAI Five)

---

### Comparison Table

| Aspect | Supervised | Offline RL | Online RL |
|--------|-----------|-----------|-----------|
| **Data Source** | GreedyAI | SFT model | Self-play |
| **Skill Ceiling** | GreedyAI level | Above teacher | Unlimited |
| **Training Time** | Hours | Days | Weeks |
| **Stability** | Very stable | Moderate | Unstable |
| **Exploration** | None | Limited | Full |
| **Infrastructure** | ✅ Done | ⚠️ 70% done | ❌ 30% done |
| **Sample Efficiency** | High | Medium | Low |
| **Use Case** | Bootstrap | Improve | Optimize |

---

## Recommended Path: Offline RL

### Why Offline RL First?

1. **Reuse existing infrastructure**: Datagen command, SFT models, Cap'n Proto schema
2. **Lower risk**: Starts from working baseline, less likely to collapse
3. **Incremental**: Add reward function → episode tracking → RL training (one at a time)
4. **Faster iteration**: Don't need self-play for initial experiments

### High-Level Plan

```
Phase 1 (Done): SFT baseline
   ↓
Phase 2: Offline RL
   ├─ Add game scoring
   ├─ Track episodes
   ├─ Implement PPO trainer
   └─ Fine-tune on outcomes
   ↓
Phase 3: Online RL (optional)
   ├─ Self-play infrastructure
   ├─ Tournament system
   └─ ELO ranking
```

---

## Implementation Roadmap

### Step 1: Game Scoring System (1-2 days)

**Goal**: Track game outcomes so we can compute rewards.

**Tasks**:

1. **Create `eu4sim-core/src/scoring.rs`**:
   ```rust
   pub fn compute_game_score(state: &WorldState, country: &Tag) -> GameScore {
       let provinces_owned = state.provinces.values()
           .filter(|p| p.owner.as_deref() == Some(country))
           .count() as u32;

       let total_development = state.provinces.values()
           .filter(|p| p.owner.as_deref() == Some(country))
           .map(|p| p.base_tax + p.base_production + p.base_manpower)
           .sum::<Fixed>();

       let country_state = &state.countries[country];

       GameScore {
           provinces_owned,
           total_development,
           prestige: country_state.prestige,
           mil_tech: country_state.milTech,
           adm_tech: country_state.admTech,
           dip_tech: country_state.dipTech,
           survived: true,  // If we're computing score, country exists
           rank: 0,  // Compute later via sorting
       }
   }

   pub fn compute_game_result(state: &WorldState) -> GameResult {
       let mut scores: Vec<(Tag, GameScore)> = state.countries.keys()
           .map(|tag| (tag.clone(), compute_game_score(state, tag)))
           .collect();

       // Sort by total score (weighted sum)
       scores.sort_by(|a, b| {
           let score_a = a.1.total_score();
           let score_b = b.1.total_score();
           score_b.partial_cmp(&score_a).unwrap()
       });

       // Assign ranks
       for (rank, (tag, score)) in scores.iter_mut().enumerate() {
           score.rank = (rank + 1) as u8;
       }

       GameResult {
           winner: scores.first().map(|(tag, _)| tag.clone()),
           country_scores: scores.into_iter().collect(),
           total_ticks: state.tick,
           end_date: state.date.clone(),
       }
   }
   ```

2. **Update `eu4sim/src/main.rs`**:
   - Call `compute_game_result()` when simulation ends
   - Log the result to console/file

3. **Test**:
   ```bash
   cargo run -p eu4sim -- --observer --ticks 365
   # Should print: "Winner: FRA (score: 156.2)"
   ```

**Deliverable**: Working game scoring, visible in CLI output.

---

### Step 2: Reward Functions (1 day)

**Goal**: Convert game outcomes into scalar rewards.

**Tasks**:

1. **Create `eu4sim-core/src/reward.rs`**:
   ```rust
   pub trait RewardFunction: Send + Sync {
       fn compute_reward(&self, result: &GameResult, country: &Tag) -> f32;
   }

   pub struct StandardReward {
       pub win_bonus: f32,
       pub province_weight: f32,
       pub dev_weight: f32,
       pub prestige_weight: f32,
       pub survival_bonus: f32,
       pub elimination_penalty: f32,
   }

   impl Default for StandardReward {
       fn default() -> Self {
           Self {
               win_bonus: 100.0,
               province_weight: 0.5,
               dev_weight: 0.01,
               prestige_weight: 0.1,
               survival_bonus: 20.0,
               elimination_penalty: -50.0,
           }
       }
   }

   impl RewardFunction for StandardReward {
       fn compute_reward(&self, result: &GameResult, country: &Tag) -> f32 {
           let score = &result.country_scores[country];

           let mut reward = 0.0;

           if result.winner == Some(*country) {
               reward += self.win_bonus;
           }

           reward += score.provinces_owned as f32 * self.province_weight;
           reward += score.total_development.to_f32() * self.dev_weight;
           reward += score.prestige.to_f32() * self.prestige_weight;

           if score.survived {
               reward += self.survival_bonus;
           } else {
               reward += self.elimination_penalty;
           }

           reward
       }
   }
   ```

2. **Add personality variants**:
   ```rust
   pub struct AggressiveReward;
   pub struct DiplomaticReward;
   pub struct EconomicReward;
   // Implement each with different weight profiles
   ```

3. **Test**:
   ```bash
   cargo test -p eu4sim-core reward
   ```

**Deliverable**: Multiple reward functions, unit tested.

---

### Step 3: Episode Tracking (2-3 days)

**Goal**: Store full trajectories instead of individual samples.

**Tasks**:

1. **Update `schemas/training.capnp`**:
   ```capnp
   struct Episode {
     country @0 :Text;
     trajectory @1 :List(TrajectoryStep);
     finalReward @2 :Float32;
     gameResult @3 :GameResult;
   }

   struct TrajectoryStep {
     tick @0 :UInt32;
     state @1 :VisibleWorldState;
     actionsTaken @2 :List(Command);
     nextState @3 :VisibleWorldState;
     reward @4 :Float32;
     done @5 :Bool;
   }
   ```

2. **Update `eu4sim-core/src/observer/datagen.rs`**:
   ```rust
   pub struct EpisodeWriter {
       episodes: HashMap<Tag, Episode>,
       reward_fn: Box<dyn RewardFunction>,
   }

   impl Observer for EpisodeWriter {
       fn on_tick(&mut self, tick: u32, state: &WorldState, actions: &HashMap<Tag, Vec<Command>>) {
           for (country, cmds) in actions {
               let episode = self.episodes.entry(country.clone()).or_insert_with(|| {
                   Episode::new(country.clone())
               });

               episode.trajectory.push(TrajectoryStep {
                   tick,
                   state: visible_state_for(state, country),
                   actions_taken: cmds.clone(),
                   next_state: VisibleWorldState::default(),  // Filled on next tick
                   reward: 0.0,  // Sparse: only at end
                   done: false,
               });
           }
       }

       fn on_game_end(&mut self, result: &GameResult) {
           for (country, episode) in &mut self.episodes {
               episode.final_reward = self.reward_fn.compute_reward(result, country);
               episode.game_result = result.clone();

               if let Some(last_step) = episode.trajectory.last_mut() {
                   last_step.reward = episode.final_reward;
                   last_step.done = true;
               }
           }
       }
   }
   ```

3. **Update datagen CLI**:
   ```bash
   cargo xtask datagen --count 10 --format episodes  # New format
   # Outputs: training_data/episode_{seed}.cpb.zip
   ```

**Deliverable**: Episode data files with full trajectories.

---

### Step 4: Python RL Trainer (3-5 days)

**Goal**: Implement PPO training on episodes.

**Tasks**:

1. **Create `scripts/load_episodes.py`**:
   ```python
   import capnp
   from pathlib import Path

   training_capnp = capnp.load('../schemas/training.capnp')

   def load_episodes(path: Path):
       with open(path, 'rb') as f:
           episode_data = training_capnp.Episode.read(f)

       episodes = []
       for ep in episode_data:
           episodes.append({
               'country': ep.country,
               'trajectory': [
                   {
                       'state': step.state,
                       'actions': step.actionsTaken,
                       'next_state': step.nextState,
                       'reward': step.reward,
                       'done': step.done,
                   }
                   for step in ep.trajectory
               ],
               'final_reward': ep.finalReward,
           })
       return episodes
   ```

2. **Create `scripts/train_rl.py`**:
   ```python
   from trl import PPOTrainer, PPOConfig, AutoModelForCausalLMWithValueHead
   from load_episodes import load_episodes

   def main():
       # 1. Load SFT model as starting point
       model = AutoModelForCausalLMWithValueHead.from_pretrained("models/gemma3_sft")

       # 2. Load episode data
       episodes = load_episodes("training_data/episode_*.cpb.zip")

       # 3. Configure PPO
       config = PPOConfig(
           learning_rate=1e-5,
           batch_size=32,
           ppo_epochs=4,
       )

       # 4. Train
       trainer = PPOTrainer(config=config, model=model)
       trainer.train(episodes)

       # 5. Save RL-tuned model
       model.save_pretrained("models/gemma3_rl_v1")
   ```

3. **Add evaluation script**:
   ```bash
   # scripts/evaluate_rl.py
   # Compare SFT vs RL models in head-to-head games
   ```

**Deliverable**: Working RL training pipeline.

---

### Step 5: Evaluation & Iteration (1-2 days)

**Goal**: Verify RL model beats SFT baseline.

**Tasks**:

1. **Run comparison**:
   ```bash
   # Train RL model
   python scripts/train_rl.py --episodes training_data/episode_*.cpb.zip

   # Evaluate
   cargo xtask llm gemma3 sft     # SFT baseline
   cargo xtask llm gemma3 rl_v1   # RL-tuned
   ```

2. **Measure win rate**:
   ```bash
   # Run 20 games SFT vs RL
   python scripts/tournament.py --model-a sft --model-b rl_v1 --games 20
   # Expected: RL wins 60-70% (if training worked)
   ```

3. **Tune reward function**:
   - If RL performs worse: Check reward function for exploits
   - If RL performs similarly: Increase training iterations
   - If RL performs better: Success! Move to self-play

**Deliverable**: RL model that beats SFT in head-to-head games.

---

## Technical Specifications

### Data Schema Evolution

**Cap'n Proto Version**:
- Current: `training.capnp` version 0
- For RL: Add `Episode`, `TrajectoryStep`, `GameResult` structs
- Backward compatible: Old `TrainingSample` still works for SFT

- SFT: `training_data/run_{seed}.cpb.zip` (TrainingSample batches)
- RL: `training_data/episode_{seed}.cpb.zip` (Episode data)

---

### Action Masking Strategy (Critical)

**Problem**: The LLM vocab size is ~32k-256k. valid actions are < 100. PPO will waste time exploring "invalid" tokens if not masked.

**Implementation**:
1. **During Rollout/Inference**:
   - Use `AvailableCommands` from state.
   - Map `Command` variants to specific Tokens in the prompt schema.
   - Set logits of all other tokens to $-\infty$.

2. **During Training (PPO Step)**:
   - Pass `action_masks` tensor to the model.
   - Ensure the KL-divergence penalty accounts for the mask (don't penalize for 0 probability on invalid actions).

---

### Reward Function Design

**Principles**:
1. **Aligned with game objectives**: Winning should give highest reward
2. **Dense when possible**: Intermediate milestones help learning
3. **Normalized**: Keep rewards in [0, 200] range for stability
4. **Non-exploitable**: Avoid rewarding actions that game the metric

**Example Dense Rewards** (per-tick shaping):
```rust
pub struct DenseReward;
impl RewardFunction for DenseReward {
    fn compute_step_reward(&self, state: &WorldState, country: &Tag) -> f32 {
        let score = compute_game_score(state, country);

        // Reward incremental progress
        let mut reward = 0.0;
        reward += score.provinces_owned as f32 * 0.01;  // +0.01 per province
        reward += score.total_development.to_f32() * 0.001;  // Small dev bonus

        // Penalty for being at war (encourages peace)
        if state.at_war(country) {
            reward -= 0.1;
        }

        reward
    }
}
```

**Caution**: Dense rewards can lead to suboptimal policies (e.g., avoiding all wars). Start sparse, add shaping only if needed.

---

### RL Algorithm Recommendations

**For Offline RL**:
- **PPO (Proximal Policy Optimization)**: Standard, stable, well-supported by TRL
- **GRPO (Group Relative Policy Optimization)**: Similar to PPO but uses group statistics
- **Conservative Q-Learning (CQL)**: More conservative, good for offline data

**For Online RL (Self-Play)**:
- **PPO + Self-Play**: Like AlphaGo Zero
- **League Training**: Maintain population of past versions, train against all

**Hyperparameters** (starting point):
```python
PPOConfig(
    learning_rate=1e-5,        # Lower than SFT (fine-tuning)
    batch_size=32,             # Depends on GPU memory
    mini_batch_size=4,
    ppo_epochs=4,              # Multiple passes over batch
    gamma=0.99,                # Discount factor (important for long games)
    gae_lambda=0.95,           # GAE parameter
    clip_range=0.2,            # PPO clip range
    vf_coef=0.5,               # Value loss coefficient
    max_grad_norm=0.5,         # Gradient clipping
)
```

---

### Self-Play Architecture (Future)

**Tournament System**:
```rust
// In eu4sim/src/tournament.rs
pub struct SwissTournament {
    pub models: Vec<ModelVersion>,
    pub rounds: u32,
    pub games_per_round: u32,
}

impl SwissTournament {
    pub fn run(&mut self) -> TournamentReport {
        for round in 0..self.rounds {
            let pairings = self.swiss_pairings();
            for (model_a, model_b) in pairings {
                let results = self.run_games(model_a, model_b);
                self.update_elo(model_a, model_b, results);
            }
        }
        self.generate_report()
    }
}
```

**ELO Rating**:
```rust
pub fn update_elo(player_a: &mut f64, player_b: &mut f64, score: f32) {
    let k = 32.0;  // ELO K-factor
    let expected_a = 1.0 / (1.0 + 10f64.powf((*player_b - *player_a) / 400.0));
    *player_a += k * (score as f64 - expected_a);
    *player_b += k * ((1.0 - score) as f64 - (1.0 - expected_a));
}
```

---

## References

### Related Documents
- [`learned-ai.md`](./learned-ai.md): Overall architecture (Phases 1-3)
- [`training-data-format.md`](../data/training-data-format.md): Cap'n Proto schema details
- [`learned-ai-musings.md`](./learned-ai-musings.md): Design philosophy

### External Resources
- **HuggingFace TRL**: https://huggingface.co/docs/trl/
- **PPO Paper**: https://arxiv.org/abs/1707.06347
- **GRPO**: Group Relative Policy Optimization
- **AlphaGo Zero**: https://www.nature.com/articles/nature24270
- **OpenAI Five**: https://openai.com/research/openai-five

### Key Files

**Rust Crates**:
1. `eu4sim-ai/src/llm_ai.rs` (431 lines): LLM inference integration
2. `eu4sim-core/src/ai/mod.rs` (250+ lines): AiPlayer trait, VisibleWorldState
3. `eu4sim-core/src/observer/datagen.rs` (500+ lines): Training data generation
4. `eu4sim-core/src/input.rs` (200+ lines): Command enum (action space)
5. `eu4sim-core/src/state.rs`: WorldState, CountryState structures

**Python Scripts**:
1. `scripts/train_ai.py` (300+ lines): SFT training
2. `scripts/load_training_data.py` (400+ lines): Data loading
3. `scripts/eu4_training_*.ipynb`: Colab notebooks

**Schemas**:
1. `schemas/training.capnp` (228 lines): Cap'n Proto schema

---

## Appendix: Quick Start Checklist

### To Enable Offline RL

- [ ] **Step 1**: Implement `compute_game_score()` in `eu4sim-core/src/scoring.rs`
- [ ] **Step 2**: Add `RewardFunction` trait in `eu4sim-core/src/reward.rs`
- [ ] **Step 3**: Update `schemas/training.capnp` with `Episode` and `TrajectoryStep`
- [ ] **Step 4**: Modify `datagen.rs` to track episodes instead of samples
- [ ] **Step 5**: Implement `scripts/load_episodes.py` for Python data loading
- [ ] **Step 6**: Implement `scripts/train_rl.py` using HuggingFace TRL
- [ ] **Step 7**: Run evaluation: SFT vs RL model comparison

### Expected Timeline

- **Game Scoring**: 1-2 days
- **Reward Functions**: 1 day
- **Episode Tracking**: 2-3 days
- **RL Trainer**: 3-5 days
- **Evaluation**: 1-2 days

**Total**: ~2 weeks for basic offline RL pipeline.

---

**Last Updated**: 2026-01-04
**Author**: Claude (Sonnet 4.5)
**Status**: Phase 3 design document - implementation not started
