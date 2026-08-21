//! Is this number a rate?
//!
//! A rate is a count divided by a window. Two things routinely arrive wearing a
//! rate's name and neither is one:
//!
//! * a **tally** — a lifetime count divided by an assumed window. It climbs with
//!   the age of whatever produced it, resets when that restarts, and falls when
//!   retention prunes the log it was counted from. Its ceiling is set by process
//!   age, so a freshly started build reads low and a long-lived one reads high,
//!   whatever either is actually doing.
//! * a **stuck reading** — a value that does not move while the clock does,
//!   because the numerator and the denominator are both frozen.
//!
//! Both plot as plausible numbers next to a threshold, and a threshold placed on
//! either arms once and never disarms. Neither announces itself. The only way to
//! tell them from a rate is to watch a series and ask what it is doing over time,
//! which is what this module does.
//!
//! ⚠ Classify **per emitter**. A restart resets a tally, so a series pooled
//! across processes shows drops that look like a well-behaved rate falling. That
//! is the mistake this module exists to make hard: [`Series::push`] carries the
//! emitter identity and [`classify_by_emitter`] refuses to mix them.

use std::collections::BTreeMap;
use std::time::Duration;

/// The absolute epoch-ms floor for "the last `window`".
///
/// `ytrace::query::*` take `since_ms` as an ABSOLUTE epoch, not a duration.
/// Both are `u128` and read identically at a call site, so passing the window
/// straight through compiles and silently widens the query to all of history —
/// which is how a lifetime tally comes to be divided by an assumed window and
/// rendered as a rate. Convert here, once, deliberately.
///
/// (ytrace grew its own `query::since_window` on the same reasoning; this
/// collapses into a re-export once the pin moves to a main that carries it.)
pub fn since_window(window: Duration) -> u128 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    now.saturating_sub(window.as_millis())
}

/// Fewer than this many samples cannot distinguish a rate from a ramp.
pub const MIN_SAMPLES: usize = 4;

/// One reading of a named quantity from one emitter.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    /// When the reading was taken (epoch ms).
    pub ts_ms: u128,
    /// The value, or None when the sensor was unreadable. An unreadable sensor
    /// is not a zero — that is the substitution this whole module argues against.
    pub value: Option<f64>,
    /// How long the emitting process had been running, when known.
    pub emitter_age_secs: Option<u64>,
}

/// What a series of readings turns out to be.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// Moves both ways over the window — behaves like a rate.
    Rate,
    /// Never falls across the series while rising overall: a cumulative count,
    /// not a rate. Carries the span so the claim can be checked.
    Counter { rose_by: f64, over_secs: u64 },
    /// The clock moved and the value did not.
    Stuck { at: f64, over_secs: u64 },
    /// Every reading was unreadable. Not a zero, not an all-clear.
    Blind,
    /// Not enough readings yet to say anything honest.
    TooFew { have: usize },
}

impl Verdict {
    /// Whether a threshold comparison against this series means anything.
    pub fn is_thresholdable(&self) -> bool {
        matches!(self, Verdict::Rate)
    }

    /// The line to render beside the number, in the operator's words.
    pub fn caveat(&self) -> Option<String> {
        match self {
            Verdict::Rate => None,
            Verdict::Counter { rose_by, over_secs } => Some(format!(
                "⛔ NOT A RATE — rose {rose_by:.1} over {over_secs}s and never fell. \
                 This is a cumulative count; it tracks emitter age, not the quantity."
            )),
            Verdict::Stuck { at, over_secs } => Some(format!(
                "⛔ NOT A RATE — pinned at {at:.1} for {over_secs}s while the clock ran. \
                 A rate that does not move between samples is not a rate."
            )),
            Verdict::Blind => Some(
                "⚠ UNREADABLE — every sample was empty. This is a blind sensor, not a zero."
                    .to_string(),
            ),
            Verdict::TooFew { have } => Some(format!(
                "⚠ {have} sample(s) — too few to tell a rate from a ramp (need {MIN_SAMPLES})."
            )),
        }
    }
}

/// A series of readings of one quantity from one emitter.
#[derive(Debug, Clone, Default)]
pub struct Series {
    pub samples: Vec<Sample>,
}

