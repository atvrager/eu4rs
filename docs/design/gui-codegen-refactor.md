# Data-Driven GUI Renderer Refactor

**Design Document**

**Status:** In Progress (Phase 0, 1 Complete)
**Author:** Claude (with user guidance)
**Created:** 2026-01-09
**Target:** eu4game GUI rendering system

---

## Executive Summary

This document describes a refactor of the GUI rendering system from **code-driven** (manual widget lists with borrow checker workarounds) to **data-driven** (build-time code generation from `.gui` files). This follows established patterns from `eu4data` and `xtask`.

**Impact:**
- **Delete** ~4000 lines of manual rendering code
- **Add** ~200 lines of code generator
- **Eliminate** borrow checker friction
- **Unify** rendering around single `WidgetCache`

**Approach:** Build-time code generation via `cargo xtask generate-gui-renderer`

---

## Phase Overview

| Phase | Goal | Lines Changed | Status | Deliverable |
|-------|------|---------------|--------|-------------|
| **Phase 0** | Quick fix for observer text | ~20 lines | ✅ Complete | Observer mode fully functional |
| **Phase 1** | Build code generator (3 files) | +800 lines | ✅ Complete | `cargo xtask generate-gui-renderer` command |
| **Phase 2** | Implement widget cache | +200 lines | 🔄 Next | Unified sprite/font caching |
| **Phase 3** | Proof of concept (left panel) | +300 generated | Pending | Left panel uses generated code |
| **Phase 4** | Port all panels | +1000 generated | Pending | All panels use generated code |
| **Phase 5** | Build automation | +50 lines | Pending | Auto-regeneration on .gui changes |
| **Phase 6** | Delete legacy code | -4000 lines | Pending | Clean, maintainable codebase |
| **Phase 7** | Parse all 114 GUI files | +100 lines | Future | Complete EU4 GUI coverage |

**Total net change:** ~-2000 lines, +massive maintainability improvement

---

## Problem Statement

Current renderer architecture fights the borrow checker due to **code-driven design** (manual widget lists) instead of **data-driven design** (walking GUI trees). We parse beautiful hierarchical trees from `.gui` files, then immediately flatten them to metadata vectors, losing the structure.

This causes:
- Borrow checker conflicts (need collect-then-execute patterns everywhere)
- Manual maintenance burden (4600+ lines of similar rendering loops)
- Data duplication (same widget info in 3 places: tree, layout struct, bound panel)
- Fragility (adding a new widget requires touching 4+ files)

## Proposed Solution: Build-Time Code Generation

Follow eu4data/xtask patterns: **Parse .gui files at build time, generate Rust rendering code**.

Just like:
- `eu4data/build.rs` generates type structs from game data schemas
- `xtask region_gen` generates UI region constants from parsed .gui files

We'll generate panel rendering methods from the GUI element trees.

### Architecture Changes

```
BEFORE (code-driven):
.gui file → Runtime parse → Flatten to Vec<(name, pos, sprite)> → Manual render loops
                                ↓
                        Throw away tree structure

AFTER (build-time generation):
.gui file → Build-time parse → Code generation → Compiled rendering methods
                                    ↓
                        Generated .rs files (gitignored)
```

### Core Design

**1. Build-Time Code Generator (`xtask/src/gui_codegen/`)**

New xtask command: `cargo xtask generate-gui-renderer`

Parses `.gui` files and generates:
- Widget cache initialization code
- Panel-specific rendering methods
- Hit box registration code

**2. Generated Code Structure**

