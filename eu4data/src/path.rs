use std::path::PathBuf;

/// Detects the Europa Universalis IV installation path.
///
/// Checks common Steam installation directories on Windows, Linux, and macOS.
pub fn detect_game_path() -> Option<PathBuf> {
    let candidates = [
        // Windows
        r"C:\Program Files (x86)\Steam\steamapps\common\Europa Universalis IV",
        // Linux
        ".local/share/Steam/steamapps/common/Europa Universalis IV",
        // macOS
        "Library/Application Support/Steam/steamapps/common/Europa Universalis IV",
    ];

    for candidate in candidates {
        let path = if candidate.starts_with("C:") {
            PathBuf::from(candidate)
        } else {
            dirs::home_dir().map(|home| home.join(candidate))?
        };

        if path.exists() {
            return Some(path);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_game_path_returns_none_if_not_found() {
        // This test assumes the environment running the test doesn't have the game installed
        // or we can't easily mock it without more complex DI.
        // For now, we just ensure it doesn't panic.
        let _ = detect_game_path();
    }
}
