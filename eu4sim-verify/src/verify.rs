use crate::extract::{CountryVerifyData, ProvinceVerifyData, VerificationData};
use crate::{MetricType, VerificationResult, VerificationSummary};
use std::collections::HashMap;
use std::path::Path;

/// Game data needed for accurate verification (goods prices, building effects)
pub struct GameData {
    /// Trade good name -> base price in ducats
    pub goods_prices: HashMap<String, f32>,
    /// Building name -> local_production_efficiency bonus
    pub building_efficiency: HashMap<String, f32>,
}

impl GameData {
    /// Load game data from EU4 installation directory
    pub fn load(game_path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        // Load trade goods prices
        let tradegoods = eu4data::tradegoods::load_tradegoods(game_path)?;
        let goods_prices: HashMap<String, f32> = tradegoods
            .into_iter()
            .filter_map(|(name, tg)| tg.base_price.map(|p| (name, p)))
            .collect();

        log::debug!("Loaded {} goods prices", goods_prices.len());

        // Building efficiency values (hardcoded for now - full parser is larger effort)
        let building_efficiency = Self::hardcoded_building_efficiency();

        Ok(Self {
            goods_prices,
            building_efficiency,
        })
    }

    /// Hardcoded building production efficiency values
    /// Based on EU4 common/buildings/*.txt
    fn hardcoded_building_efficiency() -> HashMap<String, f32> {
        let mut map = HashMap::new();
        // Basic buildings
        map.insert("workshop".to_string(), 0.5); // +50%
        map.insert("counting_house".to_string(), 1.0); // +100% (replaces workshop)

        // Manufactories (all give +100% to province trade goods size, equivalent effect)
        map.insert("textile".to_string(), 1.0);
        map.insert("weapons".to_string(), 1.0);
        map.insert("plantations".to_string(), 1.0);
        map.insert("tradecompany".to_string(), 1.0);
        map.insert("wharf".to_string(), 1.0);
        map.insert("furnace".to_string(), 1.0);
        map.insert("farm_estate".to_string(), 1.0);
        map.insert("mills".to_string(), 1.0);

        map
    }

    /// Get goods price, defaulting to 2.0 for unknown goods
    pub fn get_price(&self, trade_good: &str) -> f32 {
        self.goods_prices.get(trade_good).copied().unwrap_or(2.0)
    }

    /// Calculate total production efficiency bonus from buildings
    pub fn get_production_efficiency(&self, buildings: &[String]) -> f32 {
        buildings
            .iter()
            .filter_map(|b| self.building_efficiency.get(b))
            .sum()
    }
}

// Note: tolerance is passed as a parameter to verify_all()

/// Verify all metrics for extracted data
pub fn verify_all(
    data: &VerificationData,
    tolerance: f64,
    game_data: Option<&GameData>,
) -> VerificationSummary {
    let mut results = Vec::new();

    // Verify each country
    for (tag, country) in &data.countries {
        results.extend(verify_country(
            country,
            &data.provinces,
            &data.trade_nodes,
            tolerance,
            game_data,
        ));
        log::debug!("Verified country {}: {} metrics", tag, results.len());
    }

    VerificationSummary::new(results)
}

/// Verify metrics for a single country
pub fn verify_country(
    country: &CountryVerifyData,
    provinces: &std::collections::HashMap<u32, ProvinceVerifyData>,
    trade_nodes: &std::collections::HashMap<String, crate::ExtractedTradeNode>,
    tolerance: f64,
    game_data: Option<&GameData>,
) -> Vec<VerificationResult> {
    let mut results = Vec::new();

    // Verify max manpower
    if let Some(cached) = country.cached_max_manpower {
        results.push(verify_max_manpower(country, provinces, cached, tolerance));
    }

    // Verify monthly tax
    if let Some(cached) = country.cached_monthly_tax {
        results.push(verify_monthly_tax(country, provinces, cached, tolerance));
    }

    // Verify monthly trade
    if let Some(cached) = country.cached_monthly_trade {
        results.push(verify_monthly_trade(
            country,
            trade_nodes,
            cached,
            tolerance,
        ));
    }

    // Verify monthly production
    if let Some(cached) = country.cached_monthly_production {
        results.push(verify_monthly_production(
            country, provinces, cached, tolerance, game_data,
        ));
    }

    // Calculate and show force limits
    // Note: Force limits aren't stored in save files - EU4 calculates them on-the-fly
    // So we always show calculated values (informational, like expenses)
    results.extend(show_force_limits(country, provinces));

    // Show expense breakdown (informational - no independent verification yet)
    results.extend(show_expenses(country));

    // Show mana generation (informational - base + ruler + advisor only)
    results.extend(show_mana_generation(country));

    results
}

