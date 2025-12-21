# eu4rs

![Rust CI](https://github.com/atvrager/eu4rs/actions/workflows/ci.yml/badge.svg)

**eu4rs** is an experimental open-source game engine reimplementation ("source port") for *Europa Universalis IV*.

## Goals
- **Performance**: High-performance rendering using `wgpu` (Vulkan/Metal/DX12).
- **Compatibility**: Load and play using original game assets (data-driven design).
- **Portability**: Native support for Windows and Linux.
- **Safety**: Built in Rust for memory safety and concurrency.

## Project Structure

This project is organized as a Cargo Workspace containing:

- **`eu4viz`**: The main visualizer application for viewing and interacting with EU4 maps.
- **`eu4data`**: A library containing strong types and logic for EU4 game objects.
- **`eu4txt`**: A library (`crate`) providing a custom parser/tokenizer for the EU4 text format (Windows-1252 encoded). See [docs/file_formats.md](docs/file_formats.md).

## Installation

Ensure you have [Rust](https://www.rust-lang.org/tools/install) installed.

```bash
git clone <repository-url>
cd eu4rs
cargo build --release
```

## Usage


# Launch the Interactive Map (GUI) - Recommended default
cargo run

### Controls
- **Zoom**: Scroll Wheel (Up/Down)
- **Pan**: Middle Mouse Button (Hold & Drag)
- **Inspect**: Left Click on a province to view details
- **Toggle Map Mode**: Press `Tab` to cycle through map modes (Province, Political, Trade Goods, Religion, Culture).

# Render map to an image file (Headless)
```bash
cargo run -- snapshot --output province_map.png --mode province
cargo run -- snapshot --output political_map.png --mode political
cargo run -- snapshot --output tradegoods_map.png --mode trade-goods
cargo run -- snapshot --output religion_map.png --mode religion
cargo run -- snapshot --output culture_map.png --mode culture
```

# Parse and pretty-print files (verifies parsing logic)
```bash
cargo run -p eu4viz -- --pretty-print --eu4-path "path/to/specific/file.txt"
```

## features

- **Custom Parser**: Handles EU4's specific text format, including comments (`#`), whitespace-separated tokens, and `key=value` structures.
- **Encoding Support**: Automatically handles `WINDOWS_1252` encoding common in Paradox files.
- **Tolerant Parsing**: Designed to handle quirks in game files (mostly).
- **CI/CD**: Automated GitHub Actions pipeline for building, testing, and linting (`fmt`, `clippy`).
- **Statistics**: The `Filescanner` provides quick statistics on your installation.
- **Interactive Map**: A hardware-accelerated (Vulkan/wgpu) map viewer that renders the game's `provinces.bmp` with correct aspect ratio and windowing.
- **Political Map Mode**: Visualizes country ownership with borders and filling based on game data.
- **Headless Rendering**: Ability to render the map to an image file without opening a window (`--snapshot`), suitable for automated testing in CI.
- **Data Expansion**: Parses and visualizes complex game data including **Religion**, **Culture**, and **Trade Goods**.
- **Serde Support**: `eu4txt` implements `serde::Deserializer`, allowing direct mapping of game files to Rust structs (in `eu4data`).

## Documentation

- [System Architecture](docs/design/architecture.md): Overview of crate structure, rendering pipeline, and data flow.
- [File Formats](docs/reference/file-formats.md): Details on the EU4 text format, encoding, and syntax.
- [Data Coverage](docs/development/testing/coverage.md): How we track Parse/Visualize/Simulate progress across game data.
- [Supported Fields](docs/reference/supported-fields.md): Auto-generated matrix of every known EU4 data field.

## License

Apache-2.0
