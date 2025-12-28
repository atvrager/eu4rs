//! Ledger comparison utilities for debugging income/expense deltas

use anyhow::Result;
use std::path::Path;

/// Print full ledger breakdown from a save file for analysis
///
/// DISABLED: Version compatibility issues with eu4save library
pub fn print_ledger_comparison(_from_save: &Path, _to_save: &Path, _country: &str) -> Result<()> {
    log::info!("Ledger comparison disabled due to version compatibility issues");
    log::info!("Alternative: Use direct save file inspection or game UI comparison");
    Ok(())
}