/// Verify max manpower calculation
fn verify_max_manpower(
    country: &CountryVerifyData,
    provinces: &std::collections::HashMap<u32, ProvinceVerifyData>,
    cached: f64,
    tolerance: f64,
) -> VerificationResult {
    let metric = MetricType::MaxManpower {
        country: country.tag.clone(),
    };

    // Calculate max manpower from provinces
    // EU4 formula: base_national + sum(base_manpower * 250 * (1 - autonomy/100))
    // where base_national = 10,000 troops
    // Result is in thousands (save file stores as thousands)
    // NOTE: This is the BASE calculation only. Real formula includes:
    // - National modifiers (ideas, traditions, policies)
    // - HRE Emperor bonus (+50% if emperor)
    // - Subject interactions
    // - Many other bonuses
    // So we expect deltas for major powers with modifiers

    // Base national manpower (10k troops = 10.0 in thousands)
    let base_national = 10.0;

    let mut province_contribution = 0.0;
    for province_id in &country.owned_provinces {
        if let Some(province) = provinces.get(province_id) {
            let autonomy_multiplier = 1.0 - (province.local_autonomy / 100.0);
            // 250 troops per base_manpower, convert to thousands
            province_contribution += province.base_manpower * 250.0 / 1000.0 * autonomy_multiplier;
        }
    }

    let calculated = base_national + province_contribution;

    if country.owned_provinces.is_empty() {
        return VerificationResult::skip(metric, "No owned provinces data");
    }

    let delta = (calculated - cached).abs();
    let error_pct = if cached.abs() > 0.001 {
        (delta / cached.abs()) * 100.0
    } else {
        0.0
    };
    // Use larger tolerance since we're missing modifiers
    let effective_tolerance = tolerance * 2.0; // Allow 2x because of missing modifiers
    let threshold = effective_tolerance * cached.abs().max(1.0);
    let threshold_pct = effective_tolerance * 100.0;

    if delta <= threshold {
        VerificationResult::pass(metric, cached, calculated)
    } else {
        let closeness = if delta <= threshold * 2.0 {
            "close"
        } else if delta <= threshold * 5.0 {
            "moderate"
        } else {
            "far"
        };
        VerificationResult::fail(
            metric,
            cached,
            calculated,
            format!(
                "error {:.1}% (threshold {:.1}%) - {} [missing national modifiers]",
                error_pct, threshold_pct, closeness
            ),
        )
    }
}

/// Verify monthly tax income calculation
fn verify_monthly_tax(
    country: &CountryVerifyData,
    provinces: &std::collections::HashMap<u32, ProvinceVerifyData>,
    cached: f64,
    tolerance: f64,
) -> VerificationResult {
    let metric = MetricType::MonthlyTaxIncome {
        country: country.tag.clone(),
    };

    // Calculate monthly tax from provinces
    // Formula: sum(base_tax * 0.1 * (1 - autonomy/100)) for owned provinces
    // This is simplified - real formula has many more modifiers

    let mut calculated = 0.0;
    for province_id in &country.owned_provinces {
        if let Some(province) = provinces.get(province_id) {
            let autonomy_multiplier = 1.0 - (province.local_autonomy / 100.0);
            calculated += province.base_tax * 0.1 * autonomy_multiplier;
        }
    }

    if country.owned_provinces.is_empty() {
        return VerificationResult::skip(metric, "No owned provinces data");
    }

    let delta = (calculated - cached).abs();
    let error_pct = if cached.abs() > 0.001 {
        (delta / cached.abs()) * 100.0
    } else {
        0.0
    };
    let threshold = tolerance * cached.abs().max(1.0);
    let threshold_pct = tolerance * 100.0;

    if delta <= threshold {
        VerificationResult::pass(metric, cached, calculated)
    } else {
        let closeness = if delta <= threshold * 2.0 {
            "close"
        } else if delta <= threshold * 5.0 {
            "moderate"
        } else {
            "far"
        };
        VerificationResult::fail(
            metric,
            cached,
            calculated,
            format!(
                "error {:.1}% (threshold {:.1}%) - {}",
                error_pct, threshold_pct, closeness
            ),
        )
    }
}

