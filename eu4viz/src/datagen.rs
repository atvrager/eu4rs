//! Reader for datagen training data files.
//!
//! Supports both JSONL files and ZIP archives containing year-based JSONL files.

use eu4sim_core::observer::datagen::TrainingSample;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

/// Statistics from analyzing training samples.
#[derive(Debug, Default)]
pub struct DatagenStats {
    pub total_samples: usize,
    pub samples_by_country: HashMap<String, usize>,
    pub samples_by_year: HashMap<i32, usize>,
    pub action_distribution: HashMap<i32, usize>,
    pub min_tick: u64,
    pub max_tick: u64,
}

impl DatagenStats {
    fn new() -> Self {
        Self {
            min_tick: u64::MAX,
            max_tick: 0,
            ..Default::default()
        }
    }

    fn add_sample(&mut self, sample: &TrainingSample) {
        self.total_samples += 1;
        *self
            .samples_by_country
            .entry(sample.country.clone())
            .or_default() += 1;

        let year = 1444 + (sample.tick / 365) as i32;
        *self.samples_by_year.entry(year).or_default() += 1;

        // Multi-command: count each chosen action
        for &action_idx in &sample.chosen_actions {
            *self.action_distribution.entry(action_idx).or_default() += 1;
        }

        self.min_tick = self.min_tick.min(sample.tick);
        self.max_tick = self.max_tick.max(sample.tick);
    }
}

/// Result of reading datagen files.
pub struct DatagenReader {
    samples: Vec<TrainingSample>,
    stats: DatagenStats,
}

impl DatagenReader {
    /// Read from a JSONL or ZIP file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();

        if path.extension().map(|e| e == "zip").unwrap_or(false) {
            Self::from_zip(path)
        } else {
            Self::from_jsonl(path)
        }
    }

    /// Read from a JSONL file.
    fn from_jsonl(path: &Path) -> Result<Self, String> {
        let file =
            File::open(path).map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;
        let reader = BufReader::new(file);

        let mut samples = Vec::new();
        let mut stats = DatagenStats::new();

        for (line_num, line) in reader.lines().enumerate() {
            let line = line.map_err(|e| format!("Failed to read line {}: {}", line_num + 1, e))?;
            if line.trim().is_empty() {
                continue;
            }

            let sample: TrainingSample = serde_json::from_str(&line)
                .map_err(|e| format!("Failed to parse line {}: {}", line_num + 1, e))?;

            stats.add_sample(&sample);
            samples.push(sample);
        }

        Ok(Self { samples, stats })
    }

    /// Read from a ZIP archive containing year-based JSONL files.
    fn from_zip(path: &Path) -> Result<Self, String> {
        let file =
            File::open(path).map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("Failed to read zip: {}", e))?;

        let mut samples = Vec::new();
        let mut stats = DatagenStats::new();

        // Sort filenames for deterministic ordering
        let mut names: Vec<String> = (0..archive.len())
            .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
            .collect();
        names.sort();

        for name in names {
            if !name.ends_with(".jsonl") {
                continue;
            }

            let mut file = archive
                .by_name(&name)
                .map_err(|e| format!("Failed to read {}: {}", name, e))?;
            let mut content = String::new();
            file.read_to_string(&mut content)
                .map_err(|e| format!("Failed to read {}: {}", name, e))?;

            for (line_num, line) in content.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }

                let sample: TrainingSample = serde_json::from_str(line)
                    .map_err(|e| format!("Failed to parse {}:{}: {}", name, line_num + 1, e))?;

                stats.add_sample(&sample);
                samples.push(sample);
            }
        }

        Ok(Self { samples, stats })
    }

    /// Get all samples.
    pub fn samples(&self) -> &[TrainingSample] {
        &self.samples
    }

    /// Get computed statistics.
    pub fn stats(&self) -> &DatagenStats {
        &self.stats
    }

    /// Filter samples by country.
    pub fn filter_country(&self, country: &str) -> Vec<&TrainingSample> {
        self.samples
            .iter()
            .filter(|s| s.country == country)
            .collect()
    }
}

/// Display summary statistics.
pub fn print_stats(stats: &DatagenStats) {
    println!("=== Datagen Statistics ===\n");
    println!("Total samples: {}", stats.total_samples);
    println!("Tick range: {} - {}", stats.min_tick, stats.max_tick);

    // Year coverage
    if !stats.samples_by_year.is_empty() {
        let mut years: Vec<_> = stats.samples_by_year.keys().collect();
        years.sort();
        println!(
            "Year coverage: {} - {} ({} years)",
            years.first().unwrap(),
            years.last().unwrap(),
            years.len()
        );
    }

    // Top countries
    println!("\nTop countries by samples:");
    let mut countries: Vec<_> = stats.samples_by_country.iter().collect();
    countries.sort_by(|a, b| b.1.cmp(a.1));
    for (country, count) in countries.iter().take(10) {
        println!("  {}: {}", country, count);
    }

    // Action distribution
    println!("\nAction distribution:");
    let mut actions: Vec<_> = stats.action_distribution.iter().collect();
    actions.sort_by_key(|(action, _)| *action);
    for (action, count) in actions {
        let label = match *action {
            -1 => "Pass".to_string(),
            -2 => "Unknown".to_string(),
            n => format!("Action[{}]", n),
        };
        let pct = (*count as f64 / stats.total_samples as f64) * 100.0;
        println!("  {}: {} ({:.1}%)", label, count, pct);
    }
}

/// Display individual samples.
pub fn print_samples(samples: &[&TrainingSample], limit: usize) {
    println!(
        "\n=== Sample Viewer ({} of {}) ===\n",
        limit.min(samples.len()),
        samples.len()
    );

    for sample in samples.iter().take(limit) {
        let action_count = sample.chosen_actions.len();
        let action_str = if action_count == 0 {
            "Pass".to_string()
        } else {
            format!("{} action(s): {:?}", action_count, sample.chosen_actions)
        };

        println!(
            "Tick {}: {} chose {} ({} available)",
            sample.tick,
            sample.country,
            action_str,
            sample.available_commands.len()
        );

        for cmd in &sample.chosen_commands {
            println!("  Command: {:?}", cmd);
        }
    }
}
