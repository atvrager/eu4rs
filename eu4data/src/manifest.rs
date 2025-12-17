use serde::Serialize;

/// Manifest of all game data files used at build time.
///
/// This ensures all clients use compatible game data and enables
/// automatic cache invalidation when source files change.
#[derive(Debug, Clone, Serialize)]
pub struct GameDataManifest {
    /// Simulation library version (from Cargo.toml)
    pub sim_version: &'static str,

    /// Git commit hash (if available)
    pub git_commit: Option<&'static str>,

    /// Individual file hashes
    pub file_hashes: &'static [FileHash],

    /// Combined hash of all file hashes (deterministic order)
    pub manifest_hash: [u8; 32],
}

/// Hash of a single game data file.
#[derive(Debug, Clone, Serialize)]
pub struct FileHash {
    /// Relative path from game root
    pub path: &'static str,

    /// SHA256 hash of file contents
    pub sha256: [u8; 32],
}

// Include generated manifest from build.rs
include!("generated/manifest_generated.rs");