/// Verify monthly trade income calculation
///
/// Trade income is calculated by summing the `money` field from all trade nodes
/// where this country has data. The `money` field represents income from that node.
fn verify_monthly_trade(
    country: &CountryVerifyData,
    trade_nodes: &std::collections::HashMap<String, crate::ExtractedTradeNode>,
    cached: f64,
    tolerance: f64,
) -> VerificationResult {
    let metric = MetricType::MonthlyTradeIncome {
        country: country.tag.clone(),
    };

    // Sum money from all trade nodes for this country
    let mut calculated = 0.0;
    let mut nodes_with_income = 0;

    for node in trade_nodes.values() {
        if let Some(data) = node.country_data.get(&country.tag) {
            if data.money > 0.0 {
                calculated += data.money;
                nodes_with_income += 1;
            }
        }
    }

    if nodes_with_income == 0 && cached > 0.0 {
        return VerificationResult::skip(metric, "No trade node data for country");
    }

    let delta = (calculated - cached).abs();
    let error_pct = if cached.abs() > 0.001 {
        (delta / cached.abs()) * 100.0
    } else {
        0.0
    };
    let threshold = tolerance * cached.abs().max(1.0);
    let threshold_pct = tolerance * 100.0;

    if delta <= threshold {
        VerificationResult::pass(metric, cached, calculated)
    } else {
        let closeness = if delta <= threshold * 2.0 {
            "close"
        } else if delta <= threshold * 5.0 {
            "moderate"
        } else {
            "far"
        };
        VerificationResult::fail(
            metric,
            cached,
            calculated,
            format!(
                "error {:.1}% (threshold {:.1}%) - {} [{} nodes]",
                error_pct, threshold_pct, closeness, nodes_with_income
            ),
        )
    }
}

/// Base goods produced per point of base_production (EU4 constant from defines)
const BASE_PRODUCTION_MULTIPLIER: f64 = 0.2;

/// Verify monthly production income calculation
///
/// Formula: sum((base_production * 0.2) * goods_price * (1 + efficiency) * (1 - autonomy)) / 12
/// - 0.2 = goods produced per base_production point
/// - goods_price = actual price of the trade good (2-10 ducats typically)
/// - efficiency = sum of local_production_efficiency from buildings
/// - autonomy = local autonomy (0-100%)
fn verify_monthly_production(
    country: &CountryVerifyData,
    provinces: &std::collections::HashMap<u32, ProvinceVerifyData>,
    cached: f64,
    tolerance: f64,
    game_data: Option<&GameData>,
) -> VerificationResult {
    let metric = MetricType::MonthlyProductionIncome {
        country: country.tag.clone(),
    };

    if country.owned_provinces.is_empty() {
        return VerificationResult::skip(metric, "No owned provinces data");
    }

    let mut calculated = 0.0;
    let mut provinces_with_unknown_goods = 0;
    let has_game_data = game_data.is_some();

    for province_id in &country.owned_provinces {
        if let Some(province) = provinces.get(province_id) {
            // Get actual goods price if game data is available
            let goods_price = match (&game_data, &province.trade_good) {
                (Some(gd), Some(goods)) => gd.get_price(goods) as f64,
                _ => {
                    provinces_with_unknown_goods += 1;
                    2.0 // Default fallback price
                }
            };

            // Calculate production efficiency from buildings
            let efficiency = match game_data {
                Some(gd) => gd.get_production_efficiency(&province.buildings) as f64,
                None => 0.0,
            };

            let autonomy_multiplier = 1.0 - (province.local_autonomy / 100.0);
            let efficiency_multiplier = 1.0 + efficiency;

            // Yearly production income for this province
            let yearly_income = province.base_production
                * BASE_PRODUCTION_MULTIPLIER
                * goods_price
                * efficiency_multiplier
                * autonomy_multiplier;

            // Convert to monthly
            calculated += yearly_income / 12.0;
        }
    }

    let delta = (calculated - cached).abs();
    let error_pct = if cached.abs() > 0.001 {
        (delta / cached.abs()) * 100.0
    } else {
        0.0
    };

    // More lenient tolerance if we're missing game data
    let effective_tolerance = if !has_game_data {
        tolerance * 3.0 // 3x tolerance without game data
    } else if provinces_with_unknown_goods > 0 {
        tolerance * 1.5 // 1.5x if some goods unknown
    } else {
        tolerance
    };

    let threshold = effective_tolerance * cached.abs().max(1.0);
    let threshold_pct = effective_tolerance * 100.0;

    if delta <= threshold {
        VerificationResult::pass(metric, cached, calculated)
    } else {
        // Build informative details showing how close we are
        let closeness = if delta <= threshold * 2.0 {
            "close"
        } else if delta <= threshold * 5.0 {
            "moderate"
        } else {
            "far"
        };

        let base_details = format!(
            "error {:.1}% (threshold {:.1}%) - {}",
            error_pct, threshold_pct, closeness
        );

        let details = if !has_game_data {
            format!("{} [no game data - using estimates]", base_details)
        } else if provinces_with_unknown_goods > 0 {
            format!(
                "{} [{} provinces with unknown goods]",
                base_details, provinces_with_unknown_goods
            )
        } else {
            base_details
        };

        VerificationResult::fail(metric, cached, calculated, details)
    }
}

