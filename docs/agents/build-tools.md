# Build Performance Tools

> **Reference doc** — See [AGENTS.md](../../AGENTS.md) for core rules.

These tools significantly speed up local builds. They're optional but recommended.

## sccache (Compiler Cache)

Caches compiled crates across projects. Survives `cargo clean`. Like GitHub CI's cache layer.

```bash
# Install
cargo install sccache

# Enable globally (add to shell profile or run once per session)
export RUSTC_WRAPPER="sccache"

# Or add to your local .cargo/config.toml (NOT checked in):
# [build]
# rustc-wrapper = "sccache"
```

> **Note**: Don't add `rustc-wrapper` to the checked-in config.toml — it breaks builds for devs without sccache.

## cargo-nextest (Faster Test Runner)

~3x faster than `cargo test` due to better parallelism. Drop-in replacement.

```bash
cargo install cargo-nextest
cargo nextest run   # instead of cargo test
```

## tokei (Code Statistics)

Fast, accurate lines-of-code counter written in Rust. Pair with `tera-cli` for HTML reports. See [`docs/development/code-statistics.md`](../development/code-statistics.md) for detailed usage.

```bash
# Install
cargo install tokei tera-cli

# Quick stats (console)
tokei

# JSON output for HTML generation
tokei --output json > stats.json
```

---
*Last updated: 2025-12-23*
