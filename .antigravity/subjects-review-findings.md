# Subject Relationships Review Findings

**Reviewer**: Gemini 3 Pro (High) // A2 Unit
**Designation**: Implementation Review

## 1. Data Structure Design
**Question**: `HashMap<Tag, SubjectRelationship>` keyed by `Subject`?
**Verdict**: **Correct / Optimal**.
*   **Logic**: Relationships are N:1 (Overlord:Subjects). Keying by Subject ensures O(1) lookup for the most frequent check: "Is this tag a subject?" and "Who owns this tag?".
*   **Optimization**: Bitmasks (which you liked earlier) aren't applicable here since Tags are string-based (3-char). The Hasher overhead is unavoidable unless we intern Tags to integers globally (Tier 5 task).
*   **Validation**: Ensure `SubjectRelationship` struct contains the Overlord Tag.

## 2. War Restrictions (`in_same_realm`)
**Verdict**: **Critical Logic**.
*   **Definition**: A "Realm" is the Top Overlord + All Recursive Subjects.
*   **Recommendation**: Implement `get_top_overlord(tag) -> Tag`.
    *   If `get_top_overlord(A) == get_top_overlord(B)`, war is blocked.
*   **Tributary Exception**: Tributaries are *loose* subjects. They **are** allowed to fight other tributaries or even the overlord (independence war).
    *   *Correction*: `in_same_realm` must return `false` if the relationship is `Tributary`. They are effectively separate realms for war declarations.
Note from atv: Overlords of tributaries are allowed, but not required, to defend the tributary.

## 3. Answers to Open Questions

1.  **Colonial Nations**:
    *   **Phase 1**: Load from history **only**.
    *   **Phase 2**: Auto-formation requires defining "Colonial Regions" (map data) and checking 5-province ownership clusters. Too complex for initial pass.
Note from atv: We'll take care of CNs in another pass. But let's make sure any important things like the concept of a colonial nation is well defined.

2.  **Liberty Desire**:
    *   Start **Simplified**.
    *   Formula: `Base + (Total_Dev_Commit / Overlord_Dev) * Scale`.
    *   Ignore: Diplomatic Reputation, Trust, Events for now.
Note from atv: Let's document somewhere that this is unfinished, and we need these other values to complete. Simplefiend formula good for now.

3.  **Tributary Peace**:
    *   **Yes**, separate peace. They are not co-belligerents automatically.

4.  **Subject-of-Subject**:
    *   **Forbidden** in vanilla logic (except PUs, theoretically).
    *   **Enforce**: `add_subject(overlord, subject)` checking `!is_subject(overlord)` unless `subject_type == PU`.

5.  **Parsing Verification (`historydiplomacy.rs`)**:
    *   The schema uses generic `Vec<String>` fields but standard Paradox syntax is `key = { ... }` where logic depends on the key.
    *   **Action**: Determine if the `eu4data` parser preserves the outer key (e.g. `vassal = { ... }`). If the parser flattens everything into valid fields but loses the *type* (because `vassal` is the key, not a field), this will fail.
    *   **Risk High**: Verify this before implementation. If `eu4data` doesn't capture the bloc name, you can't distinguish a vassal from a guarantee.

## 4. Specific Review Requests

### Edge Cases
*   **PU Chains**: Austria -> Hungary -> Bohemia. `get_top_overlord` must be recursive.
*   **Integration**: Integrating a subject (Annex) transfers ownership of *their* subjects to you?
    *   *Rule*: Yes. If Austria inherits Hungary, Hungary's subjects become Austria's subjects.

## Summary
Plan is solid.
1.  Use `HashMap` key-by-subject.
2.  Implement recursive `get_top_overlord`.
3.  Treat Tributaries as independent for `same_realm` checks.
4.  **Double-check** `eu4data` parsing of the relationship *type*.
