//! SIMD-accelerated taxation using Mod32 (i32 backing).
//!
//! This module uses `Mod32` (i32 with scale 10000) instead of the original
//! `Fixed` (i64 with i128 intermediate). The key advantage:
//!
//! - `i32 × i32 → i64` fits in AVX2's `_mm256_mul_epi32`
//! - No i128 operations that force scalar fallback
//! - 8 provinces per SIMD lane instead of 4
//!
//! ## Validation
//!
//! Results are validated against the i64 scalar golden implementation
//! to ensure numerical equivalence within the Mod32 range.

#![allow(unexpected_cfgs)]

use crate::fixed_generic::Mod32;
use multiversion::multiversion;

/// Input for Mod32 tax calculation.
///
/// Uses i32 raw values (scale 10000).
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, align(16))] // Align for SIMD loads
pub struct TaxInput32 {
    pub base_tax: i32,
    pub national_mod: i32,
    pub local_mod: i32,
    pub autonomy: i32,
}

impl TaxInput32 {
    /// Create from Mod32 values.
    #[inline]
    pub fn new(base_tax: Mod32, national_mod: Mod32, local_mod: Mod32, autonomy: Mod32) -> Self {
        Self {
            base_tax: base_tax.raw(),
            national_mod: national_mod.raw(),
            local_mod: local_mod.raw(),
            autonomy: autonomy.raw(),
        }
    }

    /// Create from f32 values (convenience for tests).
    #[inline]
    pub fn from_f32(base_tax: f32, national_mod: f32, local_mod: f32, autonomy: f32) -> Self {
        Self::new(
            Mod32::from_f32(base_tax),
            Mod32::from_f32(national_mod),
            Mod32::from_f32(local_mod),
            Mod32::from_f32(autonomy),
        )
    }
}

/// Output from Mod32 tax calculation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct TaxOutput32 {
    pub monthly_income: i32,
}

impl TaxOutput32 {
    /// Convert to Mod32.
    #[inline]
    pub fn to_mod32(self) -> Mod32 {
        Mod32::from_raw(self.monthly_income)
    }

    /// Convert to f32 for display.
    #[inline]
    pub fn to_f32(self) -> f32 {
        self.to_mod32().to_f32()
    }
}

// ============================================================================
// Scalar Golden Implementation (i32 version)
// ============================================================================

/// Calculate tax for a single province - SCALAR GOLDEN (i32).
///
/// Uses i64 intermediate for multiply (no i128 needed).
#[inline]
pub fn calculate_tax_scalar32(input: &TaxInput32) -> TaxOutput32 {
    const SCALE: i32 = 10000;
    const ONE: i32 = SCALE;
    const TWELVE: i32 = 12 * SCALE;

    // efficiency = 1.0 + national_mod + local_mod
    let efficiency = ONE
        .saturating_add(input.national_mod)
        .saturating_add(input.local_mod);

    // autonomy_factor = 1.0 - clamp(autonomy, 0, 1)
    let clamped_autonomy = input.autonomy.clamp(0, ONE);
    let autonomy_factor = ONE - clamped_autonomy;

    // Fixed multiply: (a * b) / SCALE using i64 intermediate
    let yearly_step1 = ((input.base_tax as i64 * efficiency as i64) / SCALE as i64) as i32;
    let yearly_income = ((yearly_step1 as i64 * autonomy_factor as i64) / SCALE as i64) as i32;

    // Division by 12: (yearly * SCALE) / (12 * SCALE) = yearly / 12
    let monthly_income = ((yearly_income as i64 * SCALE as i64) / TWELVE as i64) as i32;

    TaxOutput32 {
        monthly_income: monthly_income.max(0),
    }
}

/// Batch scalar calculation.
pub fn calculate_taxes_scalar32(inputs: &[TaxInput32], outputs: &mut [TaxOutput32]) {
    debug_assert_eq!(inputs.len(), outputs.len());
    for (input, output) in inputs.iter().zip(outputs.iter_mut()) {
        *output = calculate_tax_scalar32(input);
    }
}

// ============================================================================
// SIMD Implementation (i32 - vectorizable!)
// ============================================================================

