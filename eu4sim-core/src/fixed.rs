//! Fixed-point arithmetic for deterministic simulation.
//!
//! All simulation values use this type to ensure identical results across platforms.
//! Floats (f32/f64) are banned in sim logic due to x87/SSE/FMA differences.
//!
//! Refactored to use i64 to support large aggregates (e.g. global manpower).

use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Sub, SubAssign};

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

    /// Create from integer (e.g., 5 → 50000)
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

    /// Multiply two fixed-point values: (a × b) / SCALE
    ///
    /// Uses i128 intermediate to prevent overflow.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn mul(self, other: Fixed) -> Fixed {
        Fixed((self.0 as i128 * other.0 as i128 / Self::SCALE as i128) as i64)
    }

    /// Divide two fixed-point values: (a × SCALE) / b
    ///
    /// Uses i128 intermediate to preserve precision.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn div(self, other: Fixed) -> Fixed {
        if other.0 == 0 {
            return Fixed::ZERO; // Safe default for division by zero
        }
        Fixed((self.0 as i128 * Self::SCALE as i128 / other.0 as i128) as i64)
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
        assert_eq!(a.mul(b), Fixed::from_int(6));

        // 0.5 × 0.5 = 0.25
        assert_eq!(Fixed::HALF.mul(Fixed::HALF), Fixed(2500));
    }

    #[test]
    fn test_divide() {
        // 6.0 / 2.0 = 3.0
        let a = Fixed::from_int(6);
        let b = Fixed::from_int(2);
        assert_eq!(a.div(b), Fixed::from_int(3));
    }

    #[test]
    fn test_determinism() {
        let calc = || {
            let base = Fixed::from_int(10);
            let price = Fixed::from_f32(2.5);
            let efficiency = Fixed::from_f32(0.15);
            let autonomy = Fixed::from_f32(0.25);

            let goods = base.mul(Fixed::POINT_TWO);
            let eff_factor = Fixed::ONE + efficiency;
            let auto_factor = Fixed::ONE - autonomy;

            goods.mul(price).mul(eff_factor).mul(auto_factor)
        };

        let result1 = calc();
        let result2 = calc();
        assert_eq!(result1, result2);
    }
}
