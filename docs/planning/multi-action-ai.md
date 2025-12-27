# Multi-Action LLM AI

> **Status**: Design Complete (2025-12-24)

## Goal

Enable LLM AI to return multiple commands per tick, like GreedyAI and RandomAI already do.

## Current State

**Infrastructure is ready**:
- `PlayerInputs::commands: Vec<Command>` - accepts multiple commands
- `step_world()` processes all commands in order
- GreedyAI/RandomAI return 5-20+ commands per tick

**Bottleneck**: LlmAI hardcoded to return 1 command (line 162-192 in `llm_ai.rs`)

## Chosen Approach: Structured Multi-Output

Since training hasn't started yet, we can design the output format from scratch.

## Prompt Format

```
=== Available Actions by Category ===

DIPLOMATIC:
  0: Pass
  1: Declare war on MNG (Ming)
  2: Offer alliance to JAP

MILITARY:
  0: Pass
  1: Move Army #1 from Seoul to Pyongyang
  2: Move Army #2 from Busan to Ulsan
  3: Recruit infantry in Seoul

ECONOMIC:
  0: Pass
  1: Develop Seoul (admin)
  2: Build marketplace in Seoul

TRADE:
  0: Pass
  1: Send merchant to Beijing (collect)

=== Your Response ===
Choose one action per category. Format: CATEGORY:INDEX
Multiple indices allowed for MILITARY (comma-separated).

DIPLOMATIC:
MILITARY:
ECONOMIC:
TRADE:
```

## Model Output

```
DIPLOMATIC:0
MILITARY:1,2
ECONOMIC:1
TRADE:0
```

This selects: Pass on diplomacy, Move both armies, Develop Seoul, Pass on trade.

## Implementation

### Phase 1: Command Categorization

```rust
// eu4sim-core/src/input.rs

#[derive(Clone, Copy, PartialEq, Eq, Hash, Display)]
pub enum CommandCategory {
    Diplomatic,   // DeclareWar, OfferPeace, alliances
    Military,     // Move, Recruit, AssignGeneral
    Economic,     // Build, Develop, Core
    Trade,        // SendMerchant, RecallMerchant
    Colonization, // StartColony, AbandonColony
    Religion,     // AssignMissionary, Convert
}

impl Command {
    pub fn category(&self) -> CommandCategory {
        match self {
            Command::DeclareWar { .. } |
            Command::OfferPeace { .. } |
            Command::AcceptPeace { .. } => CommandCategory::Diplomatic,

            Command::Move { .. } |
            Command::RecruitRegiment { .. } |
            Command::AssignGeneral { .. } => CommandCategory::Military,

            Command::BuildInProvince { .. } |
            Command::DevelopProvince { .. } |
            Command::Core { .. } => CommandCategory::Economic,

            Command::SendMerchant { .. } |
            Command::RecallMerchant { .. } => CommandCategory::Trade,

            Command::StartColony { .. } |
            Command::AbandonColony { .. } => CommandCategory::Colonization,

            Command::AssignMissionary { .. } |
            Command::RecallMissionary { .. } => CommandCategory::Religion,

            Command::Pass | Command::Quit => CommandCategory::Diplomatic,
        }
    }
}
```

### Phase 2: Structured Prompt Builder

```rust
// eu4sim-ai/src/llm_ai.rs

fn build_structured_prompt(
    available: &[Command],
    state: &VisibleWorldState,
) -> String {
    let mut prompt = String::new();

    // Group by category
    let by_category = available.iter()
        .enumerate()
        .into_group_map_by(|(_, c)| c.category());

    prompt.push_str("=== Available Actions by Category ===\n\n");

    for cat in CommandCategory::all() {
        if let Some(commands) = by_category.get(&cat) {
            prompt.push_str(&format!("{}:\n", cat));
            prompt.push_str("  0: Pass\n");
            for (idx, cmd) in commands {
                prompt.push_str(&format!("  {}: {}\n", idx + 1, cmd.display(state)));
            }
            prompt.push_str("\n");
        }
    }

    prompt.push_str("=== Your Response ===\n");
    prompt.push_str("Choose actions per category. Format: CATEGORY:INDEX\n");
    prompt.push_str("Multiple indices allowed for MILITARY (comma-separated).\n\n");

    for cat in CommandCategory::all() {
        if by_category.contains_key(&cat) {
            prompt.push_str(&format!("{}:\n", cat));
        }
    }

    prompt
}
```

### Phase 3: Response Parser

```rust
fn parse_structured_response(
    response: &str,
    available: &[Command],
) -> Vec<Command> {
    let mut result = Vec::new();

    for line in response.lines() {
        if let Some((cat_str, indices_str)) = line.split_once(':') {
            let Ok(category) = CommandCategory::from_str(cat_str.trim()) else {
                continue;
            };

            let indices: Vec<usize> = indices_str
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();

            for idx in indices {
                if idx == 0 { continue; } // 0 = pass
                // Map back to original index (1-indexed per category)
                if let Some(cmd) = find_command_by_category_index(available, category, idx) {
                    result.push(cmd.clone());
                }
            }
        }
    }

    result
}
```

### Phase 4: Training Data Format

```json
{
    "state": { "...": "..." },
    "available_by_category": {
        "DIPLOMATIC": ["Pass", "Declare war on MNG"],
        "MILITARY": ["Pass", "Move Army #1 Seoul->Pyongyang", "Move Army #2"],
        "ECONOMIC": ["Pass", "Develop Seoul"],
        "TRADE": ["Pass"]
    },
    "chosen": {
        "DIPLOMATIC": 0,
        "MILITARY": [1, 2],
        "ECONOMIC": 1,
        "TRADE": 0
    }
}
```

## Files to Modify

| File | Changes |
|------|---------|
| `eu4sim-core/src/input.rs` | Add `CommandCategory`, `Command::category()` |
| `eu4sim-ai/src/llm_ai.rs` | Structured prompt builder + response parser |
| `eu4sim-ai/src/observer.rs` | Update training data format |
| `scripts/training/` | Update model for structured output |

## Token Efficiency

| Metric | Before | After |
|--------|--------|-------|
| Prompt size | 200 tokens | 300 tokens |
| Actions per query | 1 | 5+ |
| Tokens per action | 200 | 60 |

**Net efficiency gain**: ~3x better tokens-per-action ratio.

## Success Criteria

- [ ] `CommandCategory` enum with `all()` iterator
- [ ] `Command::category()` categorizes all 30+ command variants
- [ ] Prompt groups options by category with Pass=0
- [ ] Parser handles multi-index responses (e.g., `MILITARY:1,2,3`)
- [ ] Training data includes category groupings
- [ ] LlmAI returns `Vec<Command>` with multiple items

## Design Decisions

| Decision | Rationale |
|----------|-----------|
| Category-based grouping | Mirrors how players think (army moves, then economy) |
| 0 = Pass per category | Explicit pass is clearer than empty |
| Multi-select for Military only | Armies often move together, other categories usually single-action |
| Train from scratch | No existing model to migrate, clean slate |

## Alternatives Considered

1. **Sequential queries** (one per category): 3-5x token cost, high latency
2. **Hybrid with GreedyAI**: LLM for diplomacy, heuristics for rest - limits learning
3. **Autoregressive sequence**: Complex model architecture change

Structured multi-output is the sweet spot: single query, multiple actions, trainable.
