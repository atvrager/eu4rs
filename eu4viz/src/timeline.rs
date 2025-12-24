use eu4sim_core::observer::event_log::GameEvent;
use eu4sim_core::state::{ProvinceId, Tag};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Efficient timeline for reconstructing map state at any point in time.
///
/// Uses sparse ownership changes instead of full state snapshots.
/// Memory scales with events (~10 MB for 377 years), not time.
pub struct Timeline {
    /// All events sorted by tick (used for event feed display)
    #[allow(dead_code)]
    events: Vec<GameEvent>,

    /// Sparse ownership changes: (tick, province_id, new_owner)
    /// Sorted by tick for efficient seeking
    ownership_changes: Vec<OwnershipChange>,

    /// Initial ownership state (1444 baseline)
    initial_owners: HashMap<ProvinceId, Option<Tag>>,

    /// Cached "current view" state (avoids recomputing from scratch)
    cached_state: CachedState,

    /// Timeline bounds
    start_tick: u64,
    end_tick: u64,
}

#[derive(Clone, Debug)]
struct OwnershipChange {
    tick: u64,
    date: String,
    province_id: ProvinceId,
    /// Previous owner (used for backward seek reconstruction)
    #[allow(dead_code)]
    old_owner: Option<Tag>,
    new_owner: Option<Tag>,
}

#[derive(Clone)]
struct CachedState {
    /// Current tick the cache represents
    tick: u64,
    /// Province -> current owner at cached tick
    owners: HashMap<ProvinceId, Option<Tag>>,
}

impl Timeline {
    /// Load a timeline from a JSONL event log file.
    ///
    /// # Errors
    ///
    /// Returns error if file cannot be read or parsed.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let file =
            File::open(path.as_ref()).map_err(|e| format!("Failed to open event log: {}", e))?;
        let reader = BufReader::new(file);

        let mut events = Vec::new();
        let mut ownership_changes = Vec::new();
        let mut initial_owners = HashMap::new();
        // Always start at tick 0 (1444.11.11) for consistent timeline behavior
        let start_tick = 0u64;
        let mut end_tick = 0;

        for (line_num, line) in reader.lines().enumerate() {
            let line = line.map_err(|e| format!("Failed to read line {}: {}", line_num + 1, e))?;
            let event: GameEvent = serde_json::from_str(&line)
                .map_err(|e| format!("Failed to parse line {}: {}", line_num + 1, e))?;

            // Track end bound (start is always 0)
            let tick = Self::event_tick(&event);
            end_tick = end_tick.max(tick);

            // Extract ownership changes
            if let GameEvent::ProvinceOwnerChanged {
                tick,
                date,
                province_id,
                old_owner,
                new_owner,
            } = &event
            {
                ownership_changes.push(OwnershipChange {
                    tick: *tick,
                    date: date.clone(),
                    province_id: *province_id,
                    old_owner: old_owner.clone(),
                    new_owner: new_owner.clone(),
                });

                // Track initial state (first occurrence of each province)
                initial_owners
                    .entry(*province_id)
                    .or_insert_with(|| old_owner.clone());
            }

            events.push(event);
        }

        // Sort ownership changes by tick
        ownership_changes.sort_by_key(|c| c.tick);

        // Build true initial state by applying all events at start_tick
        // (initial_owners holds the pre-game state, we need post-start_tick state)
        let mut initial_state = initial_owners.clone();
        for change in &ownership_changes {
            if change.tick == start_tick {
                initial_state.insert(change.province_id, change.new_owner.clone());
            } else {
                break; // ownership_changes is sorted
            }
        }

        // Initialize cache at start with the applied initial state
        let cached_state = CachedState {
            tick: start_tick,
            owners: initial_state.clone(),
        };

        // Replace initial_owners with the applied state for backward seeks
        let initial_owners = initial_state;

