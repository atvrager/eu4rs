//! Training data generation observer for ML model training.
//!
//! Generates JSONL training samples containing:
//! - Current visible state
//! - Available commands
//! - Chosen action (index into available commands)
//!
//! # Output Format
//!
//! ## JSONL Mode (`.jsonl` extension)
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
//! ## Archive Mode (`.zip` extension)
//! Creates a zip archive with deflate-compressed JSONL files per year:
//! ```text
//! datagen.zip/
//!   1444.jsonl  (deflate compressed)
//!   1445.jsonl
//!   ...
//! ```
//!
//! Archive mode uses a background writer thread for non-blocking I/O.
//! Serialization and compression happen off the main simulation thread.
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
use serde::Serialize;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::mpsc::{self, Sender};
// NOTE: Using std::sync::mpsc for simplicity. If the background writer thread
// becomes a bottleneck (slower than producer), consider switching to crossbeam-channel
// or std::sync::mpsc::sync_channel for bounded backpressure.
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

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

/// Message sent to the background writer thread
enum WriterMessage {
    /// Batch of samples for a given year
    Samples {
        year: i32,
        samples: Vec<TrainingSample>,
    },
    /// Shutdown signal - flush and finalize
    Shutdown,
}

/// Output mode for datagen observer
enum OutputMode {
    /// Streaming JSONL to a writer (synchronous, for stdout/simple files)
    Stream(Box<dyn Write + Send>),
    /// Async archive mode with background writer thread
    AsyncArchive {
        sender: Sender<WriterMessage>,
        /// Handle to join the writer thread on shutdown
        handle: Option<JoinHandle<()>>,
    },
}

/// Background writer thread state
struct ArchiveWriter {
    zip: ZipWriter<std::fs::File>,
    current_year: Option<i32>,
    year_buffer: Vec<u8>,
}

impl ArchiveWriter {
    fn new(file: std::fs::File) -> Self {
        Self {
            zip: ZipWriter::new(file),
            current_year: None,
            year_buffer: Vec::with_capacity(8 * 1024 * 1024), // 8MB
        }
    }

    /// Process incoming messages until shutdown
    fn run(mut self, receiver: mpsc::Receiver<WriterMessage>) {
        while let Ok(msg) = receiver.recv() {
            match msg {
                WriterMessage::Samples { year, samples } => {
                    if let Err(e) = self.handle_samples(year, samples) {
                        log::error!("ArchiveWriter error: {}", e);
                    }
                }
                WriterMessage::Shutdown => {
                    if let Err(e) = self.finalize() {
                        log::error!("ArchiveWriter finalize error: {}", e);
                    }
                    break;
                }
            }
        }
    }

    fn handle_samples(&mut self, year: i32, samples: Vec<TrainingSample>) -> Result<(), String> {
        // Year transition - flush previous year
        if let Some(prev_year) = self.current_year {
            if year != prev_year {
                self.flush_year_buffer(prev_year)?;
                self.year_buffer.clear();
            }
        }
        self.current_year = Some(year);

        // Serialize samples to buffer
        for sample in &samples {
            serde_json::to_writer(&mut self.year_buffer, sample)
                .map_err(|e| format!("JSON serialization error: {}", e))?;
            self.year_buffer.push(b'\n');
        }

        Ok(())
    }

    fn flush_year_buffer(&mut self, year: i32) -> Result<(), String> {
        if self.year_buffer.is_empty() {
            return Ok(());
        }

        let filename = format!("{}.jsonl", year);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(6));

        self.zip
            .start_file(&filename, options)
            .map_err(|e| format!("Failed to start zip file: {}", e))?;
        self.zip
            .write_all(&self.year_buffer)
            .map_err(|e| format!("Failed to write to zip: {}", e))?;

        log::debug!(
            "Wrote {}.jsonl to archive ({} bytes uncompressed)",
            year,
            self.year_buffer.len()
        );

        Ok(())
    }

    fn finalize(mut self) -> Result<(), String> {
        // Flush any remaining year data
        if let Some(year) = self.current_year {
            if !self.year_buffer.is_empty() {
                self.flush_year_buffer(year)?;
            }
        }

        // Finalize the archive
        self.zip
            .finish()
            .map_err(|e| format!("Failed to finalize zip: {}", e))?;

        log::info!("Archive finalized successfully");
        Ok(())
    }
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
/// // Generate training data to file (streaming)
/// let observer = DataGenObserver::file("training.jsonl")?;
///
/// // Generate to compressed archive (recommended for large runs)
/// let observer = DataGenObserver::file("training.zip")?;
///
/// // Or to stdout for piping
/// let observer = DataGenObserver::stdout();
/// ```
pub struct DataGenObserver {
    /// Output destination (stream or async archive)
    output: Mutex<OutputMode>,
    /// Countries to generate data for (empty = all AI countries)
    tracked_countries: Vec<Tag>,
    /// Observer configuration
    config: ObserverConfig,
}

