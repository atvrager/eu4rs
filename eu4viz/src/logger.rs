use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Shared buffer for log lines to be displayed in the UI.
/// Each line is (Level, String).
#[derive(Clone)]
pub struct ConsoleLog {
    buffer: Arc<Mutex<VecDeque<(Level, String)>>>,
    max_lines: usize,
}

impl ConsoleLog {
    pub fn new(max_lines: usize) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(VecDeque::new())),
            max_lines,
        }
    }

    pub fn push(&self, level: Level, msg: String) {
        let mut buf = self.buffer.lock().unwrap();
        if buf.len() >= self.max_lines {
            buf.pop_front();
        }
        buf.push_back((level, msg));
    }

    pub fn get_lines(&self) -> Vec<(Level, String)> {
        self.buffer.lock().unwrap().iter().cloned().collect()
    }
}

pub struct MultiLogger {
    console: ConsoleLog,
}

impl MultiLogger {
    fn new(console: ConsoleLog) -> Self {
        Self { console }
    }
}

impl log::Log for MultiLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let msg = format!("{}", record.args());

            // Suppress benign warning from wgpu/driver about unsupported present modes
            // This happens when the driver exposes a mode (e.g. 1000361000) that wgpu doesn't map.
            if record.target().starts_with("wgpu") && msg.contains("Unrecognized present mode") {
                return;
            }

            self.console.push(record.level(), msg.clone());
            println!("[{}] {}", record.level(), msg);
        }
    }

    fn flush(&self) {}
}

pub fn init(level: LevelFilter) -> Result<ConsoleLog, SetLoggerError> {
    let console = ConsoleLog::new(50); // Keep last 50 lines
    let logger = MultiLogger::new(console.clone());

    // Leak logger to make it static
    let logger: &'static MultiLogger = Box::leak(Box::new(logger));

    log::set_logger(logger)?;
    log::set_max_level(level);

    Ok(console)
}
