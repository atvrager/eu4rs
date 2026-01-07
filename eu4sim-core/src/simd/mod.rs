//! SIMD-accelerated batch operations for simulation systems.
//!
//! This module provides vectorized implementations of hot paths, with scalar
//! golden implementations for validation. All SIMD variants must produce
//! bit-identical results to their scalar counterparts.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     Runtime Dispatch                            │
//! │  is_x86_feature_detected!("avx2") → avx2_impl / scalar_impl    │
//! └─────────────────────────────────────────────────────────────────┘
//!                              │
//!          ┌───────────────────┴───────────────────┐
//!          ▼                                       ▼
//! ┌─────────────────┐                   ┌─────────────────┐
//! │  Scalar Golden  │ ◀── proptest ───▶ │   AVX2 SIMD     │
//! │  (source of     │     validates     │   (8x i32 or    │
//! │   truth)        │     bit-exact     │    4x i64)      │
//! └─────────────────┘                   └─────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```ignore
//! use eu4sim_core::simd::tax::{TaxInput, calculate_taxes_batch};
//!
//! let inputs: Vec<TaxInput> = provinces.iter().map(|p| TaxInput::from(p)).collect();
//! let results = calculate_taxes_batch(&inputs);
//! ```

pub mod tax;

/// Detect available SIMD features at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimdFeatures {
    pub sse2: bool,
    pub sse4_1: bool,
    pub avx: bool,
    pub avx2: bool,
    pub fma: bool,
    pub avx512f: bool,
}

impl SimdFeatures {
    /// Detect CPU features at runtime.
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn detect() -> Self {
        Self {
            sse2: is_x86_feature_detected!("sse2"),
            sse4_1: is_x86_feature_detected!("sse4.1"),
            avx: is_x86_feature_detected!("avx"),
            avx2: is_x86_feature_detected!("avx2"),
            fma: is_x86_feature_detected!("fma"),
            avx512f: is_x86_feature_detected!("avx512f"),
        }
    }

    /// Fallback for non-x86 architectures.
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    pub fn detect() -> Self {
        Self {
            sse2: false,
            sse4_1: false,
            avx: false,
            avx2: false,
            fma: false,
            avx512f: false,
        }
    }

    /// Best available feature level for dispatch decisions.
    pub fn best_level(&self) -> SimdLevel {
        if self.avx512f {
            SimdLevel::Avx512
        } else if self.avx2 && self.fma {
            SimdLevel::Avx2Fma
        } else if self.avx2 {
            SimdLevel::Avx2
        } else if self.sse4_1 {
            SimdLevel::Sse41
        } else {
            SimdLevel::Scalar
        }
    }
}

/// SIMD capability level for logging/debugging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdLevel {
    Scalar,
    Sse41,
    Avx2,
    Avx2Fma,
    Avx512,
}

impl std::fmt::Display for SimdLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimdLevel::Scalar => write!(f, "Scalar"),
            SimdLevel::Sse41 => write!(f, "SSE4.1"),
            SimdLevel::Avx2 => write!(f, "AVX2"),
            SimdLevel::Avx2Fma => write!(f, "AVX2+FMA"),
            SimdLevel::Avx512 => write!(f, "AVX-512"),
        }
    }
}

/// Fixed-point scale factor (must match Fixed::SCALE).
pub const FIXED_SCALE: i64 = 10000;

/// i32 scale for SIMD operations (same precision, smaller type).
/// Values are converted at batch boundaries.
pub const FIXED_SCALE_I32: i32 = 10000;

/// Log detected SIMD capabilities once at startup.
pub fn log_simd_capabilities() {
    let features = SimdFeatures::detect();
    log::info!(
        "SIMD: {} (avx2={}, fma={}, avx512f={})",
        features.best_level(),
        features.avx2,
        features.fma,
        features.avx512f
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_features() {
        let features = SimdFeatures::detect();
        // On any modern x86_64, SSE2 should be available
        #[cfg(target_arch = "x86_64")]
        assert!(features.sse2, "SSE2 should be available on x86_64");

        // best_level should return something
        let level = features.best_level();
        println!("Detected SIMD level: {}", level);
    }
}