/// Show calculated force limits (informational - no save comparison)
///
/// Force limits aren't stored in EU4 save files - the game calculates them on-the-fly.
/// We calculate them using the same formula and display as informational PASS results.
fn show_force_limits(
    country: &CountryVerifyData,
    provinces: &std::collections::HashMap<u32, ProvinceVerifyData>,
) -> Vec<VerificationResult> {
    let mut results = Vec::new();

    if country.owned_provinces.is_empty() {
        return results;
    }

    // Convert to the input format expected by the shared calculation
    let prov_inputs: std::collections::HashMap<u32, eu4sim_core::systems::ProvinceVerifyInput> =
        provinces
            .iter()
            .map(|(&id, p)| {
                (
                    id,
                    eu4sim_core::systems::ProvinceVerifyInput {
                        base_tax: p.base_tax,
                        base_production: p.base_production,
                        base_manpower: p.base_manpower,
                        local_autonomy: p.local_autonomy,
                        trade_good: p.trade_good.clone(),
                        buildings: p.buildings.clone(),
                    },
                )
            })
            .collect();

    // Calculate land force limit
    let land_fl = eu4sim_core::systems::calculate_land_force_limit_simple(
        &country.owned_provinces,
        &prov_inputs,
    );

    results.push(VerificationResult {
        metric: MetricType::LandForceLimit {
            country: country.tag.clone(),
        },
        expected: land_fl,
        actual: land_fl,
        delta: 0.0,
        status: crate::VerifyStatus::Pass,
        details: Some(format!(
            "calculated (base=6, dev_contrib={:.1})",
            land_fl - 6.0
        )),
    });

    // Calculate naval force limit
    let naval_fl = eu4sim_core::systems::calculate_naval_force_limit_simple(
        &country.owned_provinces,
        &prov_inputs,
    );

    results.push(VerificationResult {
        metric: MetricType::NavalForceLimit {
            country: country.tag.clone(),
        },
        expected: naval_fl,
        actual: naval_fl,
        delta: 0.0,
        status: crate::VerifyStatus::Pass,
        details: Some(format!(
            "calculated (base=12, dev_contrib={:.1})",
            naval_fl - 12.0
        )),
    });

    results
}

/// Show expense breakdown from save (informational - no independent verification)
///
/// We display these as PASS with the breakdown since we can't independently verify
/// without extracting army/fleet/fort counts (which requires more parsing work).
fn show_expenses(country: &CountryVerifyData) -> Vec<VerificationResult> {
    let mut results = Vec::new();

    // Total expenses
    if let Some(total) = country.cached_total_expenses {
        let breakdown = format!(
            "army={:.2} navy={:.2} fort={:.2}",
            country.cached_army_maintenance.unwrap_or(0.0),
            country.cached_navy_maintenance.unwrap_or(0.0),
            country.cached_fort_maintenance.unwrap_or(0.0),
        );
        results.push(VerificationResult {
            metric: MetricType::MonthlyExpenses {
                country: country.tag.clone(),
            },
            expected: total,
            actual: total,
            delta: 0.0,
            status: crate::VerifyStatus::Pass,
            details: Some(breakdown),
        });
    }

    results
}

