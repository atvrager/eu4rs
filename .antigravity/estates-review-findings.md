# Estate System Review Findings

**Reviewer**: Gemini 3 Pro (High)
**Designation**: Architectural Blocker Check

## Critical Blocker: Missing Government State
**Question**: "How to determine initial estates based on government type?"
**Finding**: **We cannot currently do this.**
I audited `CountryState` in `state.rs` and it **LACKS** a `government_type` field.
*   We track `treasury`, `manpower`, `tech`, `ideas`, etc.
*   We do **not** track `Monarchy`/`Republic` or specific reforms.
*   We do **not** have a `GovernmentTypeId` definition in `eu4sim-core`.

**Action Required**:
Before implementing Estates, you must implement **Government State**:
1.  Add `government_type: GovernmentTypeId` to `CountryState`.
2.  Add `reforms: HashSet<ReformId>` to `CountryState`.
3.  Ensure these are populated from history files during `eu4sim-verify/hydrate`.

## 1. Availability Logic (Option C Evaluation)
**Verdict**: **Best Approach, but Hardcoding is Mandatory**.
Since `eu4data`'s generated `Estates` struct (which I inspected) **does not expose the `trigger` block**, we cannot parse script conditions.
*   **Result**: We *must* hardcode the availability logic for MVP.
*   **Design**:
    ```rust
    fn get_available_estates(gov: &GovernmentState, religion: &str) -> Vec<EstateTypeId> {
        let mut list = vec![];
        if gov.is_monarchy() { list.push(NOBLES); }
        if !gov.is_pirate() && !gov.is_native() { list.push(CLERGY); list.push(BURGHERS); }
        // ... specific hardcoded logic
        list
    }
    ```

## 2. Answers to Questions

1.  **Is `estates_enabled: bool` sufficient?**
    *   **No**. You need `Vec<EstateTypeId>` (the "Available" list).
    *   Example: Poland has Nobles/Clergy/Burghers. The Papal State has *no* Nobles. Using a simple boolean "enabled" implies "all or nothing", which is incorrect.

2.  **Edge Case Handling (Tribal/Native/Pirate)**
    *   **Pirates**: No estates (until certain gov't reforms are passed).
    *   **Natives**: No estates (until reformed).
    *   **Revolutionary**: Replaces Nobles with Girondists/Jacobins etc.
    *   **Implementation**: This logic belongs in the `get_available_estates` function proposed above, not in the `EstateTypeDef` (since we can't parse the triggers).

3.  **Government Changes**
    *   **Requirement**: When government changes (reform/revolution), you **must** call `recompute_available_estates()`.
    *   **State Transition**:
        *   If an estate becomes unavailable: Refund land share to Crown Land (or distribute to others?). EU4 usually just deletes it and resets land share.
        *   If an estate becomes available: Initialize with base loyalty/influence (usually 50%).

## 3. Data Structure Refinement
Usage of `HashMap<EstateTypeId, EstateState>` is correct.
Suggest adding:
```rust
pub struct EstateState {
    // ...
    pub active_agenda: Option<AgendaId>, // For "Diet" mechanics
    pub interaction_cooldowns: HashMap<InteractionId, Date>, // Seize Land, etc.
}
```

## Summary & Next Steps
1.  **STOP**: Do not implement Estates yet.
2.  **PREREQUISITE**: Implement `GovernmentState` (Type + Reforms) and its history hydration.
3.  **PROCEED**: Once `CountryState` has government data, implement Option C using hardcoded mapping logic (as parsing triggers is infeasible).
