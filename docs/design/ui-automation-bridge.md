# UI Automation Bridge: Real EU4 Integration

**Status**: Design Phase
**Goal**: Enable trained AI (LlmAi with LoRA adapters) to play real Europa Universalis IV via UI automation
**Last updated**: 2025-12-21

---

## Overview

This document describes the architecture for a Rust bridge that connects a trained AI (expecting `VisibleWorldState`, outputting `Command`) to the real EU4 game via screen capture and input automation.

**Why This Matters**:
- **Transfer learning validation**: Proves AI trained in eu4sim can generalize to real game
- **Novel achievement**: First LLM to play grand strategy competently
- **Practical utility**: AI advisor overlay for human players
- **Academic interest**: Complex multi-agent long-horizon planning in real environment

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Real EU4 Game (Windows)                      │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Game UI (1920x1080)                                      │   │
│  │  - Province map                                          │   │
│  │  - Country panel (treasury, manpower, mana)              │   │
│  │  - Military panel (armies, wars)                         │   │
│  │  - Outliner (country list)                               │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
         ↓ pixels (xcap)                    ↑ clicks/keys (enigo)
┌─────────────────────────────────────────────────────────────────┐
│                    eu4-bridge (Rust Binary)                      │
│  ┌─────────────────┐              ┌─────────────────────────┐   │
│  │ State Extractor │              │ Action Translator       │   │
│  │  - OCR numbers  │              │  - Click sequences      │   │
│  │  - Template     │              │  - Hotkey macros        │   │
│  │    matching     │              │  - Console commands     │   │
│  │  - Region crops │              │  - Wait/retry logic     │   │
│  └─────────────────┘              └─────────────────────────┘   │
│         ↓                                      ↑                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │              Orchestrator (Main Loop)                    │   │
│  │  1. Pause game (Space)                                   │   │
│  │  2. Capture screen → VisibleWorldState                   │   │
│  │  3. Call AI inference (LlmAi via HTTP or in-process)     │   │
│  │  4. Translate Command → UI actions                       │   │
│  │  5. Execute actions                                      │   │
│  │  6. Unpause (Space), wait for tick                       │   │
│  │  7. Repeat                                               │   │
│  └──────────────────────────────────────────────────────────┘   │
│         ↓                                      ↑                 │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │    AI Inference Client                                  │    │
│  │  - HTTP → eu4sim-ai server (Candle + LoRA)              │    │
│  │  - Or: in-process via eu4sim-ai crate                   │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
         ↓ VisibleWorldState          ↑ Command
┌─────────────────────────────────────────────────────────────────┐
│                  Trained AI (LlmAi)                              │
│  - SmolLM2-360M base model                                       │
│  - LoRA adapter (balanced/aggressive/diplomatic/etc.)            │
│  - Inputs: Prompt (state + available actions)                   │
│  - Outputs: Action index (0-9)                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Type Mapping

### SimState → VisibleWorldState

The AI expects this structure (from `eu4sim-core/src/ai/mod.rs:62`):

```rust
pub struct VisibleWorldState {
    pub date: Date,                              // Year/month/day
    pub observer: Tag,                           // Playing as which country
    pub own_country: CountryState,               // Treasury, manpower, mana, tech, etc.
    pub at_war: bool,                            // War status
    pub known_countries: Vec<Tag>,               // Visible nations
    pub enemy_provinces: HashSet<ProvinceId>,    // Enemy territory
    pub known_country_strength: HashMap<Tag, u32>, // Military power
    pub our_war_score: HashMap<WarId, Fixed>,    // Active wars
}

pub struct CountryState {
    pub treasury: Fixed,           // Ducats
    pub manpower: Fixed,           // Manpower pool
    pub stability: BoundedInt,     // -3 to +3
    pub prestige: BoundedFixed,    // -100 to +100
    pub army_tradition: BoundedFixed, // 0 to 100
    pub adm_mana: Fixed,           // Admin points
    pub dip_mana: Fixed,           // Diplo points
    pub mil_mana: Fixed,           // Military points
    pub adm_tech: u8,              // Tech level 0-32
    pub dip_tech: u8,
    pub mil_tech: u8,
    pub embraced_institutions: HashSet<InstitutionId>,
    pub religion: Option<String>,
}
```