/// Batch calculate taxes with runtime SIMD dispatch.
///
/// This version uses i32 arithmetic with i64 intermediate, which LLVM
/// can vectorize using AVX2's `_mm256_mul_epi32` (8x i32 lanes).
#[multiversion(targets("x86_64+avx2+fma", "x86_64+avx2", "x86_64+sse4.1",))]
pub fn calculate_taxes_batch32(inputs: &[TaxInput32], outputs: &mut [TaxOutput32]) {
    debug_assert_eq!(inputs.len(), outputs.len());

    const SCALE: i32 = 10000;
    const ONE: i32 = SCALE;
    const TWELVE: i64 = 12 * SCALE as i64;

    for (input, output) in inputs.iter().zip(outputs.iter_mut()) {
        // All i32 arithmetic with i64 intermediate - SIMD friendly!
        let efficiency = ONE
            .saturating_add(input.national_mod)
            .saturating_add(input.local_mod);
        let clamped_autonomy = input.autonomy.clamp(0, ONE);
        let autonomy_factor = ONE - clamped_autonomy;

        // i32 * i32 -> i64, then / SCALE -> i32
        let yearly_step1 = ((input.base_tax as i64 * efficiency as i64) / SCALE as i64) as i32;
        let yearly_income = ((yearly_step1 as i64 * autonomy_factor as i64) / SCALE as i64) as i32;
        let monthly_income = ((yearly_income as i64 * SCALE as i64) / TWELVE) as i32;

        output.monthly_income = monthly_income.max(0);
    }
}

/// Convenience function returning Vec.
pub fn calculate_taxes32(inputs: &[TaxInput32]) -> Vec<TaxOutput32> {
    let mut outputs = vec![TaxOutput32::default(); inputs.len()];
    calculate_taxes_batch32(inputs, &mut outputs);
    outputs
}

