//! Simulation thread for running the game loop.
//!
//! Runs `step_world` in a separate thread, communicating with the
//! main render thread via channels.

use eu4sim_core::{PlayerInputs, SimConfig, SimMetrics, WorldState, step_world};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Simulation speed settings (matches EU4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SimSpeed {
    #[default]
    Paused,
    Speed1, // 1 tick/sec
    Speed2, // 3 ticks/sec
    Speed3, // 10 ticks/sec
    Speed4, // 30 ticks/sec
    Speed5, // Unlimited
}

impl SimSpeed {
    /// Returns the target delay between ticks.
    pub fn tick_delay(self) -> Option<Duration> {
        match self {
            SimSpeed::Paused => None,
            SimSpeed::Speed1 => Some(Duration::from_millis(1000)),
            SimSpeed::Speed2 => Some(Duration::from_millis(333)),
            SimSpeed::Speed3 => Some(Duration::from_millis(100)),
            SimSpeed::Speed4 => Some(Duration::from_millis(33)),
            SimSpeed::Speed5 => Some(Duration::ZERO),
        }
    }

    /// Cycles to the next speed (wraps from Speed5 to Paused).
    #[allow(dead_code)] // Will be used for Tab-cycle speed feature
    pub fn next(self) -> Self {
        match self {
            SimSpeed::Paused => SimSpeed::Speed1,
            SimSpeed::Speed1 => SimSpeed::Speed2,
            SimSpeed::Speed2 => SimSpeed::Speed3,
            SimSpeed::Speed3 => SimSpeed::Speed4,
            SimSpeed::Speed4 => SimSpeed::Speed5,
            SimSpeed::Speed5 => SimSpeed::Paused,
        }
    }

    /// Returns the speed from a number key (1-5).
    #[allow(dead_code)] // Alternative input handling path
    pub fn from_key(key: u8) -> Option<Self> {
        match key {
            1 => Some(SimSpeed::Speed1),
            2 => Some(SimSpeed::Speed2),
            3 => Some(SimSpeed::Speed3),
            4 => Some(SimSpeed::Speed4),
            5 => Some(SimSpeed::Speed5),
            _ => None,
        }
    }

    /// Display name for the speed.
    pub fn name(self) -> &'static str {
        match self {
            SimSpeed::Paused => "Paused",
            SimSpeed::Speed1 => "Speed 1",
            SimSpeed::Speed2 => "Speed 2",
            SimSpeed::Speed3 => "Speed 3",
            SimSpeed::Speed4 => "Speed 4",
            SimSpeed::Speed5 => "Speed 5",
        }
    }
}

/// Commands sent from the main thread to the simulation thread.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)] // PlayerInputs may be large, but this is fine for channel messages
pub enum SimControl {
    /// Set simulation speed.
    SetSpeed(SimSpeed),
    /// Toggle pause/resume.
    TogglePause,
    /// Enqueue player commands for the next tick.
    #[allow(dead_code)] // Will be used in Phase C for player input
    EnqueueCommands(PlayerInputs),
    /// Shutdown the simulation thread.
    Shutdown,
}

/// Events sent from the simulation thread to the main thread.
#[derive(Debug, Clone)]
pub enum SimEvent {
    /// A tick has completed with the new state.
    Tick { state: Arc<WorldState>, tick: u64 },
    /// Speed has changed.
    SpeedChanged(SimSpeed),
    /// Simulation thread has shut down.
    Shutdown,
}

/// Handle to the simulation thread.
pub struct SimHandle {
    /// Send commands to the sim thread.
    pub control_tx: Sender<SimControl>,
    /// Receive events from the sim thread.
    pub event_rx: Receiver<SimEvent>,
    /// Thread join handle.
    #[allow(dead_code)] // Can be used for graceful shutdown with join()
    pub thread: JoinHandle<()>,
}

impl SimHandle {
    /// Sets the simulation speed.
    pub fn set_speed(&self, speed: SimSpeed) {
        let _ = self.control_tx.send(SimControl::SetSpeed(speed));
    }

    /// Toggles pause/resume.
    pub fn toggle_pause(&self) {
        let _ = self.control_tx.send(SimControl::TogglePause);
    }

    /// Enqueues player commands.
    #[allow(dead_code)] // Will be used in Phase C for player input
    pub fn enqueue_commands(&self, inputs: PlayerInputs) {
        let _ = self.control_tx.send(SimControl::EnqueueCommands(inputs));
    }

    /// Shuts down the simulation thread.
    pub fn shutdown(&self) {
        let _ = self.control_tx.send(SimControl::Shutdown);
    }

    /// Polls for events without blocking.
    pub fn poll_events(&self) -> Vec<SimEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            events.push(event);
        }
        events
    }
}

