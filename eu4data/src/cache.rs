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

/// Mode for cache validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheValidationMode {
    /// Fast mode: trust local filesystem, only validate based on metadata (mtimes).
    Fast,
    /// Strict mode: verify data integrity (hashes) on load.
    Strict,
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
    /// Manifest hash this cache was built against
    #[serde(default)]
    pub manifest_hash: Option<[u8; 32]>,
    /// SHA256 of the cached data itself (for integrity verification)
    #[serde(default)]
    pub data_hash: Option<[u8; 32]>,
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
            manifest_hash: Some(crate::manifest::GAME_MANIFEST.manifest_hash),
            data_hash: None,
            generated_at: SystemTime::now(),
        })
    }

    /// Fast path: check mtimes only (for local development).
    pub fn is_valid_quick(&self, source_files: &[PathBuf]) -> bool {
        for path in source_files {
            if let Ok(metadata) = fs::metadata(path) {
                if let Ok(mtime) = metadata.modified() {
                    if let Some(cached_mtime) = self.source_mtimes.get(path) {
                        if &mtime != cached_mtime {
                            return false;
                        }
                    } else {
                        return false;
                    }
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }

    /// Check if cache is still valid for given source files.
    pub fn is_valid(&self, source_files: &[PathBuf]) -> bool {
        // Check manifest hash first (build compatibility)
        if let Some(cached_manifest) = &self.manifest_hash {
            if cached_manifest != &crate::manifest::GAME_MANIFEST.manifest_hash {
                return false;
            }
        } else if self.manifest_hash.is_none() {
            // If cache was built before Phase 3, we should probably invalidate it
            // but for migration we might just warn. For integrity, we invalidate.
            return false;
        }

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
    mode: CacheValidationMode,
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

        // Validate cache based on mode
        let valid = match mode {
            CacheValidationMode::Fast => metadata.is_valid_quick(&source_files),
            CacheValidationMode::Strict => metadata.is_valid(&source_files),
        };

        if valid {
            log::info!("Using cached {}", cache_name);
            let cache_json = fs::read_to_string(&cache_path)?;

            // In strict mode, verify data integrity
            if mode == CacheValidationMode::Strict {
                if let Some(cached_data_hash) = metadata.data_hash {
                    let current_data_hash = compute_sha256_bytes(cache_json.as_bytes());
                    if current_data_hash != cached_data_hash {
                        log::warn!(
                            "Cache data corruption detected for {}, regenerating",
                            cache_name
                        );
                        return load_or_generate(cache_name, game_path, true, mode);
                    }
                } else {
                    log::warn!(
                        "Missing data hash in Strict mode for {}, regenerating",
                        cache_name
                    );
                    return load_or_generate(cache_name, game_path, true, mode);
                }
            }

            // Try to deserialize; if it fails (corrupted cache), regenerate instead of erroring
            match serde_json::from_str::<T>(&cache_json) {
                Ok(resource) => return Ok(resource),
                Err(e) => {
                    log::warn!(
                        "Failed to deserialize cache for {} ({}), regenerating",
                        cache_name,
                        e
                    );
                    // Fall through to regeneration
                }
            }
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
    let data_hash = compute_sha256_bytes(cache_json.as_bytes());

    fs::write(&cache_path, &cache_json)?;

    let mut metadata = CacheMetadata::from_sources(&source_files)?;
    metadata.data_hash = Some(data_hash);

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

/// Compute SHA256 hash of bytes.
fn compute_sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
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

        let metadata = CacheMetadata::from_sources(std::slice::from_ref(&file1)).unwrap();

        assert_eq!(metadata.source_hashes.len(), 1);
        assert_eq!(metadata.source_mtimes.len(), 1);
        assert!(metadata.source_hashes.contains_key(&file1));
        assert!(metadata.manifest_hash.is_some());
    }

    #[test]
    fn test_cache_metadata_validation() {
        let temp = TempDir::new().unwrap();
        let file1 = temp.path().join("file1.txt");
        fs::write(&file1, b"content1").unwrap();

        let metadata = CacheMetadata::from_sources(std::slice::from_ref(&file1)).unwrap();

        // Should be valid immediately
        assert!(metadata.is_valid(std::slice::from_ref(&file1)));

        // Modify file
        std::thread::sleep(std::time::Duration::from_millis(10));
        fs::write(&file1, b"content2").unwrap();

        // Should be invalid after modification
        assert!(!metadata.is_valid(&[file1]));
    }

    #[test]
    fn test_cache_manifest_hash_validation() {
        let temp = TempDir::new().unwrap();
        let file1 = temp.path().join("file1.txt");
        fs::write(&file1, b"content1").unwrap();

        let mut metadata = CacheMetadata::from_sources(std::slice::from_ref(&file1)).unwrap();
        assert!(metadata.is_valid(std::slice::from_ref(&file1)));

        // Mutate manifest hash
        metadata.manifest_hash = Some([0u8; 32]);
        assert!(!metadata.is_valid(&[file1]));
    }

    #[test]
    fn test_cache_mtime_fast_path() {
        let temp = TempDir::new().unwrap();
        let file1 = temp.path().join("file1.txt");
        fs::write(&file1, b"content1").unwrap();

        let metadata = CacheMetadata::from_sources(std::slice::from_ref(&file1)).unwrap();

        // Fast path valid
        assert!(metadata.is_valid_quick(std::slice::from_ref(&file1)));

        // Modify mtime (and content just in case)
        std::thread::sleep(std::time::Duration::from_millis(100)); // Ensure mtime change
        fs::write(&file1, b"content2").unwrap();

        // Fast path should notice
        assert!(!metadata.is_valid_quick(&[file1]));
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

    #[derive(Serialize, Deserialize)]
    struct MockResource {
        val: String,
    }
    impl CacheableResource for MockResource {
        fn source_files(_game_path: &Path) -> Vec<PathBuf> {
            vec![]
        }
        fn generate(_game_path: &Path) -> Result<Self, CacheError> {
            Ok(Self { val: "mock".into() })
        }
    }

    #[test]
    fn test_load_or_generate_basic() {
        let temp = TempDir::new().unwrap();
        let res: MockResource =
            load_or_generate("test", temp.path(), false, CacheValidationMode::Fast).unwrap();
        assert_eq!(res.val, "mock");
    }

    #[test]
    fn test_load_or_generate_corrupt_cache_triggers_regeneration() {
        let temp = TempDir::new().unwrap();

        // First, generate a valid cache
        let res: MockResource = load_or_generate(
            "test_corrupt",
            temp.path(),
            false,
            CacheValidationMode::Fast,
        )
        .unwrap();
        assert_eq!(res.val, "mock");

        // Now corrupt the cache file with invalid JSON
        let cache_dir = get_cache_dir().unwrap();
        let cache_path = cache_dir.join("test_corrupt.json");
        fs::write(&cache_path, r#"{ "val": "corrupted", INVALID JSON }"#).unwrap();

        // Loading should regenerate instead of failing
        let res2: MockResource = load_or_generate(
            "test_corrupt",
            temp.path(),
            false,
            CacheValidationMode::Fast,
        )
        .unwrap();
        // Should get fresh "mock" value from regeneration, not corrupted value
        assert_eq!(res2.val, "mock");

        // Clean up
        let _ = fs::remove_file(&cache_path);
        let _ = fs::remove_file(cache_dir.join("test_corrupt.meta.json"));
    }
}
