# Terrain Rendering Improvement Plan

## 1. Problem Statement
The current "RealTerrain" mode renders `terrain.bmp` directly as a color texture. Since `terrain.bmp` is an **indexed image** (where distinct colors represent terrain types like "grass", "desert", "forest"), the result is a low-resolution, cartoonish "splat map" that looks blurry and lacks detail, especially when zoomed in.

## 2. Technical Architecture of EU4 Maps

EU4 uses a multi-layered approach to render its map:

| Component | File | Resolution | Format | Purpose |
|-----------|------|------------|--------|---------|
| **Heightmap** | `heightmap.bmp` | 1x (5632x2048) | Grayscale | Terrain elevation (0-255). |
| **Normal Map** | `world_normal.bmp` | 0.5x | RGB | Per-pixel normals for detailed lighting and shading. |
| **Splat Map** | `terrain.bmp` | 1x | Indexed 8-bit | Defines *which* terrain type is at each pixel (index -> terrain type). |
| **Colormap** | `colormap_*.dds` | 0.5x | DXT1/RGB | Global low-frequency color overlay (seasonality, regional tint). |
| **Water Map** | `colormap_water.dds`| 0.5x | DXT1/RGB | Base color for water provinces. |
| **Atlas** | `atlas0.dds` | High | DXT5/ARGB | Texture atlas containing tiling detail textures (grass, rock, etc.). |

## 3. Implementation Plan

### Phase 1: Enhanced Lighting & Water ✅ **COMPLETE**
**Goal:** Remove the "flat" look and give the map proper depth and global color coherence.

1.  **Normal Mapping**:
    *   Load `map/world_normal.bmp`.
    *   Bind to shader (Group 0, Binding 10).
    *   Update `shader.wgsl` to sample normals from this texture instead of approximating from the heightmap gradient. This fixes the "smooth/blurry" lighting.
2.  **Water Rendering**:
    *   Load `map/terrain/colormap_water.dds`.
    *   In the shader, if `province_is_sea`, sample from this colormap instead of looking up a static blue.
    *   Add distinct specular highlights for water.
3.  **Global Colormap**:
    *   Load `map/terrain/colormap_autumn.dds` (or a specific season).
    *   Multiply the terrain color by this global colormap to add regional variety (e.g., darker forests in north, sandy tint in deserts).

### Phase 2: Texture Splatting (The "Real" Fix) ✅ **COMPLETE**
**Goal:** Replace the low-res splat map colors with high-res tiling textures.

1.  **Texture Atlas Loading**:
    *   Load `map/terrain/atlas0.dds` and `map/terrain/atlas_normal0.dds`.
    *   These contain the actual "Grass", "Drylands", "Snow" textures arranged in a grid.
2.  **Terrain Index Parsing**:
    *   Parse `map/terrain.txt` to map `terrain.bmp` indices to Atlas Coordinates.
    *   Implemented heuristic mapping for common terrain types.
3.  **Shader Splatting Logic**:
    *   Sample `terrain.bmp` to get the terrain **index**.
    *   Sample the **Atlas** using UVs based on world position (tiling) + Index offset.
    *   Combined world normals with detail normals for micro-shading.

### Phase 3: Seasonal Cycles (Polish)
**Goal:** Dynamic seasons.

1.  Load all 4 seasonal colormaps (`autumn`, `winter`, `spring`, `summer`).
2.  Bind all (or interpolate CPU-side and bind one).
3.  Based on game date, blend between them.
4.  Implement "Snow Overlay" separate from colormap using `climate.txt` data.

## 4. Immediate Next Steps (Action Items)

We have completed **Phase 2**, adding high-detail tiling textures to the RealTerrain map mode. The next focus is **Phase 3**, adding dynamic seasonal transitions and potentially high-quality borders.

1.  **Phase 2: Texture Splatting**:
    *   Implement `map/terrain.txt` parser to map indices to atlas positions.
    *   Load `atlas0.dds` and `atlas_normal0.dds`.
    *   Update shader to perform tiling texture lookup based on terrain index.
    *   Implement multi-sampling/blending between terrain types.
