# EU4 GUI Layout Engine

This document describes the layout engine used to render EU4's authentic GUI elements.

## Design Principles

### Data-Driven Layout

**All layout values come from parsed game data files.** This is a critical principle:

- Positions, sizes, orientations, sprite names, font names, and border sizes are parsed from `.gui` and `.gfx` files
- Default values in `Default` implementations are **only fallbacks** when parsing fails
- Runtime render code uses parsed values, never hardcoded magic numbers
- This ensures UI mods work out of the box

### Why This Matters

- Different DLC/patches may change layouts
- Localization may affect text box sizes
- Mod compatibility requires data-driven approach
- Future features (scaling, themes) depend on this

## File Formats

### `.gfx` Files (Sprite Definitions)

Located in `interface/*.gfx`, these define sprite assets:

```
spriteTypes = {
    spriteType = {
        name = "GFX_speed_indicator"
        texturefile = "gfx/interface/speed_indicator.dds"
        noOfFrames = 10
    }
}
```

Key properties:
- `name`: Sprite identifier (e.g., `GFX_speed_indicator`)
- `texturefile`: Path to DDS/TGA texture
- `noOfFrames`: Number of frames in sprite strip (horizontal)

### `.gui` Files (Layout Definitions)

Located in `interface/*.gui`, these define UI structure:

```
guiTypes = {
    windowType = {
        name = "speed_controls"
        position = { x = 0 y = 0 }
        Orientation = "UPPER_RIGHT"

        iconType = {
            name = "icon_date_bg"
            position = { x = -254 y = -1 }
            spriteType = "GFX_date_bg"
            Orientation = "UPPER_RIGHT"
        }

        instantTextBoxType = {
            name = "DateText"
            position = { x = -227 y = 13 }
            font = "vic_18"
            maxWidth = 140
            maxHeight = 32
            borderSize = { x = 0 y = 4 }
            format = centre
            Orientation = "UPPER_RIGHT"
        }
    }
}
```

### `.fnt` Files (Bitmap Fonts)

EU4 uses BMFont format for text rendering:

```
info face="Adobe Garamond Pro" size=18
common lineHeight=18 base=13 scaleW=256 scaleH=256
char id=65 x=10 y=20 width=12 height=14 xoffset=0 yoffset=2 xadvance=11
```

Key properties:
- `lineHeight`: Height of a line of text in pixels
- `base`: Distance from top of cell to baseline
- Per-glyph: position in atlas, size, offsets, advance width

Texture filename is inferred from font filename: `vic_18.fnt` -> `vic_18.tga`

## Coordinate System

### Screen Coordinates

- Origin: Top-left of screen
- X: Increases rightward
- Y: Increases downward
- Units: Pixels

### Orientation Anchors

Elements use `Orientation` to specify their reference point:

| Orientation | Anchor Point |
|-------------|--------------|
| `UPPER_LEFT` | Screen top-left (0, 0) |
| `UPPER_RIGHT` | Screen top-right (width, 0) |
| `LOWER_LEFT` | Screen bottom-left (0, height) |
| `LOWER_RIGHT` | Screen bottom-right (width, height) |
| `CENTER` | Screen center |
| `CENTER_UP` | Top center |
| `CENTER_DOWN` | Bottom center |

### Position Interpretation

Positions are offsets from the anchor point:
- Positive X: rightward from anchor
- Negative X: leftward from anchor (common for `UPPER_RIGHT`)
- Positive Y: downward from anchor
- Negative Y: upward from anchor

Example: `position = { x = -227 y = 13 }` with `UPPER_RIGHT`:
- 227 pixels left of right edge
- 13 pixels down from top

## Layout Resolution

### Window Anchoring

Windows are **anchor points, not rectangles**. They have:
- A position relative to screen edge (based on orientation)
- No intrinsic size

```rust
pub fn get_window_anchor(
    window_pos: (i32, i32),
    orientation: Orientation,
    screen_size: (u32, u32),
) -> (f32, f32)
```

### Child Positioning

Child elements position relative to the window anchor:

```rust
pub fn position_from_anchor(
    anchor: (f32, f32),
    element_pos: (i32, i32),
    element_orientation: Orientation,
    element_size: (u32, u32),
) -> (f32, f32)
```

Each child has its own orientation that determines how its position offset is interpreted.

### Clip Space Conversion

Final positions are converted to GPU clip space:
- Top-left: (-1, 1)
- Bottom-right: (1, -1)

```rust
pub fn rect_to_clip_space(
    screen_pos: (f32, f32),
    size: (u32, u32),
    screen_size: (u32, u32),
) -> (f32, f32, f32, f32)
```

## Text Rendering

### BMFont System

Text uses pre-rendered bitmap font atlases:

1. Parse `.fnt` file for glyph metrics
2. Load corresponding `.tga` texture atlas
3. For each character:
   - Look up glyph in font
   - Calculate UV coordinates in atlas
   - Render quad with glyph texture

### Text Box Layout

Text boxes have:
- `maxWidth`, `maxHeight`: Bounding box size
- `borderSize`: Internal padding (x, y)
- `format`: Horizontal alignment (`left`, `centre`, `right`)

Text positioning:
- Horizontal: Centered within box (for `format = centre`)
- Vertical: `borderSize.y` offset from top (not centered)

## Module Structure

```
eu4game/src/gui/
├── mod.rs           # GuiRenderer, SpeedControls
├── parser.rs        # .gui/.gfx file parsing
├── types.rs         # GfxSprite, GuiElement, Orientation
├── sprite_cache.rs  # Texture loading with LRU cache
└── layout.rs        # Coordinate transformations
```

## Testing

Layout functions have unit tests with mock data:
- `test_upper_left`: Basic positioning
- `test_upper_right`: Negative offset positioning
- `test_to_clip_space`: Coordinate conversion
- `test_size_to_clip_space`: Size conversion

Run tests: `cargo test -p eu4game`
