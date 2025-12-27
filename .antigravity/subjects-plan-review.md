# Subject Relationships Plan - Gemini Review Request

Please review the implementation plan at:
**`/home/atv/.claude/plans/witty-kindling-koala.md`**

## Context

We're adding diplomatic subject relationships (vassals, PUs, marches, tributaries, colonial nations) to the EU4 simulation. Currently the sim allows declaring war on anyone - including what would be your own vassals. This needs to be fixed.

## Key Design Decisions to Review

1. **Data structure**: `HashMap<Tag, SubjectRelationship>` keyed by subject tag
2. **War restrictions**: `in_same_realm()` check blocks all in-realm wars
3. **Subject types**: Vassal, March, PersonalUnion, ColonialNation, Tributary, ClientState
4. **War participation**: Based on `SubjectType::joins_offensive_wars()` / `joins_defensive_wars()`

## Open Questions for Your Review

1. **Colonial nations**: Auto-form when colonizing? Or just load from history?
2. **Liberty desire**: Full formula or simplified version?
3. **Integration speed**: Fixed time or development-based?
4. **Tributary peace**: Separate peace since they don't join wars?
5. **Subject-of-subject**: EU4 forbids - should we validate?
6. **Vassal alliances**: What happens to vassal's alliances when overlord declares war?
7. **Subject diplomacy restrictions**: Vassals can't declare wars - enforce this?
8. **Missing subject types**: Daimyo, Shogunate, Trade Company, etc.?

## Specific Review Requests

1. Is the `HashMap` keyed by subject the right design?
2. Any edge cases in `in_same_realm()` logic? (e.g., PU chains like Austria-Hungary-Bohemia)
3. Should we track "original" vs "current" overlord for certain mechanics?
4. Are there critical subject interactions we're missing?
5. File parsing format verification for `history/diplomacy/*.txt`
