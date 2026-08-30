//! The Dash complaint view — ytrace incidents, read the way an operator needs them.
//!
//! Three things go wrong when an incident stream is rendered as a flat list, and
//! all three were observed on a live plane before this module existed:
//!
//! 1. **A count of samples reads as a count of problems.** One non-clearing
//!    condition re-sampled every minute becomes hundreds of "errors". The
//!    operator sees the total and believes there are hundreds of things wrong.
//! 2. **Emitters get pooled.** Several builds of the same app write into one
//!    stream on one host. Their readings interleave, and a series that is
//!    climbing inside every single process looks like it moves both ways once
//!    they are mixed — see [`crate::rate`].
//! 3. **Numbers are rendered naked.** A value with no window and no emitter age
//!    beside it cannot be judged, and the one that flatters a change just
//!    shipped is the freshly restarted process reading low.
//!
//! So this view collapses by condition, splits by emitter, and never prints a
//! quantity without what is needed to weigh it.

use crate::rate::{classify_by_emitter, since_window, Sample, Verdict};
use std::collections::BTreeMap;
use std::time::Duration;

/// One condition, however many times it was sampled.
#[derive(Debug, Clone)]
pub struct Condition {
    pub incident_id: String,
    pub severity: String,
    /// How many records this one condition accounts for.
    pub samples: usize,
    /// Distinct emitters, newest-known age first: `(pid, version, age_secs)`.
    pub emitters: Vec<(u32, String, Option<u64>)>,
    /// The most recent diagnosis line.
    pub diagnosis: String,
    /// Span covered, in seconds.
    pub span_secs: u64,
    /// Per-observed-field verdicts, keyed `field` → per-emitter verdict.
    pub field_verdicts: BTreeMap<String, BTreeMap<String, Verdict>>,
    /// Newest reading per emitter per field: `field` → `emitter` → (value, age).
    pub latest_per_emitter: BTreeMap<String, BTreeMap<String, (Option<f64>, Option<u64>)>>,
}

/// Whether a field's NAME promises a rate.
///
/// This is the half that does the misleading. A field called `sustained_secs`
/// climbs forever and deceives nobody, because its name says so. A field called
/// `blocks_per_min` that climbs forever is a tally wearing a rate's name, and a
/// threshold sitting on it will arm once and never disarm.
pub fn names_a_rate(field: &str) -> bool {
    let f = field.to_ascii_lowercase();
    matches!(f.as_str(), "per_min" | "per_sec" | "per_minute" | "per_second" | "rate" | "density" | "hz")
        || f.ends_with("_per_min")
        || f.ends_with("_per_sec")
        || f.ends_with("_per_s")
        || f.ends_with("_rate")
        || f.starts_with("rate_")
        || f.contains("_density")
        || f.ends_with("per_minute")
        || f.ends_with("_hz")
}

impl Condition {
    /// Fields NAMED like a rate that no emitter shows behaving as one.
    ///
    /// Requires every emitter to disagree with the name: one process genuinely
    /// producing a rate is enough to say the field can work, and the problem is
    /// then that emitter rather than the metric.
    pub fn untrustworthy_fields(&self) -> Vec<(&str, &Verdict)> {
        self.field_verdicts
            .iter()
            .filter(|(field, _)| names_a_rate(field))
            .filter_map(|(field, per_emitter)| {
                if per_emitter.is_empty() || per_emitter.values().any(|v| v.is_thresholdable()) {
                    return None;
                }
                per_emitter.values().next().map(|v| (field.as_str(), v))
            })
            .collect()
    }

