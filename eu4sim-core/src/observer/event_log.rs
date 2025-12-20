//! Event log observer for recording simulation events as JSONL.
//!
//! Detects notable events by comparing state between ticks and outputs
//! structured JSON lines to any `Write` destination (stdout, file, pipe).
//!
//! # Current Events (War Only)
//!
//! - `war_declared` - New war started
//! - `peace_white` - War ended with white peace
//! - `peace_provinces` - War ended with province transfer
//! - `peace_annexation` - War ended with full annexation
//! - `country_eliminated` - Country removed from the game
//!
//! # Future Extensions
//!
//! The system is designed for easy extension. Future events may include:
//! - `province_occupied` - Army occupies enemy province
//! - `religion_converted` - Province religion changes
//! - `colony_established` - Colony reaches completion
//! - `stability_changed` - Country stability shifts
//! - `alliance_formed` / `alliance_broken`
//! - `battle_fought` - Combat result summary
//!
//! Each event requires: new enum variant, extended `EventLogState`, detection logic.

use super::{ObserverConfig, ObserverError, SimObserver, Snapshot};
use crate::state::{ProvinceId, Tag, War, WarId};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Mutex;

/// Events detected by comparing state between ticks.
///
/// Uses serde's tag format for clean JSONL output:
/// ```json
/// {"type":"war_declared","tick":365,"date":"1445.11.11",...}
/// ```
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GameEvent {
    /// A new war has been declared.
    WarDeclared {
        tick: u64,
        date: String,
        /// Primary attacker (war leader)
        attacker: Tag,
        /// Primary defender (war leader)
        defender: Tag,
        war_name: String,
        war_id: WarId,
    },

    /// War ended in white peace (no territorial changes).
    PeaceWhite {
        tick: u64,
        date: String,
        war_id: WarId,
        war_name: String,
        /// Final attacker war score
        attacker_score: u8,
        /// Final defender war score
        defender_score: u8,
    },

    /// War ended with province transfer.
    PeaceProvinces {
        tick: u64,
        date: String,
        war_id: WarId,
        war_name: String,
        /// Provinces that changed hands
        provinces: Vec<ProvinceId>,
        /// Previous owner
        from_tag: Tag,
        /// New owner
        to_tag: Tag,
    },

    /// War ended with full annexation of a country.
    PeaceAnnexation {
        tick: u64,
        date: String,
        war_id: WarId,
        war_name: String,
        /// Country that was annexed
        annexed_tag: Tag,
        /// Country that did the annexing
        annexer_tag: Tag,
    },

    /// A country has been eliminated from the game.
    CountryEliminated {
        tick: u64,
        date: String,
        /// The eliminated country
        tag: Tag,
        /// Who eliminated them (if known from recent war)
        #[serde(skip_serializing_if = "Option::is_none")]
        eliminator: Option<Tag>,
    },
}

/// Minimal snapshot of war state for comparison.
#[derive(Debug, Clone)]
struct WarSnapshot {
    name: String,
    attackers: Vec<Tag>,
    defenders: Vec<Tag>,
    attacker_score: u8,
    defender_score: u8,
}

impl From<&War> for WarSnapshot {
    fn from(war: &War) -> Self {
        Self {
            name: war.name.clone(),
            attackers: war.attackers.clone(),
            defenders: war.defenders.clone(),
            attacker_score: war.attacker_score,
            defender_score: war.defender_score,
        }
    }
}

/// Cached state for detecting events between ticks.
///
/// Stores minimal data needed for comparison, not full WorldState clones.
#[derive(Debug, Default)]
struct EventLogState {
    /// War IDs that existed in the previous tick
    prev_war_ids: HashSet<WarId>,
    /// Snapshot of each war's state (for peace deal detection)
    prev_wars: HashMap<WarId, WarSnapshot>,
    /// Country tags that existed in the previous tick
    prev_country_tags: HashSet<Tag>,
    /// Province owners in the previous tick (for detecting annexation transfers)
    prev_province_owners: HashMap<ProvinceId, Option<Tag>>,
    /// Whether this is the first tick (skip event detection)
    first_tick: bool,
}

impl EventLogState {
    fn new() -> Self {
        Self {
            first_tick: true,
            ..Default::default()
        }
    }

