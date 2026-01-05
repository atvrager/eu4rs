# Mana (Monarch Power) Generation System

This document describes the EU4 monarch power ("mana") generation system and our phased implementation approach.

## EU4 Formula Overview

Monthly monarch power generation per category (ADM/DIP/MIL):

```
Monthly Mana = Base (3)
             + Ruler Stat (0-6)
             + Advisor Skill (1-5, if hired advisor of matching type)
             + National Focus (+2 if focused, -1 if not focused)
             + Power Projection (+1 if PP >= 50)
             + Estate Privileges (+1 from clergy/burghers/nobles privileges)
             + Government Reform Bonuses (various +1 sources)
             + Mission/Event Modifiers (various +1 sources)
             + Influence Nation (+1 for 10 years, from great power interaction)
             - Diplomatic Relations Over Cap (-1 DIP per excess relation)
             - Military Leaders Over Cap (-1 MIL per excess leader)
```

**Typical vanilla range:** ~2 to 20 per category per month (no hard cap - mods/special ideas can exceed this)

## Storage Cap

- Default: 999
- Modified by unembraced institutions: `cap = 999 * (100% + institution_tech_penalty)`
- Cap never decreases below current stockpile (no mana is lost when embracing)
- Cap increase also affected by corruption

## Phase 1: Core Implementation (Current)

### What We Implement

| Source | Contribution | Status |
|--------|--------------|--------|
| Base | +3 per category | Done (in mana.rs) |
| Ruler Stat | +0-6 per category | Done (in mana.rs) |
| Advisor Skill | +1-5 per category | **Adding now** |

### Files Modified

- `eu4sim-core/src/systems/mana.rs` - Add `sum_advisor_skills()` helper
- `eu4sim-verify/src/extract.rs` - Add ruler stats + advisors to `CountryVerifyData`
- `eu4sim-verify/src/lib.rs` - Add `MonthlyManaGeneration` MetricType
- `eu4sim-verify/src/verify.rs` - Add `show_mana_generation()` function
- `eu4sim-verify/src/hydrate.rs` - Export `categorize_advisor_type`

## Phase 2: Additional Modifiers (Future)

### High Priority (Extract from Saves)

| Source | Contribution | Extraction Complexity |
|--------|--------------|----------------------|
| National Focus | +2/-1 | Low - single field `national_focus` |
| Power Projection | +1 if >= 50 | Low - single field `power_projection` |
| Diplomatic Relations Over Cap | -1 per excess | Medium - need relations count + cap |
| Military Leaders Over Cap | -1 per excess | Medium - need leaders count + cap |

### Medium Priority

| Source | Contribution | Extraction Complexity |
|--------|--------------|----------------------|
| Estate Privileges | +1 per type | Medium - need privilege list parsing |
| Dynamic Storage Cap | 999 * (1 + penalty) | Medium - need institution embrace state |

### Lower Priority (Many Edge Cases)

| Source | Contribution | Notes |
|--------|--------------|-------|
| Government Reform Bonuses | Various | Zoroastrian fires, System of Councils, Nobles' Electorate, etc. |
| Mission Modifiers | Various | Country-specific temporary bonuses |
| Great Power Influence | +1 | 10-year temporary modifier |
| Steppe Nomad Razing | +25/dev | One-time, not monthly |

## Key Insights from Wiki

1. **Advisors contribute skill level (1-5), NOT a flat +2** - This was a critical correction
2. **No hard cap on monthly generation** - Mods and special ideas can exceed typical 20/month
3. **Storage cap is dynamic** - Based on institution embrace state
4. **Noble regency with "Nobles' Electorate" grants +1 to all three** - Special government reform
5. **Corruption affects power costs, not generation** - +1% per corruption point to all costs

## Verification Approach

Since the game doesn't expose a "monthly mana generation" cached value, our verification is **informational only**:

1. Calculate expected monthly generation from: base + ruler + advisor skill
2. Display in verification output with breakdown
3. Mark as PASS (no expected game value to compare against)

Future: When we add more modifiers, we can compare against actual save-to-save mana changes.

## References

- [EU4 Wiki: Monarch Power](https://eu4.paradoxwikis.com/Monarch_power)
- Wiki saved locally: `/home/atv/src/eu4rs/Monarch power - Europa Universalis 4 Wiki.mhtml`