/// Show calculated monthly mana generation (informational).
///
/// Displays monarch power generation based on:
/// - Base: +3 per category
/// - Ruler stats: +0-6 per category
/// - Advisor skills: +1-5 per hired advisor of matching type
///
/// Note: This is informational only - EU4 doesn't cache this value in saves.
/// Missing modifiers: national focus, power projection, estate privileges, etc.
fn show_mana_generation(country: &CountryVerifyData) -> Vec<VerificationResult> {
    let mut results = Vec::new();

    // Need ruler stats to calculate
    let (ruler_adm, ruler_dip, ruler_mil) =
        match (country.ruler_adm, country.ruler_dip, country.ruler_mil) {
            (Some(a), Some(d), Some(m)) => (a as i64, d as i64, m as i64),
            _ => return results, // Skip if no ruler stats
        };

    const BASE: i64 = 3;

    // Sum advisor skill levels by category (skill level = mana contribution)
    let (adm_skill, dip_skill, mil_skill) = sum_hired_advisor_skills(&country.advisors);

    let adm_gain = BASE + ruler_adm + adm_skill;
    let dip_gain = BASE + ruler_dip + dip_skill;
    let mil_gain = BASE + ruler_mil + mil_skill;
    let total = adm_gain + dip_gain + mil_gain;

    // Create informational metric
    results.push(VerificationResult {
        metric: MetricType::MonthlyManaGeneration {
            country: country.tag.clone(),
        },
        expected: total as f64,
        actual: total as f64,
        delta: 0.0,
        status: crate::VerifyStatus::Pass,
        details: Some(format!(
            "ADM={} DIP={} MIL={} (base=3, ruler={}/{}/{}, adv_skill={}/{}/{})",
            adm_gain,
            dip_gain,
            mil_gain,
            ruler_adm,
            ruler_dip,
            ruler_mil,
            adm_skill,
            dip_skill,
            mil_skill
        )),
    });

    results
}

/// Sum skill levels of hired advisors by category
fn sum_hired_advisor_skills(advisors: &[crate::ExtractedAdvisor]) -> (i64, i64, i64) {
    use crate::hydrate::categorize_advisor_type;
    use eu4sim_core::state::AdvisorType;

    let mut adm = 0;
    let mut dip = 0;
    let mut mil = 0;

    for adv in advisors.iter().filter(|a| a.is_hired) {
        let skill = adv.skill as i64;
        match categorize_advisor_type(&adv.advisor_type) {
            AdvisorType::Administrative => adm += skill,
            AdvisorType::Diplomatic => dip += skill,
            AdvisorType::Military => mil += skill,
        }
    }

    (adm, dip, mil)
}

