//! Generic fixed-point types with configurable precision and backing storage.
//!
//! # Design Goals
//!
//! 1. **Configurable precision**: Scale factor as const generic (e.g., 10000 = 0.0001)
//! 2. **Explicit backing type**: Choose i32 (SIMD-friendly) or i64 (wide range)
//! 3. **Type-safe operations**: Can't accidentally mix precisions
//! 4. **Zero runtime overhead**: All dispatch is compile-time
//!
//! # Usage
//!
//! ```ignore
//! use eu4sim_core::fixed_generic::{Fx32, Fx64};
//!
//! // SIMD-friendly: i32 backing, scale 10000
//! type Modifier = Fx32<10000>;
//! let efficiency = Modifier::ONE + Modifier::from_f32(0.25);
//!
//! // Wide range: i64 backing, scale 10000
//! type Treasury = Fx64<10000>;
//! let wealth = Treasury::from_int(1_000_000);
//! ```
//!
//! # Choosing Backing Type
//!
//! | Type | Max Value | Intermediate | SIMD Lanes (AVX2) |
//! |------|-----------|--------------|-------------------|
//! | Fx32 | ±214,748 | i64 | 8 |
//! | Fx64 | ±922 trillion | i128 | 4 |
//!
//! For game values like modifiers, tax rates, and per-province data: use Fx32.
//! For aggregates like total treasury or global manpower: use Fx64.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

// ============================================================================
// Backing Type Trait
// ============================================================================

mod sealed {
    pub trait Sealed {}
    impl Sealed for i16 {}
    impl Sealed for i32 {}
    impl Sealed for i64 {}
}

/// Trait for fixed-point backing storage types.
///
/// Sealed to prevent external implementations.
pub trait FixedBacking: sealed::Sealed + Copy + Ord + Default + Send + Sync + 'static {
    /// Wider type for intermediate calculations (prevents overflow).
    type Wide: Copy
        + From<Self>
        + TryInto<Self>
        + std::ops::Mul<Output = Self::Wide>
        + std::ops::Div<Output = Self::Wide>;

    /// Number of bits in the backing type.
    const BITS: u32;

    /// Zero value.
    const ZERO: Self;

    /// Convert to wide type for intermediate calculations.
    fn to_wide(self) -> Self::Wide;

    /// Convert from wide type (may truncate).
    fn from_wide(wide: Self::Wide) -> Self;

    /// Saturating addition.
    fn saturating_add(self, other: Self) -> Self;

    /// Saturating subtraction.
    fn saturating_sub(self, other: Self) -> Self;
}

impl FixedBacking for i16 {
    type Wide = i32;
    const BITS: u32 = 16;
    const ZERO: Self = 0;

    #[inline]
    fn to_wide(self) -> i32 {
        self as i32
    }

    #[inline]
    fn from_wide(wide: i32) -> i16 {
        wide as i16
    }

    #[inline]
    fn saturating_add(self, other: Self) -> Self {
        i16::saturating_add(self, other)
    }

    #[inline]
    fn saturating_sub(self, other: Self) -> Self {
        i16::saturating_sub(self, other)
    }
}

impl FixedBacking for i32 {
    type Wide = i64;
    const BITS: u32 = 32;
    const ZERO: Self = 0;

    #[inline]
    fn to_wide(self) -> i64 {
        self as i64
    }

    #[inline]
    fn from_wide(wide: i64) -> i32 {
        wide as i32
    }

    #[inline]
    fn saturating_add(self, other: Self) -> Self {
        i32::saturating_add(self, other)
    }

    #[inline]
    fn saturating_sub(self, other: Self) -> Self {
        i32::saturating_sub(self, other)
    }
}

impl FixedBacking for i64 {
    type Wide = i128;
    const BITS: u32 = 64;
    const ZERO: Self = 0;

    #[inline]
    fn to_wide(self) -> i128 {
        self as i128
    }

    #[inline]
    fn from_wide(wide: i128) -> i64 {
        wide as i64
    }

    #[inline]
    fn saturating_add(self, other: Self) -> Self {
        i64::saturating_add(self, other)
    }

    #[inline]
    fn saturating_sub(self, other: Self) -> Self {
        i64::saturating_sub(self, other)
    }
}

// ============================================================================
// Generic Fixed-Point Type
// ============================================================================

