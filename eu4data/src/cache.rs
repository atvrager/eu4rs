use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CacheError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Cache invalid: {0}")]
    Invalid(String),
    #[error("Source file not found: {0}")]
    SourceNotFound(PathBuf),
}

/// Metadata for cache validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMetadata {
    /// SHA256 hashes of source files used to generate cache
    #[serde(default)]
    pub source_hashes: HashMap<PathBuf, String>,
    /// Modification times of source files
    #[serde(default)]
    pub source_mtimes: HashMap<PathBuf, SystemTime>,
    /// Game version (from launcher-settings.json or game exe)
    #[serde(default)]
    pub game_version: Option<String>,
    /// Cache generation timestamp
    pub generated_at: SystemTime,
}

impl CacheMetadata {
    /// Create metadata from source files.
    pub fn from_sources(source_files: &[PathBuf]) -> Result<Self, CacheError> {
        let mut source_hashes = HashMap::new();
        let mut source_mtimes = HashMap::new();

        for path in source_files {
            if !path.exists() {
                return Err(CacheError::SourceNotFound(path.clone()));
            }

            // Get modification time
            let metadata = fs::metadata(path)?;
            let mtime = metadata.modified()?;
            source_mtimes.insert(path.clone(), mtime);

            // Compute SHA256 hash
            let hash = compute_file_hash(path)?;
            source_hashes.insert(path.clone(), hash);
        }

        Ok(Self {
            source_hashes,
            source_mtimes,
            game_version: None,
            generated_at: SystemTime::now(),
        })
    }

    /// Check if cache is still valid for given source files.
    pub fn is_valid(&self, source_files: &[PathBuf]) -> bool {
        // Check all source files still exist and have matching hashes
        for path in source_files {
            if !path.exists() {
                return false;
            }

            // Check hash first (most reliable)
            if let Ok(current_hash) = compute_file_hash(path) {
                if let Some(cached_hash) = self.source_hashes.get(path) {
                    if cached_hash != &current_hash {
                        return false;
                    }
                } else {
                    // Hash not in cache
                    return false;
                }
            } else {
                // Failed to compute hash
                return false;
            }
        }

        true
    }
}

/// Trait for resources that can be cached.
pub trait CacheableResource: Serialize + for<'de> Deserialize<'de> {
    /// Get list of source files needed to generate this resource.
    fn source_files(game_path: &Path) -> Vec<PathBuf>;

    /// Generate the resource from source files.
    fn generate(game_path: &Path) -> Result<Self, CacheError>;
}

/// Load a cached resource or generate if cache is invalid/missing.
///
/// Cache is stored at `~/.cache/eu4rs/{cache_name}.json` with metadata at
/// `~/.cache/eu4rs/{cache_name}.meta.json`.
pub fn load_or_generate<T: CacheableResource>(
    cache_name: &str,
    game_path: &Path,
    force_regenerate: bool,
) -> Result<T, CacheError> {
    let cache_dir = get_cache_dir()?;
    let cache_path = cache_dir.join(format!("{}.json", cache_name));
    let meta_path = cache_dir.join(format!("{}.meta.json", cache_name));

    let source_files = T::source_files(game_path);

    // Check if we should use existing cache
    if !force_regenerate && cache_path.exists() && meta_path.exists() {
        // Load metadata
        let meta_json = fs::read_to_string(&meta_path)?;
        let metadata: CacheMetadata = serde_json::from_str(&meta_json)?;

        // Validate cache
        if metadata.is_valid(&source_files) {
            log::info!("Using cached {}", cache_name);
            let cache_json = fs::read_to_string(&cache_path)?;
            let resource: T = serde_json::from_str(&cache_json)?;
            return Ok(resource);
        } else {
            log::info!("Cache invalid for {}, regenerating", cache_name);
        }
    } else {
        log::info!("No cache found for {}, generating", cache_name);
    }

    // Generate resource
    let resource = T::generate(game_path)?;

    // Save cache
    fs::create_dir_all(&cache_dir)?;

    let cache_json = serde_json::to_string_pretty(&resource)?;
    fs::write(&cache_path, cache_json)?;

    let metadata = CacheMetadata::from_sources(&source_files)?;
    let meta_json = serde_json::to_string_pretty(&metadata)?;
    fs::write(&meta_path, meta_json)?;

    log::info!("Cached {} at {:?}", cache_name, cache_path);

    Ok(resource)
}

/// Get the cache directory, creating it if necessary.
fn get_cache_dir() -> Result<PathBuf, CacheError> {
    let cache_dir = if let Some(home) = dirs::home_dir() {
        home.join(".cache").join("eu4rs")
    } else {
        PathBuf::from(".cache/eu4rs")
    };

    fs::create_dir_all(&cache_dir)?;
    Ok(cache_dir)
}

/// Compute SHA256 hash of a file.
fn compute_file_hash(path: &Path) -> Result<String, CacheError> {
    use sha2::{Digest, Sha256};

    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 8192];

    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    let hash = hasher.finalize();
    Ok(format!("{:x}", hash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_metadata_from_sources() {
        let temp = TempDir::new().unwrap();
        let file1 = temp.path().join("file1.txt");
        fs::write(&file1, b"content1").unwrap();

        let metadata = CacheMetadata::from_sources(&[file1.clone()]).unwrap();

        assert_eq!(metadata.source_hashes.len(), 1);
        assert_eq!(metadata.source_mtimes.len(), 1);
        assert!(metadata.source_hashes.contains_key(&file1));
    }

    #[test]
    fn test_cache_metadata_validation() {
        let temp = TempDir::new().unwrap();
        let file1 = temp.path().join("file1.txt");
        fs::write(&file1, b"content1").unwrap();

        let metadata = CacheMetadata::from_sources(&[file1.clone()]).unwrap();

        // Should be valid immediately
        assert!(metadata.is_valid(&[file1.clone()]));

        // Modify file
        std::thread::sleep(std::time::Duration::from_millis(10));
        fs::write(&file1, b"content2").unwrap();

        // Should be invalid after modification
        assert!(!metadata.is_valid(&[file1]));
    }

    #[test]
    fn test_compute_file_hash() {
        let temp = TempDir::new().unwrap();
        let file = temp.path().join("test.txt");
        fs::write(&file, b"hello world").unwrap();

        let hash = compute_file_hash(&file).unwrap();

        // SHA256 of "hello world"
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }
}