impl Series {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, s: Sample) {
        self.samples.push(s);
    }

    /// Newest emitter age in the series, for rendering alongside the number.
    pub fn emitter_age_secs(&self) -> Option<u64> {
        self.samples.iter().rev().find_map(|s| s.emitter_age_secs)
    }

    /// The most recent readable value.
    pub fn latest(&self) -> Option<f64> {
        self.samples.iter().rev().find_map(|s| s.value)
    }

    /// Decide what this series is.
    pub fn classify(&self) -> Verdict {
        let mut ordered = self.samples.clone();
        ordered.sort_by_key(|s| s.ts_ms);

        let readable: Vec<&Sample> = ordered.iter().filter(|s| s.value.is_some()).collect();
        if readable.is_empty() {
            // Distinguish "no data at all" from "data that was all unreadable":
            // only the latter is a blind sensor worth warning about.
            return if ordered.is_empty() {
                Verdict::TooFew { have: 0 }
            } else {
                Verdict::Blind
            };
        }
        if readable.len() < MIN_SAMPLES {
            return Verdict::TooFew { have: readable.len() };
        }

        let span_secs = ((readable[readable.len() - 1].ts_ms - readable[0].ts_ms) / 1000) as u64;
        let first = readable[0].value.unwrap();
        let last = readable[readable.len() - 1].value.unwrap();

        let mut ever_fell = false;
        let mut ever_rose = false;
        for pair in readable.windows(2) {
            let (a, b) = (pair[0].value.unwrap(), pair[1].value.unwrap());
            if b < a {
                ever_fell = true;
            }
            if b > a {
                ever_rose = true;
            }
        }

        match (ever_fell, ever_rose) {
            // Moves both ways — the only shape a rate has.
            (true, _) => Verdict::Rate,
            // Only ever climbs: cumulative.
            (false, true) => Verdict::Counter { rose_by: last - first, over_secs: span_secs },
            // Never moved at all.
            (false, false) => Verdict::Stuck { at: last, over_secs: span_secs },
        }
    }

    /// The number with everything needed to judge it attached — never naked.
    pub fn render(&self, label: &str, window_secs: Option<u64>) -> String {
        let verdict = self.classify();
        let value = match self.latest() {
            Some(v) => format!("{v:.1}"),
            None => "—".to_string(),
        };
        let window = match window_secs {
            Some(w) => format!("over {w}s"),
            None => "window UNSTATED".to_string(),
        };
        let age = match self.emitter_age_secs() {
            Some(a) => format!("emitter age {a}s"),
            None => "emitter age unknown".to_string(),
        };
        let mut out = format!("{label}: {value} · {window} · {age}");
        if let Some(c) = verdict.caveat() {
            out.push_str("\n    ");
            out.push_str(&c);
        }
        out
    }
}

