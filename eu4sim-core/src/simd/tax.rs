//! SIMD-accelerated taxation calculations.
//!
//! Provides batch processing of province tax income with runtime dispatch
//! to the best available SIMD implementation.
//!
//! ## Formula
//!
//! ```text
//! efficiency = 1.0 + national_mod + local_mod
//! autonomy_factor = 1.0 - clamp(autonomy, 0, 1)
//! yearly_income = base_tax × efficiency × autonomy_factor
//! monthly_income = yearly_income / 12
//! ```
//!
//! ## Validation
//!
//! All SIMD implementations are validated against the scalar golden
//! implementation via proptest to ensure bit-exact results.

// The multiversion macro generates cfg checks for target features like "retpoline"
// which trigger unexpected_cfgs warnings. This is a known issue with the crate.
#![allow(unexpected_cfgs)]

use crate::fixed::Fixed;
use multiversion::multiversion;

/// Input data for a single province's tax calculation.
///
/// Packed for efficient batch processing. All values are raw Fixed
/// internals (i64 scaled by 10000).
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct TaxInput {
    /// Province base tax (Fixed raw value)
    pub base_tax: i64,
    /// National tax modifier (Fixed raw value, e.g., 5000 = +50%)
    pub national_mod: i64,
    /// Local/provincial tax modifier (Fixed raw value)
    pub local_mod: i64,
    /// Province autonomy (Fixed raw value, 0-10000 = 0%-100%)
    pub autonomy: i64,
}

impl TaxInput {
    /// Create from Fixed values.
    pub fn new(base_tax: Fixed, national_mod: Fixed, local_mod: Fixed, autonomy: Fixed) -> Self {
        Self {
            base_tax: base_tax.raw(),
            national_mod: national_mod.raw(),
            local_mod: local_mod.raw(),
            autonomy: autonomy.raw(),
        }
    }
}

/// Output from tax calculation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct TaxOutput {
    /// Monthly income (Fixed raw value)
    pub monthly_income: i64,
}

impl TaxOutput {
    /// Convert to Fixed for integration with game state.
    pub fn to_fixed(self) -> Fixed {
        Fixed::from_raw(self.monthly_income)
    }
}

// ============================================================================
// Scalar Golden Implementation (source of truth)
// ============================================================================

/// Calculate tax for a single province - SCALAR GOLDEN IMPLEMENTATION.
///
/// This is the authoritative implementation. All SIMD variants must
/// produce bit-identical results.
#[inline]
pub fn calculate_tax_scalar(input: &TaxInput) -> TaxOutput {
    const SCALE: i64 = 10000;
    const ONE: i64 = SCALE;
    const TWELVE: i64 = 12 * SCALE;

    // efficiency = 1.0 + national_mod + local_mod
    let efficiency = ONE + input.national_mod + input.local_mod;

    // autonomy_factor = 1.0 - clamp(autonomy, 0, 1)
    let clamped_autonomy = input.autonomy.clamp(0, ONE);
    let autonomy_factor = ONE - clamped_autonomy;

    // yearly_income = base_tax × efficiency × autonomy_factor
    // Fixed multiply: (a * b) / SCALE
    let yearly_step1 = (input.base_tax as i128 * efficiency as i128 / SCALE as i128) as i64;
    let yearly_income = (yearly_step1 as i128 * autonomy_factor as i128 / SCALE as i128) as i64;

    // monthly_income = yearly_income / 12 (Fixed division by int)
    // Fixed::div formula: (a * SCALE) / b
    let monthly_income = (yearly_income as i128 * SCALE as i128 / TWELVE as i128) as i64;

    // Clamp to non-negative
    TaxOutput {
        monthly_income: monthly_income.max(0),
    }
}

/// Batch calculate taxes using scalar implementation.
pub fn calculate_taxes_scalar(inputs: &[TaxInput], outputs: &mut [TaxOutput]) {
    debug_assert_eq!(inputs.len(), outputs.len());
    for (input, output) in inputs.iter().zip(outputs.iter_mut()) {
        *output = calculate_tax_scalar(input);
    }
}

