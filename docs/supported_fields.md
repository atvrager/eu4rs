# EU4 Data Support Matrix

This document is auto-generated from `eu4data/src/coverage.rs`. **Do not edit manually.**
It defines which EU4 data fields are currently parsed and used by `eu4rs`.

## Countries

- **Total Known Fields:** 20
- **Parsed:** 1 (5.0%)
- **Used:** 1 (5.0%)

| Field | Parsed | Used | Notes |
|-------|--------|------|-------|
| `color` | ✅ | ✅ | Essential for political map |
| `<date>` | ❌ | - | Time-dependent properties |
| `all_your_core_are_belong_to_us` | ❌ | - |  |
| `army_names` | ❌ | - |  |
| `cannot_form_from_collapse_nation` | ❌ | - |  |
| `colonial_parent` | ❌ | - |  |
| `fleet_names` | ❌ | - |  |
| `graphical_culture` | ❌ | - | For unit models and city graphics |
| `historical_council` | ❌ | - |  |
| `historical_idea_groups` | ❌ | - |  |
| `historical_score` | ❌ | - |  |
| `historical_units` | ❌ | - |  |
| `leader_names` | ❌ | - |  |
| `monarch_names` | ❌ | - |  |
| `preferred_religion` | ❌ | - |  |
| `random_nation_chance` | ❌ | - |  |
| `revolutionary_colors` | ❌ | - |  |
| `right_to_bear_arms` | ❌ | - |  |
| `ship_names` | ❌ | - |  |
| `special_unit_culture` | ❌ | - |  |

## Cultures

- **Total Known Fields:** 11
- **Parsed:** 0 (0.0%)
- **Used:** 0 (0.0%)

| Field | Parsed | Used | Notes |
|-------|--------|------|-------|
| `country` | ❌ | - |  |
| `dynasty_names` | ❌ | - |  |
| `female_names` | ❌ | - |  |
| `graphical_culture` | ❌ | - |  |
| `has_samurai` | ❌ | - |  |
| `local_has_samurai` | ❌ | - |  |
| `local_has_tercio` | ❌ | - |  |
| `male_names` | ❌ | - |  |
| `primary` | ❌ | - | Tag of primary nation |
| `province` | ❌ | - |  |
| `second_graphical_culture` | ❌ | - |  |

## Province History

- **Total Known Fields:** 36
- **Parsed:** 7 (19.4%)
- **Used:** 4 (11.1%)

| Field | Parsed | Used | Notes |
|-------|--------|------|-------|
| `culture` | ✅ | ✅ | Culture map mode |
| `owner` | ✅ | ✅ | Political map ownership |
| `religion` | ✅ | ✅ | Religion map mode |
| `trade_goods` | ✅ | ✅ | Trade goods map mode |
| `base_manpower` | ✅ | - |  |
| `base_production` | ✅ | - |  |
| `base_tax` | ✅ | - | Parsed but not visualized yet |
| `<date>` | ❌ | - | Time-dependent properties |
| `add_brahmins_or_church_effect` | ❌ | - |  |
| `add_claim` | ❌ | - |  |
| `add_core` | ❌ | - |  |
| `add_jains_or_burghers_effect` | ❌ | - |  |
| `add_local_autonomy` | ❌ | - |  |
| `add_nationalism` | ❌ | - |  |
| `add_permanent_province_modifier` | ❌ | - |  |
| `add_province_triggered_modifier` | ❌ | - |  |
| `add_rajputs_or_marathas_or_nobles_effect` | ❌ | - |  |
| `add_trade_modifier` | ❌ | - |  |
| `add_vaisyas_or_burghers_effect` | ❌ | - |  |
| `capital` | ❌ | - | Province capital name |
| `center_of_trade` | ❌ | - |  |
| `controller` | ❌ | - | Wartime occupation |
| `discovered_by` | ❌ | - |  |
| `extra_cost` | ❌ | - |  |
| `fort_15th` | ❌ | - |  |
| `hre` | ❌ | - |  |
| `is_city` | ❌ | - |  |
| `latent_trade_goods` | ❌ | - |  |
| `native_ferocity` | ❌ | - |  |
| `native_hostileness` | ❌ | - |  |
| `native_size` | ❌ | - |  |
| `revolt_risk` | ❌ | - |  |
| `seat_in_parliament` | ❌ | - |  |
| `shipyard` | ❌ | - |  |
| `tribal_owner` | ❌ | - |  |
| `unrest` | ❌ | - |  |

## Religions

- **Total Known Fields:** 53
- **Parsed:** 2 (3.8%)
- **Used:** 1 (1.9%)