/// Verify institution spread for a province
pub fn verify_institution_spread(
    province: &ProvinceVerifyData,
    institution: &str,
    _cached: f64,
    _tolerance: f64,
) -> VerificationResult {
    let metric = MetricType::InstitutionSpread {
        province: province.id,
        institution: institution.to_string(),
    };

    // Institution spread calculation is complex and depends on:
    // - Development
    // - Adjacent provinces
    // - Global modifiers
    // For now, skip

    VerificationResult::skip(
        metric,
        "Institution spread verification not yet implemented",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_country(tag: &str, provinces: Vec<u32>) -> CountryVerifyData {
        CountryVerifyData {
            tag: tag.to_string(),
            cached_max_manpower: None,
            cached_monthly_tax: None,
            cached_monthly_trade: None,
            cached_monthly_production: None,
            cached_army_maintenance: None,
            cached_navy_maintenance: None,
            cached_fort_maintenance: None,
            cached_total_expenses: None,
            cached_land_force_limit: None,
            cached_naval_force_limit: None,
            owned_provinces: provinces,
            ruler_adm: None,
            ruler_dip: None,
            ruler_mil: None,
            advisors: Vec::new(),
        }
    }

    fn make_province(
        id: u32,
        base_tax: f64,
        base_prod: f64,
        base_mp: f64,
        autonomy: f64,
        trade_good: Option<&str>,
        buildings: Vec<&str>,
    ) -> ProvinceVerifyData {
        ProvinceVerifyData {
            id,
            owner: Some("TST".to_string()),
            base_tax,
            base_production: base_prod,
            base_manpower: base_mp,
            local_autonomy: autonomy,
            institution_progress: std::collections::HashMap::new(),
            trade_good: trade_good.map(String::from),
            buildings: buildings.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn test_verify_max_manpower_pass() {
        // Test the base calculation formula:
        // base_national (10k) + sum(base_manpower * 250 / 1000)
        // With 2 provinces of 5 base_manpower each:
        // 10.0 + (5 * 250 / 1000) + (5 * 250 / 1000) = 10.0 + 1.25 + 1.25 = 12.5
        let mut country = make_country("FRA", vec![1, 2]);
        country.cached_max_manpower = Some(12.5);

        let mut provinces = std::collections::HashMap::new();
        provinces.insert(
            1,
            make_province(1, 5.0, 5.0, 5.0, 0.0, Some("grain"), vec![]),
        );
        provinces.insert(
            2,
            make_province(2, 5.0, 5.0, 5.0, 0.0, Some("grain"), vec![]),
        );

        let result = verify_max_manpower(&country, &provinces, 12.5, 0.01);
        assert_eq!(result.status, crate::VerifyStatus::Pass);
    }

    #[test]
    fn test_show_force_limits_basic() {
        // Base land: 6 + 30 dev * 0.1 = 9
        // Base naval: 12 + 30 dev * 0.1 = 15
        let country = make_country("TST", vec![1]);
        let mut provinces = std::collections::HashMap::new();
        provinces.insert(1, make_province(1, 10.0, 10.0, 10.0, 0.0, None, vec![]));

        let results = show_force_limits(&country, &provinces);
        assert_eq!(results.len(), 2);

        // Land FL
        assert_eq!(results[0].status, crate::VerifyStatus::Pass);
        assert!((results[0].actual - 9.0).abs() < 0.01);

        // Naval FL
        assert_eq!(results[1].status, crate::VerifyStatus::Pass);
        assert!((results[1].actual - 15.0).abs() < 0.01);
    }

    #[test]
    fn test_show_force_limits_with_trade_goods() {
        // Land: 6 + 30 dev * 0.1 + 0.5 grain = 9.5
        // Naval: 12 + 30 dev * 0.1 = 15 (no naval supplies)
        let country = make_country("TST", vec![1]);
        let mut provinces = std::collections::HashMap::new();
        provinces.insert(
            1,
            make_province(1, 10.0, 10.0, 10.0, 0.0, Some("grain"), vec![]),
        );

        let results = show_force_limits(&country, &provinces);
        assert!((results[0].actual - 9.5).abs() < 0.01);
        assert!((results[1].actual - 15.0).abs() < 0.01);
    }

    #[test]
    fn test_show_force_limits_with_autonomy() {
        // Land: 6 + (30 dev * 0.1) * 0.5 = 7.5
        // Naval: 12 + (30 dev * 0.1) * 0.5 = 13.5
        let country = make_country("TST", vec![1]);
        let mut provinces = std::collections::HashMap::new();
        provinces.insert(1, make_province(1, 10.0, 10.0, 10.0, 50.0, None, vec![]));

        let results = show_force_limits(&country, &provinces);
        assert!((results[0].actual - 7.5).abs() < 0.01);
        assert!((results[1].actual - 13.5).abs() < 0.01);
    }

    #[test]
    fn test_show_force_limits_with_buildings() {
        // Land: 6 + 30 dev * 0.1 + 1 camp + 3 center = 13
        // Naval: 12 + 30 dev * 0.1 + 2 shipyard = 17
        let country = make_country("TST", vec![1]);
        let mut provinces = std::collections::HashMap::new();
        provinces.insert(
            1,
            make_province(
                1,
                10.0,
                10.0,
                10.0,
                0.0,
                None,
                vec!["regimental_camp", "conscription_center", "shipyard"],
            ),
        );

        let results = show_force_limits(&country, &provinces);
        assert!((results[0].actual - 13.0).abs() < 0.01);
        assert!((results[1].actual - 17.0).abs() < 0.01);
    }

    #[test]
    fn test_show_force_limits_empty_provinces() {
        let country = make_country("TST", vec![]);
        let provinces = std::collections::HashMap::new();

        let results = show_force_limits(&country, &provinces);
        assert!(results.is_empty());
    }
}
