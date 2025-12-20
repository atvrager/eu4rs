//! Training data generation observer for ML model training.
//!
//! Generates JSONL training samples containing:
//! - Current visible state
//! - Available commands
//! - Chosen action (index into available commands)
//!
//! # Output Format
//!
//! Each line is a JSON object:
//! ```json
//! {
//!   "tick": 365,
//!   "country": "FRA",
//!   "state": { "date": {...}, "observer": "FRA", ... },
//!   "available_commands": [{"Move": {...}}, {"DeclareWar": {...}}, ...],
//!   "chosen_action": 3,
//!   "chosen_command": {"DeclareWar": {...}}
//! }
//! ```
//!
//! # Usage with Training Pipelines
//!
//! The output can be processed by ML training pipelines:
//! - `state` serializes to a prompt for language models
//! - `available_commands` provides the action space (index selection)
//! - `chosen_action` is the supervision signal (-1 for Pass)
//!
//! # Command Availability
//!
//! This observer computes available commands for each country using
//! `WorldState::available_commands()`. This requires an adjacency graph
//! for movement commands; pass `None` to skip movement-related commands.

use super::{ObserverConfig, ObserverError, SimObserver, Snapshot};
use crate::ai::VisibleWorldState;
use crate::input::{Command, PlayerInputs};
use crate::state::Tag;
use eu4data::adjacency::AdjacencyGraph;
use serde::Serialize;
use std::collections::HashMap;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// A single training sample for ML model training.
#[derive(Debug, Clone, Serialize)]
pub struct TrainingSample {
    /// Current simulation tick
    pub tick: u64,
    /// Country this sample is for
    pub country: Tag,
    /// Visible state for this country (prompt input)
    pub state: VisibleWorldState,
    /// All legal commands at this moment (action space)
    pub available_commands: Vec<Command>,
    /// Index of chosen command in `available_commands`, or -1 for Pass
    pub chosen_action: i32,
    /// The actual command chosen (for debugging/analysis)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chosen_command: Option<Command>,
}

/// Observer that generates training data for ML models.
///
/// Outputs JSONL with one training sample per line, containing:
/// - Visible state (prompt input)
/// - Available commands (action space)
/// - Chosen action (supervision signal)
///
/// # Example
///
/// ```ignore
/// // Generate training data to file
/// let adjacency = Arc::new(load_adjacency_graph());
/// let observer = DataGenObserver::file("training.jsonl", Some(adjacency))?;
///
/// // Or to stdout for piping
/// let observer = DataGenObserver::stdout(Some(adjacency));
/// ```
pub struct DataGenObserver {
    /// Destination for JSONL output
    writer: Mutex<Box<dyn Write + Send>>,
    /// Countries to generate data for (empty = all AI countries)
    tracked_countries: Vec<Tag>,
    /// Adjacency graph for command availability calculation
    adjacency: Option<Arc<AdjacencyGraph>>,
    /// Observer configuration
    config: ObserverConfig,
}

impl DataGenObserver {
    /// Create observer writing to stdout.
    pub fn stdout(adjacency: Option<Arc<AdjacencyGraph>>) -> Self {
        Self::new(Box::new(std::io::stdout()), adjacency)
    }

    /// Create observer writing to a file.
    pub fn file(
        path: impl AsRef<Path>,
        adjacency: Option<Arc<AdjacencyGraph>>,
    ) -> std::io::Result<Self> {
        let file = std::fs::File::create(path)?;
        let buffered = BufWriter::new(file);
        Ok(Self::new(Box::new(buffered), adjacency))
    }

    /// Create observer with a custom writer.
    pub fn new(writer: Box<dyn Write + Send>, adjacency: Option<Arc<AdjacencyGraph>>) -> Self {
        Self {
            writer: Mutex::new(writer),
            tracked_countries: vec![],
            adjacency,
            config: ObserverConfig {
                frequency: 1, // Every tick
                notify_on_month_start: true,
            },
        }
    }

    /// Filter to specific countries (empty = track all).
    pub fn with_countries(mut self, countries: &[&str]) -> Self {
        self.tracked_countries = countries.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Generate training samples for all tracked countries.
    fn generate_samples(
        &self,
        snapshot: &Snapshot,
        inputs: &[PlayerInputs],
    ) -> Vec<TrainingSample> {
        let world = &snapshot.state;

        // Build lookup of inputs by country
        let inputs_by_country: HashMap<&str, &[Command]> = inputs
            .iter()
            .map(|pi| (pi.country.as_str(), pi.commands.as_slice()))
            .collect();

        // Determine which countries to track
        let countries_to_track: Vec<&str> = if self.tracked_countries.is_empty() {
            world.countries.keys().map(|s| s.as_str()).collect()
        } else {
            self.tracked_countries.iter().map(|s| s.as_str()).collect()
        };

        let mut samples = Vec::with_capacity(countries_to_track.len());

        for tag in countries_to_track {
            let Some(country) = world.countries.get(tag) else {
                continue;
            };

            // Check if this country is at war
            let at_war = world.diplomacy.wars.values().any(|war| {
                war.attackers.iter().any(|t| t == tag) || war.defenders.iter().any(|t| t == tag)
            });

            // Build visible state
            let visible_state = VisibleWorldState {
                date: world.date,
                observer: tag.to_string(),
                own_country: country.clone(),
                at_war,
                known_countries: vec![], // Could populate with neighbors/contacts
            };

            // Calculate available commands
            let available =
                world.available_commands(tag, self.adjacency.as_ref().map(|a| a.as_ref()));

            // Find chosen action
            let (chosen_action, chosen_command) = if let Some(commands) = inputs_by_country.get(tag)
            {
                if let Some(first_cmd) = commands.first() {
                    // Find index in available commands
                    let idx = available.iter().position(|c| c == first_cmd);
                    match idx {
                        Some(i) => (i as i32, Some(first_cmd.clone())),
                        None => (-2, Some(first_cmd.clone())), // Command executed but not in available list
                    }
                } else {
                    (-1, None) // Pass (empty command list)
                }
            } else {
                (-1, None) // No input for this country (Pass)
            };

            samples.push(TrainingSample {
                tick: snapshot.tick,
                country: tag.to_string(),
                state: visible_state,
                available_commands: available,
                chosen_action,
                chosen_command,
            });
        }

        samples
    }
}

impl SimObserver for DataGenObserver {
    fn on_tick(&self, snapshot: &Snapshot) -> Result<(), ObserverError> {
        // Should not be called directly when needs_inputs() returns true
        self.on_tick_with_inputs(snapshot, &[])
    }

