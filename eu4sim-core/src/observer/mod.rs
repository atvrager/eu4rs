//! Observer pattern for simulation state inspection.
//!
//! This module provides a trait-based system for observing simulation state
//! without affecting determinism. Observers receive immutable snapshots
//! wrapped in `Arc` for safe, zero-copy sharing.
//!
//! # Architecture
//!
//! ```text
//! SimObserver trait
//!        │
//!        ├── ConsoleObserver (terminal output)
//!        ├── [Future] GuiObserver (mpsc to renderer)
//!        └── [Future] HtmlObserver (SSE/WebSocket)
//! ```
//!
//! # Design Principles
//!
//! - **Determinism**: Observers receive immutable snapshots; they cannot affect simulation
//! - **Performance**: Uses `Arc<WorldState>` with `im::HashMap`'s O(1) structural sharing
//! - **Extensibility**: Trait-based, supports any observer implementation
//!
//! # Example
//!
//! ```ignore
//! let mut registry = ObserverRegistry::new();
//! registry.register(Box::new(ConsoleObserver::new(&["FRA", "ENG"])));
//!
//! // In simulation loop, after step_world:
//! let snapshot = Snapshot::new(state.clone(), tick, checksum);
//! registry.notify(&snapshot);
//! ```

pub mod capnp_serialize;
pub mod console;
pub mod datagen;
pub mod event_log;

use crate::input::PlayerInputs;
use crate::state::WorldState;
use std::sync::Arc;
use thiserror::Error;

/// Immutable snapshot of simulation state for observers.
///
/// Wraps `WorldState` in `Arc` to guarantee:
/// - Zero-copy sharing between multiple observers
/// - Thread-safe access (`Arc` is `Send + Sync`)
/// - Immutability (no `&mut` access possible)
#[derive(Clone)]
pub struct Snapshot {
    /// Immutable reference to world state (O(1) clone via `im::HashMap`)
    pub state: Arc<WorldState>,
    /// Monotonic tick counter (days since simulation start)
    pub tick: u64,
    /// State checksum for desync detection (0 if disabled)
    pub checksum: u64,
}

impl Snapshot {
    /// Create a new snapshot from world state.
    pub fn new(state: WorldState, tick: u64, checksum: u64) -> Self {
        Self {
            state: Arc::new(state),
            tick,
            checksum,
        }
    }

    /// Create a snapshot from an already-wrapped Arc.
    pub fn from_arc(state: Arc<WorldState>, tick: u64, checksum: u64) -> Self {
        Self {
            state,
            tick,
            checksum,
        }
    }
}

/// Errors that can occur during observation.
#[derive(Error, Debug)]
pub enum ObserverError {
    /// I/O error (e.g., writing to terminal)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Serialization error (e.g., JSON output)
    #[error("Serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
    /// Rendering/formatting error
    #[error("Render error: {0}")]
    Render(String),
    /// Observer channel disconnected (for async observers)
    #[error("Observer disconnected")]
    Disconnected,
}

/// Configuration for observer notification frequency.
#[derive(Clone, Debug)]
pub struct ObserverConfig {
    /// Notify every N ticks (1 = every tick, 30 = monthly, 365 = yearly)
    pub frequency: u32,
    /// Always notify on the 1st of the month (when economic systems run)
    pub notify_on_month_start: bool,
}

impl Default for ObserverConfig {
    fn default() -> Self {
        Self {
            frequency: 1,
            notify_on_month_start: true,
        }
    }
}

/// Trait for simulation observers.
///
/// Implementers receive immutable state snapshots after simulation ticks.
/// Observers **must not** affect simulation determinism.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to support parallel observation
/// and future async/channel-based delivery.
///
/// # Error Handling
///
/// Errors returned from `on_tick` are logged but do not block simulation.
/// Observers should handle their own error recovery internally.
pub trait SimObserver: Send + Sync {
    /// Called after each tick (or as configured by frequency).
    ///
    /// Receives an immutable snapshot that cannot affect simulation state.
    fn on_tick(&self, snapshot: &Snapshot) -> Result<(), ObserverError>;

    /// Called after each tick with both state AND the inputs that were processed.
    ///
    /// This method is only called for observers that return `true` from `needs_inputs()`.
    /// Default implementation delegates to `on_tick`, ignoring inputs.
    ///
    /// Use this for observers that need to correlate state changes with player actions,
    /// such as training data generators.
    fn on_tick_with_inputs(
        &self,
        snapshot: &Snapshot,
        _inputs: &[PlayerInputs],
    ) -> Result<(), ObserverError> {
        self.on_tick(snapshot)
    }

    /// Whether this observer needs input data.
    ///
    /// If `true`, the registry will call `on_tick_with_inputs` instead of `on_tick`.
    /// Default is `false` for backward compatibility.
    fn needs_inputs(&self) -> bool {
        false
    }

    /// Human-readable name for logging/debugging.
    fn name(&self) -> &str;

    /// Observer-specific configuration.
    ///
    /// Default: notify every tick, always on month start.
    fn config(&self) -> ObserverConfig {
        ObserverConfig::default()
    }

    /// Called when simulation ends or observer is unregistered.
    ///
    /// Default implementation is a no-op.
    fn on_shutdown(&self) {}
}

/// Registry for managing multiple observers.
///
/// Observers are stored as trait objects to allow heterogeneous collections.
pub struct ObserverRegistry {
    observers: Vec<Box<dyn SimObserver>>,
}