```rust
// eu4game/src/generated/gui/left_panel_renderer.rs (GENERATED - DO NOT EDIT)

impl GuiRenderer {
    fn render_left_panel_generated(&mut self,
        panel: &CountrySelectLeftPanel,
        screen_size: (u32, u32),
        sprite_renderer: &SpriteRenderer,
        render_pass: &mut wgpu::RenderPass,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        // Window anchor from parsed .gui file
        let left_anchor = get_window_anchor((15, 52), Orientation::UpperLeft, screen_size);

        // Observer mode button (from frontend.gui:1490)
        {
            let sprite = self.widget_cache.get_or_load("GFX_checkbox_small", device, queue);
            let pos = position_from_anchor(left_anchor, (20, -152), Orientation::LowerLeft, sprite.dimensions);
            let frame = if panel.observe_mode_button.state() == ButtonState::Pressed { 1 } else { 0 };
            sprite_renderer.draw_frame(render_pass, &sprite.bind_group, pos, frame, ...);
            self.hit_boxes.push(("observe_mode_button", HitBox { x: pos.0, y: pos.1, ... }));
        }

        // Observer mode text (from frontend.gui:1490)
        {
            let font = self.widget_cache.get_or_load_font("vic_18", device, queue);
            let text = panel.observe_mode_title.text();
            render_text(text, font, (55, -145), Orientation::LowerLeft, left_anchor, ...);
        }

        // Back button (from frontend.gui:1538)
        {
            let sprite = self.widget_cache.get_or_load("GFX_standard_button_148_24", device, queue);
            let pos = position_from_anchor(left_anchor, (20, -20), Orientation::LowerLeft, sprite.dimensions);
            sprite_renderer.draw(render_pass, &sprite.bind_group, pos, ...);
            self.hit_boxes.push(("back_button", HitBox { ... }));
        }

        // ... ALL other widgets generated automatically from tree
    }
}
```

**3. Simplified GuiRenderer**

```rust
pub struct GuiRenderer {
    // REMOVE all manual metadata:
    // - speed_controls_layout ❌
    // - topbar_layout ❌
    // - button_bind_groups ❌
    // - topbar_icons ❌
    // - frontend_button_bind_groups ❌

    // Single widget cache:
    widget_cache: WidgetCache,  ✅

    // Bound panels (still needed for dynamic state):
    left_panel: Option<CountrySelectLeftPanel>,
    topbar: Option<TopBar>,
    // ...
}
```

## Implementation Plan

### Phase 0: Simplify Observer Text Rendering (Quick Fix)

**Goal:** Get observer mode text rendering working ASAP without fighting the architecture.

**Tasks:**
1. **Inline observer text rendering** in `renderer.rs`
   - Location: `render_country_selection()` method
   - Add direct rendering code for `observe_mode_title` widget
   - Use existing `render_text_inline` helper or similar pattern
   - Match pattern used for other text widgets in the same method

2. **Remove tree-walking experiment**
   - Delete `render_left_panel_tree()` function if it exists
   - Remove `left_panel_tree: Option<GuiElement>` field if added
   - Clean up any attempted generic tree-walking code

3. **Test and verify**
   - Run `cargo test -p eu4game observer_mode_tests`
   - Verify `test_observer_text_label_exists` passes
   - Check visual output with `eu4game` binary (manual verification)
   - Commit changes with message about quick fix before refactor

**Deliverable:** Observer mode fully functional with text label, ready for proper refactor.

---

### Phase 1: Create GUI Code Generator (xtask)

**Goal:** Build infrastructure for code generation, following xtask region_gen pattern.

**Task 1.1: Create module structure**
- Create directory `xtask/src/gui_codegen/`
- Create `xtask/src/gui_codegen/mod.rs` with module declarations:
  ```rust
  mod parser;
  mod codegen;
  mod types;

  pub use self::codegen::generate_gui_renderer;
  ```
- Add `pub mod gui_codegen;` to `xtask/src/lib.rs`

**Task 1.2: Implement intermediate types** (`types.rs`)
- Define `WidgetInfo` struct:
  ```rust
  pub struct WidgetInfo {
      pub name: String,
      pub widget_type: WidgetType,
      pub sprite_name: Option<String>,
      pub position: (i32, i32),
      pub orientation: Orientation,
      pub font: Option<String>,
      pub text: Option<String>,
  }

  pub enum WidgetType {
      Button,
      TextBox,
      Icon,
      Window,
      Listbox,
      EditBox,
  }
  ```
- Define `PanelInfo` struct to hold window + widgets
- Add conversion helpers from `GuiElement` to `WidgetInfo`

**Task 1.3: Implement parser** (`parser.rs`)
- Import `eu4game::gui::parser::parse_gui_file`
- Create `parse_all_gui_files(game_path: &Path) -> Result<HashMap<String, GuiElement>>`
  - Parse `interface/frontend.gui`
  - Parse `interface/topbar.gui`
  - Parse `interface/speed_controls.gui`
  - Build HashMap keyed by window name