// ============================================================================
// SIMD Implementations
// ============================================================================

/// Batch calculate taxes with runtime dispatch to best SIMD level.
///
/// Uses multiversion to generate AVX2 and scalar variants, with
/// automatic runtime selection.
#[multiversion(targets(
    "x86_64+avx2+fma",  // Haswell+, Zen1+
    "x86_64+avx2",      // AVX2 without FMA
    "x86_64+sse4.1",    // Nehalem+
))]
pub fn calculate_taxes_batch(inputs: &[TaxInput], outputs: &mut [TaxOutput]) {
    // The compiler will autovectorize this for each target.
    // We structure the code to maximize vectorization opportunities.
    debug_assert_eq!(inputs.len(), outputs.len());

    const SCALE: i64 = 10000;
    const ONE: i64 = SCALE;
    const TWELVE: i64 = 12 * SCALE;

    // Process in chunks to help autovectorization
    for (input, output) in inputs.iter().zip(outputs.iter_mut()) {
        // efficiency = 1.0 + national_mod + local_mod
        let efficiency = ONE + input.national_mod + input.local_mod;

        // autonomy_factor = 1.0 - clamp(autonomy, 0, 1)
        let clamped_autonomy = input.autonomy.clamp(0, ONE);
        let autonomy_factor = ONE - clamped_autonomy;

        // Fixed-point multiplications (i128 intermediate to prevent overflow)
        let yearly_step1 = (input.base_tax as i128 * efficiency as i128 / SCALE as i128) as i64;
        let yearly_income = (yearly_step1 as i128 * autonomy_factor as i128 / SCALE as i128) as i64;

        // Division by 12
        let monthly_income = (yearly_income as i128 * SCALE as i128 / TWELVE as i128) as i64;

        output.monthly_income = monthly_income.max(0);
    }
}