        Ok(Self {
            events,
            ownership_changes,
            initial_owners,
            cached_state,
            start_tick,
            end_tick,
        })
    }

    /// Get the tick number from any event.
    fn event_tick(event: &GameEvent) -> u64 {
        match event {
            GameEvent::WarDeclared { tick, .. } => *tick,
            GameEvent::PeaceWhite { tick, .. } => *tick,
            GameEvent::PeaceProvinces { tick, .. } => *tick,
            GameEvent::PeaceAnnexation { tick, .. } => *tick,
            GameEvent::CountryEliminated { tick, .. } => *tick,
            GameEvent::ProvinceOwnerChanged { tick, .. } => *tick,
            GameEvent::BattleFought { tick, .. } => *tick,
            GameEvent::SiegeCompleted { tick, .. } => *tick,
        }
    }

    /// Seek to a specific tick, updating the cached state.
    ///
    /// - O(1) for adjacent ticks (forward stepping)
    /// - O(n) for backward seeks (rebuilds from start)
    pub fn seek_to(&mut self, target_tick: u64) {
        if target_tick == self.cached_state.tick {
            return; // No-op
        }

        if target_tick > self.cached_state.tick {
            // Forward: apply changes between cached and target
            for change in &self.ownership_changes {
                if change.tick > self.cached_state.tick && change.tick <= target_tick {
                    self.cached_state
                        .owners
                        .insert(change.province_id, change.new_owner.clone());
                }
            }
        } else {
            // Backward: rebuild from start
            // TODO: Add checkpoints every N ticks for faster backward seeks
            self.cached_state.owners = self.initial_owners.clone();
            for change in &self.ownership_changes {
                if change.tick <= target_tick {
                    self.cached_state
                        .owners
                        .insert(change.province_id, change.new_owner.clone());
                } else {
                    break; // ownership_changes is sorted
                }
            }
        }

        self.cached_state.tick = target_tick;
    }

    /// Get the current province ownership map.
    pub fn current_owners(&self) -> &HashMap<ProvinceId, Option<Tag>> {
        &self.cached_state.owners
    }

    /// Get the current tick.
    pub fn current_tick(&self) -> u64 {
        self.cached_state.tick
    }

    /// Get the timeline bounds.
    pub fn bounds(&self) -> (u64, u64) {
        (self.start_tick, self.end_tick)
    }

    /// Get all events (for event feed display).
    #[allow(dead_code)]
    pub fn events(&self) -> &[GameEvent] {
        &self.events
    }

    /// Get the date string for the current tick.
    ///
    /// First tries to find a date from ownership events. If none exist yet,
    /// computes the date based on the game start date (1444.11.11) plus tick days.
    pub fn current_date(&self) -> Option<String> {
        let tick = self.cached_state.tick;

        // Try to find date from ownership changes
        if let Some(change) = self.ownership_changes.iter().rev().find(|c| c.tick <= tick) {
            return Some(change.date.clone());
        }

        // Fallback: compute date from tick (game starts 1444.11.11, each tick = 1 day)
        Some(Self::tick_to_date(tick))
    }

    /// Convert a tick number to a date string.
    ///
    /// EU4 starts at 1444.11.11. Each tick is one day.
    fn tick_to_date(tick: u64) -> String {
        // Days in each month (non-leap year)
        const DAYS_IN_MONTH: [u64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

        let mut days = tick;
        let mut year = 1444u64;
        let mut month = 11u64; // November (1-indexed)
        let mut day = 11u64;

        // Add days one at a time (simple approach for correctness)
        while days > 0 {
            let days_in_current_month = if month == 2 && Self::is_leap_year(year) {
                29
            } else {
                DAYS_IN_MONTH[(month - 1) as usize]
            };

            let remaining_in_month = days_in_current_month - day;
            if days <= remaining_in_month {
                day += days;
                break;
            }

            days -= remaining_in_month + 1;
            day = 1;
            month += 1;
            if month > 12 {
                month = 1;
                year += 1;
            }
        }

        format!("{}.{}.{}", year, month, day)
    }

    /// Check if a year is a leap year (Gregorian calendar).
    fn is_leap_year(year: u64) -> bool {
        (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
    }

    /// Get events near a specific tick (for event feed).
    #[allow(dead_code)]
    pub fn events_near(&self, tick: u64, window: u64) -> Vec<&GameEvent> {
        self.events
            .iter()
            .filter(|e| {
                let event_tick = Self::event_tick(e);
                event_tick >= tick.saturating_sub(window) && event_tick <= tick + window
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Create a mock event log for testing
    fn create_test_log() -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();

        // Simulate province ownership changes over time
        // Province 151 (Constantinople): BYZ -> OTT at tick 100
        // Province 1 (Stockholm): SWE throughout
        let events = [
            r#"{"type":"province_owner_changed","tick":0,"date":"1444.11.11","province_id":151,"old_owner":null,"new_owner":"BYZ"}"#,
            r#"{"type":"province_owner_changed","tick":0,"date":"1444.11.11","province_id":1,"old_owner":null,"new_owner":"SWE"}"#,
            r#"{"type":"province_owner_changed","tick":100,"date":"1453.5.29","province_id":151,"old_owner":"BYZ","new_owner":"OTT"}"#,
            r#"{"type":"province_owner_changed","tick":200,"date":"1500.1.1","province_id":1,"old_owner":"SWE","new_owner":"DAN"}"#,
        ];

        for event in &events {
            writeln!(file, "{}", event).unwrap();
        }
        file.flush().unwrap();
        file
    }

    #[test]
    fn test_timeline_bounds() {
        let log = create_test_log();
        let timeline = Timeline::from_file(log.path()).expect("Failed to load timeline");

        let (start, end) = timeline.bounds();
        assert_eq!(start, 0, "Start tick should be 0");
        assert_eq!(end, 200, "End tick should be 200");
    }

    #[test]
    fn test_timeline_initial_state() {
        let log = create_test_log();
        let timeline = Timeline::from_file(log.path()).expect("Failed to load timeline");

        // At tick 0, Constantinople should be owned by BYZ
        let owners = timeline.current_owners();
        assert_eq!(owners.get(&151), Some(&Some("BYZ".to_string())));
        assert_eq!(owners.get(&1), Some(&Some("SWE".to_string())));
    }

    #[test]
    fn test_timeline_seek_forward() {
        let log = create_test_log();
        let mut timeline = Timeline::from_file(log.path()).expect("Failed to load timeline");

        // Seek to after Constantinople falls (tick 100)
        timeline.seek_to(100);

        let owners = timeline.current_owners();
        assert_eq!(
            owners.get(&151),
            Some(&Some("OTT".to_string())),
            "Constantinople should be Ottoman at tick 100"
        );
        assert_eq!(
            owners.get(&1),
            Some(&Some("SWE".to_string())),
            "Stockholm should still be Swedish"
        );
    }

    #[test]
    fn test_timeline_seek_backward() {
        let log = create_test_log();
        let mut timeline = Timeline::from_file(log.path()).expect("Failed to load timeline");

        // First seek forward
        timeline.seek_to(200);
        assert_eq!(
            timeline.current_owners().get(&1),
            Some(&Some("DAN".to_string())),
            "Stockholm should be Danish at tick 200"
        );

        // Then seek back
        timeline.seek_to(50);
        assert_eq!(
            timeline.current_owners().get(&151),
            Some(&Some("BYZ".to_string())),
            "Constantinople should be Byzantine at tick 50"
        );
    }

    #[test]
    fn test_current_date() {
        let log = create_test_log();
        let mut timeline = Timeline::from_file(log.path()).expect("Failed to load timeline");

        timeline.seek_to(0);
        assert_eq!(timeline.current_date(), Some("1444.11.11".to_string()));

        timeline.seek_to(100);
        assert_eq!(timeline.current_date(), Some("1453.5.29".to_string()));

        timeline.seek_to(200);
        assert_eq!(timeline.current_date(), Some("1500.1.1".to_string()));
    }

    #[test]
    fn test_timeline_current_tick() {
        let log = create_test_log();
        let mut timeline = Timeline::from_file(log.path()).expect("Failed to load timeline");

        assert_eq!(timeline.current_tick(), 0);

        timeline.seek_to(150);
        assert_eq!(timeline.current_tick(), 150);
    }

    #[test]
    fn test_tick_to_date() {
        // Tick 0 = 1444.11.11 (start date)
        assert_eq!(Timeline::tick_to_date(0), "1444.11.11");

        // Tick 20 = 1444.12.1 (11 + 20 = 31, rollover to Dec 1)
        assert_eq!(Timeline::tick_to_date(20), "1444.12.1");

        // Tick 50 = 1444.12.31 (20 days left in Nov + 30 days Dec = 50)
        // Actually: Nov has 19 days left (30-11=19), so 50-19=31 days into Dec
        // Dec 31 is tick 19+31=50 from start, but Dec only has 31 days
        // Let me recalculate: tick 0 = Nov 11
        // Nov: 30 - 11 = 19 remaining days (ticks 1-19 = Nov 12-30)
        // Dec: 31 days (ticks 20-50 = Dec 1-31)
        // tick 50 = Dec 31
        assert_eq!(Timeline::tick_to_date(50), "1444.12.31");

        // New year: tick 51 = 1445.1.1
        assert_eq!(Timeline::tick_to_date(51), "1445.1.1");

        // Leap year test: Feb 29 in 1448 (divisible by 4)
        // Days from 1444.11.11 to 1448.2.29:
        // 1444: Nov 19 + Dec 31 = 50 days
        // 1445: 365 days
        // 1446: 365 days
        // 1447: 365 days
        // 1448: Jan 31 + Feb 29 = 60 days
        // Total: 50 + 365*3 + 60 = 50 + 1095 + 60 = 1205
        assert_eq!(Timeline::tick_to_date(1205), "1448.2.29");
    }
}