- Create `extract_panel_info(tree: &GuiElement, window_name: &str) -> PanelInfo`
  - Walk tree to find specific window
  - Extract all child widgets
  - Convert to intermediate representation

**Task 1.4: Implement code generator** (`codegen.rs`)
- Create `generate_panel_renderer(panel: &PanelInfo) -> String`
  - Generate method signature with all required parameters
  - Generate window anchor calculation code
  - Walk widgets and emit rendering code for each
  - Return complete method as String
- Create `emit_button_rendering(widget: &WidgetInfo, code: &mut String)`
- Create `emit_text_rendering(widget: &WidgetInfo, code: &mut String)`
- Create `emit_icon_rendering(widget: &WidgetInfo, code: &mut String)`

**Task 1.5: Add xtask command**
- Edit `xtask/src/main.rs` to add `generate-gui-renderer` subcommand
- Wire up to `gui_codegen::generate_gui_renderer()` function
- Parse CLI args: `--panel <name>` or generate all
- Output files to `eu4game/src/generated/gui/`

**Task 1.6: Write generated file scaffolding**
- Create `eu4game/src/generated/gui/mod.rs` (manually, not generated)
- Add module declarations for generated files
- Ensure directory exists and is writable

**Deliverable:** Working `cargo xtask generate-gui-renderer` command that produces compilable (but not yet integrated) rendering code.

### Phase 2: Implement Widget Cache

**Goal:** Single unified cache for all sprites and fonts, replacing 7+ separate caches.

**Task 2.1: Create WidgetCache module**
- Create file `eu4game/src/gui/widget_cache.rs`
- Add `mod widget_cache;` and `pub use widget_cache::WidgetCache;` to `eu4game/src/gui/mod.rs`
- Define basic structure:
  ```rust
  use std::collections::HashMap;

  pub struct WidgetCache {
      sprites: HashMap<String, CachedSprite>,
      fonts: HashMap<String, CachedFont>,
  }

  struct CachedSprite {
      bind_group: wgpu::BindGroup,
      dimensions: (u32, u32),
      num_frames: u32,
  }

  struct CachedFont {
      // Font cache data (design based on existing font system)
  }
  ```

**Task 2.2: Implement sprite caching**
- Add `WidgetCache::new()` constructor
- Implement `get_or_load_sprite()` method:
  ```rust
  pub fn get_or_load_sprite(
      &mut self,
      sprite_name: &str,
      gfx_db: &GfxDatabase,
      sprite_cache: &mut SpriteCache,
      device: &wgpu::Device,
      queue: &wgpu::Queue,
      sprite_renderer: &SpriteRenderer,
  ) -> &CachedSprite
  ```
- Use `HashMap::entry()` API for lazy loading
- Extract sprite info from `gfx_db`
- Load texture via `sprite_cache`
- Create bind group via `sprite_renderer`

**Task 2.3: Implement font caching**
- Research existing font loading in `renderer.rs`
- Design `CachedFont` structure based on current implementation
- Implement `get_or_load_font()` method
- Follow same lazy-loading pattern as sprites

**Task 2.4: Add WidgetCache to GuiRenderer**
- Add field `widget_cache: WidgetCache` to `GuiRenderer` struct
- Initialize in `GuiRenderer::new()`
- Do NOT remove old caches yet (keep both during migration)

**Task 2.5: Test widget cache in isolation**
- Write unit tests in `widget_cache.rs`:
  - Test sprite caching (load once, hit cache on second call)
  - Test font caching
  - Test multiple sprites/fonts
- Run `cargo test -p eu4game widget_cache`

**Deliverable:** Working `WidgetCache` ready to be used by generated rendering code.

### Phase 3: Generate Code for Left Panel (Proof of Concept)

**Goal:** Prove generated code works for one complex panel end-to-end.

**Task 3.1: Run code generator**
- Execute: `cargo xtask generate-gui-renderer --panel left`
- Verify output file created: `eu4game/src/generated/gui/left_panel.rs`
- Check generated code contains:
  - Method signature `render_left_panel_generated()`
  - Window anchor calculation
  - All widgets from left panel (observe button, back button, date controls, etc.)
  - Widget cache calls (`self.widget_cache.get_or_load_sprite()`)
  - Hit box registration

