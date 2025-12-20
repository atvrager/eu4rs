//! Console observer for terminal-based simulation monitoring.
//!
//! Provides real-time display of country statistics with colored deltas.

use super::{ObserverConfig, ObserverError, SimObserver, Snapshot};
use crate::state::RegimentType;
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::Mutex;

/// Console observer that displays country statistics to the terminal.
///
/// Tracks month-over-month deltas for treasury and manpower, displaying
/// gains in green and losses in red.
pub struct ConsoleObserver {
    /// Country tags to observe
    tags: Vec<String>,
    /// Internal state for delta calculation (Mutex for interior mutability)
    state: Mutex<ConsoleState>,
    /// Observer configuration
    config: ObserverConfig,
}

/// Internal state for tracking deltas between months.
struct ConsoleState {
    /// Country state at the start of the current month
    month_start_treasury: HashMap<String, f32>,
    month_start_manpower: HashMap<String, f32>,
    /// Deltas from the previous completed month (displayed values)
    last_month_deltas: HashMap<String, (f32, f32)>,
    /// Number of lines printed (for cursor repositioning)
    lines_printed: usize,
    /// First output flag (skip cursor movement on first render)
    first_print: bool,
}

impl ConsoleObserver {
    /// Create a new console observer for the specified country tags.
    ///
    /// # Arguments
    /// * `tags` - Slice of country tag strings (e.g., `["FRA", "ENG", "SWE"]`)
    pub fn new(tags: &[&str]) -> Self {
        Self {
            tags: tags.iter().map(|s| s.to_string()).collect(),
            state: Mutex::new(ConsoleState {
                month_start_treasury: HashMap::new(),
                month_start_manpower: HashMap::new(),
                last_month_deltas: HashMap::new(),
                lines_printed: 0,
                first_print: true,
            }),
            config: ObserverConfig {
                frequency: 1,
                notify_on_month_start: true,
            },
        }
    }

    /// Set the notification frequency.
    pub fn with_frequency(mut self, frequency: u32) -> Self {
        self.config.frequency = frequency;
        self
    }
}

impl SimObserver for ConsoleObserver {
    fn on_tick(&self, snapshot: &Snapshot) -> Result<(), ObserverError> {
        let mut console_state = self
            .state
            .lock()
            .map_err(|_| ObserverError::Render("Lock poisoned".to_string()))?;

        let world = &snapshot.state;
        let stdout = io::stdout();
        let mut handle = stdout.lock();

        // Cursor movement is handled by the main loop controller
        // if !console_state.first_print && console_state.lines_printed > 0 {
        //     write!(handle, "\x1b[{}A", console_state.lines_printed)?;
        // }
        console_state.first_print = false;

        // Update month-start tracking on 1st of month
        if world.date.day == 1 {
            for tag in &self.tags {
                if let Some(country) = world.countries.get(tag) {
                    let current_treasury = country.treasury.to_f32();
                    let current_manpower = country.manpower.to_f32();

                    // Calculate delta from previous month if we have prior data
                    if let Some(prev_treasury) = console_state.month_start_treasury.get(tag) {
                        let dt = current_treasury - prev_treasury;
                        let prev_manpower = console_state
                            .month_start_manpower
                            .get(tag)
                            .copied()
                            .unwrap_or(0.0);
                        let dm = current_manpower - prev_manpower;
                        console_state
                            .last_month_deltas
                            .insert(tag.clone(), (dt, dm));
                    }

                    // Store current values as new month start
                    console_state
                        .month_start_treasury
                        .insert(tag.clone(), current_treasury);
                    console_state
                        .month_start_manpower
                        .insert(tag.clone(), current_manpower);
                }
            }
        }

        // Header handled by main.rs
        // writeln!(
        //     handle,
        //     "[{}] Tick: {}                    \r",
        //     world.date, snapshot.tick
        // )?;
        let mut lines = 0;

        // Country lines
        for tag in &self.tags {
            if let Some(country) = world.countries.get(tag) {
                let (delta_t, delta_m) = console_state
                    .last_month_deltas
                    .get(tag)
                    .copied()
                    .unwrap_or((0.0, 0.0));

                // Count army composition
                let (mut inf, mut cav, mut art) = (0, 0, 0);
                for army in world.armies.values() {
                    if &army.owner == tag {
                        for reg in &army.regiments {
                            match reg.type_ {
                                RegimentType::Infantry => inf += 1,
                                RegimentType::Cavalry => cav += 1,
                                RegimentType::Artillery => art += 1,
                            }
                        }
                    }
                }

                // Count forts
                let forts = world
                    .provinces
                    .values()
                    .filter(|p| p.owner.as_ref() == Some(tag) && p.has_fort)
                    .count();

                // ANSI color codes
                let color_t = delta_color(delta_t);
                let color_m = delta_color(delta_m);
                let reset = "\x1b[0m";

                writeln!(
                    handle,
                    " {}: 💰{:>8.1}({}{:>+6.1}{}) 👥{:>6.0}({}{:>+5.0}{}) | 👑{:>3.0}/{:>3.0}/{:>3.0} | ⚔️{:>3}/{:>3}/{:>3} 🏰{:>2}    \r",
                    tag,
                    country.treasury.to_f32(),
                    color_t,
                    delta_t,
                    reset,
                    country.manpower.to_f32(),
                    color_m,
                    delta_m,
                    reset,
                    country.adm_mana.to_f32(),
                    country.dip_mana.to_f32(),
                    country.mil_mana.to_f32(),
                    inf,
                    cav,
                    art,
                    forts
                )?;
                lines += 1;
            } else {
                writeln!(
                    handle,
                    " {}: \x1b[31m[ELIMINATED]\x1b[0m                                                  \r",
                    tag
                )?;
                lines += 1;
            }
        }

        handle.flush()?;
        console_state.lines_printed = lines;
        Ok(())
    }

    fn name(&self) -> &str {
        "ConsoleObserver"
    }

    fn config(&self) -> ObserverConfig {
        self.config.clone()
    }
}

/// Returns ANSI color code based on delta sign.
fn delta_color(delta: f32) -> &'static str {
    if delta > 0.0 {
        "\x1b[32m" // Green for gains
    } else if delta < 0.0 {
        "\x1b[31m" // Red for losses
    } else {
        "\x1b[90m" // Gray for no change
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_color() {
        assert_eq!(delta_color(10.0), "\x1b[32m");
        assert_eq!(delta_color(-5.0), "\x1b[31m");
        assert_eq!(delta_color(0.0), "\x1b[90m");
    }

    #[test]
    fn test_console_observer_creation() {
        let observer = ConsoleObserver::new(&["FRA", "ENG", "SWE"]);
        assert_eq!(observer.name(), "ConsoleObserver");
        assert_eq!(observer.tags.len(), 3);
        assert_eq!(observer.config().frequency, 1);
    }

    #[test]
    fn test_console_observer_with_frequency() {
        let observer = ConsoleObserver::new(&["FRA"]).with_frequency(30);
        assert_eq!(observer.config().frequency, 30);
    }
}
