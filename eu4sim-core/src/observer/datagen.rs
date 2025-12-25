//! Training data generation observer for ML model training.
//!
//! Generates training samples containing:
//! - Current visible state
//! - Available commands
//! - Chosen action (index into available commands)
//!
//! # Output Formats
//!
//! ## Binary Mode (`.cpb.zip` extension) - **Recommended**
//! Creates a zip archive with Cap'n Proto binary files per year:
//! ```text
//! datagen.cpb.zip/
//!   1444.cpb  (Cap'n Proto packed binary)
//!   1445.cpb
//!   ...
//! ```
//!
//! Cap'n Proto binary format provides:
//! - ~10x faster serialization than JSON
//! - Zero-copy reads in Python (via pycapnp)
//! - Schema-enforced type safety between Rust and Python
//!
//! ## JSONL Mode (`.jsonl` extension) - Legacy
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
//! ## JSON Archive Mode (`.zip` extension)
//! Creates a zip archive with deflate-compressed JSONL files per year.
//!
//! # Performance Notes
//!
//! Archive modes use a background writer thread for non-blocking I/O.
//! Serialization happens in parallel on the main thread (via rayon), then
//! pre-serialized bytes are sent to the background writer for compression.
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

use super::capnp_serialize;
use super::{ObserverConfig, ObserverError, SimObserver, Snapshot};
use crate::ai::VisibleWorldState;
use crate::input::{Command, PlayerInputs};
use crate::state::Tag;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::mpsc::{self, Sender, SyncSender};
use std::sync::Mutex;
use std::thread::JoinHandle;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// A single training sample for ML model training.
///
/// # Multi-Command Support
///
/// AI players can submit multiple commands per tick. For example: 1 diplomatic
/// action, N military moves, 1 economic action, and N trade actions. The
/// `chosen_actions` and `chosen_commands` fields store all commands issued.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingSample {
    /// Current simulation tick
    pub tick: u64,
    /// Country this sample is for
    pub country: Tag,
    /// Visible state for this country (prompt input)
    pub state: VisibleWorldState,
    /// All legal commands at this moment (action space)
    pub available_commands: Vec<Command>,
    /// Indices of chosen commands in `available_commands`
    /// Empty vector means Pass (no commands issued)
    pub chosen_actions: Vec<i32>,
    /// The actual commands chosen (for debugging/analysis)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub chosen_commands: Vec<Command>,
}

/// Message sent to the background writer thread
enum WriterMessage {
    /// Pre-serialized data for a given year
    SerializedBatch {
        year: i32,
        /// Already-serialized bytes (parallel serialization happened on main thread)
        data: Vec<u8>,
    },
    /// Binary samples for a given year-month (will be serialized by writer thread)
    BinarySamples {
        year: i32,
        month: u8,
        /// Raw samples to serialize as Cap'n Proto
        samples: Vec<TrainingSample>,
    },
    /// Shutdown signal - flush and finalize
    Shutdown,
}