impl ObserverRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self { observers: vec![] }
    }

    /// Register a new observer.
    pub fn register(&mut self, observer: Box<dyn SimObserver>) {
        log::info!("Registered observer: {}", observer.name());
        self.observers.push(observer);
    }

    /// Notify all observers of a tick (without input data).
    ///
    /// Convenience method that calls `notify_with_inputs` with empty inputs.
    /// Use this when you don't need to pass player inputs to observers.
    pub fn notify(&self, snapshot: &Snapshot) {
        self.notify_with_inputs(snapshot, &[])
    }

    /// Notify all observers of a tick with player inputs.
    ///
    /// For observers that need input data (`needs_inputs() == true`), calls
    /// `on_tick_with_inputs`. For others, calls `on_tick`.
    ///
    /// Errors are logged but do not propagate (non-blocking).
    pub fn notify_with_inputs(&self, snapshot: &Snapshot, inputs: &[PlayerInputs]) {
        for observer in &self.observers {
            let config = observer.config();

            // Check frequency gating
            let should_notify = snapshot.tick.is_multiple_of(config.frequency as u64)
                || (config.notify_on_month_start && snapshot.state.date.day == 1);

            if should_notify {
                let result = if observer.needs_inputs() {
                    observer.on_tick_with_inputs(snapshot, inputs)
                } else {
                    observer.on_tick(snapshot)
                };

                if let Err(e) = result {
                    log::warn!("Observer '{}' error: {}", observer.name(), e);
                }
            }
        }
    }

    /// Notify all observers of shutdown.
    pub fn shutdown(&self) {
        for observer in &self.observers {
            observer.on_shutdown();
        }
    }

    /// Number of registered observers.
    pub fn len(&self) -> usize {
        self.observers.len()
    }

    /// Returns true if no observers are registered.
    pub fn is_empty(&self) -> bool {
        self.observers.is_empty()
    }
}

impl Default for ObserverRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ObserverRegistry {
    fn drop(&mut self) {
        // Ensure all observers are properly shut down (flush buffers, finalize archives)
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::WorldStateBuilder;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Shared counter for test observers
    #[derive(Clone)]
    struct SharedCounter(Arc<AtomicU64>);

    impl SharedCounter {
        fn new() -> Self {
            Self(Arc::new(AtomicU64::new(0)))
        }

        fn get(&self) -> u64 {
            self.0.load(Ordering::SeqCst)
        }

        fn increment(&self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Test observer that counts notifications using shared counter
    struct CountingObserver {
        counter: SharedCounter,
        config: ObserverConfig,
    }

    impl CountingObserver {
        fn new(counter: SharedCounter) -> Self {
            Self {
                counter,
                config: ObserverConfig::default(),
            }
        }

        fn with_frequency(mut self, frequency: u32) -> Self {
            self.config.frequency = frequency;
            self
        }
    }

    impl SimObserver for CountingObserver {
        fn on_tick(&self, _snapshot: &Snapshot) -> Result<(), ObserverError> {
            self.counter.increment();
            Ok(())
        }

        fn name(&self) -> &str {
            "CountingObserver"
        }

        fn config(&self) -> ObserverConfig {
            self.config.clone()
        }
    }

    #[test]
    fn test_observer_notification() {
        let counter = SharedCounter::new();
        let mut registry = ObserverRegistry::new();
        registry.register(Box::new(CountingObserver::new(counter.clone())));

        let state = WorldStateBuilder::new().build();
        let snapshot = Snapshot::new(state, 1, 0);

        registry.notify(&snapshot);
        registry.notify(&snapshot);

        assert_eq!(counter.get(), 2);
    }

    #[test]
    fn test_snapshot_arc_sharing() {
        let state = WorldStateBuilder::new().with_country("SWE").build();
        let snapshot1 = Snapshot::new(state, 1, 12345);
        let snapshot2 = snapshot1.clone();

        // Same Arc, no deep copy
        assert!(Arc::ptr_eq(&snapshot1.state, &snapshot2.state));
        assert_eq!(snapshot1.tick, snapshot2.tick);
        assert_eq!(snapshot1.checksum, snapshot2.checksum);
    }

    #[test]
    fn test_frequency_filtering() {
        let counter = SharedCounter::new();
        let mut registry = ObserverRegistry::new();
        registry.register(Box::new(
            CountingObserver::new(counter.clone()).with_frequency(5),
        ));

        let mut state = WorldStateBuilder::new().build();

        // Notify 10 times with ticks 1-10
        for tick in 1..=10 {
            state.date.day = 15; // Not month start
            let snapshot = Snapshot::new(state.clone(), tick, 0);
            registry.notify(&snapshot);
        }

        // Should only notify on ticks 5 and 10 (frequency=5)
        assert_eq!(counter.get(), 2);
    }

    #[test]
    fn test_month_start_notification() {
        let counter = SharedCounter::new();
        let mut registry = ObserverRegistry::new();
        registry.register(Box::new(
            CountingObserver::new(counter.clone()).with_frequency(100),
        ));

        let mut state = WorldStateBuilder::new().build();
        state.date.day = 1; // Month start

        let snapshot = Snapshot::new(state, 50, 0); // Tick 50, not divisible by 100
        registry.notify(&snapshot);

        // Should notify because it's month start
        assert_eq!(counter.get(), 1);
    }

    #[test]
    fn test_registry_len() {
        let counter = SharedCounter::new();
        let mut registry = ObserverRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        registry.register(Box::new(CountingObserver::new(counter.clone())));
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);

        registry.register(Box::new(CountingObserver::new(counter.clone())));
        assert_eq!(registry.len(), 2);
    }
}
