# User Interface System

The `eu4rs` UI is an immediate-mode-like overlay system integrated directly into the `wgpu` rendering pipeline. This document details its architecture, state management, and rendering strategy.

## Architecture

The UI is managed by the `UIState` struct in `eu4rs/src/ui.rs`. It operates independently of the core game logic (`AppState`), handling transient visual elements such as tooltips, sidebars, and map mode indicators.

### Key Components

*   **`UIState`**: The source of truth for the UI. It tracks:
    *   **Sidebar Visibility**: Boolean flag for the right-hand province details panel.
    *   **Selection**: The currently selected province ID and its descriptive text.
    *   **Hover**: Transient tooltip text for the province under the cursor.
    *   **Cursor Position**: Used for tooltip placement and interactive checks.
    *   **Map Mode**: The active visualization mode (Province, Political, etc.).
    *   **Dirty Flag**: An optimization flag to signal when the UI texture needs regeneration.

*   **`draw_ui` Function**: A pure function that takes the `UIState` and renders it into an `RgbaImage`. This image is then uploaded to a GPU texture.

*   **`Eu4Renderer` Integration**:
    *   Uses a dedicated `wgpu::RenderPipeline` with alpha blending (`BlendState::ALPHA_BLENDING`) to overlay the UI on top of the map.
    *   Manages a separate `ui_texture` and `ui_bind_group`.

## Interaction Model

The UI system has precedence over map interactions.

1.  **Event Handling (`State::input`)**: Mouse events are first passed to `ui_state.on_click(x, width)`.
2.  **Consumption**: If `on_click` returns `true` (e.g., clicking on the sidebar), the event is consumed, and no raycasting or map selection occurs.
3.  **Map Passthrough**: If the UI ignores the event, it propagates to the map logic for panning, zooming, or province selection.

## Rendering Pipeline

1.  **State Update**: Inputs update `UIState`. If a change occurs (e.g., hover text changes), `dirty` is set to `true`.
2.  **Texture Update**: In `State::render`, if `ui_state.dirty` is true:
    *   `draw_ui` is called to generate a new software image.
    *   The image is uploaded to the `ui_texture` on the GPU.
    *   `dirty` is reset to `false`.
3.  **Draw**: The `ui_pipeline` draws a full-screen quad textured with the UI overlay after the map draw call.

## Coordinate System

*   **Sidebar**: Fixed width (300px), anchored to the right edge.
*   **Tooltips**: positioned relative to the bottom-left corner.
*   **Map Mode Indicator**: Fixed position in the top-left corner.
*   **Scaling**: Currently assumes 1:1 pixel mapping with the physical window resolution (1920x1080 preferred).