    /// Update cached state from current world state.
    fn update_from(&mut self, state: &crate::state::WorldState) {
        self.prev_war_ids = state.diplomacy.wars.keys().copied().collect();
        self.prev_wars = state
            .diplomacy
            .wars
            .iter()
            .map(|(id, war)| (*id, WarSnapshot::from(war)))
            .collect();
        self.prev_country_tags = state.countries.keys().cloned().collect();
        self.prev_province_owners = state
            .provinces
            .iter()
            .map(|(id, prov)| (*id, prov.owner.clone()))
            .collect();
        self.first_tick = false;
    }
}

/// Observer that logs simulation events as JSONL.
///
/// Detects events by comparing previous state to current state each tick.
/// Output is written to any `Write` destination (stdout, file, network pipe).
///
/// # Example
///
/// ```ignore
/// // Log to stdout (for piping to jq, etc.)
/// let observer = EventLogObserver::stdout();
///
/// // Log to file
/// let observer = EventLogObserver::file("events.jsonl")?;
///
/// // Custom writer (compression, network, etc.)
/// let pipe = Command::new("gzip").stdin(Stdio::piped()).spawn()?;
/// let observer = EventLogObserver::new(Box::new(pipe.stdin.unwrap()));
/// ```
pub struct EventLogObserver {
    /// Destination for JSONL output
    writer: Mutex<Box<dyn Write + Send>>,
    /// Cached state for event detection
    state: Mutex<EventLogState>,
    /// Observer configuration
    config: ObserverConfig,
}

impl EventLogObserver {
    /// Create observer writing to stdout.
    ///
    /// Useful for piping to tools like `jq` or other stream processors.
    pub fn stdout() -> Self {
        Self::new(Box::new(std::io::stdout()))
    }

    /// Create observer writing to a file.
    ///
    /// Uses buffered I/O for performance.
    pub fn file(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let file = std::fs::File::create(path)?;
        let buffered = BufWriter::new(file);
        Ok(Self::new(Box::new(buffered)))
    }

    /// Create observer with a custom writer.
    ///
    /// Accepts any `Write + Send` implementor (pipes, network sockets, etc.).
    pub fn new(writer: Box<dyn Write + Send>) -> Self {
        Self {
            writer: Mutex::new(writer),
            state: Mutex::new(EventLogState::new()),
            config: ObserverConfig {
                frequency: 1, // Check every tick for events
                notify_on_month_start: true,
            },
        }
    }

    /// Detect all events by comparing previous state to current.
    fn detect_events(&self, snapshot: &Snapshot, prev: &EventLogState) -> Vec<GameEvent> {
        let world = &snapshot.state;
        let mut events = Vec::new();

        // 1. Detect new wars
        for (war_id, war) in world.diplomacy.wars.iter() {
            if !prev.prev_war_ids.contains(war_id) {
                events.push(GameEvent::WarDeclared {
                    tick: snapshot.tick,
                    date: world.date.to_string(),
                    attacker: war.attackers.first().cloned().unwrap_or_default(),
                    defender: war.defenders.first().cloned().unwrap_or_default(),
                    war_name: war.name.clone(),
                    war_id: *war_id,
                });
            }
        }

        // 2. Detect ended wars (peace deals)
        for (war_id, prev_war) in &prev.prev_wars {
            if !world.diplomacy.wars.contains_key(war_id) {
                let event = self.classify_peace_deal(snapshot, prev, *war_id, prev_war);
                events.push(event);
            }
        }

        // 3. Detect country eliminations
        for tag in &prev.prev_country_tags {
            if !world.countries.contains_key(tag) {
                // Try to find who eliminated them (look for province transfers)
                let eliminator = self.find_eliminator(prev, tag, world);
                events.push(GameEvent::CountryEliminated {
                    tick: snapshot.tick,
                    date: world.date.to_string(),
                    tag: tag.clone(),
                    eliminator,
                });
            }
        }

        events
    }