/// Spawns the simulation thread.
///
/// Returns a handle for communication with the thread.
pub fn spawn_sim_thread(initial_state: WorldState) -> SimHandle {
    let (control_tx, control_rx) = mpsc::channel::<SimControl>();
    let (event_tx, event_rx) = mpsc::channel::<SimEvent>();

    let thread = thread::Builder::new()
        .name("sim".to_string())
        .spawn(move || {
            sim_thread_main(initial_state, control_rx, event_tx);
        })
        .expect("Failed to spawn sim thread");

    SimHandle {
        control_tx,
        event_rx,
        thread,
    }
}

/// Main loop for the simulation thread.
fn sim_thread_main(
    initial_state: WorldState,
    control_rx: Receiver<SimControl>,
    event_tx: Sender<SimEvent>,
) {
    let mut state = initial_state;
    let mut speed = SimSpeed::Paused;
    let mut tick: u64 = 0;
    let mut pending_inputs: Vec<PlayerInputs> = Vec::new();
    let config = SimConfig::default();
    let mut metrics = SimMetrics::default();

    // Send initial state
    let _ = event_tx.send(SimEvent::Tick {
        state: Arc::new(state.clone()),
        tick,
    });

    let mut last_tick = Instant::now();

    loop {
        // Process control messages (non-blocking)
        while let Ok(cmd) = control_rx.try_recv() {
            match cmd {
                SimControl::SetSpeed(new_speed) => {
                    speed = new_speed;
                    let _ = event_tx.send(SimEvent::SpeedChanged(speed));
                    log::debug!("Sim speed set to {:?}", speed);
                }
                SimControl::TogglePause => {
                    speed = if speed == SimSpeed::Paused {
                        SimSpeed::Speed1
                    } else {
                        SimSpeed::Paused
                    };
                    let _ = event_tx.send(SimEvent::SpeedChanged(speed));
                    log::debug!("Sim speed toggled to {:?}", speed);
                }
                SimControl::EnqueueCommands(inputs) => {
                    pending_inputs.push(inputs);
                }
                SimControl::Shutdown => {
                    log::info!("Sim thread shutting down");
                    let _ = event_tx.send(SimEvent::Shutdown);
                    return;
                }
            }
        }

        // Check if we should run a tick
        if let Some(delay) = speed.tick_delay() {
            let elapsed = last_tick.elapsed();
            if elapsed >= delay {
                // Run a tick
                state = step_world(&state, &pending_inputs, None, &config, Some(&mut metrics));
                pending_inputs.clear();
                tick += 1;
                last_tick = Instant::now();

                // Send new state to main thread
                let _ = event_tx.send(SimEvent::Tick {
                    state: Arc::new(state.clone()),
                    tick,
                });
            } else {
                // Sleep until next tick (but wake up for control messages)
                let sleep_time = (delay - elapsed).min(Duration::from_millis(10));
                thread::sleep(sleep_time);
            }
        } else {
            // Paused - sleep a bit and check for messages
            thread::sleep(Duration::from_millis(50));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sim_speed_tick_delay() {
        assert!(SimSpeed::Paused.tick_delay().is_none());
        assert!(SimSpeed::Speed1.tick_delay().unwrap() > Duration::ZERO);
        assert!(SimSpeed::Speed5.tick_delay().unwrap() == Duration::ZERO);
    }

    #[test]
    fn test_sim_speed_next() {
        assert_eq!(SimSpeed::Paused.next(), SimSpeed::Speed1);
        assert_eq!(SimSpeed::Speed5.next(), SimSpeed::Paused);
    }

    #[test]
    fn test_sim_speed_from_key() {
        assert_eq!(SimSpeed::from_key(1), Some(SimSpeed::Speed1));
        assert_eq!(SimSpeed::from_key(5), Some(SimSpeed::Speed5));
        assert_eq!(SimSpeed::from_key(0), None);
        assert_eq!(SimSpeed::from_key(6), None);
    }

    #[test]
    fn test_spawn_and_shutdown() {
        let state = WorldState::default();
        let handle = spawn_sim_thread(state);

        // Should receive initial tick
        let event = handle
            .event_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        assert!(matches!(event, SimEvent::Tick { tick: 0, .. }));

        // Shutdown
        handle.shutdown();
        let event = handle
            .event_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        assert!(matches!(event, SimEvent::Shutdown));

        handle.thread.join().unwrap();
    }

    #[test]
    fn test_toggle_pause_runs_ticks() {
        let state = WorldState::default();
        let handle = spawn_sim_thread(state);

        // Receive initial tick (paused by default)
        let _ = handle
            .event_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();

        // Unpause at Speed5 for fast ticks
        handle.set_speed(SimSpeed::Speed5);

        // Should receive speed changed
        let event = handle
            .event_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        assert!(matches!(event, SimEvent::SpeedChanged(SimSpeed::Speed5)));

        // Should receive more ticks
        let event = handle
            .event_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        if let SimEvent::Tick { tick, .. } = event {
            assert!(tick >= 1, "Should have advanced at least one tick");
        }

        handle.shutdown();
        handle.thread.join().unwrap();
    }
}