**Extraction Strategy**: See [UI → VisibleWorldState Mapping](#ui--visibleworldstate-mapping) below.

### SimAction → Command

The AI outputs one of these commands (from `eu4sim-core/src/input.rs:35`):

```rust
pub enum Command {
    // Economic
    DevelopProvince { province: ProvinceId, dev_type: DevType },
    BuildInProvince { province: ProvinceId, building: String },

    // Military
    Move { army_id: ArmyId, destination: ProvinceId },
    MoveFleet { fleet_id: FleetId, destination: ProvinceId },
    Embark { army_id: ArmyId, fleet_id: FleetId },
    Disembark { army_id: ArmyId, destination: ProvinceId },

    // Diplomatic - War
    DeclareWar { target: Tag, cb: Option<String> },
    OfferPeace { war_id: WarId, terms: PeaceTerms },
    AcceptPeace { war_id: WarId },
    RejectPeace { war_id: WarId },

    // Tech & Institutions
    BuyTech { tech_type: TechType },
    EmbraceInstitution { institution: InstitutionId },

    // Colonization
    StartColony { province: ProvinceId },
    AbandonColony { province: ProvinceId },

    // Diplomacy - Outgoing
    OfferAlliance { target: Tag },
    BreakAlliance { target: Tag },
    SetRival { target: Tag },
    // ... (see input.rs for full list)

    // Control
    Pass,
    Quit,
}
```

**Translation Strategy**: See [Command → UI Actions Mapping](#command--ui-actions-mapping) below.

---

## Module Structure

```
eu4-bridge/
├── Cargo.toml
├── src/
│   ├── main.rs              # Orchestrator main loop
│   ├── capture/
│   │   ├── mod.rs           # Window capture abstraction
│   │   └── xcap_impl.rs     # xcap implementation
│   ├── extraction/
│   │   ├── mod.rs           # UI → VisibleWorldState
│   │   ├── ocr.rs           # Text extraction (tesseract)
│   │   ├── templates.rs     # Image template matching
│   │   ├── regions.rs       # UI region definitions (coords)
│   │   ├── date.rs          # Extract date from top bar
│   │   ├── country.rs       # Extract CountryState from panels
│   │   ├── military.rs      # Extract army/war info
│   │   └── map.rs           # Province ownership (color matching)
│   ├── translation/
│   │   ├── mod.rs           # Command → UI actions
│   │   ├── military.rs      # Move, DeclareWar, OfferPeace
│   │   ├── economy.rs       # DevelopProvince, BuyTech
│   │   ├── diplomacy.rs     # Alliances, Rivals
│   │   ├── console.rs       # Console command fallback
│   │   └── macros.rs        # Common UI patterns (open panel, click button)
│   ├── input/
│   │   ├── mod.rs           # Input abstraction
│   │   └── enigo_impl.rs    # enigo mouse/keyboard
│   ├── ai/
│   │   ├── mod.rs           # AI client abstraction
│   │   ├── http_client.rs   # HTTP → eu4sim-ai server
│   │   └── local.rs         # In-process eu4sim-ai
│   ├── types/
│   │   └── mod.rs           # Re-export VisibleWorldState, Command from eu4sim-core
│   └── config.rs            # UI layout config (resolution, coords)
└── README.md
```

---

## UI → VisibleWorldState Mapping

### Extraction Regions (1920x1080 assumed)

| Field | UI Location | Extraction Method |
|-------|-------------|-------------------|
| **date** | Top bar (center) | OCR `1444.11.11` → `Date::new(1444, 11, 11)` |
| **observer** | Top-left flag + tooltip | Template match flag, OCR country name |
| **treasury** | Top bar (left side, gold icon) | OCR digits after gold icon |
| **manpower** | Top bar (crossed swords icon) | OCR digits after manpower icon |
| **adm_mana** | Top bar (ADM icon, red) | OCR digits |
| **dip_mana** | Top bar (DIP icon, green) | OCR digits |
| **mil_mana** | Top bar (MIL icon, blue) | OCR digits |
| **stability** | Country panel → Stability tab | OCR `-3` to `+3` |
| **prestige** | Country panel → Prestige | OCR number |
| **army_tradition** | Military panel → Army Tradition | OCR percentage |
| **adm_tech** | Tech panel (opened via hotkey) | OCR level number |
| **dip_tech** | Tech panel | OCR level number |
| **mil_tech** | Tech panel | OCR level number |
| **at_war** | Outliner or top bar | Check for "At war" text or red war icon |
| **known_countries** | Outliner (country list) | OCR country tags, or: scrape from save file |
| **enemy_provinces** | Map view | Color-based province ownership detection |
| **known_country_strength** | Ledger (diplomacy tab) | OCR from ledger table |
| **our_war_score** | War panel | OCR war score percentage |

**Phased Extraction**:

- **Phase 1 (MVP)**: Extract only fields used by AI prompt:
  - `date`, `treasury`, `manpower`, `adm_mana`, `dip_mana`, `mil_mana`, `at_war`
  - Stub the rest with defaults
- **Phase 2**: Add military info (`army_tradition`, tech levels)
- **Phase 3**: Add map-based province detection (enemy territories)

---

## Command → UI Actions Mapping

### Translation Patterns

Each `Command` maps to a sequence of UI actions. Key techniques:

1. **Hotkeys**: EU4 has extensive hotkey support (e.g., `T` = Tech panel, `V` = Outliner)
2. **Console Commands**: Debug console can execute many actions (e.g., `integrate TAG`)
3. **Click Sequences**: For complex actions (declare war, peace deals)
4. **Wait & Retry**: UI animations take time; must wait for panel transitions

### Command Implementations

#### Military Commands

| Command | UI Action Sequence |
|---------|-------------------|
| `Move { army_id, destination }` | 1. Click army on map (or in Military panel)<br>2. Right-click destination province<br>3. Confirm move |
| `DeclareWar { target, cb }` | 1. Open Diplomacy panel (Hotkey: `D`)<br>2. Search for target country<br>3. Click "Declare War"<br>4. Select CB from dropdown<br>5. Confirm |
| `OfferPeace { war_id, terms }` | 1. Open War panel (click war icon)<br>2. Click "Sue for Peace"<br>3. Add provinces to demands (click on map or list)<br>4. Click "Offer Peace" |
| `AcceptPeace { war_id }` | 1. Click peace offer notification<br>2. Click "Accept" |

#### Economic Commands

| Command | UI Action Sequence |
|---------|-------------------|
| `DevelopProvince { province, dev_type }` | 1. Click province on map<br>2. Click "Development" tab<br>3. Click "Tax"/"Production"/"Manpower" button<br>4. Confirm (spend mana) |
| `BuyTech { tech_type }` | 1. Open Tech panel (Hotkey: `T`)<br>2. Click ADM/DIP/MIL tab<br>3. Click "Unlock" on next tech<br>4. Confirm (spend mana) |
| `EmbraceInstitution { institution }` | 1. Open Institutions panel (Hotkey: `?`)<br>2. Find institution<br>3. Click "Embrace"<br>4. Confirm |

#### Diplomatic Commands

| Command | UI Action Sequence |
|---------|-------------------|
| `OfferAlliance { target }` | 1. Open Diplomacy (Hotkey: `D`)<br>2. Search for target<br>3. Click "Alliance"<br>4. Confirm |
| `SetRival { target }` | 1. Open Diplomacy<br>2. Search for target<br>3. Click "Set as Rival" |

#### Console Command Fallback

For actions hard to automate via UI, use the debug console:

```rust
// Console commands (require debug mode: add `-debug_mode` to EU4 launch options)
match command {
    Command::DeclareWar { target, .. } => {
        open_console();  // Hotkey: `
        type_text(&format!("declare_war {}", target));
        press_key(Key::Return);
    }
    Command::AddCore { province } => {
        open_console();
        type_text(&format!("own {}", province));
        press_key(Key::Return);
    }
    // ...
}
```

**Pros**: Fast, reliable, no UI navigation
**Cons**: Requires `-debug_mode` flag (disables achievements), feels like "cheating"

---

## Orchestrator Main Loop

```rust
// src/main.rs (pseudocode)