    /// Classify what kind of peace ended the war.
    fn classify_peace_deal(
        &self,
        snapshot: &Snapshot,
        prev: &EventLogState,
        war_id: WarId,
        prev_war: &WarSnapshot,
    ) -> GameEvent {
        let world = &snapshot.state;

        // Check if any defender country was eliminated (full annexation)
        for defender in &prev_war.defenders {
            if !world.countries.contains_key(defender) {
                // Find the annexer (likely the attacker leader)
                let annexer = prev_war.attackers.first().cloned().unwrap_or_default();
                return GameEvent::PeaceAnnexation {
                    tick: snapshot.tick,
                    date: world.date.to_string(),
                    war_id,
                    war_name: prev_war.name.clone(),
                    annexed_tag: defender.clone(),
                    annexer_tag: annexer,
                };
            }
        }

        // Check if attacker was annexed (rare but possible)
        for attacker in &prev_war.attackers {
            if !world.countries.contains_key(attacker) {
                let annexer = prev_war.defenders.first().cloned().unwrap_or_default();
                return GameEvent::PeaceAnnexation {
                    tick: snapshot.tick,
                    date: world.date.to_string(),
                    war_id,
                    war_name: prev_war.name.clone(),
                    annexed_tag: attacker.clone(),
                    annexer_tag: annexer,
                };
            }
        }

        // Check for province transfers
        let mut transferred_provinces = Vec::new();
        let mut from_tag = String::new();
        let mut to_tag = String::new();

        for (prov_id, current_owner) in world.provinces.iter() {
            if let Some(prev_owner) = prev.prev_province_owners.get(prov_id) {
                if prev_owner != &current_owner.owner {
                    // Province changed hands
                    if let (Some(prev), Some(curr)) = (prev_owner, &current_owner.owner) {
                        // Check if this transfer involves war participants
                        let involves_war = prev_war.attackers.contains(prev)
                            || prev_war.defenders.contains(prev)
                            || prev_war.attackers.contains(curr)
                            || prev_war.defenders.contains(curr);

                        if involves_war {
                            transferred_provinces.push(*prov_id);
                            from_tag = prev.clone();
                            to_tag = curr.clone();
                        }
                    }
                }
            }
        }

        if !transferred_provinces.is_empty() {
            GameEvent::PeaceProvinces {
                tick: snapshot.tick,
                date: world.date.to_string(),
                war_id,
                war_name: prev_war.name.clone(),
                provinces: transferred_provinces,
                from_tag,
                to_tag,
            }
        } else {
            // No annexation, no province transfers -> white peace
            GameEvent::PeaceWhite {
                tick: snapshot.tick,
                date: world.date.to_string(),
                war_id,
                war_name: prev_war.name.clone(),
                attacker_score: prev_war.attacker_score,
                defender_score: prev_war.defender_score,
            }
        }
    }

    /// Try to find who eliminated a country by looking at province ownership.
    fn find_eliminator(
        &self,
        prev: &EventLogState,
        eliminated_tag: &str,
        world: &crate::state::WorldState,
    ) -> Option<Tag> {
        // Look for provinces that used to belong to the eliminated country
        // and now belong to someone else
        let mut new_owners: HashMap<Tag, u32> = HashMap::new();

        for (prov_id, current) in world.provinces.iter() {
            if let Some(Some(prev_owner)) = prev.prev_province_owners.get(prov_id) {
                if prev_owner == eliminated_tag {
                    if let Some(new_owner) = &current.owner {
                        *new_owners.entry(new_owner.clone()).or_default() += 1;
                    }
                }
            }
        }

        // Return the country that took the most provinces
        new_owners
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(tag, _)| tag)
    }

    /// Write an event to the output.
    fn write_event(&self, writer: &mut dyn Write, event: &GameEvent) -> Result<(), ObserverError> {
        serde_json::to_writer(&mut *writer, event)?;
        writeln!(writer)?;
        Ok(())
    }
}