impl DataGenObserver {
    /// Create observer writing to stdout.
    pub fn stdout() -> Self {
        Self::new_stream(Box::new(std::io::stdout()))
    }

    /// Create observer writing to a file.
    ///
    /// If the path ends with `.zip`, uses async archive mode with a background
    /// writer thread for non-blocking I/O. Otherwise, uses streaming JSONL mode.
    pub fn file(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref();

        if path.extension().map(|e| e == "zip").unwrap_or(false) {
            // Async archive mode: spawn background writer thread
            let file = std::fs::File::create(path)?;
            let (sender, receiver) = mpsc::channel();

            let writer = ArchiveWriter::new(file);
            let handle = std::thread::Builder::new()
                .name("datagen-writer".into())
                .spawn(move || writer.run(receiver))
                .expect("Failed to spawn datagen writer thread");

            Ok(Self {
                output: Mutex::new(OutputMode::AsyncArchive {
                    sender,
                    handle: Some(handle),
                }),
                tracked_countries: vec![],
                config: ObserverConfig {
                    frequency: 1,
                    notify_on_month_start: true,
                },
            })
        } else {
            // Streaming mode: buffered JSONL
            let file = std::fs::File::create(path)?;
            // 8MB buffer - each tick generates ~2.5KB × 600 countries = ~1.5MB
            let buffered = BufWriter::with_capacity(8 * 1024 * 1024, file);
            Ok(Self::new_stream(Box::new(buffered)))
        }
    }