use eu4_bridge::{capture, extraction, translation, input, ai};
use eu4sim_core::{VisibleWorldState, Command};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize
    let mut capturer = capture::WindowCapturer::new("Europa Universalis IV")?;
    let extractor = extraction::StateExtractor::new()?;
    let translator = translation::ActionTranslator::new()?;
    let mut input_controller = input::InputController::new()?;
    let ai_client = ai::HttpClient::new("http://localhost:8080")?; // eu4sim-ai server

    loop {
        // 1. Pause game
        input_controller.press_key(Key::Space)?;
        tokio::time::sleep(Duration::from_millis(500)).await; // Wait for pause

        // 2. Capture screen
        let screenshot = capturer.capture_frame()?;

        // 3. Extract state
        let visible_state: VisibleWorldState = extractor.extract(&screenshot)?;

        // 4. Get available commands (hardcoded for now, or scrape from UI)
        let available_commands = get_available_commands(&visible_state);

        // 5. Call AI
        let chosen_command: Command = ai_client.decide(&visible_state, &available_commands).await?;

        // 6. Translate to UI actions
        let ui_actions = translator.translate(&chosen_command)?;

        // 7. Execute actions
        for action in ui_actions {
            input_controller.execute(&action)?;
            tokio::time::sleep(action.delay()).await; // Wait for UI transition
        }

        // 8. Unpause game
        input_controller.press_key(Key::Space)?;

        // 9. Wait for next tick (configurable delay)
        tokio::time::sleep(Duration::from_secs(5)).await; // AI thinks every 5 seconds
    }
}
```

**Key Points**:
- Always pause before capturing (ensures UI is stable)
- Add delays after each UI action (animations take time)
- Handle errors gracefully (if OCR fails, skip tick and log warning)

---

## Technical Challenges & Solutions

### Challenge 1: OCR Accuracy

**Problem**: Tesseract OCR on game fonts may misread numbers (e.g., "1000" → "l000")

**Solutions**:
1. **Font training**: Train Tesseract on EU4's UI fonts
2. **Template matching**: For common numbers (0-9), use template matching instead of OCR
3. **Validation**: Cross-check extracted values (e.g., treasury should be numeric, date format is `YYYY.MM.DD`)
4. **Fallback to save file**: If OCR fails repeatedly, read state from `save games/autosave.eu4` (see Phase B below)

### Challenge 2: Province Identification

**Problem**: Map colors change with mapmode (political, terrain, trade). Provinces have complex shapes.

**Solutions**:
1. **Force political mapmode**: Always switch to political mapmode (Hotkey: `F1`) before capture
2. **Color lookup table**: Build RGB → Province ID mapping from game files (`map/definition.csv`)
3. **Centroid sampling**: Sample color at province centroid (stored in `positions.txt`)
4. **Accept errors**: Province detection doesn't need to be perfect for Phase 1 (AI can work with incomplete info)

### Challenge 3: UI Layout Variability

**Problem**: Users have different resolutions, UI scale, mods

**Solutions**:
1. **Require 1920x1080**: Document assumes this resolution; add check on startup
2. **Config file**: Allow users to tweak region coordinates (`config.toml`)
3. **Template matching**: Use icon templates (scale-invariant) instead of hardcoded coords where possible

### Challenge 4: Action Ambiguity

**Problem**: AI outputs `Move { army_id: 42, destination: 123 }`, but how do we find army #42 on screen?

**Solutions**:
1. **Army ID tracking**: Maintain mapping `ArmyId → screen_position` via periodic scans
2. **Use outliner**: Military outliner lists all armies; click there instead of map
3. **Simplify for Phase 1**: Only support moving the *selected* army (AI picks army first, then destination)

### Challenge 5: Real-Time vs Turn-Based

**Problem**: EU4 is real-time (with pause), but AI expects discrete ticks

**Solutions**:
1. **Always play paused**: Pause after every AI decision, unpause only for animation/tick
2. **Speed 1**: If unpause is needed, use speed 1 (slowest)
3. **Tick detection**: Detect when date changes (monthly tick) via date OCR

---

## Fair Play Constraints: OCR vs Save File "Cheating"

### The Legitimacy Problem

**Core Question**: If the AI reads the save file, is it actually playing the game, or just reading a cheat sheet?

**Human Player Perspective**:
- Humans see **only the UI**: numbers on screen, tooltips, map colors
- Humans **cannot** see: exact province ownership in fog of war, precise enemy army counts, hidden diplomatic relations, exact institution spread percentages
- Humans **can** see: everything the game UI explicitly shows (treasury, visible armies, known wars)

**Save File = Omniscient Mode**:
- Reading `autosave.eu4` gives **perfect information** about the entire world state
- This is equivalent to the `VisibilityMode::Omniscient` mode in your sim
- An AI trained on omniscient data, then tested on omniscient data, proves nothing about real gameplay

**The Fairness Criterion**:

> **A legitimate "AI beats human players" claim requires the AI to use ONLY information visible in the UI, matching human fog of war constraints.**

### What's "Fair" vs "Cheating"?

| Information Source | Fair? | Rationale |
|-------------------|-------|-----------|
| **OCR from screen** | ✅ Yes | Humans read the screen |
| **UI element detection** | ✅ Yes | Humans see UI elements |
| **Template matching** | ✅ Yes | Humans recognize icons/colors |
| **Tooltip reading** | ✅ Yes | Humans hover for tooltips |
| **Save file parsing** | ❌ **Cheating** | Humans don't open save files mid-game |
| **Debug console** | ❌ **Cheating** | Humans don't use console (disables achievements) |
| **Memory reading** | ❌ **Cheating** | Obvious |

### Edge Case: UI-Hidden but Gameplay-Critical Info

**Problem**: Some information is legitimately hard to find in UI but critical for strategy.

**Example (from user)**:
- **"Did Radical Reforms happen?"** - Overpowered one-time event. No obvious UI indicator after it fires. Players might check save file to see if it already triggered.

**Whitelist Approach**:

Allow save file lookups for **specific, well-documented edge cases** where:
1. Information is gameplay-critical
2. UI provides no clear indicator, or requires extensive tooltip hunting
3. Human players would consult wiki/forum to determine the state

**Proposed Whitelist** (debatable):

| Data Point | Allowed via Save? | Justification |
|------------|-------------------|---------------|
| Radical Reforms event (fired?) | ✅ Maybe | One-time event, no UI indicator after it fires |
| Institution origin provinces | ✅ Maybe | Hard to find, but wiki-able |
| Exact province ownership (visible on map) | ❌ No | Map shows this visually |
| Enemy army strength (out of sight) | ❌ No | Fog of war is intentional |
| Hidden diplomatic relations | ❌ No | Espionage mechanic exists for this |
| Exact treasury of other nations | ❌ No | Intentionally hidden |

**Implementation**:
```rust
// config.toml
[extraction.allowlist]
# Allow specific save file lookups for UI-ambiguous data
radical_reforms_fired = true   # One-time event, no UI indicator
institution_origins = true      # Hard to find in UI
# Everything else must come from OCR
```

### Phased Approach: Engineering vs Production

**Phase 1-2 (Prototyping)**: Save file is **acceptable** as an **engineering shortcut**
- **Goal**: Prove the pipeline works (AI can control game)
- **Justification**: OCR is hard, save parsing is easy, get to "Hello World" faster
- **Label**: "Prototype Mode" - not claiming fairness

**Phase 3+ (Production)**: OCR-only is **mandatory** for **legitimate claims**
- **Goal**: "Our AI can beat human players at EU4"
- **Requirement**: AI sees ONLY what humans see (screen pixels + UI)
- **Label**: "Fair Play Mode" - credible achievement

**Training Data Implications**:

If you train the AI on save file data (omniscient), then test it on OCR data (realistic fog of war), it may fail catastrophically. The AI learned to rely on information it won't have in production.

**Recommendation**:
1. **Phase A-B**: Use save file to validate pipeline
2. **Phase C**: Transition to OCR for state extraction
3. **Phase D+**: Train AI on OCR-extracted states (realistic visibility)
4. **Final**: Publish results with "OCR-only, no save file access" disclaimer

### Why Save File Was Mentioned at All

From `learned-ai-musings.md:164`:

> **The save file parsing is the key insight—you're not doing computer vision, you're reading structured data you already know how to parse.**

**Context**: This was in the "speculative" section about feasibility. The point was:

- **Input** is tractable: EU4 uses plaintext saves (unlike StarCraft replays which need binary parsing)
- **You already have a parser**: `eu4txt` crate exists and works
- **This lowers the barrier**: Parsing EU4 state is easier than, say, parsing Dota 2 netcode

**But**: "Easy to parse" ≠ "Fair to use in production". Save file parsing is a **development tool**, not a **production strategy**.

---

## Phase B: Save File Hybrid Approach (Engineering Shortcut)

**Status**: Theoretical/future work. Not currently implemented.

> **Reality check**: As of 2025-12-21, save file parsing for real EU4 is just a design idea. Even if we wanted to "cheat" this way, it would require non-trivial implementation work. The `eu4txt` crate parses EU4 files, but integrating it into the live automation loop is not free.

**Alternative/Complement to OCR**: Read game state directly from save files.

### How It Works

1. **Enable autosave**: EU4 autosaves every month (or configure to save every day)
2. **Parse save file**: Use `eu4txt` crate (you already have this!) to parse `autosave.eu4`
3. **Extract VisibleWorldState**: Read treasury, manpower, mana, provinces, wars directly from save
4. **Execute command**: Use UI automation (or console commands)
5. **Wait for next autosave**: Detect when save file timestamp changes

**Pros**:
- Perfect accuracy (no OCR errors)
- Access to full state (provinces, diplomacy, etc.)
- Uses existing `eu4txt` parser

**Cons**:
- Slower (must wait for autosave)
- Requires autosave interval tuning
- Ironman mode might resist manipulation

**Hybrid Strategy**:
- Use save file for **state extraction** (accurate, complete)
- Use UI automation for **action execution** (console commands or clicks)

This is the approach mentioned in `learned-ai-musings.md:164`:

> **The save file parsing is the key insight—you're not doing computer vision, you're reading structured data you already know how to parse.**

---

## Static Game Data: What's Fair and Where to Use It

### The Distinction: Runtime State vs Static Game Rules

**Static game data** (✅ **Always fair**):
- Province definitions (`map/definition.csv`)
- Trade goods (`common/tradegoods/`)
- Buildings (`common/buildings/`)
- Technologies (`common/technologies/`)
- Ideas (`common/ideas/`)
- Mission trees (`missions/`)
- Events (`events/`)
- Modifiers (`common/static_modifiers/`)
- Government types (`common/governments/`)

**Runtime state** (❌ **Save file = cheating**, ✅ **UI/OCR = fair**):
- Current treasury, manpower, mana
- Army positions and strength
- Diplomatic relations
- War scores
- Province ownership

**Why static data is fair**:
- Humans consult the wiki (which is just formatted game files)
- Knowing "what does Quantity ideas do?" is not cheating
- Knowing "what's my next mission?" is reading the mission tree
- Knowing "Grain has base price 2 ducats" is game knowledge

### Where to Use Static Game Data

#### 1. Prompt Construction (eu4sim-ai)

**Enrich the prompt with relevant game knowledge:**

```rust
// Current minimal prompt
<|state|>
Treasury: 523 ducats
Manpower: 45,000
<|/state|>

