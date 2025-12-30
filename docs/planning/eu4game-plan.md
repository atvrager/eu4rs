# EU4 Source Port - Game Binary Plan

## Core Philosophy

**Performance is non-negotiable.** This is a game - if it stutters, lags, or freezes, it's worthless. Every rendering decision must consider:
- GPU over CPU for pixel manipulation
- Shader-based computation over CPU loops
- Incremental updates over full regeneration
- Data-oriented design for cache efficiency

If EU4 can do it smoothly on 2013 hardware, we have no excuse.

## Overview

Create `eu4game` - a playable EU4 source port using the existing simulation engine and EU4's graphical assets.

## Architecture Decision

**New crate `eu4game`** (not extending eu4viz)
- eu4viz is a debug/replay tool; mixing game logic violates SRP
- Clean separation: eu4viz for devs, eu4game for players
- Can reuse rendering patterns from eu4viz without coupling

## Technical Stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| GPU | wgpu 0.20.0 | Already integrated, abstracts Vulkan/Metal/DX12 |
| Window/Input | winit 0.29.15 | Already integrated, handles all input |
| UI Framework | **Custom** | Use EU4's actual UI assets (DDS sprites, fonts) for authentic look |
| Textures | image-dds (new) | Load EU4's DDS textures |
| Text | ab_glyph + EU4 fonts | Load EU4's TTF fonts, render with existing glyph system |
| Simulation | eu4sim-core | Existing simulation engine |

## Rendering Approach

**GPU-First Philosophy**: All map rendering happens in shaders. CPU never touches pixels.

### Map Rendering Pipeline (GPU-based)
```
GPU Textures:
  provinces.bmp  -> Province ID texture (R16_UINT or encoded RGB)
  lookup_tex     -> Province ID -> Color Index (small, updated on ownership change)
  palette_tex    -> Color Index -> RGBA (country colors)

Fragment Shader:
  1. Sample province ID from provinces texture
  2. Look up color index from lookup texture
  3. Look up final color from palette
  4. Sample neighbors for border detection
  5. Output final pixel color
```

This approach means:
- Province ownership changes = update ~4KB lookup texture (not 11M pixels)
- Border detection = 4 texture samples in shader (free)
- Army markers = instanced quads (not CPU pixel loops)
- Map mode switches = swap lookup texture binding

### Layered Rendering
1. **Terrain Layer** - Heightmap shading from `terrain.bmp` + DDS textures
2. **Province Layer** - Political/diplomatic colors via shader lookup
3. **Border Layer** - Detected in fragment shader (neighbor sampling)
4. **Unit Layer** - Instanced sprite rendering for armies/fleets
5. **UI Layer** - Custom widgets rendered with EU4 assets

## Game Loop Design

```
Main Thread (winit)              Sim Thread
    |                                |
    +-- Input handling               +-- step_world() loop
    +-- Update lookup textures       +-- Controlled by SimSpeed
    +-- GPU rendering (fast!)        +-- Sends Snapshot via channel
    +-- Present frame
```

**SimSpeed**: Pause, 1x, 2x, 3x, 5x (matches EU4)

## Implementation Phases

### Phase A: Skeleton [DONE]
- [x] Create `eu4game` crate, add to workspace
- [x] Copy renderer patterns from eu4viz (Texture, pipeline)
- [x] Basic winit loop with wgpu
- [x] Load and display `provinces.bmp`
- [x] Camera pan/zoom

### Phase B: Core Game Loop [DONE]
- [x] Spawn sim thread with `step_world()`
- [x] Channel communication (SimControl, SimEvent)
- [x] SimSpeed controls (Space = pause, 1-5 = speeds)
- [x] Province click selection

### Phase C: Player Interaction [DONE]
- [x] Country selection screen at startup
- [x] Input state machine (Normal, MoveArmy, DeclareWar, MovingFleet)
- [x] Political map with country colors
- [x] Province borders
- [x] Army markers (GPU instanced)
- [x] Army movement orders
- [x] War declaration
- [x] Fleet spawning near major ports
- [x] Fleet selection and movement (F key)
- [x] Fleet markers (GPU instanced, diamond shape)

### Phase C.5: GPU Map Rendering [DONE]
- [x] Province ID texture (RG8 encoding: R=low byte, G=high byte)
- [x] Lookup texture (province ID -> RGBA color, 8192x1)
- [x] Shader: sample province ID, lookup color
- [x] Shader: neighbor sampling for borders (4-neighbor detection)
- [x] Instanced rendering for army markers (squares)
- [x] Instanced rendering for fleet markers (diamonds)
- [x] Update lookup texture on ownership changes

**Exit criteria met**: Map renders at 60fps with no CPU pixel work.

### Phase D: Visual Polish [NEXT]
- [ ] DDS texture loading (image-dds crate)
- [ ] Terrain textures from EU4
- [ ] Unit sprites on map (instanced)
- [ ] Selection highlights
- [ ] 9-slice panels with EU4 textures
- [ ] Button/checkbox widgets with EU4 sprites
- [ ] Country flags in UI
- [ ] Province info panel (sidebar)
- [ ] Load EU4 fonts, render text
- [ ] Top bar: date display, speed buttons

### Phase E: Full Game Flow
- [ ] Save/load game state (serde)
- [ ] Main menu (New Game, Load, Settings, Exit)
- [ ] Settings persistence
- [ ] Ledger panels (economy, military, diplomacy)

## Crate Structure

```
eu4game/
├── Cargo.toml
└── src/
    ├── main.rs           # Entry point, winit event loop
    ├── camera.rs         # 2D camera with pan/zoom
    ├── sim_thread.rs     # Simulation runner, speed control
    ├── input.rs          # Input state machine (Normal, MoveArmy, etc.)
    ├── render.rs         # GPU rendering (shaders, textures)
    ├── shader.wgsl       # Map rendering shader
    └── (future: ui/, assets/)
```

## Keybindings

| Key | Action |
|-----|--------|
| Space | Pause/Resume |
| 1-5 | Sim speed |
| M | Move selected army |
| F | Move selected fleet |
| W | Declare war mode |
| Escape | Cancel / close panel |

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| ~~CPU rendering too slow~~ | ~~Priority: Move to GPU shaders ASAP~~ **RESOLVED** - GPU rendering implemented |
| Province ID encoding | Used RG8 packing (R=low, G=high byte) - works well |
| DDS format variants | Test image-dds early; fallback to manual BC7 decoder |
| Lookup texture updates | Keep small (~32KB for 8192 provinces), update on ownership change |

## Not In Scope (v1)

- Audio/music
- Mod loading
- Multiplayer
- Save file compatibility with real EU4
- Custom nation designer
