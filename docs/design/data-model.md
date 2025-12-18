# Data Layout

This document describes the directory structure of *Europa Universalis IV*.

## Core Data Directories
These are the primary folders `eu4rs` interacts with for game logic and static data.

### `common/`
Static definitions of game concepts. Contains hundreds of subdirectories defining:
-   **`country_tags/`**: Tag definitions.
-   **`cultures/`**: Culture definitions.
-   **`religions/`**: Religion definitions.
-   **`tradegoods/`**: Trade goods.
-   **`technologies/`**: Tech groups and levels.
-   **`governments/`**: Government forms.
-   **`buildings/`**: Building types.
-   **`ideas/`**: National ideas.
-   **`policies/`**: Policies.

### `history/`
Dynamic historical data.
-   **`countries/`**: Country history (monarchs, capital, religion).
-   **`provinces/`**: Province history (owner, tax, buildings).
-   **`wars/`**: Historical wars.
-   **`diplomacy/`**: Historical alliances and unions.
-   **`advisors/`**: Historical advisors.

### `map/`
Map definitions.
-   **`provinces.bmp`**: World map pixels.
-   **`definition.csv`**: Color-to-ID mapping.
-   **`default.map`**: Map configuration.
-   **`adjacencies.csv`**: Strait crossings.
-   **`terrain.bmp`** / **`rivers.bmp`** / **`heightmap.bmp`**: Visual map layers.
-   **`positions.txt`**: Coordinates for unit models, cities, and ports.

### `localisation/`
Text translations.
-   **`.yml` files**: Key-value pairs for localizing text into English, French, German, Spanish.

## Scripted Logic
Files that define dynamic behavior and events.

### `events/`
-   Scripted events (pop-ups) triggered by MTTH (Mean Time To Happen) or on_actions.

### `decisions/`
-   National decisions (buttons) available to the player.

### `missions/`
-   Mission trees and mission definitions.

## Assets & Interface

### `gfx/`
Visual assets.
-   **`interface/`**: UI sprites (`.dds`).
-   **`flags/`**: Country flags (`.tga`).
-   **`fonts/`**: Bitmap fonts.
-   **`models/`**: 3D meshes and animations (`.mesh`).

### `interface/`
-   **`.gui` files**: UI layout definitions (syntax similar to PDS script but for UI widgets).

### `sound/` & `music/` & `soundtrack/`
-   **`sound/`**: SFX definitions (`.asset`) and `.wav` files.
-   **`music/` / `soundtrack/`**: Music tracks (`.ogg`) and playlist definitions.

## Miscellaneous / Other
Directories we typically ignore but exist in the structure.

-   **`chras_reporter/`**: Crash dumps.
-   **`dlc/`**: DLC metadata and archives.
-   **`hints/`**: Old tutorial hint text.
-   **`tutorial/`**: Scripted tutorial logic.
-   **`tests/`**: Internal PDS tests.
-   **`tools/`**: Modding tools (e.g., Clausewitz map editor configs).
-   **`legal_notes/`** & **`licenses/`**: Legal info.