// Enhanced prompt with static data
<|state|>
Treasury: 523 ducats
Manpower: 45,000

National Ideas (France):
- Elan! (+20% morale of armies)
- French Musketeers (+10% infantry combat ability)

Active Missions:
- "Secure the Throne" ✓ (completed)
- "Consolidate Power" (requires 20 provinces, currently 15/20)
- "Italian Ambition" (locked until previous complete)

<|/state|>
```

**Benefits**:
- AI understands **why** certain actions are valuable (missions guide strategy)
- AI knows **what bonuses** it has (morale boost → aggressive military)
- AI can plan **long-term** (mission trees are multi-step goals)

**Where to source**:
- Load mission files from EU4 game directory: `<EU4_DIR>/missions/`
- Load idea files: `<EU4_DIR>/common/ideas/`
- Parse with `eu4txt` crate (you already have the parser!)

**Fair play check**: ✅ Humans see this in the UI (Missions tab, Ideas tab)

#### 2. State Extraction Validation (eu4-bridge)

**Cross-reference OCR results with static game data:**

```rust
// Province color detection
let rgb = sample_province_color(screenshot, province_centroid);
let province_id = lookup_province_by_color(rgb, &definition_csv)?;

// Trade good icon recognition
let icon = crop_province_panel(screenshot, "trade_good_icon");
let trade_good = match_template(icon, &tradegood_icons)?;
let base_price = lookup_tradegood_price(&trade_good, &tradegoods_data);
```

**Benefits**:
- Validate OCR results (if treasury reads as "l000" instead of "1000", catch it)
- Resolve ambiguous UI elements (icon → trade good → price)
- Convert UI visuals to structured data (color → province ID)

**Where to source**:
- `map/definition.csv` for province RGB → ID mapping
- `gfx/interface/` for icon templates
- `common/tradegoods/` for trade good effects

**Fair play check**: ✅ This is just interpreting the UI correctly

#### 3. Available Commands Filtering (eu4sim-core)

**Use game rules to determine legal actions:**

```rust
// Can we build a Cathedral?
fn can_build_cathedral(province: &Province, country: &Country, buildings_data: &BuildingsDB) -> bool {
    let cathedral = &buildings_data["cathedral"];

    // Check prerequisites from game files
    if province.religion != country.religion {
        return false; // Can't build wrong-religion building
    }
    if !country.has_tech(cathedral.required_adm_tech) {
        return false; // Tech requirement not met
    }
    if province.has_building("temple") {
        return false; // Conflicts with existing building
    }
    true
}
```

**Benefits**:
- Filter out impossible commands (AI doesn't waste time considering them)
- Accurate action space (better training signal)
- Validate AI outputs (if AI picks invalid action, log warning)

**Where to source**:
- `common/buildings/` for building prerequisites
- `common/technologies/` for tech unlocks
- Mission tree files for mission prerequisites

**Fair play check**: ✅ Humans can't issue illegal commands either (UI grays out unavailable buttons)

#### 4. Training Data Annotation (eu4sim)

**Label training samples with game context:**

```jsonl
{
  "state": { "treasury": 523, "manpower": 45000 },
  "available": [ "DevelopParis", "BuyAdmTech", "Pass" ],
  "chosen": "DevelopParis",
  "context": {
    "missions": {
      "active": "Consolidate Power",
      "progress": "15/20 provinces"
    },
    "ideas": {
      "unlocked": ["Elan!", "French Musketeers"],
      "next": "Diplomatic Corps"
    },
    "province_info": {
      "Paris": {
        "trade_good": "Grain",
        "base_price": 2.0,
        "current_dev": 12
      }
    }
  }
}
```

**Benefits**:
- Richer training signal (AI learns mission-guided strategy)
- Better prompt diversity (context varies across games)
- Debugging aid (why did AI pick this action? Check context.)

**Where to source**:
- Same as #1 (missions, ideas, trade goods)
- Load once at sim startup, reuse across all ticks

**Fair play check**: ✅ All this is UI-visible or wiki-able

#### 5. Action Translation Helpers (eu4-bridge)

**Use game data to generate UI click sequences:**

```rust
// Translate: DeclareWar { target: "ENG", cb: Some("Reconquest") }
fn translate_declare_war(target: &str, cb: Option<&str>, cbs_data: &CasusbelliDB) -> Vec<UiAction> {
    let mut actions = vec![
        UiAction::Hotkey(Key::D),  // Open Diplomacy
        UiAction::TypeText(target), // Search for England
        UiAction::Click(Button::DeclareWar),
    ];

    if let Some(cb_name) = cb {
        // Look up CB position in dropdown based on game files
        let cb_index = cbs_data.get_index(cb_name)?;
        actions.push(UiAction::ClickDropdown(cb_index));
    }

    actions.push(UiAction::Click(Button::Confirm));
    actions
}
```

**Benefits**:
- Know what CBs exist (from `common/cb_types/`)
- Know CB order in dropdown (alphabetical or file order)
- Handle game updates (if CB names change, read from game files)

**Where to source**:
- `common/cb_types/` for casus belli definitions
- UI layout files (if modding exposes them)

**Fair play check**: ✅ This is just automating what humans click

### Recommended Static Data Pipeline

**At startup**:
1. Detect EU4 game directory (Steam install path, or user config)
2. Load static game files with `eu4txt`:
   ```rust
   let eu4_dir = detect_eu4_install()?;
   let missions = parse_missions(&eu4_dir.join("missions"))?;
   let ideas = parse_ideas(&eu4_dir.join("common/ideas"))?;
   let tradegoods = parse_tradegoods(&eu4_dir.join("common/tradegoods"))?;
   let buildings = parse_buildings(&eu4_dir.join("common/buildings"))?;
   ```
3. Pass to AI as context (part of `VisibleWorldState` or separate struct)

**At each tick**:
1. Extract runtime state via OCR
2. Augment with relevant static data (active missions, unlocked ideas)
3. Build enriched prompt
4. Call AI inference
5. Validate AI output against game rules

### What NOT to Include

**Avoid giving AI perfect information it shouldn't have:**
- ❌ Event triggers (MTTH, conditions) - too much foresight
- ❌ AI behavior scripts (how vanilla AI decides) - meta-gaming
- ❌ Optimal builds (pre-computed strategies) - defeats the purpose
- ✅ Event **effects** when they fire - humans see event popups
- ✅ AI **weights** if visible in tooltips - humans see "AI likelihood: High"

### Training vs Inference: Preventing Overfitting

**The Problem**: If you provide too much static data during training, the AI might **memorize** specific scenarios rather than learn general strategy.

**Example of overfitting**:
```python
# BAD: Training data includes full mission requirements
{
  "state": { "treasury": 523, "provinces": 15 },
  "mission": "Consolidate Power requires 20 provinces",  # Too specific!
  "chosen": "ConquerCalais"
}

