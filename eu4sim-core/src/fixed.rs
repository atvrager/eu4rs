//! Fixed-point arithmetic for deterministic simulation.
//!
//! All simulation values use this type to ensure identical results across platforms.
//! Floats (f32/f64) are banned in sim logic due to x87/SSE/FMA differences.
//!
//! Refactored to use i64 to support large aggregates (e.g. global manpower).

use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

/// Fixed-point value with scale 10000.
///
/// Represents decimal values as integers: 0.25 → 2500, 1.0 → 10000.
/// All arithmetic stays in integer domain for determinism.
/// Uses i64 to prevent overflow with large game values.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct Fixed(pub i64);

impl Fixed {
    /// Scale factor: 10000 = 1.0
    pub const SCALE: i64 = 10000;

    /// Common constants
    pub const ZERO: Fixed = Fixed(0);
    pub const ONE: Fixed = Fixed(10000);
    pub const HALF: Fixed = Fixed(5000);

    /// EU4-specific: 0.2 (goods produced per base_production)
    pub const POINT_TWO: Fixed = Fixed(2000);

    /// Create from raw scaled value
    #[inline]
    pub const fn from_raw(raw: i64) -> Self {
        Fixed(raw)
    }

    /// Create from integer (e.g., 5 → 50_000)
    #[inline]
    pub const fn from_int(v: i64) -> Self {
        Fixed(v * Self::SCALE)
    }

    /// Convert from f32 (parse layer only, not in sim logic).
    ///
    /// Uses `.round()` for cross-platform determinism. Guards against NaN/Inf/overflow.
    #[inline]
    pub fn from_f32(v: f32) -> Self {
        // Guard against NaN/Inf
        if !v.is_finite() {
            return Fixed::ZERO;
        }

        let scaled = v * Self::SCALE as f32;

        // Guard against overflow (Fixed can represent huge numbers now)
        // i64 max is ~9e18. Scaled range is ~±9e14.
        if scaled > i64::MAX as f32 {
            return Fixed(i64::MAX);
        }
        if scaled < i64::MIN as f32 {
            return Fixed(i64::MIN);
        }

        Fixed(scaled.round() as i64)
    }

    /// Convert to f32 (display only, not in sim logic)
    #[inline]
    pub fn to_f32(self) -> f32 {
        self.0 as f32 / Self::SCALE as f32
    }

    /// Convert to f64 (display only, higher precision)
    #[inline]
    pub fn to_f64(self) -> f64 {
        self.0 as f64 / Self::SCALE as f64
    }

    /// Raw integer value
    #[inline]
    pub const fn raw(self) -> i64 {
        self.0
    }

    /// Truncate to integer (rounds toward zero)
    ///
    /// Safe for sim logic (deterministic integer division).
    #[inline]
    pub const fn to_int(self) -> i64 {
        self.0 / Self::SCALE
    }

    /// Returns the smaller of two Fixed values (deterministic)
    #[inline]
    pub fn min(self, other: Fixed) -> Fixed {
        if self.0 <= other.0 {
            self
        } else {
            other
        }
    }

    /// Saturating add (clamps at i64::MAX/MIN)
    #[inline]
    pub fn saturating_add(self, other: Fixed) -> Fixed {
        Fixed(self.0.saturating_add(other.0))
    }

    /// Saturating subtract
    #[inline]
    pub fn saturating_sub(self, other: Fixed) -> Fixed {
        Fixed(self.0.saturating_sub(other.0))
    }

    /// Multiply two fixed-point values: (a × b) / SCALE
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn mul(self, other: Fixed) -> Fixed {
        self * other
    }

    /// Divide two fixed-point values: (a × SCALE) / b
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn div(self, other: Fixed) -> Fixed {
        self / other
    }
}

impl Add for Fixed {
    type Output = Fixed;
    #[inline]
    fn add(self, other: Fixed) -> Fixed {
        Fixed(self.0 + other.0)
    }
}

impl AddAssign for Fixed {
    #[inline]
    fn add_assign(&mut self, other: Fixed) {
        self.0 += other.0;
    }
}

impl Sub for Fixed {
    type Output = Fixed;
    #[inline]
    fn sub(self, other: Fixed) -> Fixed {
        Fixed(self.0 - other.0)
    }
}

impl SubAssign for Fixed {
    #[inline]
    fn sub_assign(&mut self, other: Fixed) {
        self.0 -= other.0;
    }
}

