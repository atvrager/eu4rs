use crate::{ExtractedCountry, ExtractedProvince, ExtractedState};

/// Extract verification-relevant data from parsed state
pub fn extract_for_verification(state: &ExtractedState) -> VerificationData {
    VerificationData {
        countries: state
            .countries
            .iter()
            .map(|(tag, c)| (tag.clone(), extract_country_data(c)))
            .collect(),
        provinces: state
            .provinces
            .iter()
            .map(|(id, p)| (*id, extract_province_data(p)))
            .collect(),
    }
}

/// Data extracted for verification purposes
#[derive(Debug, Clone)]
pub struct VerificationData {
    pub countries: std::collections::HashMap<String, CountryVerifyData>,
    pub provinces: std::collections::HashMap<u32, ProvinceVerifyData>,
}

/// Country data needed for verification
#[derive(Debug, Clone)]
pub struct CountryVerifyData {
    pub tag: String,

    // Cached values (what game calculated)
    pub cached_max_manpower: Option<f64>,
    pub cached_monthly_tax: Option<f64>,
    pub cached_monthly_trade: Option<f64>,
    pub cached_monthly_production: Option<f64>,

    // Expense breakdown (from game ledger)
    pub cached_army_maintenance: Option<f64>,
    pub cached_navy_maintenance: Option<f64>,
    pub cached_fort_maintenance: Option<f64>,
    pub cached_total_expenses: Option<f64>,

    // Input data for recalculation
    pub owned_provinces: Vec<u32>,
}

/// Province data needed for verification
#[derive(Debug, Clone)]
pub struct ProvinceVerifyData {
    pub id: u32,
    pub owner: Option<String>,

    // Development values
    pub base_tax: f64,
    pub base_production: f64,
    pub base_manpower: f64,

    // Modifiers
    pub local_autonomy: f64,

    // Institutions
    pub institution_progress: std::collections::HashMap<String, f64>,

    // Trade good produced (for production income calculation)
    pub trade_good: Option<String>,

    // Buildings present (for efficiency calculation)
    pub buildings: Vec<String>,
}

fn extract_country_data(country: &ExtractedCountry) -> CountryVerifyData {
    CountryVerifyData {
        tag: country.tag.clone(),
        cached_max_manpower: country.max_manpower,
        cached_monthly_tax: country.monthly_income.as_ref().map(|i| i.tax),
        cached_monthly_trade: country.monthly_income.as_ref().map(|i| i.trade),
        cached_monthly_production: country.monthly_income.as_ref().map(|i| i.production),
        cached_army_maintenance: country.army_maintenance,
        cached_navy_maintenance: country.navy_maintenance,
        cached_fort_maintenance: country.fort_maintenance,
        cached_total_expenses: country.total_monthly_expenses,
        owned_provinces: country.owned_province_ids.clone(),
    }
}

fn extract_province_data(province: &ExtractedProvince) -> ProvinceVerifyData {
    ProvinceVerifyData {
        id: province.id,
        owner: province.owner.clone(),
        base_tax: province.base_tax.unwrap_or(0.0),
        base_production: province.base_production.unwrap_or(0.0),
        base_manpower: province.base_manpower.unwrap_or(0.0),
        local_autonomy: province.local_autonomy.unwrap_or(0.0),
        institution_progress: province.institutions.clone(),
        trade_good: province.trade_good.clone(),
        buildings: province.buildings.clone(),
    }
}
