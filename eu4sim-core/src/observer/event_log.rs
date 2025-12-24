//! Event log observer for recording simulation events as JSONL.
//!
//! Detects notable events by comparing state between ticks and outputs
//! structured JSON lines to any `Write` destination (stdout, file, pipe).
//!
//! # Current Events
//!
//! - `war_declared` - New war started
//! - `peace_white` - War ended with white peace
//! - `peace_provinces` - War ended with province transfer
//! - `peace_annexation` - War ended with full annexation
//! - `country_eliminated` - Country removed from the game
//! - `province_owner_changed` - Province ownership changed (for timeline reconstruction)
//! - `battle_fought` - Land battle resolved with casualties
//! - `siege_completed` - Fort siege completed, control changed
//!
//! # Future Extensions
//!
//! The system is designed for easy extension. Future events may include:
//! - `province_occupied` - Army occupies enemy province
//! - `religion_converted` - Province religion changes
//! - `colony_established` - Colony reaches completion
//! - `stability_changed` - Country stability shifts
//! - `alliance_formed` / `alliance_broken`
//!
//! Each event requires: new enum variant, extended `EventLogState`, detection logic.

use super::{ObserverConfig, ObserverError, SimObserver, Snapshot};
use crate::state::{Battle, BattleId, BattleResult, ProvinceId, Siege, Tag, War, WarId};
use serde::{Deserialize, Serialize};
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    /// Province ownership changed (for timeline reconstruction).
    ProvinceOwnerChanged {
        tick: u64,
        date: String,
        province_id: ProvinceId,
        /// Previous owner (None if uncolonized)
        #[serde(skip_serializing_if = "Option::is_none")]
        old_owner: Option<Tag>,
        /// New owner (None if province became uncolonized)
        #[serde(skip_serializing_if = "Option::is_none")]
        new_owner: Option<Tag>,
    },

    /// A land battle was fought and resolved.
    BattleFought {
        tick: u64,
        date: String,
        battle_id: BattleId,
        province_id: ProvinceId,
        /// Winning side's tag (first army's owner)
        winner: Tag,
        /// Losing side's tag (first army's owner)
        loser: Tag,
        /// Attacker casualties
        attacker_casualties: u32,
        /// Defender casualties
        defender_casualties: u32,
        /// Whether the loser was stackwiped
        stackwiped: bool,
    },

    /// A siege completed (province control changed).
    SiegeCompleted {
        tick: u64,
        date: String,
        province_id: ProvinceId,
        /// Country that won the siege
        besieger: Tag,
        /// Country that lost control
        defender: Tag,
        /// Fort level that was sieged
        fort_level: u8,
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

/// Snapshot of a battle for event detection.
#[derive(Debug, Clone)]
struct BattleSnapshot {
    province: ProvinceId,
    attacker_owner: Tag,
    defender_owner: Tag,
    attacker_casualties: u32,
    defender_casualties: u32,
    result: Option<BattleResult>,
}

impl BattleSnapshot {
    fn from_battle(battle: &Battle, state: &crate::state::WorldState) -> Self {
        // Get owner from first attacking/defending army
        let attacker_owner = battle
            .attackers
            .first()
            .and_then(|id| state.armies.get(id))
            .map(|a| a.owner.clone())
            .unwrap_or_default();
        let defender_owner = battle
            .defenders
            .first()
            .and_then(|id| state.armies.get(id))
            .map(|a| a.owner.clone())
            .unwrap_or_default();

        Self {
            province: battle.province,
            attacker_owner,
            defender_owner,
            attacker_casualties: battle.attacker_casualties,
            defender_casualties: battle.defender_casualties,
            result: battle.result.clone(),
        }
    }
}

/// Snapshot of a siege for event detection.
/// Note: province_id is stored as the HashMap key, not in the struct.
#[derive(Debug, Clone)]
struct SiegeSnapshot {
    attacker: Tag,
    fort_level: u8,
    defender: Tag,
}

impl SiegeSnapshot {
    fn from_siege(siege: &Siege, _state: &crate::state::WorldState) -> Self {
        Self {
            attacker: siege.attacker.clone(),
            fort_level: siege.fort_level,
            defender: siege.defender.clone(),
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
    /// Battles in progress during the previous tick
    prev_battles: HashMap<BattleId, BattleSnapshot>,
    /// Sieges in progress during the previous tick
    prev_sieges: HashMap<ProvinceId, SiegeSnapshot>,
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
        self.prev_battles = state
            .battles
            .iter()
            .map(|(&id, battle)| (id, BattleSnapshot::from_battle(battle, state)))
            .collect();
        self.prev_sieges = state
            .sieges
            .iter()
            .map(|(&prov_id, siege)| (prov_id, SiegeSnapshot::from_siege(siege, state)))
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
    /// Uses buffered I/O to reduce syscall overhead during high-frequency logging.
    pub fn stdout() -> Self {
        Self::new(Box::new(BufWriter::new(std::io::stdout())))
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

        // 4. Detect province ownership changes
        for (prov_id, current_prov) in world.provinces.iter() {
            if let Some(prev_owner) = prev.prev_province_owners.get(prov_id) {
                if prev_owner != &current_prov.owner {
                    events.push(GameEvent::ProvinceOwnerChanged {
                        tick: snapshot.tick,
                        date: world.date.to_string(),
                        province_id: *prov_id,
                        old_owner: prev_owner.clone(),
                        new_owner: current_prov.owner.clone(),
                    });
                }
            }
        }

        // 5. Detect completed battles
        // A battle is complete when it existed in prev but is gone now (was removed after resolving)
        for (battle_id, prev_battle) in &prev.prev_battles {
            if !world.battles.contains_key(battle_id) {
                // Battle was resolved and removed
                if let Some(result) = &prev_battle.result {
                    let (winner, loser, stackwiped) = match result {
                        BattleResult::AttackerVictory { stackwiped, .. } => (
                            prev_battle.attacker_owner.clone(),
                            prev_battle.defender_owner.clone(),
                            *stackwiped,
                        ),
                        BattleResult::DefenderVictory { stackwiped, .. } => (
                            prev_battle.defender_owner.clone(),
                            prev_battle.attacker_owner.clone(),
                            *stackwiped,
                        ),
                        BattleResult::Draw => {
                            // For draws, just use attacker as "winner" for logging
                            (
                                prev_battle.attacker_owner.clone(),
                                prev_battle.defender_owner.clone(),
                                false,
                            )
                        }
                    };

                    events.push(GameEvent::BattleFought {
                        tick: snapshot.tick,
                        date: world.date.to_string(),
                        battle_id: *battle_id,
                        province_id: prev_battle.province,
                        winner,
                        loser,
                        attacker_casualties: prev_battle.attacker_casualties,
                        defender_casualties: prev_battle.defender_casualties,
                        stackwiped,
                    });
                }
            }
        }

        // 6. Detect completed sieges
        // A siege is complete when it existed in prev but is gone now AND controller changed
        for (prov_id, prev_siege) in &prev.prev_sieges {
            if !world.sieges.contains_key(prov_id) {
                // Siege was removed - check if controller changed
                if let Some(current_prov) = world.provinces.get(prov_id) {
                    if current_prov.controller.as_ref() == Some(&prev_siege.attacker) {
                        // Controller is now the besieger = successful siege
                        events.push(GameEvent::SiegeCompleted {
                            tick: snapshot.tick,
                            date: world.date.to_string(),
                            province_id: *prov_id,
                            besieger: prev_siege.attacker.clone(),
                            defender: prev_siege.defender.clone(),
                            fort_level: prev_siege.fort_level,
                        });
                    }
                }
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

    #[test]
    fn test_battle_fought_event() {
        use crate::state::{Army, BattleLine, CombatPhase, Regiment, RegimentType};
        use crate::Fixed;

        let output = capture_output();
        let writer: Box<dyn Write + Send> = Box::new(OutputCapture(output.clone()));
        let observer = EventLogObserver::new(writer);

        // First tick: battle in progress with result set
        let mut state1 = WorldStateBuilder::new()
            .with_country("FRA")
            .with_country("ENG")
            .build();

        // Add armies
        state1.armies.insert(
            1,
            Army {
                id: 1,
                name: "French Army".to_string(),
                owner: "FRA".to_string(),
                location: 100,
                previous_location: None,
                regiments: vec![Regiment {
                    type_: RegimentType::Infantry,
                    strength: Fixed::from_int(1000),
                    morale: Fixed::from_int(4),
                }],
                movement: None,
                embarked_on: None,
                general: None,
                in_battle: Some(1),
                infantry_count: 1,
                cavalry_count: 0,
                artillery_count: 0,
            },
        );
        state1.armies.insert(
            2,
            Army {
                id: 2,
                name: "English Army".to_string(),
                owner: "ENG".to_string(),
                location: 100,
                previous_location: None,
                regiments: vec![Regiment {
                    type_: RegimentType::Infantry,
                    strength: Fixed::from_int(1000),
                    morale: Fixed::from_int(4),
                }],
                movement: None,
                embarked_on: None,
                general: None,
                in_battle: Some(1),
                infantry_count: 1,
                cavalry_count: 0,
                artillery_count: 0,
            },
        );

        // Add battle with result
        state1.battles.insert(
            1,
            Battle {
                id: 1,
                province: 100,
                attacker_origin: None,
                start_date: state1.date,
                phase_day: 0,
                phase: CombatPhase::Fire,
                attacker_dice: 5,
                defender_dice: 3,
                attackers: vec![1],
                defenders: vec![2],
                attacker_line: BattleLine::default(),
                defender_line: BattleLine::default(),
                attacker_casualties: 500,
                defender_casualties: 800,
                result: Some(BattleResult::AttackerVictory {
                    pursuit_casualties: 100,
                    stackwiped: false,
                }),
            },
        );

        let snapshot1 = Snapshot::new(state1, 0, 0);
        observer.on_tick(&snapshot1).unwrap();

        // Second tick: battle cleaned up (removed)
        let mut state2 = WorldStateBuilder::new()
            .with_country("FRA")
            .with_country("ENG")
            .build();
        state2.armies = state2.armies.clone(); // Keep empty armies map
        let snapshot2 = Snapshot::new(state2, 1, 0);
        observer.on_tick(&snapshot2).unwrap();

        // Check output
        let output_data = output.lock().unwrap();
        let output_str = String::from_utf8_lossy(output_data.get_ref());
        assert!(
            output_str.contains("\"type\":\"battle_fought\""),
            "Expected battle_fought event in output: {}",
            output_str
        );
        assert!(output_str.contains("\"winner\":\"FRA\""));
        assert!(output_str.contains("\"loser\":\"ENG\""));
        assert!(output_str.contains("\"attacker_casualties\":500"));
        assert!(output_str.contains("\"defender_casualties\":800"));
    }

    #[test]
    fn test_siege_completed_event() {
        let output = capture_output();
        let writer: Box<dyn Write + Send> = Box::new(OutputCapture(output.clone()));
        let observer = EventLogObserver::new(writer);

        // First tick: siege in progress, controller is defender
        let mut state1 = WorldStateBuilder::new()
            .with_country("FRA")
            .with_country("ENG")
            .with_province(100, Some("ENG"))
            .build();

        // Set controller to ENG (defender)
        if let Some(prov) = state1.provinces.get_mut(&100) {
            prov.controller = Some("ENG".to_string());
            prov.fort_level = 2;
        }

        // Add siege
        state1.sieges.insert(
            100,
            Siege {
                id: 1,
                province: 100,
                attacker: "FRA".to_string(),
                defender: "ENG".to_string(),
                besieging_armies: vec![1],
                fort_level: 2,
                garrison: 1000,
                progress_modifier: 5,
                days_in_phase: 20,
                start_date: state1.date,
                is_blockaded: false,
                breached: false,
            },
        );

        let snapshot1 = Snapshot::new(state1, 0, 0);
        observer.on_tick(&snapshot1).unwrap();

        // Second tick: siege completed, controller changed to attacker, siege removed
        let mut state2 = WorldStateBuilder::new()
            .with_country("FRA")
            .with_country("ENG")
            .with_province(100, Some("ENG"))
            .build();

        // Controller is now FRA (besieger won)
        if let Some(prov) = state2.provinces.get_mut(&100) {
            prov.controller = Some("FRA".to_string());
        }
        // Siege removed (not in state2.sieges)

        let snapshot2 = Snapshot::new(state2, 1, 0);
        observer.on_tick(&snapshot2).unwrap();

        // Check output
        let output_data = output.lock().unwrap();
        let output_str = String::from_utf8_lossy(output_data.get_ref());
        assert!(
            output_str.contains("\"type\":\"siege_completed\""),
            "Expected siege_completed event in output: {}",
            output_str
        );
        assert!(output_str.contains("\"besieger\":\"FRA\""));
        assert!(output_str.contains("\"defender\":\"ENG\""));
        assert!(output_str.contains("\"fort_level\":2"));
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
