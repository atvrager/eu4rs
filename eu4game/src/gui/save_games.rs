//! Save game discovery and listing.
//!
//! Scans EU4 save directories to find available save files.
//! For now, we only extract filename and modification time.
//! Full save file parsing is deferred to a future phase.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A discovered save game file.
#[derive(Debug, Clone)]
pub struct SaveGameEntry {
    /// Display name (filename without extension).
    pub name: String,
    /// Full path to the save file (used in Phase 9 for loading).
    #[allow(dead_code)]
    pub path: PathBuf,
    /// Last modification time.
    pub modified: Option<SystemTime>,
}

impl SaveGameEntry {
    /// Format the modification time as a human-readable string.
    pub fn modified_str(&self) -> String {
        match self.modified {
            Some(time) => {
                // Convert to duration since UNIX_EPOCH
                if let Ok(duration) = time.duration_since(SystemTime::UNIX_EPOCH) {
                    // Simple date formatting (year-month-day)
                    let secs = duration.as_secs();
                    let days = secs / 86400;
                    // Approximate: days since 1970-01-01
                    let years = 1970 + days / 365;
                    let remaining_days = days % 365;
                    let month = remaining_days / 30 + 1;
                    let day = remaining_days % 30 + 1;
                    format!("{}-{:02}-{:02}", years, month.min(12), day.min(31))
                } else {
                    "Unknown".to_string()
                }
            }
            None => "Unknown".to_string(),
        }
    }
}

/// Discover save games from known EU4 save directories.
///
/// Searches in order:
/// 1. Steam cloud saves (~/.local/share/Steam/userdata/*/236850/remote/save games/)
/// 2. Local saves (~/.local/share/Paradox Interactive/Europa Universalis IV/save games/)
/// 3. Documents folder (~/Documents/Paradox Interactive/Europa Universalis IV/save games/)
pub fn discover_save_games() -> Vec<SaveGameEntry> {
    let mut saves = Vec::new();
    let mut seen_names = std::collections::HashSet::new();

    // Get home directory from environment
    let Ok(home_str) = std::env::var("HOME") else {
        log::warn!("HOME environment variable not set, cannot discover save games");
        return saves;
    };
    let home = PathBuf::from(home_str);

    // Try Steam cloud saves first (most likely to have saves)
    // Steam userdata - need to find the user ID directory
    let steam_userdata = home.join(".local/share/Steam/userdata");
    if steam_userdata.exists()
        && let Ok(entries) = std::fs::read_dir(&steam_userdata)
    {
        for entry in entries.flatten() {
            let save_dir = entry.path().join("236850/remote/save games");
            scan_save_directory(&save_dir, &mut saves, &mut seen_names);
        }
    }

    // Local Paradox saves
    let local_saves =
        home.join(".local/share/Paradox Interactive/Europa Universalis IV/save games");
    scan_save_directory(&local_saves, &mut saves, &mut seen_names);

    // Documents folder (Windows-style path on Linux via Proton)
    let docs_saves = home.join("Documents/Paradox Interactive/Europa Universalis IV/save games");
    scan_save_directory(&docs_saves, &mut saves, &mut seen_names);

    // Sort by modification time (newest first)
    saves.sort_by(|a, b| match (&b.modified, &a.modified) {
        (Some(b_time), Some(a_time)) => b_time.cmp(a_time),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.name.cmp(&b.name),
    });

    saves
}

/// Scan a directory for .eu4 save files.
fn scan_save_directory(
    dir: &Path,
    saves: &mut Vec<SaveGameEntry>,
    seen_names: &mut std::collections::HashSet<String>,
) {
    if !dir.exists() {
        return;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "eu4") {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string();

            // Skip duplicates (same save in multiple locations)
            if seen_names.contains(&name) {
                continue;
            }
            seen_names.insert(name.clone());

            let modified = entry.metadata().ok().and_then(|m| m.modified().ok());

            saves.push(SaveGameEntry {
                name,
                path,
                modified,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_save_games_does_not_panic() {
        // Just verify it doesn't crash - actual saves may or may not exist
        let saves = discover_save_games();
        // Log what we found for debugging
        for save in &saves {
            println!("Found save: {} at {:?}", save.name, save.path);
        }
    }

    #[test]
    fn test_modified_str_formats_correctly() {
        let entry = SaveGameEntry {
            name: "test".to_string(),
            path: PathBuf::from("/tmp/test.eu4"),
            modified: Some(
                SystemTime::UNIX_EPOCH
                    + std::time::Duration::from_secs(86400 * 365 * 54 + 86400 * 180),
            ),
        };
        let date = entry.modified_str();
        // Should be approximately 2024-07-01 (54 years + 180 days from 1970)
        assert!(date.starts_with("2024"), "Expected 2024, got {}", date);
    }

    #[test]
    fn test_modified_str_handles_none() {
        let entry = SaveGameEntry {
            name: "test".to_string(),
            path: PathBuf::from("/tmp/test.eu4"),
            modified: None,
        };
        assert_eq!(entry.modified_str(), "Unknown");
    }
}