/// Convenience function returning a Vec (allocates).
pub fn calculate_taxes(inputs: &[TaxInput]) -> Vec<TaxOutput> {
    let mut outputs = vec![TaxOutput::default(); inputs.len()];
    calculate_taxes_batch(inputs, &mut outputs);
    outputs
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixed::Fixed;

    #[test]
    fn test_basic_tax_calculation() {
        // base_tax = 12, no mods, no autonomy
        // yearly = 12 * 1.0 * 1.0 = 12
        // monthly = 12 / 12 = 1.0
        let input = TaxInput::new(Fixed::from_f32(12.0), Fixed::ZERO, Fixed::ZERO, Fixed::ZERO);
        let output = calculate_tax_scalar(&input);
        assert_eq!(output.to_fixed(), Fixed::ONE);
    }

    #[test]
    fn test_with_modifiers() {
        // base_tax = 12, +50% national, 50% autonomy
        // efficiency = 1.0 + 0.5 = 1.5
        // autonomy_factor = 1.0 - 0.5 = 0.5
        // yearly = 12 * 1.5 * 0.5 = 9.0
        // monthly = 9.0 / 12 = 0.75
        let input = TaxInput::new(
            Fixed::from_f32(12.0),
            Fixed::from_f32(0.5),
            Fixed::ZERO,
            Fixed::from_f32(0.5),
        );
        let output = calculate_tax_scalar(&input);
        assert_eq!(output.to_fixed(), Fixed::from_f32(0.75));
    }

    #[test]
    fn test_batch_matches_scalar() {
        let inputs = vec![
            TaxInput::new(Fixed::from_f32(12.0), Fixed::ZERO, Fixed::ZERO, Fixed::ZERO),
            TaxInput::new(
                Fixed::from_f32(24.0),
                Fixed::from_f32(0.25),
                Fixed::ZERO,
                Fixed::from_f32(0.1),
            ),
            TaxInput::new(
                Fixed::from_f32(6.0),
                Fixed::from_f32(-0.1),
                Fixed::from_f32(0.2),
                Fixed::from_f32(0.75),
            ),
        ];

        // Scalar results
        let scalar_outputs: Vec<_> = inputs.iter().map(calculate_tax_scalar).collect();

        // Batch results
        let batch_outputs = calculate_taxes(&inputs);

        // Must be identical
        assert_eq!(scalar_outputs, batch_outputs);
    }

    #[test]
    fn test_negative_income_clamped() {
        // Efficiency could go negative with extreme negative mods
        let input = TaxInput::new(
            Fixed::from_f32(10.0),
            Fixed::from_f32(-2.0), // -200% national mod
            Fixed::ZERO,
            Fixed::ZERO,
        );
        let output = calculate_tax_scalar(&input);
        // Negative income should be clamped to 0
        assert!(output.monthly_income >= 0);
    }

    #[test]
    fn test_autonomy_clamping() {
        // Autonomy > 100% should clamp
        let input = TaxInput::new(
            Fixed::from_f32(12.0),
            Fixed::ZERO,
            Fixed::ZERO,
            Fixed::from_f32(1.5), // 150% autonomy - should clamp to 100%
        );
        let output = calculate_tax_scalar(&input);
        // With 100% autonomy, income should be 0
        assert_eq!(output.monthly_income, 0);

        // Negative autonomy should clamp to 0
        let input2 = TaxInput::new(
            Fixed::from_f32(12.0),
            Fixed::ZERO,
            Fixed::ZERO,
            Fixed::from_f32(-0.5), // -50% autonomy - should clamp to 0%
        );
        let output2 = calculate_tax_scalar(&input2);
        // With 0% autonomy (clamped from -50%), monthly = 12/12 = 1.0
        assert_eq!(output2.to_fixed(), Fixed::ONE);
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    // Generate realistic game values
    fn base_tax() -> impl Strategy<Value = f32> {
        0.0f32..50.0 // EU4 base tax range
    }

    fn modifier() -> impl Strategy<Value = f32> {
        -1.0f32..2.0 // -100% to +200%
    }

    fn autonomy() -> impl Strategy<Value = f32> {
        -0.5f32..1.5 // Test clamping behavior
    }

    proptest! {
        /// CRITICAL: SIMD batch must produce bit-identical results to scalar.
        #[test]
        fn simd_matches_scalar(
            base in base_tax(),
            nat_mod in modifier(),
            loc_mod in modifier(),
            auto in autonomy(),
        ) {
            let input = TaxInput::new(
                Fixed::from_f32(base),
                Fixed::from_f32(nat_mod),
                Fixed::from_f32(loc_mod),
                Fixed::from_f32(auto),
            );

            let scalar_result = calculate_tax_scalar(&input);
            let batch_result = calculate_taxes(&[input])[0];

            prop_assert_eq!(
                scalar_result, batch_result,
                "SIMD mismatch for base={}, nat={}, loc={}, auto={}",
                base, nat_mod, loc_mod, auto
            );
        }

        /// Batch processing must match element-wise scalar.
        #[test]
        fn batch_matches_individual(
            bases in prop::collection::vec(base_tax(), 1..100),
            nat_mods in prop::collection::vec(modifier(), 1..100),
            loc_mods in prop::collection::vec(modifier(), 1..100),
            autos in prop::collection::vec(autonomy(), 1..100),
        ) {
            // Use minimum length across all vectors
            let len = bases.len().min(nat_mods.len()).min(loc_mods.len()).min(autos.len());
            if len == 0 {
                return Ok(());
            }

            let inputs: Vec<TaxInput> = (0..len)
                .map(|i| TaxInput::new(
                    Fixed::from_f32(bases[i]),
                    Fixed::from_f32(nat_mods[i]),
                    Fixed::from_f32(loc_mods[i]),
                    Fixed::from_f32(autos[i]),
                ))
                .collect();

            // Individual scalar
            let scalar_results: Vec<_> = inputs.iter().map(calculate_tax_scalar).collect();

            // Batch
            let batch_results = calculate_taxes(&inputs);

            for i in 0..len {
                prop_assert_eq!(
                    scalar_results[i], batch_results[i],
                    "Mismatch at index {}", i
                );
            }
        }

        /// Income must never be negative.
        #[test]
        fn income_never_negative(
            base in base_tax(),
            nat_mod in modifier(),
            loc_mod in modifier(),
            auto in autonomy(),
        ) {
            let input = TaxInput::new(
                Fixed::from_f32(base),
                Fixed::from_f32(nat_mod),
                Fixed::from_f32(loc_mod),
                Fixed::from_f32(auto),
            );

            let result = calculate_tax_scalar(&input);
            prop_assert!(result.monthly_income >= 0, "Negative income: {}", result.monthly_income);
        }
    }
}

#[cfg(test)]
mod benchmarks {
    use super::*;
    use std::time::Instant;

    fn generate_test_inputs(count: usize) -> Vec<TaxInput> {
        // Deterministic pseudo-random inputs for reproducible benchmarks
        (0..count)
            .map(|i| {
                let base = ((i * 17 + 3) % 30) as f32 + 1.0;
                let nat_mod = ((i * 13 + 7) % 100) as f32 / 100.0 - 0.25;
                let loc_mod = ((i * 11 + 5) % 50) as f32 / 100.0;
                let auto = ((i * 19 + 11) % 75) as f32 / 100.0;
                TaxInput::new(
                    Fixed::from_f32(base),
                    Fixed::from_f32(nat_mod),
                    Fixed::from_f32(loc_mod),
                    Fixed::from_f32(auto),
                )
            })
            .collect()
    }

    /// Benchmark comparing scalar vs multiversion batch processing.
    ///
    /// Run with: cargo test -p eu4sim-core --release bench_scalar_vs_batch -- --nocapture
    #[test]
    fn bench_scalar_vs_batch() {
        const PROVINCE_COUNT: usize = 3000; // ~EU4's province count
        const ITERATIONS: usize = 1000;

        let inputs = generate_test_inputs(PROVINCE_COUNT);
        let mut scalar_outputs = vec![TaxOutput::default(); PROVINCE_COUNT];
        let mut batch_outputs = vec![TaxOutput::default(); PROVINCE_COUNT];

        // Warmup
        for _ in 0..10 {
            calculate_taxes_scalar(&inputs, &mut scalar_outputs);
            calculate_taxes_batch(&inputs, &mut batch_outputs);
        }

        // Benchmark scalar
        let scalar_start = Instant::now();
        for _ in 0..ITERATIONS {
            calculate_taxes_scalar(&inputs, &mut scalar_outputs);
        }
        let scalar_elapsed = scalar_start.elapsed();

        // Benchmark batch (multiversion)
        let batch_start = Instant::now();
        for _ in 0..ITERATIONS {
            calculate_taxes_batch(&inputs, &mut batch_outputs);
        }
        let batch_elapsed = batch_start.elapsed();

        // Verify correctness
        assert_eq!(scalar_outputs, batch_outputs, "Results must match!");

        let scalar_ns_per_province =
            scalar_elapsed.as_nanos() as f64 / (ITERATIONS * PROVINCE_COUNT) as f64;
        let batch_ns_per_province =
            batch_elapsed.as_nanos() as f64 / (ITERATIONS * PROVINCE_COUNT) as f64;
        let speedup = scalar_elapsed.as_secs_f64() / batch_elapsed.as_secs_f64();

        println!("\n=== Tax Calculation Benchmark ===");
        println!("Provinces: {}, Iterations: {}", PROVINCE_COUNT, ITERATIONS);
        println!(
            "SIMD level: {}",
            crate::simd::SimdFeatures::detect().best_level()
        );
        println!();
        println!(
            "Scalar:     {:>8.2} ns/province ({:>8.3} ms total)",
            scalar_ns_per_province,
            scalar_elapsed.as_secs_f64() * 1000.0
        );
        println!(
            "Batch:      {:>8.2} ns/province ({:>8.3} ms total)",
            batch_ns_per_province,
            batch_elapsed.as_secs_f64() * 1000.0
        );
        println!("Speedup:    {:>8.2}x", speedup);
        println!();
    }
}