# AI learns: "If mission says 'requires 20 provinces', always conquer"
# This is memorization, not strategy
```

**Solution**: Distinguish **training-time** context (general knowledge) from **inference-time** context (specific guidance).

#### Training Time: General Patterns Only

**What to include in training data**:
- ✅ Mission **exists** (France has a mission tree)
- ✅ Mission **progress** (15/20 provinces)
- ✅ Mission **reward** (+1 stability)
- ❌ Mission **exact requirements** (which provinces to conquer)

**Why**: You want the AI to learn "missions are important" and "progress toward missions is good", not "memorize every mission in the game".

**Training prompt (good)**:
```
Active Mission: "Consolidate Power" (15/20 progress)
Reward: +1 Stability on completion
```

The AI learns: "Being close to mission completion is valuable, actions that increase progress are good"

#### Inference Time: Specific Context

**What to provide only at inference** (not during training):
- ✅ How to complete current mission (provinces needed, specific actions)
- ✅ What buildings unlock at next tech level
- ✅ Which ideas are available to unlock
- ✅ Event chain context (if this event fires, expect followup)

**Why**: This information changes per scenario. Providing it only at inference prevents memorization and allows the AI to generalize.

**Inference prompt (enhanced)**:
```
Active Mission: "Consolidate Power" (15/20 progress)
Requirements: Own 20 provinces (currently 15)
Target provinces: Calais, Picardy, Normandy, Brittany, Gascony (all bordering)
Reward: +1 Stability

