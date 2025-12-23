use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Accumulated timing metrics for simulation performance.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct SimMetrics {
    pub total_ticks: u64,
    pub total_time: Duration,
    pub movement_time: Duration,
    pub combat_time: Duration,
    pub occupation_time: Duration,
    pub economy_time: Duration, // monthly systems combined (excludes trade)
    pub trade_time: Duration,   // trade systems (value + power + income)
    pub ai_time: Duration,
    pub war_score_time: Duration,
    /// Time spent in observers (datagen, event log, etc.)
    pub observer_time: Duration,
    /// Wall clock time from first tick to last
    pub wall_time: Duration,
}

impl SimMetrics {
    pub fn tick_avg_ms(&self) -> f64 {
        if self.total_ticks == 0 {
            0.0
        } else {
            self.total_time.as_secs_f64() * 1000.0 / self.total_ticks as f64
        }
    }

    pub fn years_per_second(&self, years_simulated: f64) -> f64 {
        if self.total_time.as_secs_f64() == 0.0 {
            0.0
        } else {
            years_simulated / self.total_time.as_secs_f64()
        }
    }
}