    /// Create observer with a custom writer (streaming mode).
    fn new_stream(writer: Box<dyn Write + Send>) -> Self {
        Self {
            output: Mutex::new(OutputMode::Stream(writer)),
            tracked_countries: vec![],
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

    /// Generate training samples from precomputed PlayerInputs.
    ///
    /// Uses the available_commands from PlayerInputs (computed in AI loop)
    /// instead of recomputing them, which is much faster.
    fn generate_samples(
        &self,
        snapshot: &Snapshot,
        inputs: &[PlayerInputs],
    ) -> Vec<TrainingSample> {
        let world = &snapshot.state;

        // Filter to tracked countries if specified
        let tracked_set: Option<std::collections::HashSet<&str>> =
            if self.tracked_countries.is_empty() {
                None // Track all
            } else {
                Some(self.tracked_countries.iter().map(|s| s.as_str()).collect())
            };

        // Pre-compute country strength once (O(armies), shared across all samples via Arc)
        let known_country_strength: Arc<std::collections::HashMap<String, u32>> =
            Arc::new(world.armies.values().fold(
                std::collections::HashMap::new(),
                |mut acc, army| {
                    *acc.entry(army.owner.clone()).or_default() += army.regiments.len() as u32;
                    acc
                },
            ));

        let mut samples = Vec::with_capacity(inputs.len());

        for input in inputs {
            let tag = input.country.as_str();

            // Skip if not in tracked set
            if let Some(ref tracked) = tracked_set {
                if !tracked.contains(tag) {
                    continue;
                }
            }

            let Some(country) = world.countries.get(tag) else {
                continue;
            };

            // Check if this country is at war
            let at_war = world.diplomacy.wars.values().any(|war| {
                war.attackers.iter().any(|t| t == tag) || war.defenders.iter().any(|t| t == tag)
            });

            // Calculate war scores for this observer
            let mut our_war_score = std::collections::HashMap::new();
            let mut enemy_provinces = std::collections::HashSet::new();
            for war in world.diplomacy.wars.values() {
                let is_attacker = war.attackers.iter().any(|t| t == tag);
                let is_defender = war.defenders.iter().any(|t| t == tag);

                if is_attacker || is_defender {
                    let score = if is_attacker {
                        crate::fixed::Fixed::from_int(war.attacker_score as i64)
                            - crate::fixed::Fixed::from_int(war.defender_score as i64)
                    } else {
                        crate::fixed::Fixed::from_int(war.defender_score as i64)
                            - crate::fixed::Fixed::from_int(war.attacker_score as i64)
                    };
                    our_war_score.insert(war.id, score);

                    let enemy_tags: Vec<&String> = if is_attacker {
                        war.defenders.iter().collect()
                    } else {
                        war.attackers.iter().collect()
                    };

                    for (prov_id, prov) in &world.provinces {
                        if let Some(owner) = &prov.owner {
                            if enemy_tags.contains(&owner) {
                                enemy_provinces.insert(*prov_id);
                            }
                        }
                    }
                }
            }

            // Build visible state
            let visible_state = VisibleWorldState {
                date: world.date,
                observer: tag.to_string(),
                own_country: country.clone(),
                at_war,
                known_countries: vec![],
                enemy_provinces,
                known_country_strength: (*known_country_strength).clone(),
                our_war_score,
            };

            // Use precomputed available_commands from PlayerInputs
            let available = &input.available_commands;

            // Find chosen action index
            let (chosen_action, chosen_command) = if let Some(first_cmd) = input.commands.first() {
                let idx = available.iter().position(|c| c == first_cmd);
                match idx {
                    Some(i) => (i as i32, Some(first_cmd.clone())),
                    None => {
                        // Command was executed but not in our precomputed available list.
                        // This can happen if available_commands was computed at a different
                        // point than where the AI made its decision.
                        log::debug!(
                            "{}: Command {:?} not in available list ({} options)",
                            tag,
                            first_cmd,
                            available.len()
                        );
                        (-2, Some(first_cmd.clone()))
                    }
                }
            } else {
                (-1, None) // Pass (empty command list)
            };

            samples.push(TrainingSample {
                tick: snapshot.tick,
                country: tag.to_string(),
                state: visible_state,
                available_commands: available.clone(),
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

        if samples.is_empty() {
            return Ok(());
        }

        let year = snapshot.state.date.year;

        let mut output = self
            .output
            .lock()
            .map_err(|_| ObserverError::Render("DataGenObserver output lock poisoned".into()))?;

        match &mut *output {
            OutputMode::Stream(writer) => {
                // Synchronous write for streaming mode
                for sample in &samples {
                    serde_json::to_writer(&mut *writer, sample)?;
                    writeln!(writer)?;
                }
            }
            OutputMode::AsyncArchive { sender, .. } => {
                // Non-blocking send to background thread
                // Serialization happens in the background, not here!
                if sender
                    .send(WriterMessage::Samples { year, samples })
                    .is_err()
                {
                    return Err(ObserverError::Render("Writer thread disconnected".into()));
                }
            }
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
        if let Ok(mut output) = self.output.lock() {
            match &mut *output {
                OutputMode::Stream(writer) => {
                    let _ = writer.flush();
                }
                OutputMode::AsyncArchive { sender, handle } => {
                    // Signal shutdown and wait for writer to finish
                    let _ = sender.send(WriterMessage::Shutdown);

                    if let Some(h) = handle.take() {
                        if let Err(e) = h.join() {
                            log::error!("Writer thread panicked: {:?}", e);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::WorldStateBuilder;
    use std::io::Cursor;
    use std::sync::Arc;

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
        let observer = DataGenObserver::new_stream(writer);

        // Create state with one country
        let state = WorldStateBuilder::new().with_country("SWE").build();
        let snapshot = Snapshot::new(state, 0, 0);

        // Simulate AI choosing a command with precomputed available_commands
        let inputs = vec![PlayerInputs {
            country: "SWE".to_string(),
            commands: vec![Command::Pass],
            available_commands: vec![Command::Pass],
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
        let observer = DataGenObserver::new_stream(writer);

        let state = WorldStateBuilder::new().with_country("FRA").build();
        let snapshot = Snapshot::new(state, 0, 0);

        // Empty commands = Pass, with precomputed available_commands
        let inputs = vec![PlayerInputs {
            country: "FRA".to_string(),
            commands: vec![],
            available_commands: vec![Command::Pass],
        }];

        observer.on_tick_with_inputs(&snapshot, &inputs).unwrap();

        // Check output indicates pass (-1)
        let output_data = output.lock().unwrap();
        let output_str = String::from_utf8_lossy(output_data.get_ref());
        assert!(output_str.contains("\"chosen_action\":-1"));
    }

    #[test]
    fn test_datagen_needs_inputs() {
        let observer = DataGenObserver::stdout();
        assert!(observer.needs_inputs());
    }

    #[test]
    fn test_datagen_with_country_filter() {
        let output = capture_output();
        let writer: Box<dyn Write + Send> = Box::new(OutputCapture(output.clone()));
        let observer = DataGenObserver::new_stream(writer).with_countries(&["SWE"]);

        // State with multiple countries
        let state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_country("FRA")
            .with_country("ENG")
            .build();
        let snapshot = Snapshot::new(state, 0, 0);

        // Provide inputs for all countries, but only SWE should be tracked
        let inputs = vec![
            PlayerInputs {
                country: "SWE".to_string(),
                commands: vec![],
                available_commands: vec![Command::Pass],
            },
            PlayerInputs {
                country: "FRA".to_string(),
                commands: vec![],
                available_commands: vec![Command::Pass],
            },
            PlayerInputs {
                country: "ENG".to_string(),
                commands: vec![],
                available_commands: vec![Command::Pass],
            },
        ];

        observer.on_tick_with_inputs(&snapshot, &inputs).unwrap();

        // Should only have SWE in output
        let output_data = output.lock().unwrap();
        let output_str = String::from_utf8_lossy(output_data.get_ref());
        assert!(output_str.contains("\"country\":\"SWE\""));
        assert!(!output_str.contains("\"country\":\"FRA\""));
        assert!(!output_str.contains("\"country\":\"ENG\""));
    }
}