    /// Rate-named fields where simultaneous emitters report incompatible values.
    ///
    /// This is the falsifier that does not depend on how wide a window is. When
    /// several processes on one host measure the same host-scoped quantity at
    /// the same moment and disagree by orders of magnitude, at least all but one
    /// are wrong — a host does not have four different block rates at once.
    ///
    /// Monotonicity cannot catch this on its own: read over an hour, a tally
    /// that is pruned by log retention falls often enough to pass as a rate. The
    /// spread between emitters stays visible at every window, and its ordering
    /// against emitter age names the mechanism — the oldest process reports the
    /// largest number because it has had longest to accumulate.
    ///
    /// Returns `(field, readings)` with readings newest-per-emitter, largest
    /// first, as `(emitter, value, age_secs)`.
    pub fn disagreeing_rate_fields(&self) -> Vec<(&str, Vec<(String, f64, Option<u64>)>)> {
        /// Below this spread, ordinary sampling jitter explains the difference.
        const SUSPICIOUS_SPREAD: f64 = 20.0;

        let mut out = Vec::new();
        for (field, per_emitter) in &self.field_verdicts {
            if !names_a_rate(field) || per_emitter.len() < 2 {
                continue;
            }
            let mut readings: Vec<(String, f64, Option<u64>)> = self
                .latest_per_emitter
                .get(field)
                .map(|m| {
                    m.iter()
                        .filter_map(|(e, (v, age))| v.map(|v| (e.clone(), v, *age)))
                        .collect()
                })
                .unwrap_or_default();
            if readings.len() < 2 {
                continue;
            }
            readings.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let max = readings[0].1;
            let min = readings[readings.len() - 1].1;
            // Zero as the floor still counts: one emitter seeing nothing while
            // another sees hundreds is the same contradiction.
            let spread_is_suspicious = if min > 0.0 {
                max / min >= SUSPICIOUS_SPREAD
            } else {
                max >= SUSPICIOUS_SPREAD
            };
            if spread_is_suspicious {
                out.push((field.as_str(), readings));
            }
        }
        out
    }

    /// Fields not named like a rate that only ever climb.
    ///
    /// These are LEVELS. A level is not wrong to climb, but a threshold on one
    /// alarms until something reclaims it — and if nothing does, it alarms
    /// forever. Worth stating; not worth shouting, because the name did not lie.
    pub fn cumulative_fields(&self) -> Vec<&str> {
        self.field_verdicts
            .iter()
            .filter(|(field, _)| !names_a_rate(field) && *field != "sustained_secs")
            .filter(|(_, per_emitter)| {
                !per_emitter.is_empty()
                    && per_emitter.values().all(|v| matches!(v, Verdict::Counter { .. }))
            })
            .map(|(field, _)| field.as_str())
            .collect()
    }
}

