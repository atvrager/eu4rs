//! Bookmark parsing for EU4 historical start dates.
//!
//! Bookmarks define recommended start dates with suggested nations.
//! Parsed from `common/bookmarks/*.txt` files.

use crate::Eu4Date;
use std::path::Path;

/// Derive the valid year range from a list of bookmarks.
///
/// Returns (min_year, max_year) based on the earliest and latest bookmark dates.
/// If no bookmarks are provided, returns vanilla EU4's default range (1444-1821).
///
/// This approach supports mods like Extended Timeline that have bookmarks outside
/// the vanilla range.
pub fn get_year_range_from_bookmarks(bookmarks: &[BookmarkEntry]) -> (i32, i32) {
    if bookmarks.is_empty() {
        return (Eu4Date::VANILLA_MIN_YEAR, Eu4Date::VANILLA_MAX_YEAR);
    }

    let min_year = bookmarks.iter().map(|b| b.date.year()).min().unwrap();
    let max_year = bookmarks.iter().map(|b| b.date.year()).max().unwrap();

    // Bookmarks define specific start dates, but allow some flexibility
    // (e.g., vanilla has bookmarks for 1444, 1492, 1618, etc., but allows
    // custom dates in between). Use a reasonable buffer.
    let buffer = 100; // Allow dates 100 years before/after earliest/latest bookmark
    (
        (min_year - buffer).max(1),    // Don't go below year 1
        (max_year + buffer).min(9999), // Don't go above year 9999
    )
}

/// A bookmark entry representing a historical start date.
#[derive(Debug, Clone)]
pub struct BookmarkEntry {
    /// Bookmark ID (from filename or internal ID).
    pub id: String,
    /// Localization key for the bookmark name (e.g., "BMARK_1444").
    pub name: String,
    /// Start date for this bookmark.
    pub date: Eu4Date,
    /// Recommended nation tags for this bookmark.
    pub countries: Vec<String>,
}

/// Parse all bookmarks from a game directory.
///
/// Reads all `.txt` files in `game_path/common/bookmarks/` and parses
/// bookmark entries from each.
pub fn parse_bookmarks(game_path: &Path) -> Vec<BookmarkEntry> {
    let bookmarks_dir = game_path.join("common/bookmarks");

    if !bookmarks_dir.exists() {
        return Vec::new();
    }

    let mut bookmarks = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&bookmarks_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("txt")
                && let Ok(content) = std::fs::read_to_string(&path)
            {
                bookmarks.extend(parse_bookmark_content(&content));
            }
        }
    }

    // Sort by date (earliest first)
    bookmarks.sort_by_key(|b| b.date);

    bookmarks
}

/// Parse bookmark entries from file content.
///
/// Handles the Paradox script format:
/// ```text
/// bookmark = {
///     name = "BMARK_1444"
///     desc = "BMARK_1444_DESC"
///     date = 1444.11.11
///     country = TUR
///     country = MOS
///     country = ENG
/// }
/// ```
#[allow(clippy::while_let_on_iterator)]
pub fn parse_bookmark_content(content: &str) -> Vec<BookmarkEntry> {
    let mut bookmarks = Vec::new();

    // Simple parser: find each "bookmark = {" block
    let mut chars = content.char_indices().peekable();

    while let Some((_, c)) = chars.next() {
        if c == 'b' {
            // Check if this is "bookmark"
            let start =
                content[chars.peek().map(|(i, _)| *i).unwrap_or(content.len())..].trim_start();
            if start.starts_with("ookmark") {
                // Skip "ookmark"
                for _ in 0..7 {
                    chars.next();
                }

                // Skip to '{'
                while let Some((_, ch)) = chars.next() {
                    if ch == '{' {
                        break;
                    }
                }

                // Extract the block content
                let mut depth = 1;
                let block_start = chars.peek().map(|(i, _)| *i).unwrap_or(content.len());
                let mut block_end = block_start;

                while let Some((i, ch)) = chars.next() {
                    if ch == '{' {
                        depth += 1;
                    } else if ch == '}' {
                        depth -= 1;
                        if depth == 0 {
                            block_end = i;
                            break;
                        }
                    }
                }

                if block_end > block_start {
                    let block = &content[block_start..block_end];
                    if let Some(bookmark) = parse_bookmark_block(block) {
                        bookmarks.push(bookmark);
                    }
                }
            }
        }
    }

    bookmarks
}

