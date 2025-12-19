# eu4rs Documentation

Welcome to the comprehensive documentation for **eu4rs**, a Rust-based source port and toolset for *Europa Universalis IV*.

## Table of Contents

### 📐 Design Documents
High-level system architecture and design decisions.

- **[Architecture](design/architecture.md)** - System overview, crate structure, rendering pipeline
- **[Data Model](design/data-model.md)** - Data structures, storage layout, and relationships
- **[Type System](design/type-system.md)** - Type inference, fixed-point arithmetic, determinism
- **[UI/UX Design](design/ui.md)** - User interface and visualization design
- **[Game Lobby System](design/lobby.md)** - Networking, P2P architecture, State machines

#### Simulation Subsystems
- **[Simulation Overview](design/simulation/overview.md)** - Core simulation architecture and pure functional model
- **[Integrity System](design/simulation/integrity.md)** - Determinism, checksums, multiplayer validation
- **[Economic Model](design/simulation/economic-model.md)** - Production, taxation, trade mechanics
- **[Military System](design/simulation/military.md)** - Combat, movement, pathfinding, naval transport
- **[Calendar System](design/simulation/calendar.md)** - Temporal logic and non-Gregorian extensions
- **[CLI Pipeline](design/cli-pipeline.md)** - Argument parsing, profiles, and runner configuration
- **[State Management](architecture/state-management.md)** - Persistent data structures and functional core patterns

---

### 📚 Technical Reference
Detailed technical specifications and API documentation.

#### Crate Documentation
- **[eu4data](reference/crates/eu4data.md)** - Game data loading library
- **[eu4txt](reference/crates/eu4txt.md)** - EU4 text format parser
- **[eu4viz](reference/crates/eu4viz.md)** - Visualization and rendering binary

#### Game Data Formats
- **[File Formats](reference/file-formats.md)** - EU4 file format specifications
- **[Supported Fields](reference/supported-fields.md)** - Coverage matrix for EU4 data structures
- **[Tolerant Deserialization](reference/tolerant-deserialize.md)** - Parsing strategy for partial/evolving data

---

### 🛠️ Development Guides
Tools, workflows, and best practices for contributors.

#### Testing
- **[Property-Based Testing](development/testing/property-based-testing.md)** - Simulation verification philosophy (SVA analogy)
- **[Code Coverage](development/testing/coverage.md)** - Coverage targets, tools, and metrics

#### Tools & Automation
- **[Code Generation](development/code-generation.md)** - Auto-codegen from EU4 data schemas
- **[Code Statistics](development/code-statistics.md)** - Metrics, reporting, and visualization
- **[Performance Measurement](development/performance.md)** - Benchmarking, metrics, and profiling

---

### 📋 Planning & Roadmap
Project status, upcoming features, and backlog.

- **[Roadmap](planning/roadmap.md)** - Implementation phases and current status
- **[Future Features](planning/future-features.md)** - Deferred features and ideas

---

## Quick Start

New to the project? Start here:

1. **[Architecture](design/architecture.md)** - Understand the overall system
2. **[Simulation Overview](design/simulation/overview.md)** - Learn how the simulation works
3. **[Roadmap](planning/roadmap.md)** - See what's implemented and what's next

## Contributing

Before implementing a new feature:

1. Check **[Roadmap](planning/roadmap.md)** for current phase priorities
2. Review **[Property-Based Testing](development/testing/property-based-testing.md)** for testing philosophy
3. Follow the **Design by Invariants** workflow for simulation systems

## Philosophy

This project prioritizes:

- **Determinism**: Lockstep simulation for multiplayer/replay
- **Verification**: Property-based testing as continuous assertion monitoring
- **Simplicity**: Avoid over-engineering; solve the current problem
- **Documentation**: Explain *why*, not just *what*

---

*Last updated: 2025-12-19*