/// Generic fixed-point number with configurable backing type and scale.
///
/// # Type Parameters
///
/// - `B`: Backing storage type (i32 or i64)
/// - `SCALE`: Scale factor (e.g., 10000 means 0.0001 precision)
///
/// # Representation
///
/// The value `v` represents `v / SCALE`. For example, with SCALE=10000:
/// - Raw 25000 = 2.5
/// - Raw 10000 = 1.0
/// - Raw 5000 = 0.5
#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Fixed<B: FixedBacking, const SCALE: u32>(B);

// Type aliases for convenience
pub type Fx16<const SCALE: u32> = Fixed<i16, SCALE>;
pub type Fx32<const SCALE: u32> = Fixed<i32, SCALE>;
pub type Fx64<const SCALE: u32> = Fixed<i64, SCALE>;

// Common precision aliases
//
// | Type | Backing | Max Value | Precision | SIMD Lanes (AVX2) |
// |------|---------|-----------|-----------|-------------------|
// | Prestige16 | i16 | ±327 | 0.01 | 16 |
// | Mod32 | i32 | ±214k | 0.0001 | 8 |
// | Mod64 | i64 | ±922T | 0.0001 | 4 |
//
pub type Prestige16 = Fx16<100>; // Prestige/stability (±327 range, 0.01 precision, 16 lanes!)
pub type Mod32 = Fx32<10000>; // Modifiers, rates (±214k range, 0.0001 precision)
pub type Mod64 = Fx64<10000>; // Large aggregates (±922T range, 0.0001 precision)

// ============================================================================
// Interop with original Fixed type
// ============================================================================

impl Mod32 {
    /// Convert from the original Fixed type (may truncate large values).
    #[inline]
    pub fn from_fixed(other: crate::fixed::Fixed) -> Self {
        // Both use scale 10000, just narrow i64 -> i32
        Fixed(other.raw() as i32)
    }

    /// Convert to the original Fixed type (widening, lossless).
    #[inline]
    pub fn to_fixed(self) -> crate::fixed::Fixed {
        crate::fixed::Fixed::from_raw(self.0 as i64)
    }
}

impl<B: FixedBacking, const SCALE: u32> Fixed<B, SCALE> {
    /// The scale factor for this fixed-point type.
    pub const SCALE: u32 = SCALE;

    /// Zero value.
    pub const ZERO: Self = Fixed(B::ZERO);

    /// Create from raw scaled value.
    #[inline]
    pub const fn from_raw(raw: B) -> Self {
        Fixed(raw)
    }

    /// Get the raw scaled value.
    #[inline]
    pub const fn raw(self) -> B {
        self.0
    }

    /// Minimum of two values.
    #[inline]
    pub fn min(self, other: Self) -> Self {
        if self.0 <= other.0 {
            self
        } else {
            other
        }
    }

    /// Maximum of two values.
    #[inline]
    pub fn max(self, other: Self) -> Self {
        if self.0 >= other.0 {
            self
        } else {
            other
        }
    }

    /// Clamp to range.
    #[inline]
    pub fn clamp(self, min: Self, max: Self) -> Self {
        self.max(min).min(max)
    }

    /// Saturating addition.
    #[inline]
    pub fn saturating_add(self, other: Self) -> Self {
        Fixed(self.0.saturating_add(other.0))
    }

    /// Saturating subtraction.
    #[inline]
    pub fn saturating_sub(self, other: Self) -> Self {
        Fixed(self.0.saturating_sub(other.0))
    }
}

// ============================================================================
// i16-backed specializations (16 SIMD lanes!)
// ============================================================================

impl<const SCALE: u32> Fixed<i16, SCALE> {
    /// One (1.0).
    pub const ONE: Self = Fixed(SCALE as i16);

    /// Half (0.5).
    pub const HALF: Self = Fixed((SCALE / 2) as i16);

    /// Create from integer.
    #[inline]
    pub const fn from_int(v: i16) -> Self {
        Fixed((v as i32 * SCALE as i32) as i16)
    }

    /// Convert from f32 (for initialization only).
    #[inline]
    pub fn from_f32(v: f32) -> Self {
        if !v.is_finite() {
            return Self::ZERO;
        }
        let scaled = v * SCALE as f32;
        if scaled > i16::MAX as f32 {
            return Fixed(i16::MAX);
        }
        if scaled < i16::MIN as f32 {
            return Fixed(i16::MIN);
        }
        Fixed(scaled.round() as i16)
    }

