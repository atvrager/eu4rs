//! Next-step prediction: run sim forward and compare to future save
//!
//! Validates simulation stepping by:
//! 1. Loading save at time T
//! 2. Hydrating to WorldState
//! 3. Running step_world() N times
//! 4. Comparing predicted state to save at time T+N

use crate::hydrate::hydrate_from_save;
use crate::ledger_comparison::print_ledger_comparison;
use crate::parse::load_save;
use crate::ExtractedState;
use anyhow::Result;
use eu4sim_core::config::SimConfig;
use eu4sim_core::state::Date;
use eu4sim_core::step::step_world;
use eu4sim_core::WorldState;
use std::path::Path;

/// Result of a single metric prediction
#[derive(Debug, Clone)]
pub struct PredictionResult {
    pub metric: String,
    pub predicted: f64,
    pub actual: f64,
    pub delta: f64,
    pub status: PredictionStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredictionStatus {
    Pass,  // Within 5%
    Close, // Within 10%
    Fail,  // >10% off
    Skip,  // Data not available
}

/// Summary of prediction run
#[derive(Debug)]
pub struct PredictionSummary {
    pub from_date: String,
    pub to_date: String,
    pub days_simulated: u32,
    pub country: String,
    pub results: Vec<PredictionResult>,
}

/// Run prediction from save T to save T+N
pub fn run_prediction(
    game_path: &Path,
    from_save: &Path,
    to_save: &Path,
    country: &str,
) -> Result<PredictionSummary> {
    // 0. Print ledger comparison for debugging
    print_ledger_comparison(from_save, to_save, country)?;

    // 1. Load and parse both saves
    log::info!("Loading source save: {:?}", from_save);
    let from_state = load_save(from_save)?;
    let from_date = from_state.meta.date.clone();

    log::info!("Loading target save: {:?}", to_save);
    let to_state = load_save(to_save)?;
    let to_date = to_state.meta.date.clone();

    // 2. Calculate days between
    let from_date_parsed = parse_date(&from_date)?;
    let to_date_parsed = parse_date(&to_date)?;
    let days = days_between(&from_date_parsed, &to_date_parsed);
    log::info!(
        "Simulating {} days: {} -> {} (from_epoch: {}, to_epoch: {})",
        days,
        from_date,
        to_date,
        from_date_parsed.days_from_epoch(),
        to_date_parsed.days_from_epoch()
    );

    // 3. Hydrate from source save
    let (mut world, adjacency) = hydrate_from_save(game_path, &from_state)?;
    log::info!("Hydrated WorldState at {}", from_date);

    // Debug: Log starting treasury
    if let Some(c) = world.countries.get(country) {
        log::debug!(
            "{} starting treasury: {} ducats",
            country,
            c.treasury.to_f32()
        );
    }

    // 4. Run simulation for N days (passive - no inputs)
    let config = SimConfig {
        checksum_frequency: 0, // Disable checksums for speed
    };

    // IMPORTANT: EU4 saves capture state AFTER monthly ticks have run.
    // A save dated "1445.1.1" has the January monthly tick already applied.
    // To match this, we need to simulate FROM 12.1 TO 1.1 (inclusive),
    // which means running the monthly tick on 1.1.
    //
    // Since step_world() advances the date first, then checks for monthly tick,
    // we need exactly `days` iterations to reach the target date and trigger its monthly tick.
    let iterations = days; // Run full days to reach target date and trigger monthly tick

    for day in 0..iterations {
        let prev_date = world.date;
        let prev_treasury = if let Some(c) = world.countries.get(country) {
            c.treasury.to_f32()
        } else {
            0.0
        };

        world = step_world(&world, &[], Some(&adjacency), &config, None);

        let new_treasury = if let Some(c) = world.countries.get(country) {
            c.treasury.to_f32()
        } else {
            0.0
        };

        if day == 0 || day == iterations - 1 || prev_date.day == 1 || world.date.day == 1 {
            log::debug!(
                "Step {}/{}: {} -> {} (day {} -> {}) Treasury: {:.2} -> {:.2} ({:+.2})",
                day,
                iterations - 1,
                prev_date,
                world.date,
                prev_date.day,
                world.date.day,
                prev_treasury,
                new_treasury,
                new_treasury - prev_treasury
            );
        }
    }
    log::info!(
        "Simulation complete: {} (ran {} iterations)",
        world.date,
        iterations
    );

    // 5. Compare predicted state to actual
    // Also log the actual treasury delta from saves
    if let (Some(from_country), Some(to_country)) = (
        from_state.countries.get(country),
        to_state.countries.get(country),
    ) {
        if let (Some(from_treasury), Some(to_treasury)) =
            (from_country.treasury, to_country.treasury)
        {
            let actual_delta = to_treasury - from_treasury;
            log::info!("=== Actual Treasury Change (from saves) ===");
            log::info!("  From ({})  : {:>8.2} ducats", from_date, from_treasury);
            log::info!("  To ({})    : {:>8.2} ducats", to_date, to_treasury);
            log::info!(
                "  Change      : {:>8.2} ducats over {} days",
                actual_delta,
                days
            );
            log::info!("  Monthly rate: {:>8.2} ducats/month", actual_delta);
        }

        // Log BOTH ledgers to see the difference
        if let Some(ref from_income) = from_country.monthly_income {
            log::info!("=== FROM Save Ledger (Dec) - What WILL happen in Dec ===");
            log::info!("  Tax:        {:>8.2} ducats", from_income.tax);
            log::info!("  Production: {:>8.2} ducats", from_income.production);
            log::info!("  Trade:      {:>8.2} ducats", from_income.trade);
            log::info!("  Total:      {:>8.2} ducats", from_income.total);
        }
        if let Some(ref to_income) = to_country.monthly_income {
            log::info!("=== TO Save Ledger (Jan) - What WILL happen in Jan ===");
            log::info!("  Tax:        {:>8.2} ducats", to_income.tax);
            log::info!("  Production: {:>8.2} ducats", to_income.production);
            log::info!("  Trade:      {:>8.2} ducats", to_income.trade);
            log::info!("  Total:      {:>8.2} ducats", to_income.total);
        }
    }

    let results = compare_country(&world, &to_state, country);

    Ok(PredictionSummary {
        from_date,
        to_date,
        days_simulated: days,
        country: country.to_string(),
        results,
    })
}

/// Compare predicted WorldState to actual save for a specific country
fn compare_country(
    predicted: &WorldState,
    actual: &ExtractedState,
    tag: &str,
) -> Vec<PredictionResult> {
    let mut results = Vec::new();

    // Get predicted country state
    let pred_country = match predicted.countries.get(tag) {
        Some(c) => c,
        None => {
            log::warn!("Country {} not found in predicted state", tag);
            return vec![PredictionResult {
                metric: "Country".to_string(),
                predicted: 0.0,
                actual: 0.0,
                delta: 0.0,
                status: PredictionStatus::Skip,
            }];
        }
    };

    // Get actual country from save
    let actual_country = match actual.countries.get(tag) {
        Some(c) => c,
        None => {
            log::warn!("Country {} not found in actual save", tag);
            return vec![PredictionResult {
                metric: "Country".to_string(),
                predicted: 0.0,
                actual: 0.0,
                delta: 0.0,
                status: PredictionStatus::Skip,
            }];
        }
    };

    // Log income/expense comparison from EU4 ledger
    if let Some(ref income) = actual_country.monthly_income {
        log::info!("=== EU4 Ledger (Monthly Income) ===");
        log::info!("  Tax:        {:>8.2} ducats", income.tax);
        log::info!("  Production: {:>8.2} ducats", income.production);
        log::info!("  Trade:      {:>8.2} ducats", income.trade);
        log::info!("  Gold:       {:>8.2} ducats", income.gold);
        log::info!("  Tariffs:    {:>8.2} ducats", income.tariffs);
        log::info!("  Subsidies:  {:>8.2} ducats", income.subsidies);
        log::info!("  TOTAL:      {:>8.2} ducats", income.total);
    }

    if let (Some(army), Some(navy), Some(fort)) = (
        actual_country.army_maintenance,
        actual_country.navy_maintenance,
        actual_country.fort_maintenance,
    ) {
        let state_maint = actual_country.state_maintenance.unwrap_or(0.0);
        let corruption = actual_country.root_out_corruption.unwrap_or(0.0);
        let advisors: f64 = actual_country
            .advisors
            .iter()
            .map(|a| 5.0 * (a.skill as f64).powi(2))
            .sum();
        let total = army + navy + fort + state_maint + corruption + advisors;

        log::info!("=== EU4 Ledger (Monthly Expenses) ===");
        log::info!("  State:      {:>8.2} ducats", state_maint);
        log::info!("  Army:       {:>8.2} ducats", army);
        log::info!("  Navy:       {:>8.2} ducats", navy);
        log::info!("  Fort:       {:>8.2} ducats", fort);
        log::info!("  Corruption: {:>8.2} ducats", corruption);
        log::info!("  Advisors:   {:>8.2} ducats", advisors);
        log::info!("  TOTAL:      {:>8.2} ducats", total);
    }

    log::info!("=== Our Simulation (Monthly) ===");
    log::info!(
        "  Tax:        {:>8.2} ducats",
        pred_country.income.taxation.to_f32()
    );
    log::info!(
        "  Production: {:>8.2} ducats",
        pred_country.income.production.to_f32()
    );
    log::info!(
        "  Trade:      {:>8.2} ducats",
        pred_country.income.trade.to_f32()
    );
    log::info!(
        "  Expenses:   {:>8.2} ducats",
        pred_country.income.expenses.to_f32()
    );

    // Compare manpower (sim stores raw men, save stores thousands)
    // Display as raw men for clarity
    if let Some(actual_mp) = actual_country.current_manpower {
        let pred_mp = pred_country.manpower.to_f32() as f64;
        let actual_mp_raw = actual_mp * 1000.0;
        results.push(compare_metric("Manpower", pred_mp, actual_mp_raw));
    }

    // Compare treasury
    if let Some(actual_treasury) = actual_country.treasury {
        let pred_treasury = pred_country.treasury.to_f32() as f64;

        // Debug: Show income breakdown
        log::debug!(
            "{} income breakdown - Tax: {}, Prod: {}, Trade: {}, Expenses: {}",
            tag,
            pred_country.income.taxation,
            pred_country.income.production,
            pred_country.income.trade,
            pred_country.income.expenses
        );

        results.push(compare_metric("Treasury", pred_treasury, actual_treasury));
    }

    // Compare monarch power
    if let Some(actual_adm) = actual_country.adm_power {
        let pred_adm = pred_country.adm_mana.to_f32() as f64;
        results.push(compare_metric("ADM Power", pred_adm, actual_adm));
    }
    if let Some(actual_dip) = actual_country.dip_power {
        let pred_dip = pred_country.dip_mana.to_f32() as f64;
        results.push(compare_metric("DIP Power", pred_dip, actual_dip));
    }
    if let Some(actual_mil) = actual_country.mil_power {
        let pred_mil = pred_country.mil_mana.to_f32() as f64;
        results.push(compare_metric("MIL Power", pred_mil, actual_mil));
    }

    results
}

/// Compare a single metric and determine status
fn compare_metric(name: &str, predicted: f64, actual: f64) -> PredictionResult {
    let delta = predicted - actual;
    let pct_diff = if actual.abs() > 0.001 {
        (delta / actual).abs() * 100.0
    } else {
        delta.abs() * 100.0
    };

    let status = if pct_diff <= 5.0 {
        PredictionStatus::Pass
    } else if pct_diff <= 10.0 {
        PredictionStatus::Close
    } else {
        PredictionStatus::Fail
    };

    PredictionResult {
        metric: name.to_string(),
        predicted,
        actual,
        delta,
        status,
    }
}

/// Parse date string "YYYY.MM.DD" into Date
fn parse_date(date_str: &str) -> Result<Date> {
    let parts: Vec<&str> = date_str.split('.').collect();
    if parts.len() != 3 {
        anyhow::bail!("Invalid date format: {}", date_str);
    }

    let year: i32 = parts[0].parse()?;
    let month: u8 = parts[1].parse()?;
    let day: u8 = parts[2].parse()?;

    Ok(Date::new(year, month, day))
}

/// Calculate days between two dates
fn days_between(from: &Date, to: &Date) -> u32 {
    let from_days = from.days_from_epoch();
    let to_days = to.days_from_epoch();

    if to_days > from_days {
        (to_days - from_days) as u32
    } else {
        0
    }
}

/// Print prediction summary to stdout
pub fn print_prediction_report(summary: &PredictionSummary) {
    println!();
    println!(
        "=== Prediction: {} → {} ({} days) ===",
        summary.from_date, summary.to_date, summary.days_simulated
    );
    println!("Country: {}", summary.country);
    println!();
    println!(
        "{:<20} {:>12} {:>12} {:>12} {:>8}",
        "Metric", "Predicted", "Actual", "Delta", "Status"
    );
    println!("{}", "─".repeat(68));

    for result in &summary.results {
        let status_str = match result.status {
            PredictionStatus::Pass => "PASS",
            PredictionStatus::Close => "CLOSE",
            PredictionStatus::Fail => "FAIL",
            PredictionStatus::Skip => "SKIP",
        };

        println!(
            "{:<20} {:>12.2} {:>12.2} {:>+12.2} {:>8}",
            result.metric, result.predicted, result.actual, result.delta, status_str
        );
    }

    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Date parsing tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_parse_date_valid() {
        let date = parse_date("1444.11.11").unwrap();
        assert_eq!(date.year, 1444);
        assert_eq!(date.month, 11);
        assert_eq!(date.day, 11);
    }

    #[test]
    fn test_parse_date_single_digit() {
        let date = parse_date("1444.1.1").unwrap();
        assert_eq!(date.year, 1444);
        assert_eq!(date.month, 1);
        assert_eq!(date.day, 1);
    }

    #[test]
    fn test_parse_date_end_date() {
        let date = parse_date("1821.1.1").unwrap();
        assert_eq!(date.year, 1821);
        assert_eq!(date.month, 1);
        assert_eq!(date.day, 1);
    }

    #[test]
    fn test_parse_date_invalid_format() {
        assert!(parse_date("1444-11-11").is_err());
        assert!(parse_date("1444.11").is_err());
        assert!(parse_date("invalid").is_err());
    }

    // -------------------------------------------------------------------------
    // Days between tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_days_between_same_date() {
        let date = Date::new(1444, 11, 11);
        assert_eq!(days_between(&date, &date), 0);
    }

