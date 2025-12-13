# eu4rs

![Rust CI](https://github.com/atvrager/eu4rs/actions/workflows/ci.yml/badge.svg)

A# eu4rs

**eu4rs** is an experimental open-source game engine reimplementation ("source port") for *Europa Universalis IV*.

## Goals
- **Performance**: High-performance rendering using `wgpu` (Vulkan/Metal/DX12).
- **Compatibility**: Load and play using original game assets (data-driven design).
- **Portability**: Native support for Windows and Linux.
- **Safety**: Built in Rust for memory safety and concurrency.

## Project Structure

This project is organized as a Cargo Workspace containing:

- **`eu4rs`**: The main command-line application that scans and processes EU4 files.
- **`eu4data`**: A library containing strong types and logic for EU4 game objects.
- **`eu4txt`**: A library (`crate`) providing a custom parser/tokenizer for the EU4 text format (Windows-1252 encoded).

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

# Render map to an image file (Headless)
cargo run -- snapshot --output my_map.png

# Parse and pretty-print files (verifies parsing logic)
cargo run -p eu4rs -- --pretty-print --eu4-path "path/to/specific/file.txt"
```

## features

- **Custom Parser**: Handles EU4's specific text format, including comments (`#`), whitespace-separated tokens, and `key=value` structures.
- **Encoding Support**: Automatically handles `WINDOWS_1252` encoding common in Paradox files.
- **Tolerant Parsing**: Designed to handle quirks in game files (mostly).
- **CI/CD**: Automated GitHub Actions pipeline for building, testing, and linting (`fmt`, `clippy`).
- **Statistics**: The `Filescanner` provides quick statistics on your installation.
- **Interactive Map**: A hardware-accelerated (Vulkan/wgpu) map viewer that renders the game's `provinces.bmp` with correct aspect ratio and windowing.
- **Headless Rendering**: Ability to render the map to an image file without opening a window (`--snapshot`), suitable for automated testing in CI.
- **Serde Support**: `eu4txt` implements `serde::Deserializer`, allowing direct mapping of game files to Rust structs (in `eu4data`).

## License

Apache-2.0