**Task 3.2: Fix compilation errors in generated code**
- Run `cargo check -p eu4game`
- Fix any syntax errors in code generator templates
- Fix any missing imports in generated file
- Iterate on generator until code compiles cleanly

**Task 3.3: Integrate generated method into renderer**
- Open `eu4game/src/gui/renderer.rs`
- Find `render_country_selection()` method
- Locate manual left panel rendering code (lines ~1963-2600)
- Add call to generated method ALONGSIDE existing code (don't delete yet):
  ```rust
  // In render_country_selection():
  if let Some(ref panel) = self.left_panel {
      // OLD CODE (keep for now)
      // ... existing manual rendering ...

      // NEW GENERATED CODE (test in parallel)
      self.render_left_panel_generated(
          panel,
          screen_size,
          sprite_renderer,
          render_pass,
          device,
          queue
      );
  }
  ```

**Task 3.4: Add feature flag for generated code**
- Add feature `generated-renderer` to `eu4game/Cargo.toml`
- Wrap generated code call in `#[cfg(feature = "generated-renderer")]`
- This allows A/B testing: manual vs generated rendering

**Task 3.5: Test generated rendering**
- Run tests with generated renderer: `cargo test -p eu4game --features generated-renderer`
- Verify observer mode tests pass:
  - `test_observer_button_exists_and_renders`
  - `test_observer_button_click_returns_toggle_action`
  - `test_observer_text_label_exists`
- Run `eu4game` binary with feature flag
- Manually verify all left panel widgets render correctly

**Task 3.6: Compare rendering output**
- Generate snapshots with manual renderer: `cargo test -p eu4game`
- Generate snapshots with generated renderer: `cargo test -p eu4game --features generated-renderer`
- Use image diff tool to verify pixel-perfect match
- Fix any rendering discrepancies in code generator

**Deliverable:** Left panel rendering via generated code, proven equivalent to manual code.

### Phase 4: Port All Panels to Generated Code

**Goal:** All panels use generated code, eliminating ~4000 lines of manual rendering.

**Task 4.1: Generate topbar renderer**
- Run: `cargo xtask generate-gui-renderer --panel topbar`
- Output: `eu4game/src/generated/gui/topbar.rs`
- Verify method `render_topbar_generated()` contains:
  - All topbar icons (treasury, manpower, stability, etc.)
  - Proper clipping calculations for variable-width displays
  - Text rendering for numeric values
- Test compilation: `cargo check -p eu4game`

**Task 4.2: Integrate topbar generated code**
- Find manual topbar rendering in `renderer.rs` (lines ~854-915)
- Add feature-flagged call to `render_topbar_generated()`
- Test with: `cargo test -p eu4game --features generated-renderer topbar`
- Compare snapshots to ensure identical rendering
- Once verified, remove manual code and feature flag

**Task 4.3: Generate speed controls renderer**
- Run: `cargo xtask generate-gui-renderer --panel speed_controls`
- Output: `eu4game/src/generated/gui/speed_controls.rs`
- Verify method `render_speed_controls_generated()` contains:
  - Speed buttons (pause, speed 1-5)
  - Current speed indicator
  - Proper positioning for lower-right corner
- Test compilation

**Task 4.4: Integrate speed controls generated code**
- Find manual speed controls rendering in `renderer.rs` (lines ~1048-1082)
- Replace with generated method call
- Test and verify snapshots match
- Remove manual code

**Task 4.5: Generate top panel renderer**
- Run: `cargo xtask generate-gui-renderer --panel top`
- Output: `eu4game/src/generated/gui/top_panel.rs`
- Verify all country selection top panel widgets included
- Test compilation and integration

**Task 4.6: Generate lobby controls renderer**
- Run: `cargo xtask generate-gui-renderer --panel lobby_controls`
- Output: `eu4game/src/generated/gui/lobby_controls.rs`
- Verify play button and other lobby controls
- Test compilation and integration

**Task 4.7: Enable generated renderer by default**
- Remove feature flag checks
- Make generated methods the only rendering path
- Update `eu4game/src/generated/gui/mod.rs` to export all panels
- Run full test suite: `cargo test -p eu4game`

**Deliverable:** All GUI rendering uses generated code, manual rendering code deleted.

### Phase 5: Integrate into Build Process

**Goal:** Automatic regeneration when .gui files change, seamless developer experience.

**Task 5.1: Start with manual xtask approach (Option A)**
- Document in README: "Run `cargo xtask generate-gui-renderer` after modifying .gui files"
- Commit generated files to git (following `regions.rs` pattern)
- Generated files are source of truth for rendering

**Task 5.2: Add CI verification**
- Add step to CI workflow: `cargo xtask generate-gui-renderer --verify`
- Implement `--verify` flag that checks if generated code is up-to-date
  - Run generator in-memory
  - Compare output to existing files
  - Fail if differences found
- This prevents committing stale generated code

**Task 5.3: (Optional) Migrate to build.rs automation**
- Create `eu4game/build.rs` if it doesn't exist
- Add dependency on `xtask` crate (or extract shared code to `eu4-codegen` library crate)
- Call GUI code generator during build:
  ```rust
  fn main() {
      // Detect if .gui files changed
      let frontend_gui = "path/to/frontend.gui";
      println!("cargo:rerun-if-changed={}", frontend_gui);

      // Run generator
      gui_codegen::generate_all().expect("Code generation failed");
  }
  ```
- Update `.gitignore`:
  ```
  eu4game/src/generated/gui/*.rs
  !eu4game/src/generated/gui/mod.rs
  ```
- Generated files are gitignored and rebuild automatically

**Task 5.4: Choose and document approach**
- Decide: Option A (manual + CI verify) or Option B (build.rs auto)
- Update project documentation with chosen approach
- Add developer guide section on code generation

**Deliverable:** Automated, maintainable code generation workflow.

### Phase 6: Remove Legacy Code

**Goal:** Clean, maintainable codebase driven entirely by .gui files. Delete ~4000 lines.

**Task 6.1: Remove manual rendering code from renderer.rs**
- Open `eu4game/src/gui/renderer.rs`
- Locate and delete:
  - Lines ~854-915: `render_topbar()` manual rendering
  - Lines ~1048-1082: Speed controls manual rendering
  - Lines ~1515-1689: Country select top panel manual rendering
  - Lines ~1963-2600: Left panel manual rendering
- Replace with calls to generated methods (already done in Phases 3-4)
- Verify file compiles: `cargo check -p eu4game`

**Task 6.2: Remove layout metadata fields from GuiRenderer struct**
- In `renderer.rs`, locate `GuiRenderer` struct definition
- Delete fields:
  - `speed_controls_layout: SpeedControlsLayout`
  - `topbar_layout: TopBarLayout`
  - `country_select_layout: CountrySelectLayout`
- Remove initialization code in `GuiRenderer::new()`
- Remove imports of these types

**Task 6.3: Remove bind group caches from GuiRenderer struct**
- Delete fields (all replaced by `widget_cache`):
  - `button_bind_groups: Vec<...>`
  - `topbar_icons: Vec<...>`
  - `frontend_button_bind_groups: Vec<...>`
  - `speed_icon_bind_groups: Vec<...>`
- Remove initialization code in `GuiRenderer::new()`
- Search for remaining references and update to use `widget_cache` instead

**Task 6.4: Delete obsolete files**
- Delete `eu4game/src/gui/layout_types.rs`
  - Contains `SpeedControlsLayout`, `TopBarLayout`, `CountrySelectLayout`
  - All obsolete, replaced by generated code
- Delete `eu4game/src/gui/panel_loaders.rs`
  - Contains extraction functions that flatten GUI trees to metadata
  - Obsolete, code generator does this at build time
- Remove module declarations from `eu4game/src/gui/mod.rs`

**Task 6.5: Verify deletion didn't break anything**
- Run: `cargo check --workspace`
- Run: `cargo test --workspace`
- Run: `cargo xtask ci`
- All should pass with no compilation errors

**Task 6.6: Run tokei and verify line count reduction**
- Before: `cargo xtask tokei -- eu4game/src/gui/renderer.rs`
- After: Should show ~4000 fewer lines across deleted files
- Celebrate! 🎉

**Deliverable:** Clean codebase with ~4000 lines of legacy rendering code removed.

## Benefits

1. **Borrow Checker Peace**: No more collect-then-execute patterns
   - Tree walking doesn't hold long-lived borrows
   - Widget cache access is isolated per widget

2. **Maintainability**: Adding a new widget is trivial
   - Add field to panel struct
   - Implement `BoundPanel` method
   - Widget renders automatically from tree

3. **Correctness**: Single source of truth
   - GUI tree from `.gui` file defines everything
   - No sync issues between tree/layout/panel

4. **Performance**: Lazy loading
   - Only cache what's actually rendered
   - No upfront loading of all widgets

5. **Simplicity**: ~4000 lines removed
   - Generic tree-walking replaces manual loops
   - One widget cache replaces 7+ separate caches

## Verification Plan

### Tests
- All existing tests should pass unchanged (they use the same GUI trees)
- Specifically verify:
  - `observer_mode_tests.rs` - observer button still clickable
  - `state_machine_tests.rs` - screen transitions work
  - Visual snapshots match (no rendering changes)

### Manual Testing
1. Run `eu4game` binary
2. Navigate to Single Player screen
3. Verify all buttons clickable (back, observe mode, date controls)
4. Verify text renders (observe mode label, date display)
5. Check hit boxes with debug overlay

### CI
```bash
cargo test --workspace
cargo xtask ci  # Full CI suite
```

## Critical Files by Phase

### Phase 0: Quick Fix
- `eu4game/src/gui/renderer.rs` - Add inline observer text rendering (~20 lines)

### Phase 1: Code Generator
- `xtask/src/gui_codegen/mod.rs` - New module (50 lines)
- `xtask/src/gui_codegen/types.rs` - Intermediate representation (150 lines)
- `xtask/src/gui_codegen/parser.rs` - GUI file parser (200 lines)
- `xtask/src/gui_codegen/codegen.rs` - Code generation logic (400 lines)
- `xtask/src/main.rs` - Add subcommand (20 lines)

### Phase 2: Widget Cache
- `eu4game/src/gui/widget_cache.rs` - New unified cache (200 lines)
- `eu4game/src/gui/renderer.rs` - Add `widget_cache` field (10 lines)

### Phase 3-4: Generated Code
- `eu4game/src/generated/gui/left_panel.rs` - Generated (300 lines)
- `eu4game/src/generated/gui/topbar.rs` - Generated (250 lines)
- `eu4game/src/generated/gui/speed_controls.rs` - Generated (150 lines)
- `eu4game/src/generated/gui/top_panel.rs` - Generated (200 lines)
- `eu4game/src/generated/gui/lobby_controls.rs` - Generated (100 lines)
- `eu4game/src/generated/gui/mod.rs` - Manual scaffolding (20 lines)

### Phase 5: Build Integration
- `.github/workflows/ci.yml` - Add verification step (5 lines)
- `eu4game/build.rs` - Optional build-time generation (30 lines)

### Phase 6: Deletions
- `eu4game/src/gui/renderer.rs` - Delete ~4000 lines of manual rendering
- `eu4game/src/gui/layout_types.rs` - DELETE entire file
- `eu4game/src/gui/panel_loaders.rs` - DELETE entire file

**Net result:** ~2000 lines removed, much more maintainable

## Risks & Mitigations

**Risk:** Tree-walking performance vs manual loops
- **Mitigation:** Tree depth is shallow (max 4-5 levels), cache lookup is O(1)

**Risk:** Breaking existing click detection
- **Mitigation:** Hit box registration stays the same, just in tree walker

**Risk:** Text rendering integration
- **Mitigation:** Port observer text first (simplest case), then expand

## Open Questions

None - architecture is clear based on eu4data patterns.

---

## Document Repository Location

This design document will be committed to the repository at:
- **Path:** `docs/design/gui-codegen-refactor.md`
- **Purpose:** Historical record and implementation guide
- **Updates:** Update status to "In Progress" / "Complete" as phases finish

The plan file serves as the single source of truth for this refactor effort.
