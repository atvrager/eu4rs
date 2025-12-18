# Simulation Calendar System

The `eu4sim` project uses a deterministic calendar system to drive the simulation tick by tick.

## Current Implementation: Simplified Gregorian

Currently, the simulation uses a "simplified" calendar for computational efficiency and predictability:
- **Months**: All months are exactly 30 days.
- **Years**: exactly 12 months (360 days).
- **Leap Years**: None.

### Rational
This simplification ensures that "monthly ticks" (economic systems) occur consistently every 30 days, simplifying debt, interest, and production calculations.

## Future Plans: Pluggable Calendars

To support historical accuracy and diverse game starts, we plan to implement a pluggable `Calendar` trait:

```rust
pub trait Calendar {
    fn add_days(&self, date: Date, days: u32) -> Date;
    fn is_month_start(&self, date: Date) -> bool;
    fn is_year_start(&self, date: Date) -> bool;
}
```

### Potential Extensions
- **Standard Gregorian**: Accurate month lengths and leap years (as used in vanilla EU4).
- **Roman Calendar**: For "Imperator" style mods.
- **Islamic/Hebrew Calendars**: For culturally specific simulation modes.
- **Mayan/Aztec Calendars**: For pre-Columbian accurate simulations.
