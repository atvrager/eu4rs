//! Unit tests for predict.rs functions.

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