    /// Convert to f32 (for display only).
    #[inline]
    pub fn to_f32(self) -> f32 {
        self.0 as f32 / SCALE as f32
    }

    /// Truncate to integer.
    #[inline]
    pub const fn to_int(self) -> i16 {
        self.0 / SCALE as i16
    }
}

// ============================================================================
// i32-backed specializations
// ============================================================================

impl<const SCALE: u32> Fixed<i32, SCALE> {
    /// One (1.0).
    pub const ONE: Self = Fixed(SCALE as i32);

    /// Half (0.5).
    pub const HALF: Self = Fixed((SCALE / 2) as i32);

    /// Create from integer.
    #[inline]
    pub const fn from_int(v: i32) -> Self {
        Fixed(v * SCALE as i32)
    }

    /// Convert from f32 (for initialization only, not simulation).
    #[inline]
    pub fn from_f32(v: f32) -> Self {
        if !v.is_finite() {
            return Self::ZERO;
        }
        let scaled = v * SCALE as f32;
        if scaled > i32::MAX as f32 {
            return Fixed(i32::MAX);
        }
        if scaled < i32::MIN as f32 {
            return Fixed(i32::MIN);
        }
        Fixed(scaled.round() as i32)
    }

    /// Convert to f32 (for display only).
    #[inline]
    pub fn to_f32(self) -> f32 {
        self.0 as f32 / SCALE as f32
    }

    /// Truncate to integer.
    #[inline]
    pub const fn to_int(self) -> i32 {
        self.0 / SCALE as i32
    }
}

// ============================================================================
// i64-backed specializations
// ============================================================================

impl<const SCALE: u32> Fixed<i64, SCALE> {
    /// One (1.0).
    pub const ONE: Self = Fixed(SCALE as i64);

    /// Half (0.5).
    pub const HALF: Self = Fixed((SCALE / 2) as i64);

    /// Create from integer.
    #[inline]
    pub const fn from_int(v: i64) -> Self {
        Fixed(v * SCALE as i64)
    }

    /// Convert from f32 (for initialization only).
    #[inline]
    pub fn from_f32(v: f32) -> Self {
        if !v.is_finite() {
            return Self::ZERO;
        }
        let scaled = v * SCALE as f32;
        if scaled > i64::MAX as f32 {
            return Fixed(i64::MAX);
        }
        if scaled < i64::MIN as f32 {
            return Fixed(i64::MIN);
        }
        Fixed(scaled.round() as i64)
    }

    /// Convert to f32 (for display only).
    #[inline]
    pub fn to_f32(self) -> f32 {
        self.0 as f32 / SCALE as f32
    }

    /// Convert to f64 (higher precision display).
    #[inline]
    pub fn to_f64(self) -> f64 {
        self.0 as f64 / SCALE as f64
    }

    /// Truncate to integer.
    #[inline]
    pub const fn to_int(self) -> i64 {
        self.0 / SCALE as i64
    }

    /// Convert from Fx32 (widening conversion).
    #[inline]
    pub fn from_fx32(other: Fixed<i32, SCALE>) -> Self {
        Fixed(other.0 as i64)
    }
}

// ============================================================================
// Arithmetic Operations
// ============================================================================

impl<B: FixedBacking, const SCALE: u32> Add for Fixed<B, SCALE> {
    type Output = Self;
    #[inline]
    fn add(self, other: Self) -> Self {
        // Note: Can overflow! Use saturating_add for safety.
        Fixed(self.0.saturating_add(other.0))
    }
}

impl<B: FixedBacking, const SCALE: u32> AddAssign for Fixed<B, SCALE> {
    #[inline]
    fn add_assign(&mut self, other: Self) {
        self.0 = self.0.saturating_add(other.0);
    }
}

impl<B: FixedBacking, const SCALE: u32> Sub for Fixed<B, SCALE> {
    type Output = Self;
    #[inline]
    fn sub(self, other: Self) -> Self {
        Fixed(self.0.saturating_sub(other.0))
    }
}

impl<B: FixedBacking, const SCALE: u32> SubAssign for Fixed<B, SCALE> {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.0 = self.0.saturating_sub(other.0);
    }
}

