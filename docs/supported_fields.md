# EU4 Data Support Matrix

This document is auto-generated from `eu4data/src/coverage.rs`. **Do not edit manually.**
It defines which EU4 data fields are currently parsed and used by `eu4rs`.

## Countries

- **Total Known Fields:** 9
- **Parsed:** 1 (11.1%)
- **Used:** 1 (11.1%)

| Field | Parsed | Used | Notes |
|-------|--------|------|-------|
| `color` | ✅ | ✅ | Essential for political map |
| `graphical_culture` | ❌ | - | For unit models and city graphics |
| `historical_idea_groups` | ❌ | - |  |
| `historical_units` | ❌ | - |  |
| `monarch_names` | ❌ | - |  |
| `leader_names` | ❌ | - |  |
| `ship_names` | ❌ | - |  |
| `army_names` | ❌ | - |  |
| `fleet_names` | ❌ | - |  |

## Cultures

- **Total Known Fields:** 4
- **Parsed:** 0 (0.0%)
- **Used:** 0 (0.0%)

| Field | Parsed | Used | Notes |
|-------|--------|------|-------|
| `primary` | ❌ | - |  |
| `dynasty_names` | ❌ | - |  |
| `male_names` | ❌ | - |  |
| `female_names` | ❌ | - |  |

## Province History

- **Total Known Fields:** 13
- **Parsed:** 7 (53.8%)
- **Used:** 4 (30.8%)

| Field | Parsed | Used | Notes |
|-------|--------|------|-------|
| `owner` | ✅ | ✅ | Political map ownership |
| `controller` | ❌ | - | Wartime occupation |
| `add_core` | ❌ | - |  |
| `culture` | ✅ | ✅ | Culture map mode |
| `religion` | ✅ | ✅ | Religion map mode |
| `base_tax` | ✅ | - | Parsed but not visualized yet |
| `base_production` | ✅ | - |  |
| `base_manpower` | ✅ | - |  |
| `trade_goods` | ✅ | ✅ | Trade goods map mode |
| `capital` | ❌ | - | Province capital name |
| `is_city` | ❌ | - |  |
| `hre` | ❌ | - |  |
| `discovered_by` | ❌ | - |  |

## Religions

- **Total Known Fields:** 6
- **Parsed:** 2 (33.3%)
- **Used:** 1 (16.7%)

| Field | Parsed | Used | Notes |
|-------|--------|------|-------|
| `color` | ✅ | ✅ | Map color |
| `icon` | ✅ | - |  |
| `allowed_conversion` | ❌ | - |  |
| `country` | ❌ | - | Country modifiers |
| `province` | ❌ | - | Province modifiers |
| `heretic` | ❌ | - |  |

## Trade Goods

- **Total Known Fields:** 6
- **Parsed:** 4 (66.7%)
- **Used:** 1 (16.7%)

| Field | Parsed | Used | Notes |
|-------|--------|------|-------|
| `color` | ✅ | ✅ | Map color |
| `modifier` | ✅ | - | Production bonuses |
| `province` | ✅ | - | Province scope modifiers |
| `chance` | ✅ | - | Spawn chance (scripted) |
| `base_price` | ❌ | - |  |
| `gold_type` | ❌ | - |  |