Available actions:
0: Conquer Calais (counts toward mission)
1: Develop Paris (does NOT count toward mission)
2: Pass
```

**Benefit**: AI can plan strategically ("I'm 5 provinces short → conquering advances mission") without memorizing "France always conquers Calais".

#### Implementation: Two-Stage Context

```rust
// Startup: Load static game rules (used for ALL training and inference)
struct GameRules {
    provinces: HashMap<ProvinceId, ProvinceDefinition>,
    tradegoods: HashMap<TradegoodId, TradegoodData>,
    buildings: HashMap<BuildingId, BuildingData>,
    // General knowledge only
}

// Inference: Build scenario-specific context (used ONLY at inference)
struct InferenceContext {
    // Specific to this game state
    current_mission_requirements: Vec<MissionRequirement>,
    available_mission_targets: Vec<ProvinceId>,
    next_unlocked_buildings: Vec<BuildingId>,
    // Detailed, actionable guidance
}

fn build_prompt(
    state: &VisibleWorldState,
    rules: &GameRules,  // Training + inference
    context: Option<&InferenceContext>,  // Inference only
) -> String {
    let mut prompt = format_state(state);

    // Always include: mission progress (general)
    prompt.push_str(&format!("Mission: {} ({}/{})\n",
        state.current_mission, state.mission_progress, state.mission_target));

    // Only at inference: specific guidance
    if let Some(ctx) = context {
        prompt.push_str("Requirements:\n");
        for req in &ctx.current_mission_requirements {
            prompt.push_str(&format!("  - {}\n", req));
        }
        prompt.push_str("Target provinces: ");
        prompt.push_str(&ctx.available_mission_targets.join(", "));
        prompt.push_str("\n");
    }

    prompt
}
```

**Training**: Call with `context = None` → AI learns general patterns
**Inference**: Call with `context = Some(...)` → AI gets specific help

#### Preventing Prompt Leakage

**Risk**: If training data accidentally includes inference-time context, the AI learns to expect it.

**Mitigation**:
1. **Separate data pipelines**: Training data generator never calls `build_inference_context()`
2. **Schema validation**: Training samples rejected if they contain inference-only fields
3. **Prompt templates**: Use different templates for training vs inference (training is subset of inference)

```rust
// config.toml
[training]
include_mission_requirements = false  # Only mission progress
include_building_unlocks = false      # Only current buildings
include_idea_recommendations = false  # Only current ideas