fn parse_bookmark_block(block: &str) -> Option<BookmarkEntry> {
    let mut name = String::new();
    let mut date = Eu4Date::default();
    let mut countries = Vec::new();

    for line in block.lines() {
        let line = line.trim();

        if let Some(rest) = line.strip_prefix("name") {
            let rest = rest.trim().trim_start_matches('=').trim();
            name = rest.trim_matches('"').to_string();
        } else if let Some(rest) = line.strip_prefix("date") {
            let rest = rest.trim().trim_start_matches('=').trim();
            if let Some(parsed_date) = parse_eu4_date(rest.as_bytes()) {
                date = parsed_date;
            }
        } else if let Some(rest) = line.strip_prefix("country") {
            let rest = rest.trim().trim_start_matches('=').trim();
            countries.push(rest.trim_matches('"').to_string());
        }
    }

    if name.is_empty() && date == Eu4Date::default() {
        return None;
    }

    let id = if !name.is_empty() {
        name.clone()
    } else {
        format!("bookmark_{}", date.year())
    };

    Some(BookmarkEntry {
        id,
        name,
        date,
        countries,
    })
}

/// Parse EU4 date format: "1444.11.11"
fn parse_eu4_date(bytes: &[u8]) -> Option<Eu4Date> {
    let s = std::str::from_utf8(bytes).ok()?;
    let parts: Vec<&str> = s.split('.').collect();

    if parts.len() != 3 {
        return None;
    }

    let year: i32 = parts[0].parse().ok()?;
    let month: u8 = parts[1].parse().ok()?;
    let day: u8 = parts[2].parse().ok()?;

    Some(Eu4Date::from_ymd(year, month, day))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bookmark_content() {
        let input = r#"
            bookmark = {
                name = "BMARK_TEST"
                desc = "Test Bookmark"
                date = 1444.11.11
                country = HAB
                country = FRA
                country = TUR
            }
        "#;

        let bookmarks = parse_bookmark_content(input);
        assert_eq!(bookmarks.len(), 1);

        let bookmark = &bookmarks[0];
        assert_eq!(bookmark.name, "BMARK_TEST");
        assert_eq!(bookmark.date.year(), 1444);
        assert_eq!(bookmark.date.month(), 11);
        assert_eq!(bookmark.date.day(), 11);
        assert_eq!(bookmark.countries.len(), 3);
        assert_eq!(bookmark.countries[0], "HAB");
        assert_eq!(bookmark.countries[1], "FRA");
        assert_eq!(bookmark.countries[2], "TUR");
    }

    #[test]
    fn test_parse_multiple_bookmarks() {
        let input = r#"
            bookmark = {
                name = "BMARK_1444"
                date = 1444.11.11
                country = TUR
            }
            bookmark = {
                name = "BMARK_1492"
                date = 1492.1.1
                country = CAS
            }
        "#;

        let bookmarks = parse_bookmark_content(input);
        assert_eq!(bookmarks.len(), 2);
        assert_eq!(bookmarks[0].name, "BMARK_1444");
        assert_eq!(bookmarks[1].name, "BMARK_1492");
    }

    #[test]
    fn test_parse_empty_content() {
        let bookmarks = parse_bookmark_content("");
        assert!(bookmarks.is_empty());
    }

    #[test]
    fn test_parse_eu4_date() {
        let date = parse_eu4_date(b"1444.11.11").unwrap();
        assert_eq!(date.year(), 1444);
        assert_eq!(date.month(), 11);
        assert_eq!(date.day(), 11);
    }

    #[test]
    fn test_parse_invalid_date() {
        assert!(parse_eu4_date(b"invalid").is_none());
        assert!(parse_eu4_date(b"1444.11").is_none());
        assert!(parse_eu4_date(b"not.a.date").is_none());
    }
}
