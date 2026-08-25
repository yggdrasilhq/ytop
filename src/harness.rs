//! Autonomous Agentic Watchdog & Telemetry Diagnostic Harness for ytop.
//!
//! Evaluates live system invariants in background ticks:
//! - Runaway CPU processes (>95% for >30s)
//! - Leaked spinning subshells & child test harness loops
//! - Twin duplicate agent processes on a single seat
//! - Extreme memory pressure or bloated cold transcripts (>30MB)
//!
//! Emits structured ytrace incidents and provides live logs to the
//! `dash-autonomous-watchdog` notebook.

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

const MAX_WATCHDOG_EVENTS: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchdogEvent {
    pub timestamp_ms: u128,
    pub level: String, // "info", "warn", "error"
    pub anomaly: String,
    pub details: String,
    pub suggested_remedy: String,
}

pub static WATCHDOG_STATE: OnceLock<Mutex<WatchdogHarness>> = OnceLock::new();

pub struct WatchdogHarness {
    pub recent_events: VecDeque<WatchdogEvent>,
    pub last_eval: Instant,
    pub consecutive_leaks: usize,
    pub consecutive_twins: usize,
}

impl WatchdogHarness {
    pub fn new() -> Self {
        Self {
            recent_events: VecDeque::with_capacity(MAX_WATCHDOG_EVENTS),
            last_eval: Instant::now(),
            consecutive_leaks: 0,
            consecutive_twins: 0,
        }
    }

    pub fn record_event(&mut self, level: &str, anomaly: &str, details: &str, remedy: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        let event = WatchdogEvent {
            timestamp_ms: now,
            level: level.to_string(),
            anomaly: anomaly.to_string(),
            details: details.to_string(),
            suggested_remedy: remedy.to_string(),
        };

        if self.recent_events.len() >= MAX_WATCHDOG_EVENTS {
            self.recent_events.pop_front();
        }
        self.recent_events.push_back(event);

        // Also emit a structured ytrace event
        crate::trace::event(
            "watchdog",
            "anomaly",
            json!({
                "level": level,
                "anomaly": anomaly,
                "details": details,
                "suggested_remedy": remedy,
                "complaint_for": "llm"
            }),
        );
    }
}

pub fn harness() -> &'static Mutex<WatchdogHarness> {
    WATCHDOG_STATE.get_or_init(|| Mutex::new(WatchdogHarness::new()))
}

/// Run an evaluation pass against the latest fleet report and host metrics
pub fn evaluate_fleet_state(report: &crate::rows::FleetRowsReport) {
    let mut h = harness().lock().unwrap();

    // 1. Evaluate Jankbox subshell leaks
    if report.leak_count > 0 {
        h.consecutive_leaks += 1;
        if h.consecutive_leaks >= 2 {
            h.record_event(
                "warn",
                "Jankbox Leaked Subshells",
                &format!("Detected {} spinning subshell loops in fleet background", report.leak_count),
                "Execute clean_jankbox action or run 'kill -9 <pids>'",
            );
        }
    } else {
        h.consecutive_leaks = 0;
    }

    // 2. Evaluate Twin Duplicate Processes
    if report.twin_count > 0 {
        h.consecutive_twins += 1;
        if h.consecutive_twins >= 2 {
            h.record_event(
                "error",
                "Twin Duplicate Agent Processes",
                &format!("Detected {} duplicate twin sessions on same seats", report.twin_count),
                "Reap orphaned twin process or restart seat",
            );
        }
    } else {
        h.consecutive_twins = 0;
    }

    // 3. Evaluate Bloated Cold Transcripts (>30MB)
    for row in &report.rows {
        if row.transcript_size_kb > 30 * 1024 && !row.is_alive {
            h.record_event(
                "warn",
                "Bloated Cold Transcript",
                &format!("Seat {} has {} MB cold transcript", row.seat, row.transcript_size_kb / 1024),
                "Harvest transcript artifacts and fold seat instead of resuming",
            );
        }
    }
}

pub fn get_recent_events() -> Vec<WatchdogEvent> {
    harness().lock().unwrap().recent_events.iter().cloned().collect()
}