    fn on_tick_with_inputs(
        &self,
        snapshot: &Snapshot,
        inputs: &[PlayerInputs],
    ) -> Result<(), ObserverError> {
        let samples = self.generate_samples(snapshot, inputs);

        if !samples.is_empty() {
            let mut writer = self.writer.lock().map_err(|_| {
                ObserverError::Render("DataGenObserver writer lock poisoned".into())
            })?;

            for sample in &samples {
                serde_json::to_writer(&mut *writer, sample)?;
                writeln!(&mut *writer)?;
            }
            writer.flush()?;
        }

        Ok(())
    }

    fn needs_inputs(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "DataGenObserver"
    }

    fn config(&self) -> ObserverConfig {
        self.config.clone()
    }

    fn on_shutdown(&self) {
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writer.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::WorldStateBuilder;
    use std::io::Cursor;

    /// Helper to capture JSONL output.
    fn capture_output() -> Arc<Mutex<Cursor<Vec<u8>>>> {
        Arc::new(Mutex::new(Cursor::new(Vec::new())))
    }

    /// Wrapper to make Arc<Mutex<Cursor>> implement Write
    struct OutputCapture(Arc<Mutex<Cursor<Vec<u8>>>>);

    impl Write for OutputCapture {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().write(buf)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.0.lock().unwrap().flush()
        }
    }

    #[test]
    fn test_datagen_sample_generation() {
        let output = capture_output();
        let writer: Box<dyn Write + Send> = Box::new(OutputCapture(output.clone()));
        let observer = DataGenObserver::new(writer, None);

        // Create state with one country
        let state = WorldStateBuilder::new().with_country("SWE").build();
        let snapshot = Snapshot::new(state, 0, 0);

        // Simulate AI choosing a command
        let inputs = vec![PlayerInputs {
            country: "SWE".to_string(),
            commands: vec![Command::Pass],
        }];

        observer.on_tick_with_inputs(&snapshot, &inputs).unwrap();

        // Check output
        let output_data = output.lock().unwrap();
        let output_str = String::from_utf8_lossy(output_data.get_ref());
        assert!(output_str.contains("\"country\":\"SWE\""));
        assert!(output_str.contains("\"tick\":0"));
    }

    #[test]
    fn test_datagen_pass_action() {
        let output = capture_output();
        let writer: Box<dyn Write + Send> = Box::new(OutputCapture(output.clone()));
        let observer = DataGenObserver::new(writer, None);

        let state = WorldStateBuilder::new().with_country("FRA").build();
        let snapshot = Snapshot::new(state, 0, 0);

        // No inputs means Pass
        let inputs: Vec<PlayerInputs> = vec![];

        observer.on_tick_with_inputs(&snapshot, &inputs).unwrap();

        // Check output indicates pass (-1)
        let output_data = output.lock().unwrap();
        let output_str = String::from_utf8_lossy(output_data.get_ref());
        assert!(output_str.contains("\"chosen_action\":-1"));
    }

    #[test]
    fn test_datagen_needs_inputs() {
        let observer = DataGenObserver::stdout(None);
        assert!(observer.needs_inputs());
    }

    #[test]
    fn test_datagen_with_country_filter() {
        let output = capture_output();
        let writer: Box<dyn Write + Send> = Box::new(OutputCapture(output.clone()));
        let observer = DataGenObserver::new(writer, None).with_countries(&["SWE"]);

        // State with multiple countries
        let state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_country("FRA")
            .with_country("ENG")
            .build();
        let snapshot = Snapshot::new(state, 0, 0);

        observer.on_tick_with_inputs(&snapshot, &[]).unwrap();

        // Should only have SWE in output
        let output_data = output.lock().unwrap();
        let output_str = String::from_utf8_lossy(output_data.get_ref());
        assert!(output_str.contains("\"country\":\"SWE\""));
        assert!(!output_str.contains("\"country\":\"FRA\""));
        assert!(!output_str.contains("\"country\":\"ENG\""));
    }
}