/// Roll a raw incident stream up into conditions.
pub fn summarize_incidents(records: &[ytrace::YtraceRecord]) -> Vec<Condition> {
    let mut by_id: BTreeMap<String, Vec<&ytrace::YtraceRecord>> = BTreeMap::new();
    for r in records {
        let id = r
            .payload
            .get("incident_id")
            .and_then(|v| v.as_str())
            .unwrap_or("(unnamed)")
            .to_string();
        by_id.entry(id).or_default().push(r);
    }

    let mut out = Vec::new();
    for (incident_id, mut recs) in by_id {
        recs.sort_by_key(|r| r.ts_ms);

        // Emitters, in first-seen order, carrying the newest age each reported.
        let mut emitters: Vec<(u32, String, Option<u64>)> = Vec::new();
        for r in &recs {
            let age = r
                .payload
                .get("observed")
                .and_then(|o| o.get("sustained_secs"))
                .and_then(|v| v.as_u64());
            match emitters.iter_mut().find(|(p, _, _)| *p == r.pid) {
                Some(e) => {
                    if age.is_some() {
                        e.2 = age;
                    }
                }
                None => emitters.push((r.pid, r.app_version.clone(), age)),
            }
        }

        // Every numeric field under `observed`, as a per-emitter series.
        let mut fields: BTreeMap<String, Vec<(String, Sample)>> = BTreeMap::new();
        for r in &recs {
            let Some(obs) = r.payload.get("observed").and_then(|o| o.as_object()) else {
                continue;
            };
            for (field, value) in obs {
                // A null here is an unreadable sensor, and must stay unreadable
                // rather than becoming a zero. Non-numerics are not quantities.
                if !(value.is_number() || value.is_null()) {
                    continue;
                }
                fields.entry(field.clone()).or_default().push((
                    format!("pid{}", r.pid),
                    Sample {
                        ts_ms: r.ts_ms,
                        value: value.as_f64(),
                        emitter_age_secs: obs.get("sustained_secs").and_then(|v| v.as_u64()),
                    },
                ));
            }
        }
        let mut latest_per_emitter: BTreeMap<String, BTreeMap<String, (Option<f64>, Option<u64>)>> =
            BTreeMap::new();
        for (field, readings) in &fields {
            let per = latest_per_emitter.entry(field.clone()).or_default();
            for (emitter, sample) in readings {
                // Readings arrive in timestamp order, so the last write wins.
                per.insert(emitter.clone(), (sample.value, sample.emitter_age_secs));
            }
        }
        let field_verdicts = fields
            .into_iter()
            .map(|(field, readings)| (field, classify_by_emitter(&readings)))
            .collect();

        let span_secs = recs
            .last()
            .zip(recs.first())
            .map(|(l, f)| ((l.ts_ms - f.ts_ms) / 1000) as u64)
            .unwrap_or(0);

        out.push(Condition {
            incident_id,
            severity: recs
                .last()
                .and_then(|r| r.payload.get("severity"))
                .and_then(|v| v.as_str())
                .unwrap_or("?")
                .to_string(),
            samples: recs.len(),
            emitters,
            diagnosis: recs
                .last()
                .and_then(|r| r.payload.get("diagnosis"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            span_secs,
            field_verdicts,
            latest_per_emitter,
        });
        }

    // Loudest first: most samples, since that is what dominates a naive count.
    out.sort_by(|a, b| b.samples.cmp(&a.samples));
    out
}

/// Read the live plane and roll it up. `window` bounds the query honestly.
pub fn read_live(app: &str, window: Duration) -> (Vec<Condition>, usize) {
    let home = ytrace::compat::resolve_home(app);
    let records = ytrace::query::incidents(&home, Some(since_window(window)));
    let total = records.len();
    (summarize_incidents(&records), total)
}

/// Render the complaint view as text for `--once`.
pub fn render(conditions: &[Condition], total_records: usize, window: Duration) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "\n  ── YTRACE COMPLAINTS · window {}s ─────────────────────────────────────\n",
        window.as_secs()
    ));
    if conditions.is_empty() {
        out.push_str("  no incidents in this window.\n");
        return out;
    }
    out.push_str(&format!(
        "  {} record(s) · {} distinct condition(s) — a count of samples is not a count of problems.\n\n",
        total_records,
        conditions.len()
    ));

    for c in conditions {
        let emitters: Vec<String> = c
            .emitters
            .iter()
            .map(|(pid, ver, age)| match age {
                Some(a) => format!("pid{pid}/{ver} age {a}s"),
                None => format!("pid{pid}/{ver}"),
            })
            .collect();
        out.push_str(&format!(
            "  [{}] {} · {} sample(s) over {}s · {} emitter(s)\n",
            c.severity.to_uppercase(),
            c.incident_id,
            c.samples,
            c.span_secs,
            c.emitters.len()
        ));
        if !c.diagnosis.is_empty() {
            out.push_str(&format!("      {}\n", c.diagnosis));
        }
        out.push_str(&format!("      emitters: {}\n", emitters.join(" · ")));

        for (field, verdict) in c.untrustworthy_fields() {
            if let Some(caveat) = verdict.caveat() {
                out.push_str(&format!("      {field} → {caveat}\n"));
            }
        }
        for (field, readings) in c.disagreeing_rate_fields() {
            let shown: Vec<String> = readings
                .iter()
                .map(|(e, v, age)| match age {
                    Some(a) => format!("{e}={v:.1} (age {a}s)"),
                    None => format!("{e}={v:.1}"),
                })
                .collect();
            out.push_str(&format!(
                "      {field} → ⛔ EMITTERS DISAGREE {}× — {}\n\
                 \x20         one host, one value. The spread tracks emitter age, which is\n\
                 \x20         what a cumulative tally divided by an assumed window does.\n",
                (readings[0].1 / readings[readings.len() - 1].1.max(1e-9)).round() as i64,
                shown.join(" · ")
            ));
        }
        let levels = c.cumulative_fields();
        if !levels.is_empty() {
            out.push_str(&format!(
                "      level(s) that only climb: {} — a threshold here stays armed \n\
                 \x20     until something reclaims them.\n",
                levels.join(", ")
            ));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rec(pid: u32, ver: &str, ts_ms: u128, id: &str, obs: serde_json::Value) -> ytrace::YtraceRecord {
        ytrace::YtraceRecord {
            v: 1,
            ts_ms,
            pid,
            app: "demoapp".into(),
            app_version: ver.into(),
            component: "heartbeat".into(),
            category: "heartbeat".into(),
            name: "panic".into(),
            clock: "wall".into(),
            duration_ms: None,
            payload: json!({
                "incident": true,
                "incident_id": id,
                "severity": "error",
                "diagnosis": "a condition that nothing clears",
                "observed": obs,
            }),
        }
    }

    /// The headline defect: many samples of one condition read as many problems.
    #[test]
    fn one_condition_sampled_often_is_still_one_condition() {
        let records: Vec<_> = (0..313)
            .map(|i| rec(100, "1.0.0", 1_000_000 + i * 61_000, "level_never_clears", json!({"held_bytes": 5_400_000_000u64})))
            .collect();
        let (conds, total) = (summarize_incidents(&records), records.len());
        assert_eq!(total, 313);
        assert_eq!(conds.len(), 1, "313 samples are one condition");
        assert_eq!(conds[0].samples, 313);

        let text = render(&conds, total, Duration::from_secs(86_400));
        assert!(text.contains("313 record(s) · 1 distinct condition(s)"));
        assert!(text.contains("a count of samples is not a count of problems"));
    }

    /// Two builds writing into one stream must not be averaged together.
    #[test]
    fn emitters_are_kept_apart_and_their_ages_shown() {
        let mut records = Vec::new();
        // Old process: a tally that has climbed for hours.
        for i in 0..6u128 {
            records.push(rec(777, "1.0.12", 1_000_000 + i * 61_000, "thrash",
                json!({"blocks_per_min": 300.0 + i as f64 * 10.0, "sustained_secs": 50_000 + i * 61})));
        }
        // Young process: same tally, still near zero because it just started.
        for i in 0..6u128 {
            records.push(rec(999, "1.0.23", 1_000_500 + i * 61_000, "thrash",
                json!({"blocks_per_min": 0.2 + i as f64 * 0.4, "sustained_secs": 60 + i * 61})));
        }

        let conds = summarize_incidents(&records);
        assert_eq!(conds.len(), 1);
        assert_eq!(conds[0].emitters.len(), 2, "two builds, two emitters");

        // Both are ramps, so neither may be compared against a threshold.
        let per_emitter = &conds[0].field_verdicts["blocks_per_min"];
        assert!(matches!(per_emitter["pid777"], Verdict::Counter { .. }));
        assert!(matches!(per_emitter["pid999"], Verdict::Counter { .. }));

        let text = render(&conds, records.len(), Duration::from_secs(3600));
        assert!(text.contains("NOT A RATE"));
        // The ages are what explain why one build "looks better" than the other.
        assert!(text.contains("age"), "emitter age must be rendered");
        assert!(text.contains("1.0.12") && text.contains("1.0.23"));
    }

    /// A null sensor must not be laundered into a zero.
    #[test]
    fn an_unreadable_sensor_is_reported_blind_not_clear() {
        let records: Vec<_> = (0..5u128)
            .map(|i| rec(42, "1.0.0", 1_000_000 + i * 61_000, "blindspot",
                json!({"blocks_per_min": serde_json::Value::Null, "sustained_secs": 60 + i * 61})))
            .collect();
        let conds = summarize_incidents(&records);
        assert!(matches!(conds[0].field_verdicts["blocks_per_min"]["pid42"], Verdict::Blind));
        let text = render(&conds, records.len(), Duration::from_secs(600));
        assert!(text.contains("UNREADABLE"));
        assert!(text.contains("not a zero"));
    }

    /// A field that genuinely behaves like a rate must not be flagged.
    #[test]
    fn a_real_rate_draws_no_caveat() {
        let vals = [4.0, 7.5, 3.2, 9.1, 5.5, 2.0];
        let records: Vec<_> = vals.iter().enumerate()
            .map(|(i, v)| rec(7, "1.0.0", 1_000_000 + i as u128 * 61_000, "honest",
                json!({"blocks_per_min": v, "sustained_secs": 60 + i as u64 * 61})))
            .collect();
        let conds = summarize_incidents(&records);
        assert_eq!(conds[0].field_verdicts["blocks_per_min"]["pid7"], Verdict::Rate);
        assert!(conds[0].untrustworthy_fields().is_empty());
        assert!(!render(&conds, records.len(), Duration::from_secs(600)).contains("NOT A RATE"));
    }

    #[test]
    fn only_fields_that_promise_a_rate_get_the_loud_caveat() {
        assert!(names_a_rate("blocks_per_min"));   // by suffix
        assert!(names_a_rate("per_min"));          // by bare name
        assert!(names_a_rate("ui_block_density_per_min"));
        assert!(names_a_rate("error_rate"));
        assert!(names_a_rate("frames_per_sec"));
        // These climb forever and mislead nobody, because the name says so.
        assert!(!names_a_rate("sustained_secs"));
        assert!(!names_a_rate("runtime_tmpfs_bytes"));
        assert!(!names_a_rate("swap_used_gib"));
    }

    /// A climbing LEVEL is reported, but not as a broken rate — the swap and
    /// tmpfs alarms are this shape: nothing reclaims them, so they never clear.
    #[test]
    fn a_climbing_level_is_noted_without_being_called_a_broken_rate() {
        let records: Vec<_> = (0..6u128)
            .map(|i| rec(5, "1.0.0", 1_000_000 + i * 61_000, "level_never_clears",
                json!({"held_bytes": 5_000_000_000u64 + i as u64 * 1_000_000,
                       "sustained_secs": 60 + i * 61})))
            .collect();
        let conds = summarize_incidents(&records);
        assert!(conds[0].untrustworthy_fields().is_empty(), "name never promised a rate");
        assert_eq!(conds[0].cumulative_fields(), vec!["held_bytes"]);
        let text = render(&conds, records.len(), Duration::from_secs(600));
        assert!(!text.contains("NOT A RATE"));
        assert!(text.contains("only climb"));
        assert!(text.contains("held_bytes"));
    }

    /// The window-independent falsifier: four processes on one host reporting
    /// the same host-scoped quantity at the same moment, disagreeing 135-fold,
    /// with the spread ordered by how long each has been running.
    #[test]
    fn simultaneous_emitters_cannot_disagree_about_one_host() {
        let mut records = Vec::new();
        // Oldest process: a tally that has had 17 hours to accumulate.
        for i in 0..5u128 {
            records.push(rec(777, "1.0.12", 1_000_000 + i * 61_000, "thrash",
                json!({"blocks_per_min": 351.6, "sustained_secs": 63_000_u64 + (i as u64) * 61})));
        }
        // Three young processes on the SAME host, same instant, near zero.
        for (pid, ver, v, age) in [(211u32, "1.0.21", 0.2, 1515u64), (222, "1.0.22", 2.6, 1030), (233, "1.0.23", 2.6, 1757)] {
            for i in 0..5u128 {
                records.push(rec(pid, ver, 1_000_500 + i * 61_000, "thrash",
                    json!({"blocks_per_min": v, "sustained_secs": age + (i as u64) * 61})));
            }
        }

        let conds = summarize_incidents(&records);
        let disagreements = conds[0].disagreeing_rate_fields();
        assert_eq!(disagreements.len(), 1, "blocks_per_min must be flagged");
        let (field, readings) = &disagreements[0];
        assert_eq!(*field, "blocks_per_min");
        assert_eq!(readings.len(), 4);
        // Largest first, and it is the oldest emitter.
        assert_eq!(readings[0].0, "pid777");
        assert!((readings[0].1 - 351.6).abs() < 0.01);
        assert_eq!(readings[0].2, Some(63_000_u64 + 4 * 61));

        let text = render(&conds, records.len(), Duration::from_secs(3600));
        assert!(text.contains("EMITTERS DISAGREE"));
        assert!(text.contains("one host, one value"));
    }

    /// Emitters that broadly agree are ordinary sampling noise, not a defect.
    #[test]
    fn emitters_that_agree_are_left_alone() {
        let mut records = Vec::new();
        for (pid, v) in [(1u32, 5.0), (2, 5.5), (3, 4.5)] {
            for i in 0..5u128 {
                records.push(rec(pid, "1.0.0", 1_000_000 + i * 61_000, "fine",
                    json!({"blocks_per_min": v, "sustained_secs": 600 + i * 61})));
            }
        }
        assert!(summarize_incidents(&records)[0].disagreeing_rate_fields().is_empty());
    }

    /// One emitter blind while another reports hundreds is the same contradiction.
    #[test]
    fn a_zero_against_a_large_reading_still_counts_as_disagreement() {
        let mut records = Vec::new();
        for i in 0..5u128 {
            records.push(rec(1, "1.0.0", 1_000_000 + i * 61_000, "thrash",
                json!({"blocks_per_min": 300.0, "sustained_secs": 50_000_u64 + (i as u64) * 61})));
            records.push(rec(2, "1.0.0", 1_000_100 + i * 61_000, "thrash",
                json!({"blocks_per_min": 0.0, "sustained_secs": 120_u64 + (i as u64) * 61})));
        }
        assert_eq!(summarize_incidents(&records)[0].disagreeing_rate_fields().len(), 1);
    }

    /// Ages differ legitimately between processes — never flag that as a defect.
    #[test]
    fn differing_emitter_ages_are_not_themselves_a_disagreement() {
        let mut records = Vec::new();
        for (pid, age) in [(1u32, 60u64), (2, 60_000)] {
            for i in 0..5u128 {
                records.push(rec(pid, "1.0.0", 1_000_000 + i * 61_000, "c",
                    json!({"sustained_secs": age + (i as u64) * 61})));
            }
        }
        // sustained_secs is not rate-named, so it is never compared this way.
        assert!(summarize_incidents(&records)[0].disagreeing_rate_fields().is_empty());
    }

    #[test]
    fn conditions_are_ordered_by_how_much_noise_they_make() {
        let mut records = Vec::new();
        for i in 0..3u128 {
            records.push(rec(1, "1.0.0", 1_000_000 + i * 1000, "quiet", json!({})));
        }
        for i in 0..40u128 {
            records.push(rec(1, "1.0.0", 1_000_000 + i * 1000, "loud", json!({})));
        }
        let conds = summarize_incidents(&records);
        assert_eq!(conds[0].incident_id, "loud");
        assert_eq!(conds[1].incident_id, "quiet");
    }

    #[test]
    fn an_empty_window_says_so_rather_than_looking_healthy() {
        let text = render(&[], 0, Duration::from_secs(300));
        assert!(text.contains("no incidents in this window"));
    }
}
