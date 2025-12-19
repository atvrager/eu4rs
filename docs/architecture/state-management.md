# State Management: Functional Core, Imperative Shell

This document describes our architectural approach to state management in the `eu4rs` simulation, specifically the transition to **Persistent Data Structures** and the **Functional Core, Imperative Shell** pattern.

## The Problem: Mutability vs. Scalability

In a complex simulation like EU4, we face two competing requirements:
1.  **Performance**: We need to perform thousands of updates per tick (economy, movement, etc.).
2.  **Safety & Observability**: We need cheap snapshots for AI simulations, UI rendering without locking, and deterministic replays.

Standard mutable state (`std::collections::HashMap`) makes "cloning the world" an $O(N)$ operation, where $N$ is the number of provinces/countries. As the game grows, `state.clone()` becomes a major bottleneck.

## The Solution: Persistent Data Structures (`im` crate)

We utilize the `im` crate to replace standard collections with **Persistent Data Structures** (specifically Indexed Trees/Bitmapped Vector Tries).

### Key Properties
-   **Cheap Clones**: Cloning a `WorldState` is now $O(1)$. It simply increments an internal reference count.
-   **Structural Sharing**: When you modify one element in a map of 3,000 provinces, only the path to that element in the tree is copied ($O(\log N)$). The rest of the provinces are shared with the previous version.
-   **Thread Safety**: `im::HashMap` is `Send + Sync`. You can hand a snapshot of the world to a background thread for pathfinding while the main simulation loop continues.

## Pattern: Functional Core, Imperative Shell

While our state is persistent, Rust's borrow checker and logic often prefer an imperative style. We balance this using:

### 1. Functional Core (Persistence)
The `WorldState` is essentially a collection of immutable pointers. Every tick produces a "new" version of the world, but technically it's a "modified view" of the old world.

### 2. Imperative Shell (Systems)
Our simulation systems use `&mut WorldState` for ergonomics.

```rust
pub fn run_taxation_tick(state: &mut WorldState) {
    // We collect keys first because im::HashMap doesn't support in-place iter_mut()
    let tags: Vec<_> = state.countries.keys().cloned().collect();
    
    for tag in tags {
        // .get_mut() works via "copy-on-write" at the tree node level
        if let Some(country) = state.countries.get_mut(&tag) {
            country.treasury += income;
        }
    }
}
```

## Best Use Cases

| Scenario | Recommendation |
| :--- | :--- |
| **AI "What-If"** | Excellent. AI can clone the world, simulate a battle, and discard it without any overhead. |
| **Multithreading** | Mandatory. Hand off a `WorldState` reference to a thread pool without mutexes. |
| **Undo/Timeline** | Natural. Storing the state of every day for a year only costs slightly more memory than the current state. |
| **Hot-Path Iteration** | Use with caution. Iteration is slightly slower than `std::HashMap`. If a system only **reads** data, prefer `std` or flat arrays. |

## Future Directions
-   **Multithreaded AI**: Implementing the decision loop in parallel using `rayon`, which is now trivial since the state is immutable.
-   **Deltas**: Using structural sharing to compute "what changed" between two ticks for efficient network syncing.