/// Returns the actual SIMD target being used for tax32 calculations.
///
/// This uses the same dispatch logic as `calculate_taxes_batch32` to report
/// which variant was selected at runtime.
#[multiversion(targets("x86_64+avx2+fma", "x86_64+avx2", "x86_64+sse4.1",))]
pub fn tax32_selected_target() -> multiversion::target::Target {
    multiversion::target::selected_target!()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_calculation() {
        // 12 base tax, no mods, no autonomy -> monthly = 1.0
        let input = TaxInput32::from_f32(12.0, 0.0, 0.0, 0.0);
        let output = calculate_tax_scalar32(&input);
        assert_eq!(output.to_f32(), 1.0);
    }

    #[test]
    fn test_with_modifiers() {
        // 12 base, +50% efficiency, 50% autonomy
        // yearly = 12 * 1.5 * 0.5 = 9
        // monthly = 9 / 12 = 0.75
        let input = TaxInput32::from_f32(12.0, 0.5, 0.0, 0.5);
        let output = calculate_tax_scalar32(&input);
        assert!((output.to_f32() - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_batch_matches_scalar() {
        let inputs = vec![
            TaxInput32::from_f32(12.0, 0.0, 0.0, 0.0),
            TaxInput32::from_f32(24.0, 0.25, 0.0, 0.1),
            TaxInput32::from_f32(6.0, -0.1, 0.2, 0.75),
        ];

        let scalar: Vec<_> = inputs.iter().map(calculate_tax_scalar32).collect();
        let batch = calculate_taxes32(&inputs);

        assert_eq!(scalar, batch);
    }

    #[test]
    fn test_negative_income_clamped() {
        // Extreme negative modifier
        let input = TaxInput32::from_f32(10.0, -2.0, 0.0, 0.0);
        let output = calculate_tax_scalar32(&input);
        assert!(output.monthly_income >= 0);
    }

    /// Verify which SIMD target is actually being dispatched.
    ///
    /// Run: cargo test -p eu4sim-core --release test_dispatch_target -- --nocapture
    #[test]
    fn test_dispatch_target() {
        let target = tax32_selected_target();
        let target_str = format!("{:?}", target);

        // Extract feature names for readable output
        let features: Vec<&str> = target.features().map(|f| f.name()).collect();

        println!("\n=== tax32 dispatch target ===");
        println!("Dispatched to: {:?}", features);
        println!(
            "CPU supports:  {}",
            crate::simd::SimdFeatures::detect().best_level()
        );

        // Confirm we got AVX2+FMA (our best target)
        let has_avx2 = features.contains(&"avx2");
        let has_fma = features.contains(&"fma");
        println!("Using AVX2: {}, FMA: {}", has_avx2, has_fma);

        // Should be one of our defined targets (or default/fallback)
        assert!(
            target_str.contains("avx2")
                || target_str.contains("sse")
                || target_str.contains("Default"),
            "Unexpected target: {:?}",
            target
        );
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    fn base_tax() -> impl Strategy<Value = f32> {
        0.0f32..50.0
    }

    fn modifier() -> impl Strategy<Value = f32> {
        -1.0f32..2.0
    }

    fn autonomy() -> impl Strategy<Value = f32> {
        -0.5f32..1.5
    }

    proptest! {
        #[test]
        fn simd32_matches_scalar32(
            base in base_tax(),
            nat in modifier(),
            loc in modifier(),
            auto in autonomy(),
        ) {
            let input = TaxInput32::from_f32(base, nat, loc, auto);
            let scalar = calculate_tax_scalar32(&input);
            let batch = calculate_taxes32(&[input])[0];
            prop_assert_eq!(scalar, batch);
        }

        #[test]
        fn income_never_negative(
            base in base_tax(),
            nat in modifier(),
            loc in modifier(),
            auto in autonomy(),
        ) {
            let input = TaxInput32::from_f32(base, nat, loc, auto);
            let result = calculate_tax_scalar32(&input);
            prop_assert!(result.monthly_income >= 0);
        }
    }
}

#[cfg(test)]
mod benchmarks {
    use super::*;
    use std::time::Instant;

    fn generate_inputs(count: usize) -> Vec<TaxInput32> {
        (0..count)
            .map(|i| {
                let base = ((i * 17 + 3) % 30) as f32 + 1.0;
                let nat = ((i * 13 + 7) % 100) as f32 / 100.0 - 0.25;
                let loc = ((i * 11 + 5) % 50) as f32 / 100.0;
                let auto = ((i * 19 + 11) % 75) as f32 / 100.0;
                TaxInput32::from_f32(base, nat, loc, auto)
            })
            .collect()
    }

    /// Benchmark Mod32 (i32) vs original Fixed (i64).
    ///
    /// Run: cargo test -p eu4sim-core --release bench_i32_vs_i64 -- --nocapture
    #[test]
    fn bench_i32_vs_i64() {
        use crate::simd::tax::{
            calculate_taxes_batch, calculate_taxes_scalar, TaxInput, TaxOutput,
        };

        const COUNT: usize = 3000;
        const ITERS: usize = 1000;

        // Generate i32 inputs
        let inputs32 = generate_inputs(COUNT);
        let mut outputs32 = vec![TaxOutput32::default(); COUNT];

        // Generate equivalent i64 inputs
        let inputs64: Vec<TaxInput> = inputs32
            .iter()
            .map(|i| TaxInput {
                base_tax: i.base_tax as i64,
                national_mod: i.national_mod as i64,
                local_mod: i.local_mod as i64,
                autonomy: i.autonomy as i64,
            })
            .collect();
        let mut outputs64 = vec![TaxOutput::default(); COUNT];

        // Warmup
        for _ in 0..10 {
            calculate_taxes_batch32(&inputs32, &mut outputs32);
            calculate_taxes_batch(&inputs64, &mut outputs64);
        }

        // Benchmark i64 scalar
        let start64_scalar = Instant::now();
        for _ in 0..ITERS {
            calculate_taxes_scalar(&inputs64, &mut outputs64);
        }
        let elapsed64_scalar = start64_scalar.elapsed();

        // Benchmark i64 batch (multiversion)
        let start64_batch = Instant::now();
        for _ in 0..ITERS {
            calculate_taxes_batch(&inputs64, &mut outputs64);
        }
        let elapsed64_batch = start64_batch.elapsed();

        // Benchmark i32 scalar
        let start32_scalar = Instant::now();
        for _ in 0..ITERS {
            calculate_taxes_scalar32(&inputs32, &mut outputs32);
        }
        let elapsed32_scalar = start32_scalar.elapsed();

        // Benchmark i32 batch (multiversion)
        let start32_batch = Instant::now();
        for _ in 0..ITERS {
            calculate_taxes_batch32(&inputs32, &mut outputs32);
        }
        let elapsed32_batch = start32_batch.elapsed();

        let ns = |d: std::time::Duration| d.as_nanos() as f64 / (ITERS * COUNT) as f64;

        println!("\n=== i32 vs i64 Tax Benchmark ===");
        println!("Provinces: {}, Iterations: {}", COUNT, ITERS);
        println!("SIMD: {}", crate::simd::SimdFeatures::detect().best_level());
        println!();
        println!(
            "i64 (Fixed):   scalar {:>6.2} ns   batch {:>6.2} ns   speedup {:>5.2}x",
            ns(elapsed64_scalar),
            ns(elapsed64_batch),
            elapsed64_scalar.as_secs_f64() / elapsed64_batch.as_secs_f64()
        );
        println!(
            "i32 (Mod32):   scalar {:>6.2} ns   batch {:>6.2} ns   speedup {:>5.2}x",
            ns(elapsed32_scalar),
            ns(elapsed32_batch),
            elapsed32_scalar.as_secs_f64() / elapsed32_batch.as_secs_f64()
        );
        println!();
        println!(
            "i32 vs i64 batch: {:>5.2}x faster",
            elapsed64_batch.as_secs_f64() / elapsed32_batch.as_secs_f64()
        );
        println!();
    }
}