    #[test]
    fn test_days_between_one_day() {
        let from = Date::new(1444, 11, 11);
        let to = Date::new(1444, 11, 12);
        assert_eq!(days_between(&from, &to), 1);
    }

    #[test]
    fn test_days_between_month() {
        let from = Date::new(1444, 11, 1);
        let to = Date::new(1444, 12, 1);
        // November has 30 days
        assert_eq!(days_between(&from, &to), 30);
    }

    #[test]
    fn test_days_between_year() {
        let from = Date::new(1444, 1, 1);
        let to = Date::new(1445, 1, 1);
        // EU4 uses a simplified calendar: 12 months × 30 days = 360 days/year
        assert_eq!(days_between(&from, &to), 360);
    }

    #[test]
    fn test_days_between_reversed_returns_zero() {
        let from = Date::new(1445, 1, 1);
        let to = Date::new(1444, 1, 1);
        assert_eq!(days_between(&from, &to), 0);
    }

    // -------------------------------------------------------------------------
    // Metric comparison tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_compare_metric_exact_match() {
        let result = compare_metric("Treasury", 1000.0, 1000.0);
        assert_eq!(result.metric, "Treasury");
        assert_eq!(result.predicted, 1000.0);
        assert_eq!(result.actual, 1000.0);
        assert_eq!(result.delta, 0.0);
        assert_eq!(result.status, PredictionStatus::Pass);
    }

    #[test]
    fn test_compare_metric_within_5_percent() {
        // 1040 vs 1000 = 4% diff -> PASS
        let result = compare_metric("Treasury", 1040.0, 1000.0);
        assert_eq!(result.status, PredictionStatus::Pass);
    }

    #[test]
    fn test_compare_metric_within_10_percent() {
        // 1080 vs 1000 = 8% diff -> CLOSE
        let result = compare_metric("Treasury", 1080.0, 1000.0);
        assert_eq!(result.status, PredictionStatus::Close);
    }

    #[test]
    fn test_compare_metric_over_10_percent() {
        // 1150 vs 1000 = 15% diff -> FAIL
        let result = compare_metric("Treasury", 1150.0, 1000.0);
        assert_eq!(result.status, PredictionStatus::Fail);
    }

    #[test]
    fn test_compare_metric_negative_delta() {
        // 900 vs 1000 = -10% diff
        let result = compare_metric("Treasury", 900.0, 1000.0);
        assert_eq!(result.delta, -100.0);
        // 10% is exactly at the boundary, should be CLOSE
        assert_eq!(result.status, PredictionStatus::Close);
    }

    #[test]
    fn test_compare_metric_near_zero() {
        // Special case: actual is near zero
        let result = compare_metric("Gold", 0.5, 0.0);
        // When actual is ~0, we use delta directly
        assert_eq!(result.status, PredictionStatus::Fail); // 50% diff
    }

    // -------------------------------------------------------------------------
    // PredictionStatus tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_prediction_status_equality() {
        assert_eq!(PredictionStatus::Pass, PredictionStatus::Pass);
        assert_ne!(PredictionStatus::Pass, PredictionStatus::Fail);
    }

    // -------------------------------------------------------------------------
    // PredictionResult tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_prediction_result_debug() {
        let result = compare_metric("Test", 100.0, 100.0);
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("Test"));
        assert!(debug_str.contains("100"));
    }

    // -------------------------------------------------------------------------
    // PredictionSummary tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_prediction_summary_creation() {
        let summary = PredictionSummary {
            from_date: "1444.11.11".to_string(),
            to_date: "1444.12.1".to_string(),
            days_simulated: 20,
            country: "TUR".to_string(),
            results: vec![compare_metric("Treasury", 500.0, 500.0)],
        };

        assert_eq!(summary.from_date, "1444.11.11");
        assert_eq!(summary.to_date, "1444.12.1");
        assert_eq!(summary.days_simulated, 20);
        assert_eq!(summary.country, "TUR");
        assert_eq!(summary.results.len(), 1);
    }
}