| Field | Parsed | Used | Notes |
|-------|--------|------|-------|
| `color` | ✅ | ✅ | Map color |
| `icon` | ✅ | - |  |
| `allow_female_defenders_of_the_faith` | ❌ | - |  |
| `allowed_center_conversion` | ❌ | - |  |
| `allowed_conversion` | ❌ | - |  |
| `ancestors` | ❌ | - |  |
| `aspects` | ❌ | - |  |
| `aspects_name` | ❌ | - |  |
| `authority` | ❌ | - |  |
| `blessings` | ❌ | - |  |
| `can_have_secondary_religion` | ❌ | - |  |
| `celebrate` | ❌ | - |  |
| `country` | ❌ | - | Country modifiers |
| `country_as_secondary` | ❌ | - |  |
| `date` | ❌ | - |  |
| `declare_war_in_regency` | ❌ | - |  |
| `doom` | ❌ | - |  |
| `fervor` | ❌ | - |  |
| `fetishist_cult` | ❌ | - |  |
| `flag_emblem_index_range` | ❌ | - |  |
| `flags_with_emblem_percentage` | ❌ | - |  |
| `gurus` | ❌ | - |  |
| `hanafi_school` | ❌ | - |  |
| `hanbali_school` | ❌ | - |  |
| `harmonized_modifier` | ❌ | - |  |
| `has_patriarchs` | ❌ | - |  |
| `heretic` | ❌ | - |  |
| `holy_sites` | ❌ | - |  |
| `hre_heretic_religion` | ❌ | - |  |
| `hre_religion` | ❌ | - |  |
| `ismaili_school` | ❌ | - |  |
| `jafari_school` | ❌ | - |  |
| `maliki_school` | ❌ | - |  |
| `misguided_heretic` | ❌ | - |  |
| `on_convert` | ❌ | - |  |
| `orthodox_icons` | ❌ | - |  |
| `papacy` | ❌ | - |  |
| `personal_deity` | ❌ | - |  |
| `province` | ❌ | - | Province modifiers |
| `reform_tooltip` | ❌ | - |  |
| `religious_reforms` | ❌ | - |  |
| `require_reformed_for_institution_development` | ❌ | - |  |
| `shafii_school` | ❌ | - |  |
| `uses_anglican_power` | ❌ | - |  |
| `uses_church_power` | ❌ | - |  |
| `uses_harmony` | ❌ | - |  |
| `uses_hussite_power` | ❌ | - |  |
| `uses_isolationism` | ❌ | - |  |
| `uses_judaism_power` | ❌ | - |  |
| `uses_karma` | ❌ | - |  |
| `uses_piety` | ❌ | - |  |
| `will_get_center` | ❌ | - |  |
| `zaidi_school` | ❌ | - |  |

## Trade Goods

- **Total Known Fields:** 64
- **Parsed:** 4 (6.2%)
- **Used:** 1 (1.6%)

| Field | Parsed | Used | Notes |
|-------|--------|------|-------|
| `color` | ✅ | ✅ | Map color |
| `chance` | ✅ | - | Spawn chance (scripted) |
| `modifier` | ✅ | - | Production bonuses |
| `province` | ✅ | - | Province scope modifiers |
| `adm_tech_cost_modifier` | ❌ | - |  |
| `advisor_cost` | ❌ | - |  |
| `base_price` | ❌ | - |  |
| `cavalry_cost` | ❌ | - |  |
| `development_cost` | ❌ | - |  |
| `devotion` | ❌ | - |  |
| `dip_tech_cost_modifier` | ❌ | - |  |
| `diplomatic_reputation` | ❌ | - |  |
| `factor` | ❌ | - |  |
| `garrison_growth` | ❌ | - |  |
| `global_colonial_growth` | ❌ | - |  |
| `global_institution_spread` | ❌ | - |  |
| `global_regiment_cost` | ❌ | - |  |
| `global_regiment_recruit_speed` | ❌ | - |  |
| `global_sailors_modifier` | ❌ | - |  |
| `global_ship_cost` | ❌ | - |  |
| `global_spy_defence` | ❌ | - |  |
| `global_tariffs` | ❌ | - |  |
| `global_trade_goods_size_modifier` | ❌ | - |  |
| `global_unrest` | ❌ | - |  |
| `gold_type` | ❌ | - |  |
| `heir_chance` | ❌ | - |  |
| `horde_unity` | ❌ | - |  |
| `inflation_reduction` | ❌ | - |  |
| `land_forcelimit` | ❌ | - |  |
| `land_forcelimit_modifier` | ❌ | - |  |
| `land_maintenance_modifier` | ❌ | - |  |
| `legitimacy` | ❌ | - |  |
| `local_autonomy` | ❌ | - |  |
| `local_build_cost` | ❌ | - |  |
| `local_build_time` | ❌ | - |  |
| `local_defensiveness` | ❌ | - |  |
| `local_development_cost` | ❌ | - |  |
| `local_friendly_movement_speed` | ❌ | - |  |
| `local_institution_spread` | ❌ | - |  |
| `local_manpower_modifier` | ❌ | - |  |
| `local_missionary_strength` | ❌ | - |  |
| `local_monthly_devastation` | ❌ | - |  |
| `local_production_efficiency` | ❌ | - |  |
| `local_sailors_modifier` | ❌ | - |  |
| `local_state_maintenance_modifier` | ❌ | - |  |
| `local_tax_modifier` | ❌ | - |  |
| `local_unrest` | ❌ | - |  |
| `manpower_recovery_speed` | ❌ | - |  |
| `merc_maintenance_modifier` | ❌ | - |  |
| `meritocracy` | ❌ | - |  |
| `naval_forcelimit` | ❌ | - |  |
| `naval_forcelimit_modifier` | ❌ | - |  |
| `num_accepted_cultures` | ❌ | - |  |
| `prestige` | ❌ | - |  |
| `province_trade_power_modifier` | ❌ | - |  |
| `province_trade_power_value` | ❌ | - |  |
| `regiment_recruit_speed` | ❌ | - |  |
| `republican_tradition` | ❌ | - |  |
| `spy_offence` | ❌ | - |  |
| `supply_limit_modifier` | ❌ | - |  |
| `tolerance_own` | ❌ | - |  |
| `trade_efficiency` | ❌ | - |  |
| `trade_value_modifier` | ❌ | - |  |
| `war_exhaustion_cost` | ❌ | - |  |

