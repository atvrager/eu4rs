# Formal Verification Vision

> **Status**: Conceptual / "Vibes" Level  
> **Target**: Post-Mid-term Goal  

This document outlines the vision for integrating formal methods into the `eu4rs` simulation. Our goal is to move beyond unit testing toward mathematical proofs of core simulation invariants.

## Why Formal Methods?

Grand strategy simulations are notoriously prone to "ghost bugs"—subtle desyncs or economic drifts that only manifest after hundreds of hours. By employing formal methods, we can:

1.  **Prove Determinism**: Mathematically guarantee that `step_world(s, i) = s'` is identical across all architectures.
2.  **Verify Algorithms**: Prove that pathfinding (A*) never loops and always finds the optimal path under a given cost function.
3.  **Ensure Economic Stability**: Prove that the global treasury cannot overflow and that production logic preserves conservation of mana/wealth where intended.

## The Verification Tiers

We envision a multi-tool approach to verification:

### Tier 1: SMT-Based Verification (Kani / Verus)
- **Goal**: Prove absence of panics, overflows, and out-of-bounds access in the Rust core.
- **Tooling**: [Kani](https://model-checking.github.io/kani/) or [Verus](https://github.com/verus-lang/verus).
- **Vibe**: "My code literally cannot crash."

### Tier 2: Algorithmic Proofs (Lean 4)
- **Goal**: Prove properties of high-level game logic.
- **Example**: "If a country has a truce with another, it is impossible for the `available_commands()` function to return a `DeclareWar` command for that target."
- **Tooling**: [Lean 4](https://lean-lang.org/).
- **Vibe**: "The rules of the game follow a consistent logic that cannot be subverted."

### Tier 3: Correct-by-Construction (Rocq/Coq)
- **Goal**: Re-implement core math libraries (Fixed-point, Date) in a proof assistant and extract them to verified Rust/C.
- **Tooling**: [Rocq (formerly Coq)](https://coq.inria.fr/).
- **Vibe**: "The very foundations of the simulation are built on mathematical truth."

## Roadmap for "Formal Vibes"

1.  **Specification**: Formally specify the `WorldState` invariants using Z-notation or high-level TLA+.
2.  **Bounded Model Checking**: Introduce Kani harnesses for the `fixed.rs` math routines to prove no overflow occurs with standard game values.
3.  **Lean 4 Integration**: Experiment with `lean-rust` bridges to prove that the War Score calculation correctly sums to 100% across all provinces.

---

"No matter how far you go, never forget your foundations." — *Hazuki Dojo Wisdom*