impl SimObserver for EventLogObserver {
    fn on_tick(&self, snapshot: &Snapshot) -> Result<(), ObserverError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ObserverError::Render("EventLogObserver state lock poisoned".into()))?;

        // Skip event detection on first tick (no previous state to compare)
        if !state.first_tick {
            let events = self.detect_events(snapshot, &state);

            if !events.is_empty() {
                let mut writer = self.writer.lock().map_err(|_| {
                    ObserverError::Render("EventLogObserver writer lock poisoned".into())
                })?;

                for event in &events {
                    self.write_event(&mut *writer, event)?;
                }
                writer.flush()?;
            }
        }

        // Update cached state for next tick
        state.update_from(&snapshot.state);

        Ok(())
    }

    fn name(&self) -> &str {
        "EventLogObserver"
    }

    fn config(&self) -> ObserverConfig {
        self.config.clone()
    }

    fn on_shutdown(&self) {
        // Final flush on shutdown
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writer.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{DiplomacyState, War};
    use crate::testing::WorldStateBuilder;
    use std::io::Cursor;
    use std::sync::Arc;

    /// Helper to capture JSONL output.
    fn capture_output() -> Arc<Mutex<Cursor<Vec<u8>>>> {
        Arc::new(Mutex::new(Cursor::new(Vec::new())))
    }

    #[test]
    fn test_war_declared_event() {
        let output = capture_output();
        let writer: Box<dyn Write + Send> = Box::new(OutputCapture(output.clone()));
        let observer = EventLogObserver::new(writer);

        // First tick: no wars
        let state1 = WorldStateBuilder::new()
            .with_country("FRA")
            .with_country("ENG")
            .build();
        let snapshot1 = Snapshot::new(state1, 0, 0);
        observer.on_tick(&snapshot1).unwrap();

        // Second tick: war declared
        let mut state2 = WorldStateBuilder::new()
            .with_country("FRA")
            .with_country("ENG")
            .build();
        state2.diplomacy = DiplomacyState {
            wars: [(
                1,
                War {
                    id: 1,
                    name: "Anglo-French War".to_string(),
                    attackers: vec!["FRA".to_string()],
                    defenders: vec!["ENG".to_string()],
                    start_date: state2.date,
                    attacker_score: 0,
                    attacker_battle_score: 0,
                    defender_score: 0,
                    defender_battle_score: 0,
                    pending_peace: None,
                },
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        };
        let snapshot2 = Snapshot::new(state2, 1, 0);
        observer.on_tick(&snapshot2).unwrap();

        // Check output
        let output_data = output.lock().unwrap();
        let output_str = String::from_utf8_lossy(output_data.get_ref());
        assert!(output_str.contains("\"type\":\"war_declared\""));
        assert!(output_str.contains("\"attacker\":\"FRA\""));
        assert!(output_str.contains("\"defender\":\"ENG\""));
    }

    #[test]
    fn test_white_peace_event() {
        let output = capture_output();
        let writer: Box<dyn Write + Send> = Box::new(OutputCapture(output.clone()));
        let observer = EventLogObserver::new(writer);

        // First tick: war in progress
        let mut state1 = WorldStateBuilder::new()
            .with_country("FRA")
            .with_country("ENG")
            .build();
        state1.diplomacy = DiplomacyState {
            wars: [(
                1,
                War {
                    id: 1,
                    name: "Anglo-French War".to_string(),
                    attackers: vec!["FRA".to_string()],
                    defenders: vec!["ENG".to_string()],
                    start_date: state1.date,
                    attacker_score: 30,
                    attacker_battle_score: 20,
                    defender_score: 25,
                    defender_battle_score: 15,
                    pending_peace: None,
                },
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        };
        let snapshot1 = Snapshot::new(state1, 0, 0);
        observer.on_tick(&snapshot1).unwrap();

        // Second tick: war ended (no war in diplomacy)
        let state2 = WorldStateBuilder::new()
            .with_country("FRA")
            .with_country("ENG")
            .build();
        let snapshot2 = Snapshot::new(state2, 1, 0);
        observer.on_tick(&snapshot2).unwrap();

        // Check output
        let output_data = output.lock().unwrap();
        let output_str = String::from_utf8_lossy(output_data.get_ref());
        assert!(output_str.contains("\"type\":\"peace_white\""));
        assert!(output_str.contains("\"attacker_score\":30"));
    }

    #[test]
    fn test_country_eliminated_event() {
        let output = capture_output();
        let writer: Box<dyn Write + Send> = Box::new(OutputCapture(output.clone()));
        let observer = EventLogObserver::new(writer);

        // First tick: both countries exist
        let state1 = WorldStateBuilder::new()
            .with_country("FRA")
            .with_country("BUR")
            .build();
        let snapshot1 = Snapshot::new(state1, 0, 0);
        observer.on_tick(&snapshot1).unwrap();

        // Second tick: BUR is gone
        let state2 = WorldStateBuilder::new().with_country("FRA").build();
        let snapshot2 = Snapshot::new(state2, 1, 0);
        observer.on_tick(&snapshot2).unwrap();

        // Check output
        let output_data = output.lock().unwrap();
        let output_str = String::from_utf8_lossy(output_data.get_ref());
        assert!(output_str.contains("\"type\":\"country_eliminated\""));
        assert!(output_str.contains("\"tag\":\"BUR\""));
    }

    /// Helper struct to capture output through Arc<Mutex<Cursor>>
    struct OutputCapture(Arc<Mutex<Cursor<Vec<u8>>>>);

    impl Write for OutputCapture {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().write(buf)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.0.lock().unwrap().flush()
        }
    }
}
