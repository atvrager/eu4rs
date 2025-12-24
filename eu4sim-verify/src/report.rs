use crate::{VerificationSummary, VerifyStatus};
use std::io::Write;

/// Generate a human-readable report of verification results
pub fn print_report(summary: &VerificationSummary, writer: &mut impl Write) -> std::io::Result<()> {
    writeln!(writer, "\n=== Verification Report ===")?;
    writeln!(writer)?;

    // Summary stats
    writeln!(
        writer,
        "Total: {} | Passed: {} | Failed: {} | Skipped: {}",
        summary.total, summary.passed, summary.failed, summary.skipped
    )?;

    if summary.passed + summary.failed > 0 {
        writeln!(writer, "Pass Rate: {:.1}%", summary.pass_rate())?;
    }

    writeln!(writer)?;

    // Group results by status
    let failures: Vec<_> = summary
        .results
        .iter()
        .filter(|r| r.status == VerifyStatus::Fail)
        .collect();

    let passes: Vec<_> = summary
        .results
        .iter()
        .filter(|r| r.status == VerifyStatus::Pass)
        .collect();

    let skipped: Vec<_> = summary
        .results
        .iter()
        .filter(|r| r.status == VerifyStatus::Skip)
        .collect();

    // Show failures first (most important)
    if !failures.is_empty() {
        writeln!(writer, "--- FAILURES ---")?;
        for result in &failures {
            writeln!(
                writer,
                "[FAIL] {}: expected={:.2}, actual={:.2}, delta={:.2}",
                result.metric, result.expected, result.actual, result.delta
            )?;
            if let Some(details) = &result.details {
                writeln!(writer, "       {}", details)?;
            }
        }
        writeln!(writer)?;
    }

    // Show passes
    if !passes.is_empty() {
        writeln!(writer, "--- PASSES ---")?;
        for result in &passes {
            writeln!(
                writer,
                "[PASS] {}: expected={:.2}, actual={:.2}",
                result.metric, result.expected, result.actual
            )?;
        }
        writeln!(writer)?;
    }

    // Show skipped
    if !skipped.is_empty() {
        writeln!(writer, "--- SKIPPED ---")?;
        for result in &skipped {
            write!(writer, "[SKIP] {}", result.metric)?;
            if let Some(details) = &result.details {
                write!(writer, ": {}", details)?;
            }
            writeln!(writer)?;
        }
        writeln!(writer)?;
    }

    Ok(())
}

/// Generate a JSON report
pub fn json_report(summary: &VerificationSummary) -> serde_json::Result<String> {
    serde_json::to_string_pretty(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MetricType, VerificationResult};

    #[test]
    fn test_print_report() {
        let results = vec![
            VerificationResult::pass(
                MetricType::MaxManpower {
                    country: "FRA".to_string(),
                },
                10000.0,
                10050.0,
            ),
            VerificationResult::fail(
                MetricType::MonthlyTaxIncome {
                    country: "FRA".to_string(),
                },
                100.0,
                80.0,
                "Large delta",
            ),
            VerificationResult::skip(
                MetricType::MonthlyTradeIncome {
                    country: "FRA".to_string(),
                },
                "Not implemented",
            ),
        ];

        let summary = VerificationSummary::new(results);
        let mut output = Vec::new();
        print_report(&summary, &mut output).unwrap();

        let report = String::from_utf8(output).unwrap();
        assert!(report.contains("Total: 3"));
        assert!(report.contains("Passed: 1"));
        assert!(report.contains("Failed: 1"));
        assert!(report.contains("[FAIL] MonthlyTax(FRA)"));
        assert!(report.contains("[PASS] MaxManpower(FRA)"));
    }
}