/// Split readings by emitter and classify each separately.
///
/// Pooling emitters is the trap: a restart resets a tally, so the pooled series
/// shows a fall that reads as a healthy rate coming back down.
pub fn classify_by_emitter(readings: &[(String, Sample)]) -> BTreeMap<String, Verdict> {
    let mut by: BTreeMap<String, Series> = BTreeMap::new();
    for (emitter, s) in readings {
        by.entry(emitter.clone()).or_default().push(s.clone());
    }
    by.into_iter().map(|(k, v)| (k, v.classify())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(ts_secs: u128, v: Option<f64>, age: Option<u64>) -> Sample {
        Sample { ts_ms: ts_secs * 1000, value: v, emitter_age_secs: age }
    }

    /// The shape actually observed on the live plane: a "per minute" reading
    /// that climbed monotonically for the whole span and never once fell.
    #[test]
    fn since_window_produces_an_epoch_not_a_duration() {
        let five_min = since_window(Duration::from_secs(300));
        // An epoch, not the 300_000 that a straight pass-through would give.
        assert!(five_min > 1_600_000_000_000, "got {five_min}, which is duration-shaped");
        let now = since_window(Duration::ZERO);
        let delta = now.saturating_sub(five_min);
        assert!((299_000..=301_000).contains(&delta), "delta was {delta}");
    }

    #[test]
    fn a_monotone_ramp_is_a_counter_not_a_rate() {
        let mut ser = Series::new();
        for (i, v) in [58.2, 58.2, 58.2, 58.4, 60.0, 60.8, 66.2, 71.8, 78.4]
            .iter()
            .enumerate()
        {
            ser.push(s(1000 + i as u128 * 61, Some(*v), Some(53617 + i as u64 * 61)));
        }
        match ser.classify() {
            Verdict::Counter { rose_by, .. } => assert!((rose_by - 20.2).abs() < 0.01),
            other => panic!("expected Counter, got {other:?}"),
        }
        assert!(!ser.classify().is_thresholdable());
        assert!(ser.classify().caveat().unwrap().contains("NOT A RATE"));
    }

    /// The original symptom: identical across consecutive samples.
    #[test]
    fn a_value_that_does_not_move_while_the_clock_does_is_stuck() {
        let mut ser = Series::new();
        for i in 0..6 {
            ser.push(s(1000 + i * 60, Some(97.0), Some(600)));
        }
        match ser.classify() {
            Verdict::Stuck { at, over_secs } => {
                assert_eq!(at, 97.0);
                assert_eq!(over_secs, 300);
            }
            other => panic!("expected Stuck, got {other:?}"),
        }
    }

    #[test]
    fn a_series_that_moves_both_ways_is_a_rate() {
        let mut ser = Series::new();
        for (i, v) in [4.0, 7.5, 3.2, 9.1, 5.5, 2.0].iter().enumerate() {
            ser.push(s(1000 + i as u128 * 60, Some(*v), Some(900)));
        }
        assert_eq!(ser.classify(), Verdict::Rate);
        assert!(ser.classify().is_thresholdable());
        assert!(ser.classify().caveat().is_none());
    }

    /// The newest build read `null` on every sample. That is a blind sensor,
    /// and calling it zero would turn it into an all-clear.
    #[test]
    fn all_unreadable_is_blind_not_zero() {
        let mut ser = Series::new();
        for i in 0..5 {
            ser.push(s(1000 + i * 60, None, Some(300)));
        }
        assert_eq!(ser.classify(), Verdict::Blind);
        assert!(!ser.classify().is_thresholdable());
        assert!(ser.classify().caveat().unwrap().contains("not a zero"));
        assert_eq!(ser.latest(), None);
    }

    #[test]
    fn too_few_samples_says_so_rather_than_guessing() {
        let mut ser = Series::new();
        ser.push(s(1000, Some(5.0), None));
        ser.push(s(1060, Some(6.0), None));
        assert_eq!(ser.classify(), Verdict::TooFew { have: 2 });
        assert!(!ser.classify().is_thresholdable());
    }

    #[test]
    fn an_empty_series_is_not_a_blind_sensor() {
        assert_eq!(Series::new().classify(), Verdict::TooFew { have: 0 });
    }

    /// Pooling two emitters hides the ramp: the restart looks like a fall.
    #[test]
    fn emitters_must_not_be_pooled() {
        // Old process ramping high, new process ramping from zero.
        let readings: Vec<(String, Sample)> = vec![
            ("old".into(), s(1000, Some(300.0), Some(50_000))),
            ("old".into(), s(1061, Some(310.0), Some(50_061))),
            ("old".into(), s(1122, Some(320.0), Some(50_122))),
            ("old".into(), s(1183, Some(330.0), Some(50_183))),
            ("new".into(), s(1200, Some(0.2), Some(60))),
            ("new".into(), s(1261, Some(0.6), Some(121))),
            ("new".into(), s(1322, Some(1.0), Some(182))),
            ("new".into(), s(1383, Some(1.6), Some(243))),
        ];

        // Pooled, the old→new handover reads as a fall, so it passes as a rate.
        let mut pooled = Series::new();
        for (_, sample) in &readings {
            pooled.push(sample.clone());
        }
        assert_eq!(pooled.classify(), Verdict::Rate, "pooling hides the ramp");

        // Split by emitter, both are correctly caught as counters.
        let by = classify_by_emitter(&readings);
        assert!(matches!(by["old"], Verdict::Counter { .. }));
        assert!(matches!(by["new"], Verdict::Counter { .. }));
    }

    #[test]
    fn render_never_shows_a_number_without_window_and_age() {
        let mut ser = Series::new();
        for (i, v) in [10.0, 20.0, 30.0, 40.0].iter().enumerate() {
            ser.push(s(1000 + i as u128 * 61, Some(*v), Some(3600)));
        }
        let out = ser.render("ui_blocks_per_min", Some(300));
        assert!(out.contains("40.0"));
        assert!(out.contains("over 300s"));
        assert!(out.contains("emitter age 3600s"));
        assert!(out.contains("NOT A RATE"));

        // An unstated window is said out loud rather than assumed.
        assert!(ser.render("x", None).contains("window UNSTATED"));
    }
}