/// Output mode for datagen observer
enum OutputMode {
    /// Streaming JSONL to a writer (synchronous, for stdout/simple files)
    Stream(Box<dyn Write + Send>),
    /// Async JSON archive mode with background writer thread
    AsyncJsonArchive {
        sender: Sender<WriterMessage>,
        /// Handle to join the writer thread on shutdown
        handle: Option<JoinHandle<()>>,
    },
    /// Async binary (Cap'n Proto) archive mode with background writer thread
    /// Uses SyncSender for backpressure to prevent OOM with large datasets
    AsyncBinaryArchive {
        sender: SyncSender<WriterMessage>,
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
                WriterMessage::SerializedBatch { year, data } => {
                    if let Err(e) = self.handle_batch(year, data) {
                        log::error!("ArchiveWriter error: {}", e);
                    }
                }
                WriterMessage::BinarySamples { .. } => {
                    log::warn!("ArchiveWriter received binary samples, ignoring");
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

    /// Append pre-serialized bytes to the year buffer (I/O only, no CPU work)
    fn handle_batch(&mut self, year: i32, data: Vec<u8>) -> Result<(), String> {
        // Year transition - flush previous year
        if let Some(prev_year) = self.current_year {
            if year != prev_year {
                self.flush_year_buffer(prev_year)?;
                self.year_buffer.clear();
            }
        }
        self.current_year = Some(year);

        // Just append the pre-serialized bytes
        self.year_buffer.extend_from_slice(&data);

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

/// Background writer thread state for binary (Cap'n Proto) archives
struct BinaryArchiveWriter {
    zip: ZipWriter<std::fs::File>,
    /// Current (year, month) being buffered
    current_period: Option<(i32, u8)>,
    /// Samples buffered for current month (reduced from yearly to avoid OOM)
    month_samples: Vec<TrainingSample>,
}

impl BinaryArchiveWriter {
    fn new(file: std::fs::File) -> Self {
        Self {
            zip: ZipWriter::new(file),
            current_period: None,
            // ~20k samples per month with 666 countries × 30 days
            month_samples: Vec::with_capacity(20_000),
        }
    }

    /// Process incoming messages until shutdown
    fn run(mut self, receiver: mpsc::Receiver<WriterMessage>) {
        while let Ok(msg) = receiver.recv() {
            match msg {
                WriterMessage::BinarySamples {
                    year,
                    month,
                    samples,
                } => {
                    if let Err(e) = self.handle_samples(year, month, samples) {
                        log::error!("BinaryArchiveWriter error: {}", e);
                    }
                }
                WriterMessage::SerializedBatch { .. } => {
                    log::warn!("BinaryArchiveWriter received JSON batch, ignoring");
                }
                WriterMessage::Shutdown => {
                    if let Err(e) = self.finalize() {
                        log::error!("BinaryArchiveWriter finalize error: {}", e);
                    }
                    break;
                }
            }
        }
    }

    /// Buffer samples for the current month, flush on month transition
    fn handle_samples(
        &mut self,
        year: i32,
        month: u8,
        samples: Vec<TrainingSample>,
    ) -> Result<(), String> {
        let new_period = (year, month);

        // Month transition - flush previous month
        if let Some(prev_period) = self.current_period {
            if new_period != prev_period {
                self.flush_month(prev_period.0, prev_period.1)?;
                self.month_samples.clear();
            }
        }
        self.current_period = Some(new_period);

        // Collect samples
        self.month_samples.extend(samples);

        Ok(())
    }

    fn flush_month(&mut self, year: i32, month: u8) -> Result<(), String> {
        if self.month_samples.is_empty() {
            return Ok(());
        }

        // Serialize all samples for this month to Cap'n Proto
        let mut buf = Vec::with_capacity(1024 * 1024); // 1MB initial
        capnp_serialize::serialize_batch(&mut buf, year as i16, &self.month_samples)
            .map_err(|e| format!("Failed to serialize Cap'n Proto batch: {}", e))?;

        // Write to ZIP archive with deflate compression
        // Filename: YYYY_MM.cpb (e.g., 1444_11.cpb)
        let filename = format!("{}_{:02}.cpb", year, month);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(6));

        self.zip
            .start_file(&filename, options)
            .map_err(|e| format!("Failed to start zip file: {}", e))?;
        self.zip
            .write_all(&buf)
            .map_err(|e| format!("Failed to write to zip: {}", e))?;

        log::debug!(
            "Wrote {}_{:02}.cpb to archive ({} samples, {} bytes)",
            year,
            month,
            self.month_samples.len(),
            buf.len()
        );

        Ok(())
    }

    fn finalize(mut self) -> Result<(), String> {
        // Flush any remaining month data
        if let Some((year, month)) = self.current_period {
            if !self.month_samples.is_empty() {
                self.flush_month(year, month)?;
            }
        }

        // Finalize the archive
        self.zip
            .finish()
            .map_err(|e| format!("Failed to finalize zip: {}", e))?;

        log::info!("Binary archive finalized successfully");
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
/// // Generate to JSON archive
/// let observer = DataGenObserver::file("training.zip")?;
///
/// // Generate to binary archive (recommended for ML pipelines)
/// let observer = DataGenObserver::file("training.cpb.zip")?;
///
/// // Or to stdout for piping
/// let observer = DataGenObserver::stdout();
/// ```
pub struct DataGenObserver {
    /// Output destination (stream or async archive)
    output: Mutex<OutputMode>,
    /// Countries to generate data for (empty = all AI countries)
    tracked_countries: Vec<Tag>,
    /// Countries to exclude from data generation (e.g., LLM-controlled)
    excluded_countries: Vec<Tag>,
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
    /// Output format is determined by file extension:
    /// - `.cpb.zip`: Binary Cap'n Proto archive (recommended for ML pipelines)
    /// - `.zip`: JSON archive (legacy, deflate compressed)
    /// - `.jsonl`: Streaming JSONL (legacy)
    pub fn file(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref();
        let path_str = path.to_string_lossy();

        // Check for .cpb.zip (binary mode) first
        if path_str.ends_with(".cpb.zip") {
            // Binary archive mode: spawn background writer thread
            // Use bounded channel (5 months) to apply backpressure and prevent OOM
            let file = std::fs::File::create(path)?;
            let (sender, receiver) = mpsc::sync_channel(5);

            let writer = BinaryArchiveWriter::new(file);
            let handle = std::thread::Builder::new()
                .name("datagen-binary-writer".into())
                .spawn(move || writer.run(receiver))
                .expect("Failed to spawn datagen binary writer thread");

            Ok(Self {
                output: Mutex::new(OutputMode::AsyncBinaryArchive {
                    sender,
                    handle: Some(handle),
                }),
                tracked_countries: vec![],
                excluded_countries: vec![],
                config: ObserverConfig {
                    frequency: 1,
                    notify_on_month_start: true,
                },
            })
        } else if path.extension().map(|e| e == "zip").unwrap_or(false) {
            // JSON archive mode: spawn background writer thread
            let file = std::fs::File::create(path)?;
            let (sender, receiver) = mpsc::channel();

            let writer = ArchiveWriter::new(file);
            let handle = std::thread::Builder::new()
                .name("datagen-writer".into())
                .spawn(move || writer.run(receiver))
                .expect("Failed to spawn datagen writer thread");

            Ok(Self {
                output: Mutex::new(OutputMode::AsyncJsonArchive {
                    sender,
                    handle: Some(handle),
                }),
                tracked_countries: vec![],
                excluded_countries: vec![],
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
            excluded_countries: vec![],
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

    /// Exclude specific countries from data generation (e.g., LLM-controlled).
    pub fn exclude_countries(mut self, countries: &[&str]) -> Self {
        self.excluded_countries = countries.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Generate training samples from precomputed PlayerInputs.
    ///
    /// Uses both visible_state and available_commands from PlayerInputs
    /// (computed in the AI loop) to avoid redundant work.
    fn generate_samples(
        &self,
        snapshot: &Snapshot,
        inputs: &[PlayerInputs],
    ) -> Vec<TrainingSample> {
        // Filter to tracked countries if specified
        let tracked_set: Option<std::collections::HashSet<&str>> =
            if self.tracked_countries.is_empty() {
                None // Track all
            } else {
                Some(self.tracked_countries.iter().map(|s| s.as_str()).collect())
            };

        // Build exclusion set
        let excluded_set: std::collections::HashSet<&str> =
            self.excluded_countries.iter().map(|s| s.as_str()).collect();

        let mut samples = Vec::with_capacity(inputs.len());

        for input in inputs {
            let tag = input.country.as_str();

            // Skip if in exclusion set (e.g., LLM-controlled countries)
            if excluded_set.contains(tag) {
                continue;
            }

            // Skip if not in tracked set
            if let Some(ref tracked) = tracked_set {
                if !tracked.contains(tag) {
                    continue;
                }
            }

            // Use precomputed visible_state from PlayerInputs (avoids recomputing war scores, etc.)
            let Some(visible_state) = input.visible_state.clone() else {
                // No precomputed state - skip this country
                log::debug!("{}: No precomputed visible_state, skipping", tag);
                continue;
            };

            // Use precomputed available_commands from PlayerInputs
            let available = &input.available_commands;

            // Find indices for ALL chosen commands (multi-command support)
            let mut chosen_actions = Vec::with_capacity(input.commands.len());
            let mut chosen_commands = Vec::with_capacity(input.commands.len());

            for cmd in &input.commands {
                let idx = available.iter().position(|c| c == cmd);
                match idx {
                    Some(i) => chosen_actions.push(i as i32),
                    None => {
                        // Command was executed but not in our precomputed available list
                        log::debug!(
                            "{}: Command {:?} not in available list ({} options)",
                            tag,
                            cmd,
                            available.len()
                        );
                        chosen_actions.push(-2); // Mark as "not in list"
                    }
                }
                chosen_commands.push(cmd.clone());
            }

            samples.push(TrainingSample {
                tick: snapshot.tick,
                country: tag.to_string(),
                state: visible_state,
                available_commands: available.clone(),
                chosen_actions,
                chosen_commands,
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
        let month = snapshot.state.date.month;

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
            OutputMode::AsyncJsonArchive { sender, .. } => {
                // Parallelize JSON serialization across all CPU cores.
                // Each sample serializes to its own buffer in parallel, then we flatten.
                let buffers: Vec<Vec<u8>> = samples
                    .par_iter()
                    .map(|sample| {
                        let mut buf = serde_json::to_vec(sample).unwrap_or_default();
                        buf.push(b'\n');
                        buf
                    })
                    .collect();

                // Single-pass flatten (O(n) total copies, not O(n log n))
                let total_len: usize = buffers.iter().map(|b| b.len()).sum();
                let mut serialized = Vec::with_capacity(total_len);
                for buf in buffers {
                    serialized.extend_from_slice(&buf);
                }

                // Send pre-serialized bytes to background writer (I/O only)
                if sender
                    .send(WriterMessage::SerializedBatch {
                        year,
                        data: serialized,
                    })
                    .is_err()
                {
                    return Err(ObserverError::Render("Writer thread disconnected".into()));
                }
            }
            OutputMode::AsyncBinaryArchive { sender, .. } => {
                // Send samples to background writer for Cap'n Proto serialization
                // Flushing happens on month boundaries (not yearly) to limit memory usage
                if sender
                    .send(WriterMessage::BinarySamples {
                        year,
                        month,
                        samples,
                    })
                    .is_err()
                {
                    return Err(ObserverError::Render(
                        "Binary writer thread disconnected".into(),
                    ));
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
                OutputMode::AsyncJsonArchive { sender, handle } => {
                    // Signal shutdown and wait for writer to finish
                    let _ = sender.send(WriterMessage::Shutdown);

                    if let Some(h) = handle.take() {
                        if let Err(e) = h.join() {
                            log::error!("Writer thread panicked: {:?}", e);
                        }
                    }
                }
                OutputMode::AsyncBinaryArchive { sender, handle } => {
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

    /// Create a minimal visible state for testing
    fn mock_visible_state(tag: &str) -> VisibleWorldState {
        VisibleWorldState {
            observer: tag.to_string(),
            ..Default::default()
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

        // Simulate AI choosing a command with precomputed available_commands and visible_state
        let inputs = vec![PlayerInputs {
            country: "SWE".to_string(),
            commands: vec![Command::Pass],
            available_commands: vec![Command::Pass],
            visible_state: Some(mock_visible_state("SWE")),
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
            visible_state: Some(mock_visible_state("FRA")),
        }];

        observer.on_tick_with_inputs(&snapshot, &inputs).unwrap();

        // Check output indicates pass (empty chosen_actions array)
        let output_data = output.lock().unwrap();
        let output_str = String::from_utf8_lossy(output_data.get_ref());
        assert!(output_str.contains("\"chosen_actions\":[]"));
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
                visible_state: Some(mock_visible_state("SWE")),
            },
            PlayerInputs {
                country: "FRA".to_string(),
                commands: vec![],
                available_commands: vec![Command::Pass],
                visible_state: Some(mock_visible_state("FRA")),
            },
            PlayerInputs {
                country: "ENG".to_string(),
                commands: vec![],
                available_commands: vec![Command::Pass],
                visible_state: Some(mock_visible_state("ENG")),
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
