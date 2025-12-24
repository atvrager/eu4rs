use crate::extract::{CountryVerifyData, ProvinceVerifyData, VerificationData};
use crate::{MetricType, VerificationResult, VerificationSummary};

// Note: tolerance is passed as a parameter to verify_all()

/// Verify all metrics for extracted data
pub fn verify_all(data: &VerificationData, tolerance: f64) -> VerificationSummary {
    let mut results = Vec::new();

    // Verify each country
    for (tag, country) in &data.countries {
        results.extend(verify_country(country, &data.provinces, tolerance));
        log::debug!("Verified country {}: {} metrics", tag, results.len());
    }

    VerificationSummary::new(results)
}

/// Verify metrics for a single country
pub fn verify_country(
    country: &CountryVerifyData,
    provinces: &std::collections::HashMap<u32, ProvinceVerifyData>,
    tolerance: f64,
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
        results.push(verify_monthly_trade(country, cached, tolerance));
    }

    // Verify monthly production
    if let Some(cached) = country.cached_monthly_production {
        results.push(verify_monthly_production(
            country, provinces, cached, tolerance,
        ));
    }

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
    // Use larger tolerance since we're missing modifiers
    let effective_tolerance = tolerance * 2.0; // Allow 2x because of missing modifiers
    if delta <= effective_tolerance * cached.abs().max(1.0) {
        VerificationResult::pass(metric, cached, calculated)
    } else {
        VerificationResult::fail(
            metric,
            cached,
            calculated,
            format!(
                "Delta {:.2} (base only - missing national modifiers, HRE bonus, etc.)",
                delta
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
    if delta <= tolerance * cached.abs().max(1.0) {
        VerificationResult::pass(metric, cached, calculated)
    } else {
        VerificationResult::fail(
            metric,
            cached,
            calculated,
            format!("Delta {} exceeds tolerance", delta),
        )
    }
}

/// Verify monthly trade income calculation
fn verify_monthly_trade(
    country: &CountryVerifyData,
    _cached: f64,
    _tolerance: f64,
) -> VerificationResult {
    let metric = MetricType::MonthlyTradeIncome {
        country: country.tag.clone(),
    };

    // Trade income requires trade node data which we don't have yet
    VerificationResult::skip(metric, "Trade verification not yet implemented")
}

/// Verify monthly production income calculation
fn verify_monthly_production(
    country: &CountryVerifyData,
    provinces: &std::collections::HashMap<u32, ProvinceVerifyData>,
    cached: f64,
    tolerance: f64,
) -> VerificationResult {
    let metric = MetricType::MonthlyProductionIncome {
        country: country.tag.clone(),
    };

    // Calculate monthly production from provinces
    // Formula: sum(base_production * goods_price * (1 - autonomy/100)) for owned provinces
    // This is simplified - we don't have goods prices

    let mut calculated = 0.0;
    let estimated_goods_price = 0.2; // Rough average

    for province_id in &country.owned_provinces {
        if let Some(province) = provinces.get(province_id) {
            let autonomy_multiplier = 1.0 - (province.local_autonomy / 100.0);
            calculated += province.base_production * estimated_goods_price * autonomy_multiplier;
        }
    }

    if country.owned_provinces.is_empty() {
        return VerificationResult::skip(metric, "No owned provinces data");
    }

    let delta = (calculated - cached).abs();
    if delta <= tolerance * cached.abs().max(1.0) {
        VerificationResult::pass(metric, cached, calculated)
    } else {
        // For production, we expect higher variance due to missing goods prices
        VerificationResult::fail(
            metric,
            cached,
            calculated,
            format!("Delta {} (note: goods prices not loaded)", delta),
        )
    }
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

    #[test]
    fn test_verify_max_manpower_pass() {
        // Test the base calculation formula:
        // base_national (10k) + sum(base_manpower * 250 / 1000)
        // With 2 provinces of 5 base_manpower each:
        // 10.0 + (5 * 250 / 1000) + (5 * 250 / 1000) = 10.0 + 1.25 + 1.25 = 12.5
        let country = CountryVerifyData {
            tag: "FRA".to_string(),
            cached_max_manpower: Some(12.5), // Expected value in thousands
            cached_monthly_tax: None,
            cached_monthly_trade: None,
            cached_monthly_production: None,
            owned_provinces: vec![1, 2],
        };

        let mut provinces = std::collections::HashMap::new();
        provinces.insert(
            1,
            ProvinceVerifyData {
                id: 1,
                owner: Some("FRA".to_string()),
                base_tax: 5.0,
                base_production: 5.0,
                base_manpower: 5.0,
                local_autonomy: 0.0,
                institution_progress: std::collections::HashMap::new(),
            },
        );
        provinces.insert(
            2,
            ProvinceVerifyData {
                id: 2,
                owner: Some("FRA".to_string()),
                base_tax: 5.0,
                base_production: 5.0,
                base_manpower: 5.0,
                local_autonomy: 0.0,
                institution_progress: std::collections::HashMap::new(),
            },
        );

        let result = verify_max_manpower(&country, &provinces, 12.5, 0.01);
        assert_eq!(result.status, crate::VerifyStatus::Pass);
    }
}
