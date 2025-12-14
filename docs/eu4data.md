# eu4data (Library)

`eu4data` is a library responsible for defining the domain model of Europa Universalis IV and providing the logic to load this data from game files.

## Scope

This library targets the *data definition* aspect of the game. It does not handle rendering or game simulation logic directly, but rather provides the "Static Data" that the game engine needs.

## Data Models

The library defines Rust structs that map to game data scopes.

### Examples

-   **`Province`**: Represents a single province (ID, tax, manpower, trade good, terrain).
-   **`Country`**: Represents a tag (e.g., FRA, ENG) with its color, ideas, and history.
-   **`TradeGood`**: Definitions of trade goods (Grain, Iron, etc.).
-   **`Religion`**: Definitions of religions and their colors.
-   **`Culture`**: Definitions of cultures and hashed color generation.
-   **`Localisation`**: Handling of `.yml` localisation files (UTF-8 with BOM) and language filtering.

## Mocking and Testing

To ensure robust testing without requiring a full game installation, `eu4data` includes capabilities to load data from mock strings or "virtual implementation" files.

> [!NOTE]
> The library supports `definition.csv` for map data, country loading, and a robust **Localisation** system supporting multiple languages.