// Multiplication: (a * b) / SCALE using wide intermediate
impl<const SCALE: u32> Mul for Fixed<i16, SCALE> {
    type Output = Self;
    #[inline]
    fn mul(self, other: Self) -> Self {
        let wide = self.0 as i32 * other.0 as i32;
        Fixed((wide / SCALE as i32) as i16)
    }
}

impl<const SCALE: u32> Mul for Fixed<i32, SCALE> {
    type Output = Self;
    #[inline]
    fn mul(self, other: Self) -> Self {
        let wide = self.0 as i64 * other.0 as i64;
        Fixed((wide / SCALE as i64) as i32)
    }
}

impl<const SCALE: u32> Mul for Fixed<i64, SCALE> {
    type Output = Self;
    #[inline]
    fn mul(self, other: Self) -> Self {
        let wide = self.0 as i128 * other.0 as i128;
        Fixed((wide / SCALE as i128) as i64)
    }
}

// Division: (a * SCALE) / b using wide intermediate
impl<const SCALE: u32> Div for Fixed<i16, SCALE> {
    type Output = Self;
    #[inline]
    fn div(self, other: Self) -> Self {
        if other.0 == 0 {
            return Self::ZERO;
        }
        let wide = self.0 as i32 * SCALE as i32;
        Fixed((wide / other.0 as i32) as i16)
    }
}

impl<const SCALE: u32> Div for Fixed<i32, SCALE> {
    type Output = Self;
    #[inline]
    fn div(self, other: Self) -> Self {
        if other.0 == 0 {
            return Self::ZERO;
        }
        let wide = self.0 as i64 * SCALE as i64;
        Fixed((wide / other.0 as i64) as i32)
    }
}

impl<const SCALE: u32> Div for Fixed<i64, SCALE> {
    type Output = Self;
    #[inline]
    fn div(self, other: Self) -> Self {
        if other.0 == 0 {
            return Self::ZERO;
        }
        let wide = self.0 as i128 * SCALE as i128;
        Fixed((wide / other.0 as i128) as i64)
    }
}

// Negation
impl<const SCALE: u32> Neg for Fixed<i16, SCALE> {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Fixed(-self.0)
    }
}

impl<const SCALE: u32> Neg for Fixed<i32, SCALE> {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Fixed(-self.0)
    }
}

impl<const SCALE: u32> Neg for Fixed<i64, SCALE> {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Fixed(-self.0)
    }
}

// ============================================================================
// Comparison
// ============================================================================

impl<B: FixedBacking, const SCALE: u32> PartialEq for Fixed<B, SCALE> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<B: FixedBacking, const SCALE: u32> Eq for Fixed<B, SCALE> {}

impl<B: FixedBacking, const SCALE: u32> PartialOrd for Fixed<B, SCALE> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<B: FixedBacking, const SCALE: u32> Ord for Fixed<B, SCALE> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl<B: FixedBacking, const SCALE: u32> std::hash::Hash for Fixed<B, SCALE>
where
    B: std::hash::Hash,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

// ============================================================================
// Display
// ============================================================================

impl<B: FixedBacking + Into<i64>, const SCALE: u32> fmt::Debug for Fixed<B, SCALE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let val: i64 = self.0.into();
        let float_val = val as f64 / SCALE as f64;
        write!(f, "Fixed<{}>({} = {})", SCALE, val, float_val)
    }
}

