use serde::{Deserialize, Serialize};

/// Simulation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimConfig {
    /// Compute checksum every N ticks (0 = disabled).
    ///
    /// Recommended values:
    /// - `1`: Every tick (safest, ~0.5ms overhead)
    /// - `30`: Every month (balanced)
    /// - `365`: Every year (lowest overhead)
    pub checksum_frequency: u32,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            // Default to monthly checksums (30 ticks)
            checksum_frequency: 30,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SimConfig::default();
        assert_eq!(config.checksum_frequency, 30);
    }
}
