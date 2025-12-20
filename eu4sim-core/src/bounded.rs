use crate::fixed::Fixed;
use serde::{Deserialize, Serialize};

/// A value clamped to a Fixed-point range (for continuous values).
/// Used for: prestige (-100 to +100), army tradition (0 to 100), etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BoundedFixed {
    value: Fixed,
    min: Fixed,
    max: Fixed,
}

impl BoundedFixed {
    pub const fn new(value: Fixed, min: Fixed, max: Fixed) -> Self {
        let value = if value.raw() < min.raw() {
            min
        } else if value.raw() > max.raw() {
            max
        } else {
            value
        };
        Self { value, min, max }
    }

    pub fn get(&self) -> Fixed {
        self.value
    }

    pub fn min(&self) -> Fixed {
        self.min
    }

    pub fn max(&self) -> Fixed {
        self.max
    }

    pub fn add(&mut self, delta: Fixed) {
        self.value = (self.value + delta).max(self.min).min(self.max);
    }

    pub fn set(&mut self, value: Fixed) {
        self.value = value.max(self.min).min(self.max);
    }

    /// Ratio from 0.0 to 1.0 as Fixed.
    /// Returns 0 if max == min.
    pub fn ratio(&self) -> Fixed {
        let range = self.max - self.min;
        if range == Fixed::ZERO {
            return Fixed::ZERO;
        }
        (self.value - self.min).div(range)
    }

    /// Decay toward a target by a rate (e.g., 0.05 = 5%)
    ///
    /// Deterministic: `value = value + (target - value) * rate`
    pub fn decay_toward(&mut self, target: Fixed, rate: Fixed) {
        let delta = (target - self.value).mul(rate);
        self.value += delta;
        // Clamp (safe arithmetic)
        self.value = self.value.max(self.min).min(self.max);
    }
}

/// A value clamped to an integer range (for discrete values).
/// Used for: stability (-3 to +3), mercantilism (0 to 100), etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BoundedInt {
    value: i32,
    min: i32,
    max: i32,
}

impl BoundedInt {
    pub const fn new(value: i32, min: i32, max: i32) -> Self {
        let value = if value < min {
            min
        } else if value > max {
            max
        } else {
            value
        };
        Self { value, min, max }
    }

    pub fn get(&self) -> i32 {
        self.value
    }

    pub fn min(&self) -> i32 {
        self.min
    }

    pub fn max(&self) -> i32 {
        self.max
    }

    pub fn add(&mut self, delta: i32) {
        self.value = (self.value + delta).clamp(self.min, self.max);
    }

    pub fn set(&mut self, value: i32) {
        self.value = value.clamp(self.min, self.max);
    }

    /// Ratio from 0.0 to 1.0 as Fixed.
    /// Returns 0 if max == min.
    pub fn ratio(&self) -> Fixed {
        let range = self.max - self.min;
        if range == 0 {
            return Fixed::ZERO;
        }
        Fixed::from_int((self.value - self.min) as i64).div(Fixed::from_int(range as i64))
    }
}

pub type Stability = BoundedInt;
pub type Prestige = BoundedFixed;
pub type Tradition = BoundedFixed;

// Factory functions
pub const fn new_stability() -> BoundedInt {
    BoundedInt::new(0, -3, 3)
}

pub const fn new_prestige() -> BoundedFixed {
    BoundedFixed::new(Fixed::ZERO, Fixed::from_int(-100), Fixed::from_int(100))
}

pub const fn new_tradition() -> BoundedFixed {
    BoundedFixed::new(Fixed::ZERO, Fixed::ZERO, Fixed::from_int(100))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounded_fixed_clamps() {
        let mut b = BoundedFixed::new(Fixed::ZERO, Fixed::from_int(-10), Fixed::from_int(10));

        b.add(Fixed::from_int(5));
        assert_eq!(b.get(), Fixed::from_int(5));

        b.add(Fixed::from_int(10)); // Should clamp to 10
        assert_eq!(b.get(), Fixed::from_int(10));

        b.add(Fixed::from_int(-30)); // Should clamp to -10
        assert_eq!(b.get(), Fixed::from_int(-10));
    }

    #[test]
    fn test_bounded_int_clamps() {
        let mut b = BoundedInt::new(0, -5, 5);

        b.add(3);
        assert_eq!(b.get(), 3);

        b.add(10); // Should clamp to 5
        assert_eq!(b.get(), 5);

        b.add(-20); // Should clamp to -5
        assert_eq!(b.get(), -5);
    }

    #[test]
    fn test_ratio_calculation() {
        // Range 0 to 100, val 50 => 0.5
        let b = BoundedFixed::new(Fixed::from_int(50), Fixed::ZERO, Fixed::from_int(100));
        assert_eq!(b.ratio(), Fixed::HALF);

        // Range -100 to 100, val 0 => 0.5
        let p = BoundedFixed::new(Fixed::ZERO, Fixed::from_int(-100), Fixed::from_int(100));
        assert_eq!(p.ratio(), Fixed::HALF);

        // Range -3 to 3, val 0 => 0.5
        let s = BoundedInt::new(0, -3, 3);
        assert_eq!(s.ratio(), Fixed::HALF);
    }

    #[test]
    fn test_decay_toward_zero() {
        let mut val = BoundedFixed::new(Fixed::from_int(100), Fixed::ZERO, Fixed::from_int(100));
        let rate = Fixed::from_f32(0.5); // Decay by 50% each step

        val.decay_toward(Fixed::ZERO, rate);
        assert_eq!(val.get(), Fixed::from_int(50));

        val.decay_toward(Fixed::ZERO, rate);
        assert_eq!(val.get(), Fixed::from_int(25));
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_bounded_fixed_updates_stay_within_bounds(
            initial in -1000..1000i64,
            updates in proptest::collection::vec(-1000..1000i64, 1..20)
        ) {
            let mut b = BoundedFixed::new(
                Fixed::from_int(initial),
                Fixed::from_int(-100),
                Fixed::from_int(100)
            );

            for update in updates {
                b.add(Fixed::from_int(update));
                assert!(b.get() >= b.min());
                assert!(b.get() <= b.max());
            }
        }
    }
}