impl<B: FixedBacking + Into<i64>, const SCALE: u32> fmt::Display for Fixed<B, SCALE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let val: i64 = self.0.into();
        let float_val = val as f64 / SCALE as f64;
        write!(f, "{:.4}", float_val)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fx32_basic() {
        type M = Fx32<10000>;

        assert_eq!(M::ONE.raw(), 10000);
        assert_eq!(M::HALF.raw(), 5000);
        assert_eq!(M::from_int(5).raw(), 50000);
        assert_eq!(M::from_f32(0.25).raw(), 2500);
    }

    #[test]
    fn test_fx32_arithmetic() {
        type M = Fx32<10000>;

        let a = M::from_f32(2.0);
        let b = M::from_f32(3.0);

        // 2 + 3 = 5
        assert_eq!((a + b).to_f32(), 5.0);

        // 2 * 3 = 6
        assert_eq!((a * b).to_f32(), 6.0);

        // 6 / 2 = 3
        let six = M::from_f32(6.0);
        assert_eq!((six / a).to_f32(), 3.0);
    }

    #[test]
    fn test_fx32_simd_friendly_multiply() {
        type M = Fx32<10000>;

        // i32 * i32 -> i64 intermediate, no i128 needed!
        let a = M::from_f32(100.0); // raw = 1_000_000
        let b = M::from_f32(100.0); // raw = 1_000_000
                                    // 1M * 1M = 1T, fits in i64, then / 10000 = 100M, fits in i32
        let result = a * b;
        assert_eq!(result.to_f32(), 10000.0);
    }

    #[test]
    fn test_fx64_wide_range() {
        type T = Fx64<10000>;

        let million = T::from_int(1_000_000);
        let thousand = T::from_int(1_000);

        // 1M * 1000 = 1B
        let result = million * thousand;
        assert_eq!(result.to_int(), 1_000_000_000);
    }

    #[test]
    fn test_type_safety() {
        // These are different types - can't mix them!
        type Precision1000 = Fx32<1000>;
        type Precision10000 = Fx32<10000>;

        let _a: Precision1000 = Precision1000::from_f32(1.5);
        let _b: Precision10000 = Precision10000::from_f32(1.5);

        // This won't compile: let _c = _a + _b;
        // Good! Type system prevents mixing precisions.
    }

    #[test]
    fn test_mod32_for_taxation() {
        // Mod32 is the recommended type for game modifiers
        let base_tax = Mod32::from_f32(12.0);
        let efficiency = Mod32::ONE + Mod32::from_f32(0.5); // 150%
        let autonomy_factor = Mod32::ONE - Mod32::from_f32(0.25); // 75%

        let yearly = base_tax * efficiency * autonomy_factor;
        let monthly = yearly / Mod32::from_int(12);

        // 12 * 1.5 * 0.75 / 12 = 1.125
        assert!((monthly.to_f32() - 1.125).abs() < 0.001);
    }

    #[test]
    fn test_widening_conversion() {
        let narrow: Fx32<10000> = Fx32::from_f32(42.5);
        let wide: Fx64<10000> = Fx64::from_fx32(narrow);

        assert_eq!(wide.to_f32(), 42.5);
    }
}

#[cfg(test)]
mod simd_comparison {
    use super::*;

    /// Demonstrate that Fx32 multiplication uses i64 (not i128) intermediate.
    ///
    /// This is SIMD-friendly because AVX2 has native i32*i32->i64 operations
    /// (_mm256_mul_epi32), while i128 requires scalar fallback.
    #[test]
    fn fx32_avoids_i128() {
        type M = Fx32<10000>;

        // Maximum safe values for i32 fixed-point with scale 10000:
        // max_raw = i32::MAX = 2,147,483,647
        // max_value = max_raw / 10000 = 214,748.3647
        //
        // For multiply: max_raw * max_raw = 4.6e18, fits in i64 (max 9.2e18)
        // Then divide by SCALE to get result.

        let big = M::from_int(200); // raw = 2,000,000
        let result = big * big; // raw = 4e12 / 10000 = 4e8

        assert_eq!(result.to_int(), 40000);
    }

    /// Prestige16: i16 backing with scale 100 (0.01 precision, ±327 range).
    ///
    /// Uses i32 intermediate for multiply - 16 SIMD lanes with AVX2!
    #[test]
    fn prestige16_basic_ops() {
        // ±100 range is common for prestige, stability, legitimacy
        let prestige = Prestige16::from_f32(50.0);
        let modifier = Prestige16::from_f32(1.5); // +50% modifier

        // 50 * 1.5 = 75
        let boosted = prestige * modifier;
        assert!((boosted.to_f32() - 75.0).abs() < 0.1);

        // Negation
        let negative = -Prestige16::from_f32(25.0);
        assert_eq!(negative.to_int(), -25);

        // Division
        let half = prestige / Prestige16::from_int(2);
        assert_eq!(half.to_int(), 25);
    }

    /// Verify i16 range limits for Prestige16.
    #[test]
    fn prestige16_range() {
        // Scale 100 means: max_raw = 32767, max_value = 327.67
        // This covers ±100 prestige range with plenty of headroom

        let max = Prestige16::from_int(100);
        let min = Prestige16::from_int(-100);

        assert_eq!(max.raw(), 10000); // 100 * 100 scale
        assert_eq!(min.raw(), -10000);

        // Can represent values up to ±327
        let big = Prestige16::from_int(300);
        assert_eq!(big.to_int(), 300);
    }
}