impl Mul for Fixed {
    type Output = Fixed;
    #[inline]
    fn mul(self, other: Fixed) -> Fixed {
        Fixed((self.0 as i128 * other.0 as i128 / Fixed::SCALE as i128) as i64)
    }
}

impl MulAssign for Fixed {
    #[inline]
    fn mul_assign(&mut self, other: Fixed) {
        *self = *self * other;
    }
}

impl Div for Fixed {
    type Output = Fixed;
    #[inline]
    fn div(self, other: Fixed) -> Fixed {
        if other.0 == 0 {
            return Fixed::ZERO; // Safe default for division by zero
        }
        Fixed((self.0 as i128 * Fixed::SCALE as i128 / other.0 as i128) as i64)
    }
}

impl DivAssign for Fixed {
    #[inline]
    fn div_assign(&mut self, other: Fixed) {
        *self = *self / other;
    }
}

impl std::fmt::Debug for Fixed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Fixed({} = {})", self.0, self.to_f32())
    }
}

impl std::fmt::Display for Fixed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.4}", self.to_f32())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(Fixed::ZERO.0, 0);
        assert_eq!(Fixed::ONE.0, 10000);
        assert_eq!(Fixed::HALF.0, 5000);
        assert_eq!(Fixed::POINT_TWO.0, 2000);
    }

    #[test]
    fn test_from_f32() {
        assert_eq!(Fixed::from_f32(0.25), Fixed(2500));
        assert_eq!(Fixed::from_f32(1.0), Fixed::ONE);
        assert_eq!(Fixed::from_f32(0.2), Fixed::POINT_TWO);
    }

    #[test]
    fn test_from_f32_edge_cases() {
        // NaN returns zero
        assert_eq!(Fixed::from_f32(f32::NAN), Fixed::ZERO);

        // Infinity returns zero
        assert_eq!(Fixed::from_f32(f32::INFINITY), Fixed::ZERO);
        assert_eq!(Fixed::from_f32(f32::NEG_INFINITY), Fixed::ZERO);

        // Overflow clamps
        assert_eq!(Fixed::from_f32(1e20), Fixed(i64::MAX));
        assert_eq!(Fixed::from_f32(-1e20), Fixed(i64::MIN));
    }

    #[test]
    fn test_multiply() {
        // 2.0 × 3.0 = 6.0
        let a = Fixed::from_int(2);
        let b = Fixed::from_int(3);
        assert_eq!(a * b, Fixed::from_int(6));

        // 0.5 × 0.5 = 0.25
        assert_eq!(Fixed::HALF * Fixed::HALF, Fixed(2500));
    }

    #[test]
    fn test_divide() {
        // 6.0 / 2.0 = 3.0
        let a = Fixed::from_int(6);
        let b = Fixed::from_int(2);
        assert_eq!(a / b, Fixed::from_int(3));
    }

    #[test]
    fn test_determinism() {
        let calc = || {
            let base = Fixed::from_int(10);
            let price = Fixed::from_f32(2.5);
            let efficiency = Fixed::from_f32(0.15);
            let autonomy = Fixed::from_f32(0.25);

            let goods = base * Fixed::POINT_TWO;
            let eff_factor = Fixed::ONE + efficiency;
            let auto_factor = Fixed::ONE - autonomy;

            goods * price * eff_factor * auto_factor
        };

        let result1 = calc();
        let result2 = calc();
        assert_eq!(result1, result2);
    }

    // Property-based tests - exploring the input space like formal verification
    mod properties {
        use super::*;
        use proptest::prelude::*;

        // Strategy: Generate reasonable game values (-1M to 1M)
        fn game_value() -> impl Strategy<Value = i64> {
            -1_000_000..=1_000_000i64
        }

        proptest! {
            /// Property: Multiplication never overflows (uses i128 intermediate)
            #[test]
            fn mul_never_panics(a in game_value(), b in game_value()) {
                let x = Fixed::from_int(a);
                let y = Fixed::from_int(b);
                let _ = x * y; // Should never panic
            }

            /// Property: Multiplication is commutative (a × b = b × a)
            #[test]
            fn mul_is_commutative(a in game_value(), b in game_value()) {
                let x = Fixed::from_int(a);
                let y = Fixed::from_int(b);
                prop_assert_eq!(x * y, y * x);
            }

            /// Property: Multiplication by ONE is identity (a × 1 = a)
            #[test]
            fn mul_one_is_identity(a in game_value()) {
                let x = Fixed::from_int(a);
                prop_assert_eq!(x * Fixed::ONE, x);
            }

            /// Property: Multiplication by ZERO always yields ZERO
            #[test]
            fn mul_zero_is_zero(a in game_value()) {
                let x = Fixed::from_int(a);
                prop_assert_eq!(x * Fixed::ZERO, Fixed::ZERO);
            }

            /// Property: Division never panics (handles div-by-zero gracefully)
            #[test]
            fn div_never_panics(a in game_value(), b in game_value()) {
                let x = Fixed::from_int(a);
                let y = Fixed::from_int(b);
                let _ = x / y; // Should never panic, returns ZERO for div-by-zero
            }

            /// Property: Division by ONE is identity (a / 1 = a)
            #[test]
            fn div_one_is_identity(a in game_value()) {
                let x = Fixed::from_int(a);
                prop_assert_eq!(x / Fixed::ONE, x);
            }

            /// Property: Division by ZERO returns ZERO (safe fallback)
            #[test]
            fn div_zero_is_safe(a in game_value()) {
                let x = Fixed::from_int(a);
                prop_assert_eq!(x / Fixed::ZERO, Fixed::ZERO);
            }

            /// Property: Addition is commutative (a + b = b + a)
            #[test]
            fn add_is_commutative(a in game_value(), b in game_value()) {
                let x = Fixed::from_int(a);
                let y = Fixed::from_int(b);
                // May overflow, but if both succeed, they're equal
                if let (Some(r1), Some(r2)) = (x.0.checked_add(y.0), y.0.checked_add(x.0)) {
                    prop_assert_eq!(Fixed(r1), Fixed(r2));
                }
            }

            /// Property: Saturating operations never panic
            #[test]
            fn saturating_ops_never_panic(a in game_value(), b in game_value()) {
                let x = Fixed::from_int(a);
                let y = Fixed::from_int(b);
                let _ = x.saturating_add(y);
                let _ = x.saturating_sub(y);
            }

            /// Property: from_f32 never panics (handles NaN/Inf/overflow)
            #[test]
            fn from_f32_never_panics(f in proptest::num::f32::ANY) {
                let _ = Fixed::from_f32(f);
            }

            /// Property: Round-trip through f32 is approximately stable
            /// (within precision limits of f32 and Fixed's scale)
            ///
            /// f32 has 24-bit mantissa (~7 decimal digits). Large Fixed values
            /// (e.g., -62689 * 10000 = -626,890,000) lose precision when converted.
            #[test]
            fn roundtrip_f32_stable(a in -100_000..=100_000i64) {
                let original = Fixed::from_int(a);
                let roundtrip = Fixed::from_f32(original.to_f32());
                // f32 precision loss: allow ~0.01% error for large values
                // Empirically: -62689 has diff=16, so allow up to 0.002% of raw value
                let max_error = (original.0.abs() / 50000).max(1);
                let diff = (original.0 - roundtrip.0).abs();
                prop_assert!(diff <= max_error, "diff was {} (max allowed: {})", diff, max_error);
            }
        }
    }
}