[inference]
include_mission_requirements = true   # Full guidance
include_building_unlocks = true       # What becomes available
include_idea_recommendations = true   # Strategic suggestions
```

#### When Inference Context is Fair

**Question**: Is providing detailed mission requirements "cheating"?

**Answer**: No, because humans see this in the UI (Missions tab).

**Fair play check**:
- ✅ Mission requirements → visible in Missions panel
- ✅ Building prerequisites → visible in tooltips
- ✅ Idea effects → visible in Ideas panel
- ❌ Future event triggers → NOT visible (cheating)

**The rule**: If a human would click a UI button to see this info, it's fair to provide it in the prompt.

### Example: Mission-Guided Strategy

**Without static data:**
```
AI prompt:
Treasury: 523 ducats
Actions: DevelopParis, ConquerCalais, Pass

AI thinks: "More dev is always good" → DevelopParis
```

**With mission tree:**
```
AI prompt:
Treasury: 523 ducats
Mission: "Consolidate Power" (15/20 provinces, reward: +1 stability)
Actions: DevelopParis, ConquerCalais, Pass

AI thinks: "5 more provinces → mission reward → conquering is optimal" → ConquerCalais
```

**Result**: AI follows historical expansion paths (missions guide it toward France's natural targets)

---

## Implementation Phases

### Phase A: Proof of Concept (Week 1)

**Goal**: Get AI to make *one* decision in real EU4

**Deliverables**:
1. Window capture (screenshot of EU4)
2. Extract date + treasury via OCR
3. Stub `VisibleWorldState` (minimal fields)
4. Call AI inference (HTTP to eu4sim-ai)
5. Translate `Command::Pass` → no-op
6. Log full loop to console

**Success Criteria**: Program runs without crashing, captures screen, calls AI, logs decision

### Phase B: Save File Reader (Week 2)

**Goal**: Extract accurate state from save files

**Deliverables**:
1. Enable autosave every month in EU4
2. Use `eu4txt` to parse `autosave.eu4`
3. Build full `VisibleWorldState` from save data
4. Compare to OCR-based extraction (validate accuracy)

**Success Criteria**: `VisibleWorldState` matches real game state 100%

### Phase C: Simple Command Execution (Week 3)

**Goal**: Execute one command type (e.g., `DevelopProvince`)

**Deliverables**:
1. Implement `translation::economy::develop_province()`
2. Click province on map
3. Open development panel
4. Click "Tax" button
5. Confirm (spend mana)
6. Verify in next save file that province dev increased

**Success Criteria**: AI successfully develops a province in real EU4

### Phase D: Multi-Command Support (Month 2)

**Goal**: Support 5-10 critical commands

**Priority Commands**:
1. `DevelopProvince` (economy)
2. `BuyTech` (tech)
3. `Move` (military)
4. `DeclareWar` (war)
5. `OfferPeace` (peace)

**Success Criteria**: AI can play a full year (1444-1445) autonomously

### Phase E: Full Integration (Month 3+)

**Goal**: AI plays a full campaign (1444-1821)

**Deliverables**:
1. Support all 30+ command types
2. Handle edge cases (UI glitches, unexpected popups)
3. Performance optimization (faster OCR, parallel extraction)
4. Logging & debugging (record full decision history)
5. **Video proof**: Record timelapse of AI playing full game

**Success Criteria**: AI completes 1444→1821 without human intervention

---

## Crate Dependencies

```toml
[dependencies]
# Core simulation types
eu4sim-core = { path = "../eu4sim-core" }
eu4sim-ai = { path = "../eu4sim-ai" }

# Screen capture
xcap = "0.0.10"          # Cross-platform window capture

# Input automation
enigo = "0.2"            # Mouse/keyboard control

# OCR
tesseract = "0.14"       # Text extraction from images

# Image processing
image = "0.25"           # Image manipulation, template matching

# Async runtime
tokio = { version = "1", features = ["full"] }

# HTTP client (for AI inference)
reqwest = { version = "0.12", features = ["json"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# EU4 save parsing (Phase B)
eu4txt = { path = "../eu4txt" }  # Your existing save file parser

# Logging
log = "0.4"
env_logger = "0.11"
```

---

## Configuration Example

```toml
# config.toml

[display]
resolution = "1920x1080"
target_window = "Europa Universalis IV"

[extraction]
# FAIR PLAY MODE: OCR only, no save file (competitive/legitimate)
# PROTOTYPE MODE: Allow save file for state extraction (engineering validation)
fair_play_mode = true  # Set to false for prototyping

# Whitelist for save file lookups (only used if fair_play_mode = false)
# Even in prototype mode, you can restrict what data comes from save file
[extraction.allowlist]
radical_reforms_fired = false   # One-time event, no UI indicator after firing
institution_origins = false      # Where institutions spawned (hard to find in UI)
# All other fields must come from OCR when fair_play_mode = true

[extraction.regions]
# Top bar (1920x1080)
date = { x = 860, y = 5, width = 200, height = 30 }
treasury = { x = 100, y = 5, width = 120, height = 30 }
manpower = { x = 250, y = 5, width = 120, height = 30 }
adm_mana = { x = 400, y = 5, width = 80, height = 30 }
dip_mana = { x = 500, y = 5, width = 80, height = 30 }
mil_mana = { x = 600, y = 5, width = 80, height = 30 }

[ai]
# eu4sim-ai server endpoint
endpoint = "http://localhost:8080/decide"
# Or: use local inference
local = true
adapter_path = "models/adapter/balanced"

[orchestrator]
tick_interval_secs = 5
pause_before_capture = true
enable_console_commands = false  # Requires -debug_mode
```

---

## Legal & Ethical Considerations

### Paradox ToS

**Check**: Paradox Interactive's Terms of Service for EU4 may prohibit automation tools. Review before public release.

**Mitigation**:
- This is for **research/education** (ML agent training validation)
- No competitive advantage in multiplayer (only works in single-player)
- No modification of game files (only reads screen, sends input)

### Achievements

**Impact**: Using `-debug_mode` or automation disables Steam achievements. Document this clearly for users.

### Ironman Mode

**Compatibility**: Ironman mode prevents console commands and frequent saves. Phase A-D may not work in Ironman; Phase B (save file reading) also restricted.

**Recommendation**: Only support non-Ironman single-player for now.

---

## Success Metrics

### Prototype Mode (Save File Allowed)

| Milestone | Metric | Target |
|-----------|--------|--------|
| **Phase A** | Program runs without crash | 100% |
| **Phase B** | State extraction accuracy (save file) | >95% field accuracy |
| **Phase C** | Single command success rate | >80% successful executions |
| **Phase D** | Multi-command (1 year) | AI completes 12 ticks without hang |
| **Phase E** | Full game (377 years) | AI reaches 1821 |

### Fair Play Mode (OCR Only, No Save File)

| Milestone | Metric | Target | Notes |
|-----------|--------|--------|-------|
| **OCR Accuracy** | State extraction accuracy (OCR) | >85% field accuracy | Lower than save file, but acceptable |
| **Visibility Fidelity** | Match human fog of war | No omniscient info | Validates `VisibilityMode::Realistic` |
| **Benchmark vs Human** | Win rate vs experienced players | >50% | Legitimate "beats humans" claim |
| **Benchmark vs Vanilla AI** | Win rate vs Very Hard AI | >70% | Baseline skill check |
| **Full Campaign** | Complete 1444→1821 (OCR only) | AI reaches 1821 | No save file, no console |
| **Competitive Score** | Finish in top 3 Great Powers | >50% of games | Consistent strong play |

**Key Distinction**:
- **Prototype mode** = Engineering validation ("Can it control the game?")
- **Fair play mode** = Competitive validation ("Can it beat humans fairly?")

---

## Future Extensions

### AI Advisor Overlay

Instead of full automation, display AI's suggested action to human player:

```
┌─────────────────────────────────────┐
│ AI Advisor (Balanced Personality)  │
│                                     │
│ Suggested Action:                  │
│ → Develop Paris (Tax)              │
│                                     │
│ Reasoning:                          │
│ "Treasury healthy (1200 ducats),   │
│  ADM mana at 200. Paris is high-   │
│  dev capital, good ROI."            │
│                                     │
│ [ Accept ] [ Ignore ] [ Pause AI ] │
└─────────────────────────────────────┘
```

**Use Case**: Human learns from AI, or uses AI as "co-pilot"

### Opponent Modeling

Track how AI performs against real EU4 AI:

- Win rate vs vanilla AI (Normal/Hard/Very Hard)
- Time to conquer specific regions
- Economic efficiency (dev/year)

**Goal**: "Our AI beats EU4 on Very Hard difficulty"

### Multi-Agent Play

Run multiple AIs with different personalities (balanced, aggressive, diplomatic) on different countries simultaneously:

- Balanced AI plays France
- Aggressive AI plays Ottomans
- Diplomatic AI plays Austria

Watch them interact in real game!

---

## Open Questions

1. **OCR vs Save File**: Which should be primary state source? (Recommendation: Save file for Phase B+)
2. **Console Commands**: Allow or ban? (Affects "legitimacy" of achievement)
3. **Speed**: Real-time with pause, or play at speed 5? (Affects thinking time budget)
4. **Error Recovery**: What if UI action fails? Retry? Skip? Pause for human?
5. **Multiplayer**: Could this work in MP? (Probably not - desyncs, host control)

---

## References

- `docs/design/simulation/learned-ai.md` - Full ML architecture
- `docs/design/simulation/learned-ai-musings.md` - "Playing Real EU4" section (line 156)
- `eu4sim-core/src/ai/mod.rs` - `VisibleWorldState` definition
- `eu4sim-core/src/input.rs` - `Command` enum
- `eu4sim-ai/` - LLM inference with Candle

---

*Last updated: 2025-12-21*
*Status: Design phase - awaiting Phase A implementation*