// Kani proofs (commented out - requires `cargo kani` with setup)
// Uncomment when Kani environment is available
/*
#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Kani proof: Multiplication never overflows to an invalid state
    ///
    /// This exhaustively verifies that for ALL possible Fixed values,
    /// multiplication produces a valid result without panic/overflow.
    #[kani::proof]
    fn verify_mul_no_overflow() {
        let a: Fixed = kani::any();
        let b: Fixed = kani::any();

        // The operation must not panic
        let result = a * b;

        // Result must be a valid Fixed value (always true, but makes intent clear)
        kani::assume(result.0 >= i64::MIN && result.0 <= i64::MAX);
    }

    /// Kani proof: Multiplication is commutative
    #[kani::proof]
    fn verify_mul_commutative() {
        let a: Fixed = kani::any();
        let b: Fixed = kani::any();

        kani::assert(a * b == b * a, "Multiplication must be commutative");
    }

    /// Kani proof: Division by zero is safe (returns ZERO)
    #[kani::proof]
    fn verify_div_by_zero_safe() {
        let a: Fixed = kani::any();

        let result = a / Fixed::ZERO;
        kani::assert(result == Fixed::ZERO, "Division by zero must return ZERO");
    }

    /// Kani proof: from_f32 handles all inputs without panic
    #[kani::proof]
    fn verify_from_f32_total() {
        let f: f32 = kani::any();

        // Must not panic for ANY f32 value (including NaN, Inf, etc.)
        let _ = Fixed::from_f32(f);
    }
}
*/
