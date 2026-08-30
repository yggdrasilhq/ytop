//! SysInternals — the supervision system, rendered rather than re-decided.
//!
//! ⛔ THE BOUNDARY THIS FILE KEEPS, and it is the one `booter.rs` keeps for the
//! same reason: **MEMBERSHIP IS A FACT, DUENESS IS A JUDGEMENT.**
//!
//! Which row ids appear under `relay/booter/` and `relay/monitor/` is set
//! membership — two directory listings, with no rules to interpret and so none
//! to duplicate. Whether a subscription is DUE, LAPSED, deferred, or being
//! counted down as gone is the watchdogs' own reasoning; a second copy of it
//! here would drift and then disagree about a live row on the day it mattered.
//! So this module renders the two sets and the fields the stores themselves
//! wrote, and it names the verb that owns each verdict.
//!
//! ⭐ WHY THE ASYMMETRY IS WORTH A PAGE. A row can be armed on the booter and
//! subscribed to nothing: a stall still gets a wake, but if the wake does not
//! take, the escalation rings into an empty room. The reverse exists too — an
//! escalation target with nothing to wake the row in the first place. Both read
//! as one cheerful "⚡ Armed" chip in a single-plane view, which is exactly why
//! a single-plane view is the wrong instrument for this question.
//!
//! ⛔ AND A WATCHER THAT IS ALIVE IS NOT A WATCHER THAT IS AUDIBLE. The booter
//! writes a heartbeat carrying both its loop instant and the last instant it
//! managed to write to its log, because a process ticking into a closed file
//! looks perfectly healthy from the outside and supervises nobody.

use serde_json::{json, Value};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::rows::FleetRowsReport;

fn relay() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".yggterm")
        .join("relay")
}

fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or_default()
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}
fn f(v: &Value, k: &str) -> f64 {
    v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0)
}
fn i(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}
fn b(v: &Value, k: &str) -> bool {
    v.get(k).and_then(|x| x.as_bool()).unwrap_or(false)
}

/// `1h 04m` / `12m` / `41s` — an age a person can read without arithmetic.
pub fn ago(secs: f64) -> String {
    if secs < 0.0 {
        return "in the future".to_string();
    }
    let s = secs as u64;
    if s < 90 {
        format!("{s}s")
    } else if s < 5400 {
        format!("{}m", s / 60)
    } else if s < 172_800 {
        format!("{}h {:02}m", s / 3600, (s % 3600) / 60)
    } else {
        format!("{}d {}h", s / 86400, (s % 86400) / 3600)
    }
}

fn short(uuid: &str) -> String {
    uuid.chars().take(8).collect()
}

/// One line of a chart, normalised against its own maximum.
///
/// ⚠ THE SCALE IS PER-SERIES AND MUST BE PRINTED BESIDE IT. A sparkline with no
/// maximum beside it says only "this shape happened", and two of them stacked
/// invite a comparison the drawing cannot support.
pub fn spark(values: &[f64]) -> String {
    let max = values.iter().cloned().fold(0.0_f64, f64::max);
    if values.is_empty() {
        return "—".to_string();
    }
    if max <= 0.0 {
        return "▁".repeat(values.len());
    }
    values
        .iter()
        .map(|v| crate::schema::spark_char((v / max) * 100.0))
        .collect()
}

// ── the two subscription stores ───────────────────────────────────────────────

/// Everything the booter store itself recorded about one subscriber.
///
/// These are the store's OWN fields, not inferences from them: how many times it
/// has been booted, whether it already escalated, how many times the watcher
/// looked and found no row. What they add up to is the booter's business.
#[derive(Debug, Clone)]
pub struct Armed {
    pub uuid: String,
    pub campaign: String,
    pub kind: String,
    pub note: String,
    pub host: String,
    pub age_secs: f64,
    pub max_hours: f64,
    pub boots: i64,
    pub escalated: bool,
    pub blind_skips: i64,
    pub gone_sightings: i64,
    pub lapsed: bool,
    pub lapsed_reason: String,
}

/// Everything the monitor store recorded about one supervised row.
#[derive(Debug, Clone)]
pub struct Watched {
    pub uuid: String,
    pub seat: String,
    pub role: String,
    pub campaign: String,
    pub escalate_to: String,
    pub escalate_host: String,
    pub owner_pinned: bool,
    pub age_secs: f64,
    pub intent: String,
}

#[derive(Debug, Default)]
pub struct Planes {
    pub armed: Vec<Armed>,
    pub watched: Vec<Watched>,
    pub never_arm: Vec<String>,
    /// Stores that exist but could not be read — never silently counted as zero.
    pub unreadable: Vec<String>,
}

fn read_store(dir: &Path, unreadable: &mut Vec<String>) -> Vec<Value> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            unreadable.push(format!("{}: {e}", dir.display()));
            return out;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        match std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
            Some(v) => out.push(v),
            None => unreadable.push(
                path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            ),
        }
    }
    out
}

pub fn planes() -> Planes {
    let relay = relay();
    let now = now_secs();
    let mut p = Planes::default();

    for v in read_store(&relay.join("booter"), &mut p.unreadable) {
        let uuid = s(&v, "uuid");
        if uuid.is_empty() {
            continue;
        }
        p.armed.push(Armed {
            age_secs: now - f(&v, "subscribed_at"),
            uuid,
            campaign: s(&v, "campaign"),
            kind: s(&v, "kind"),
            note: s(&v, "note"),
            host: s(&v, "host"),
            max_hours: f(&v, "max_hours"),
            boots: i(&v, "boots"),
            escalated: b(&v, "escalated") || b(&v, "blind_escalated"),
            blind_skips: i(&v, "blind_skips"),
            gone_sightings: i(&v, "gone_sightings"),
            lapsed: b(&v, "lapsed"),
            lapsed_reason: s(&v, "lapsed_reason"),
        });
    }

    for v in read_store(&relay.join("monitor"), &mut p.unreadable) {
        let uuid = s(&v, "uuid");
        if uuid.is_empty() {
            continue;
        }
        p.watched.push(Watched {
            age_secs: now - f(&v, "since"),
            uuid,
            seat: s(&v, "seat"),
            role: s(&v, "role"),
            campaign: s(&v, "campaign"),
            escalate_to: s(&v, "escalate_to"),
            escalate_host: s(&v, "escalate_host"),
            owner_pinned: b(&v, "owner_pinned"),
            intent: s(&v, "intent"),
        });
    }

    // ⛔ never-arm is not a third plane, it is a DENIAL of both — the file
    //    asserts that a human types at this address, so a row on it must never
    //    be counted as an unsupervised gap.
    if let Ok(text) = std::fs::read_to_string(relay.join("never-arm.tsv")) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(first) = line.split('\t').next() {
                p.never_arm.push(short(first));
            }
        }
    }

    p.armed.sort_by(|a, b| a.campaign.cmp(&b.campaign).then(a.uuid.cmp(&b.uuid)));
    p.watched.sort_by(|a, b| a.seat.cmp(&b.seat).then(a.uuid.cmp(&b.uuid)));
    p
}

// ── the watchers, and whether they are still audible ──────────────────────────

/// A watcher's last-fired reading.
///
/// `verdict` is deliberately coarse — FRESH / STALE / SILENT / MUTE — because
/// the useful question is not "how many seconds" but "is anybody still doing
/// this job", and a bare timestamp makes a reader do that subtraction.
#[derive(Debug, Clone)]
pub struct Watcher {
    pub name: String,
    pub what_for: String,
    pub last_fired_secs: Option<f64>,
    pub cadence_secs: Option<f64>,
    pub cadence_source: &'static str,
    pub verdict: String,
    pub detail: String,
}

fn tail_text(path: &Path, bytes: u64) -> String {
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let from = len.saturating_sub(bytes);
    if file.seek(SeekFrom::Start(from)).is_err() {
        return String::new();
    }
    let mut buf = Vec::new();
    let _ = file.take(bytes).read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).to_string()
}

fn mtime_secs(path: &Path) -> Option<f64> {
    std::fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs_f64())
}

fn last_nonempty_line(text: &str) -> String {
    text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string()
}

/// `interval 3600s` as the watcher itself printed it — a declared cadence beats
/// an assumed one, and saying which is which is the point of `cadence_source`.
fn declared_interval(text: &str) -> Option<f64> {
    let idx = text.find("interval ")?;
    let rest = &text[idx + 9..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<f64>().ok()
}

fn verdict_for(age: Option<f64>, cadence: Option<f64>) -> String {
    let nominal = cadence.unwrap_or(300.0);
    match age {
        None => "NEVER SEEN — no log, so nothing here is a measurement".to_string(),
        Some(a) if a <= nominal * 2.0 => "FRESH".to_string(),
        Some(a) if a <= nominal * 6.0 => "STALE — one cadence has already been missed".to_string(),
        Some(_) => "SILENT — assume nobody is doing this job".to_string(),
    }
}

pub fn watchers() -> Vec<Watcher> {
    let relay = relay();
    let now = now_secs();
    let mut out = Vec::new();

    // ── the booter: the only watcher that reports alive AND audible ──────────
    let hb_path = relay.join("booter.heartbeat");
    let hb: Option<Value> = std::fs::read_to_string(&hb_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    let booter_log = relay.join("booter.log");
    let booter_cadence = declared_interval(&tail_text(&booter_log, 8192));
    match hb {
        Some(hb) => {
            let loop_age = now - f(&hb, "ts");
            let write_age = now - f(&hb, "last_log_write_ts");
            let up = now - f(&hb, "started_ts");
            // ⛔ ALIVE IS NOT AUDIBLE. A loop instant far fresher than the last
            //    log write is a watcher ticking into a file nobody can read.
            let mute = write_age > loop_age.max(60.0) * 6.0;
            out.push(Watcher {
                name: "booter watcher".to_string(),
                what_for: "kicks a stalled row that subscribed to it".to_string(),
                last_fired_secs: Some(loop_age),
                cadence_secs: booter_cadence,
                cadence_source: if booter_cadence.is_some() { "declared in its own log" } else { "assumed 300s" },
                verdict: if mute {
                    format!("⛔ MUTE — loop ticked {} ago but its log has not moved for {}", ago(loop_age), ago(write_age))
                } else {
                    verdict_for(Some(loop_age), booter_cadence)
                },
                detail: format!("pid {} · up {} · last log write {} ago", i(&hb, "pid"), ago(up), ago(write_age)),
            });
        }
        None => out.push(Watcher {
            name: "booter watcher".to_string(),
            what_for: "kicks a stalled row that subscribed to it".to_string(),
            last_fired_secs: mtime_secs(&booter_log).map(|m| now - m),
            cadence_secs: booter_cadence,
            cadence_source: "assumed",
            verdict: "⚠ no heartbeat file — the log's mtime is the only evidence".to_string(),
            detail: "`booter.heartbeat` absent: alive-but-mute cannot be told from dead".to_string(),
        }),
    }

    // ── the monitor: judgement, escalation, and no heartbeat of its own ──────
    let mon_log = relay.join("monitor.log");
    let mon_age = mtime_secs(&mon_log).map(|m| now - m);
    let episodes = std::fs::read_dir(relay.join("monitor-episodes"))
        .map(|d| d.flatten().count())
        .unwrap_or(0);
    out.push(Watcher {
        name: "monitor watcher".to_string(),
        what_for: "decides WHY a row is quiet, then wakes, relays or escalates".to_string(),
        last_fired_secs: mon_age,
        cadence_secs: declared_interval(&tail_text(&mon_log, 8192)),
        cadence_source: "assumed — it declares none",
        // ⚠ Deliberately weaker than the booter's line, because the evidence is
        //   weaker: mtime says the log moved, not that the loop is well.
        verdict: format!(
            "{}  ⚠ no heartbeat file — mtime cannot separate ALIVE-BUT-MUTE from DEAD",
            verdict_for(mon_age, None)
        ),
        detail: format!("{episodes} escalation episode(s) on file"),
    });

    // ── the roll watcher, and the fold sweep that rides along with it ────────
    let roll_log = relay.join("roll-watch.log");
    let roll_tail = tail_text(&roll_log, 65536);
    let roll_head = {
        let mut buf = String::new();
        if let Ok(mut fh) = std::fs::File::open(&roll_log) {
            let mut b = vec![0u8; 4096];
            if let Ok(n) = fh.read(&mut b) {
                buf = String::from_utf8_lossy(&b[..n]).to_string();
            }
        }
        buf
    };
    let roll_cadence = declared_interval(&roll_head).or_else(|| declared_interval(&roll_tail));
    let roll_age = mtime_secs(&roll_log).map(|m| now - m);
    out.push(Watcher {
        name: "roll watcher".to_string(),
        what_for: "notices main has moved past the running daemon and rolls the fleet".to_string(),
        last_fired_secs: roll_age,
        cadence_secs: roll_cadence,
        cadence_source: if roll_cadence.is_some() { "declared in its own log" } else { "assumed" },
        verdict: verdict_for(roll_age, roll_cadence),
        detail: last_nonempty_line(&roll_tail),
    });

    let fold_line = roll_tail
        .lines()
        .rev()
        .find(|l| l.contains("ygg-fold"))
        .unwrap_or("")
        .trim()
        .to_string();
    let folded = std::fs::read_dir(relay.join("folded")).map(|d| d.flatten().count()).unwrap_or(0);
    out.push(Watcher {
        name: "fold sweep".to_string(),
        what_for: "classifies finished, stalled and dead rows across all four planes".to_string(),
        last_fired_secs: roll_age,
        cadence_secs: roll_cadence,
        cadence_source: "rides the roll watcher's cadence",
        verdict: if fold_line.is_empty() {
            "NOT SEEN in the roll watcher's recent log".to_string()
        } else {
            verdict_for(roll_age, roll_cadence)
        },
        detail: format!("{folded} row(s) folded to date · last line: {fold_line}"),
    });

    out
}

// ── ytrace, bucketed into something with a shape ──────────────────────────────

fn ytrace_homes(provider: &str) -> Vec<PathBuf> {
    let mut homes = vec![ytrace::compat::resolve_home(provider)];
    if let Some(xdg) = dirs::home_dir().map(|h| h.join(".local").join("share").join("ytrace").join(provider)) {
        if !homes.contains(&xdg) && xdg.exists() {
            homes.push(xdg);
        }
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        let p = PathBuf::from(xdg).join("ytrace").join(provider);
        if !homes.contains(&p) && p.exists() {
            homes.push(p);
        }
    }
    homes
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}

/// Count records per equal-width bucket — the shape a count-over-time graph needs.
fn bucket_counts(ts: &[u128], window_ms: u128, buckets: usize) -> Vec<f64> {
    let now = now_ms();
    let start = now.saturating_sub(window_ms);
    let width = (window_ms / buckets.max(1) as u128).max(1);
    let mut out = vec![0.0; buckets];
    for t in ts {
        if *t < start {
            continue;
        }
        let idx = (((*t - start) / width) as usize).min(buckets - 1);
        out[idx] += 1.0;
    }
    out
}

/// Mean duration per bucket — a latency graph, where an empty bucket is 0 and
/// must be read as "nothing happened", never as "it got fast".
fn bucket_means(points: &[(u128, f64)], window_ms: u128, buckets: usize) -> Vec<f64> {
    let now = now_ms();
    let start = now.saturating_sub(window_ms);
    let width = (window_ms / buckets.max(1) as u128).max(1);
    let mut sums = vec![0.0; buckets];
    let mut counts = vec![0.0; buckets];
    for (t, v) in points {
        if *t < start {
            continue;
        }
        let idx = (((*t - start) / width) as usize).min(buckets - 1);
        sums[idx] += v;
        counts[idx] += 1.0;
    }
    sums.iter().zip(counts.iter()).map(|(s, c)| if *c > 0.0 { s / c } else { 0.0 }).collect()
}

// ── the live blocks a notebook page can ask for ───────────────────────────────

/// ⭐ THE ARMINGS, ON BOTH PLANES AT ONCE.
///
/// The join is by row id and nothing else. Every column is a field one of the
/// two stores wrote about itself; the reading of what the asymmetry MEANS is
/// left to the verb that owns it, and named at the bottom of the block.
pub fn armings_md(report: &FleetRowsReport) -> String {
    let p = planes();
    let armed_ids: std::collections::BTreeSet<&str> = p.armed.iter().map(|a| a.uuid.as_str()).collect();
    let watched_ids: std::collections::BTreeSet<&str> = p.watched.iter().map(|w| w.uuid.as_str()).collect();

    let both: Vec<&Armed> = p.armed.iter().filter(|a| watched_ids.contains(a.uuid.as_str())).collect();
    let booter_only: Vec<&Armed> = p.armed.iter().filter(|a| !watched_ids.contains(a.uuid.as_str())).collect();
    let monitor_only: Vec<&Watched> = p.watched.iter().filter(|w| !armed_ids.contains(w.uuid.as_str())).collect();

    // ⛔ A LAPSED SUBSCRIPTION IS STILL A FILE IN THE STORE, and counting it as
    //    membership would overstate the net badly. The booter writes `lapsed`
    //    with its own reason when it stops acting — max hours passed, or the row
    //    went missing from three listings running — and from that moment the row
    //    is armed on paper and watched by nothing. So the count is split rather
    //    than footnoted: a headline number that has to be corrected by a column
    //    further down is a headline number people quote wrong.
    let let_go = |a: &&&Armed| a.lapsed;
    let both_let_go = both.iter().filter(let_go).count();
    let only_let_go = booter_only.iter().filter(let_go).count();

    let mut md = String::from("## The two planes, right now\n\n");
    md.push_str("| plane | rows | of which the booter has let go | what a row in this state actually gets |\n| :--- | ---: | ---: | :--- |\n");
    md.push_str(&format!(
        "| ⚡🛡 **both** | **{}** | {} | a stall is woken, and if the wake does not take, somebody hears |\n",
        both.len(),
        both_let_go
    ));
    md.push_str(&format!(
        "| ⚡ **booter only** | **{}** | {} | it gets woken — the escalation has no address |\n",
        booter_only.len(),
        only_let_go
    ));
    md.push_str(&format!(
        "| 🛡 **monitor only** | **{}** | — | somebody would hear — but nothing wakes it first |\n",
        monitor_only.len()
    ));
    md.push_str(&format!(
        "| 🙋 **never-arm** | {} | — | a human types at this address; neither plane may touch it |\n",
        p.never_arm.len()
    ));
    if both_let_go + only_let_go > 0 {
        md.push_str(&format!(
            "\n⛔ **{} subscription(s) are on file that the booter has already stopped acting on.** \"Let go\" is not supervision: if that row is still live, the wake plane is gone and only the column below says so. The reason is per row, and the booter recorded it itself.\n",
            both_let_go + only_let_go
        ));
    }
    match &report.quota_hold {
        Some(h) => md.push_str(&format!("\n⏸ **Quota hold ACTIVE — {h}.** Every plane is standing down; a quiet fleet right now is the hold, not a fault.\n")),
        None => md.push_str("\n⚡ No quota hold. Both planes are free to act.\n"),
    }

    if !booter_only.is_empty() {
        md.push_str("\n### ⚡ Armed, escalating to nobody\n\n");
        md.push_str("> Not every row here is a gap, and the store says which. `gone` is the booter counting down a retired row — it will drop it on its own. `lapsed` has already expired. What remains is the real thing: a wake with no fallback.\n\n");
        md.push_str("| row | campaign | kind | age / max | boots | the store's own countdown | what it subscribed FOR |\n| :--- | :--- | :--- | ---: | ---: | :--- | :--- |\n");
        for a in booter_only.iter().take(14) {
            let countdown = if a.lapsed {
                format!("lapsed — {}", if a.lapsed_reason.is_empty() { "no reason recorded" } else { &a.lapsed_reason })
            } else if a.gone_sightings > 0 {
                format!("gone ×{} — being retired, not a gap", a.gone_sightings)
            } else if a.escalated {
                "escalated already — a human owns it".to_string()
            } else if a.blind_skips > 0 {
                format!("{} blind skip(s) — it could not be looked at", a.blind_skips)
            } else {
                "— none: this one is the real gap".to_string()
            };
            // ⭐ The note is the subscriber's own statement of purpose, and it is
            //    the only field on this row that can tell "nobody attached it"
            //    from "this one is meant to run alone" — which is the fork the
            //    reader is standing at.
            let purpose: String = match a.note.trim() {
                "" => "— none recorded".to_string(),
                n if n.chars().count() > 56 => format!("{}…", n.chars().take(56).collect::<String>()),
                n => n.to_string(),
            };
            let host = if a.host.is_empty() { String::new() } else { format!(" @{}", a.host) };
            md.push_str(&format!(
                "| `{}`{} | {} | {} | {} / {:.0}h | {} | {} | {} |\n",
                short(&a.uuid),
                host,
                if a.campaign.is_empty() { "—" } else { &a.campaign },
                if a.kind.is_empty() { "task" } else { &a.kind },
                ago(a.age_secs),
                a.max_hours,
                a.boots,
                countdown,
                purpose
            ));
        }
        if booter_only.len() > 14 {
            md.push_str(&format!("\n*{} more not listed — the page shows the first 14 by campaign.*\n", booter_only.len() - 14));
        }
    }

    if !monitor_only.is_empty() {
        md.push_str("\n### 🛡 Watched, but nothing will wake it\n\n");
        md.push_str("| row | seat | campaign | role | escalates to | age | intent |\n| :--- | :--- | :--- | :--- | :--- | ---: | :--- |\n");
        for w in monitor_only.iter().take(10) {
            // ⛔ A PINNED ROW IS NOT AN UNSUPERVISED ONE. The owner has taken it
            //    back by hand; every verb skips it deliberately, so counting it
            //    as a hole in the net would send somebody to arm a row a person
            //    is holding.
            let target = if w.owner_pinned {
                "🙋 owner-pinned — deliberately out of automation".to_string()
            } else if w.escalate_to.is_empty() {
                "—".to_string()
            } else if w.escalate_host.is_empty() {
                w.escalate_to.clone()
            } else {
                format!("{} @{}", w.escalate_to, w.escalate_host)
            };
            md.push_str(&format!(
                "| `{}` | {} | {} | {} | {} | {} | {} |\n",
                short(&w.uuid),
                if w.seat.is_empty() { "—" } else { &w.seat },
                if w.campaign.is_empty() { "—" } else { &w.campaign },
                if w.role.is_empty() { "—" } else { &w.role },
                target,
                ago(w.age_secs),
                if w.intent.is_empty() { "—" } else { &w.intent }
            ));
        }
    }

    if !p.unreadable.is_empty() {
        md.push_str(&format!(
            "\n⛔ **{} subscription file(s) could not be read.** They are not counted above, and \"I could not look\" is not \"it is not armed\": {}\n",
            p.unreadable.len(),
            p.unreadable.join(", ")
        ));
    }

    md.push_str("\n---\n\n⚖ **The counts here are membership; the verdict is not ytop's to give.** Whether a one-plane row is a gap, a deliberate stand-down or a corpse mid-countdown is the monitor's reasoning, and a second copy of it in this pane would drift. Run `ygg-monitor.py list` for the ruling, and `ygg-booter.py list --json --due` for who is about to be kicked.\n");
    md
}

/// The seat census — who is live, who is cold, and what a resume would cost.
pub fn census_md(report: &FleetRowsReport) -> String {
    let mut md = format!(
        "## The seat census — {} seat(s), {} live\n\n",
        report.total_rows, report.live_count
    );
    md.push_str(&format!(
        "Agent CPU **{:.1}%** · agent RAM **{:.1} MB** · context on disk **{:.1} MB** across every seat.\n\n",
        report.total_agent_cpu_pct, report.total_agent_rss_mb, report.total_transcript_mb
    ));

    if report.rows.is_empty() {
        md.push_str("> ⛔ No seats were read. That is not \"the fleet is empty\" — it is this host having no relay seat membership to read. Check `~/.yggterm/relay/seat-membership.json` before believing a zero.\n");
        return md;
    }

    let mut shown = 0usize;
    for (campaign, rows) in &report.campaigns {
        md.push_str(&format!("\n### {} — {} seat(s)\n\n", campaign, rows.len()));
        md.push_str("| seat | role | live | cpu | rss | context | last moved | supervision |\n| :--- | :--- | :--- | ---: | ---: | ---: | :--- | :--- |\n");
        for r in rows.iter().take(12) {
            shown += 1;
            md.push_str(&format!(
                "| `{}` | {} | {} | {:.1}% | {} MB | {:.1} MB | {} | {} |\n",
                r.seat,
                r.role,
                if r.is_alive { format!("LIVE ×{}", r.pids.len()) } else { "cold".to_string() },
                r.cpu_pct,
                r.rss_kb / 1024,
                r.transcript_size_kb as f64 / 1024.0,
                r.last_active_mtime,
                r.supervision_state
            ));
        }
        if rows.len() > 12 {
            md.push_str(&format!("\n*{} more seat(s) in this campaign not listed.*\n", rows.len() - 12));
        }
    }
    md.push_str(&format!("\n---\n\n{shown} seat(s) drawn. "));
    md.push_str("⛔ **A cold seat is not a dead seat and neither is a failure.** A lane that finished its work sits cold on purpose; the question this table answers is only *which*, so that the next reader does not prompt one that should have been harvested.\n");
    md
}

/// When each watcher last fired — the page that makes a stopped watcher visible.
pub fn watchers_md() -> String {
    let mut md = String::from("## When each last fired\n\n");
    md.push_str("> ⛔ **Silence is the failure mode with no symptom.** A watcher that has stopped produces exactly what a quiet fleet produces — nothing — so the only way to tell them apart is to look at the clock rather than the inbox.\n\n");
    md.push_str("| watcher | what it is for | last fired | cadence | verdict |\n| :--- | :--- | ---: | ---: | :--- |\n");
    for w in watchers() {
        md.push_str(&format!(
            "| **{}** | {} | {} | {} | {} |\n",
            w.name,
            w.what_for,
            w.last_fired_secs.map(|a| format!("{} ago", ago(a))).unwrap_or_else(|| "never".to_string()),
            w.cadence_secs
                .map(|c| format!("{} ({})", ago(c), w.cadence_source))
                .unwrap_or_else(|| w.cadence_source.to_string()),
            w.verdict
        ));
    }
    md.push_str("\n### What each one last said\n\n");
    for w in watchers() {
        if !w.detail.is_empty() {
            md.push_str(&format!("* **{}** — {}\n", w.name, w.detail));
        }
    }
    md.push_str("\n---\n\n⚖ **FRESH means the clock moved, not that the job was done well.** The booter is the only one of these that reports both its loop instant and the last instant it managed to write — the pair is what separates a healthy watcher from one ticking into a file nobody reads. The others have no heartbeat, so their row is weaker evidence and says so.\n");
    md
}

/// The booter's own action column, tallied — a wake ledger rather than a guess.
pub fn wakes_md() -> String {
    let path = relay().join("booter.log");
    let text = tail_text(&path, 512 * 1024);
    let ends = mtime_secs(&path).map(|m| ago(now_secs() - m)).unwrap_or_else(|| "unknown".to_string());

    // `HH:MM:SS ygg-booter <VERDICT> <age> <ACTION> <row> win=<n>` — the action
    // column is the watchdog saying what it DID, which is the only wake record
    // that exists: the watchers emit no ytrace of their own yet.
    let mut tally: std::collections::BTreeMap<String, (usize, String)> = std::collections::BTreeMap::new();
    let mut lines_seen = 0usize;
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 6 || f.get(1) != Some(&"ygg-booter") {
            continue;
        }
        let action = f[4];
        let row = f[5];
        if row.len() != 8 || !row.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        lines_seen += 1;
        if action == "-" {
            continue;
        }
        let e = tally.entry(action.to_string()).or_insert((0, String::new()));
        e.0 += 1;
        e.1 = f[0].to_string();
    }

    let mut md = String::from("## The wake ledger — what the booter did, not what it is armed for\n\n");
    md.push_str(&format!(
        "Window: the last 512 KB of the booter's log — **{lines_seen} classified row-passes**, ending {ends} ago. Stamps are the log's own clock.\n\n"
    ));
    if tally.is_empty() {
        md.push_str("> No actions in the window. Either the fleet has been calm, or the watcher is not writing — check the last-fired page before reading this as calm.\n");
        return md;
    }
    md.push_str("| action | passes | last at | what it means |\n| :--- | ---: | ---: | :--- |\n");
    let mut rows: Vec<_> = tally.into_iter().collect();
    rows.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
    for (action, (count, last)) in rows {
        let meaning = match action.as_str() {
            // ⛔ A BOOT THAT WAS NOT DELIVERED IS A FAILED WAKE WEARING THE WORD
            //    "BOOT". Folding it in with the successful ones would report the
            //    safety net as having caught something it did not.
            a if a.starts_with("BOOT#") && a.contains("NOT-DELIVERED") => "⛔ the wake was attempted and did not land — the row was NOT woken",
            a if a.starts_with("BOOT#") => "the wake itself — a PTY write, because the composer races the agent's own input",
            "BOOTING" => "a wake in flight",
            "ESCALATE" => "the wake did not take; this went to a human or an orchestrator",
            "RATE-LIMITED" => "the account cannot spend — the session is fine, so booting it would only burn a refused turn",
            "HOLD:rate-limit" => "the fleet-wide hold is on; one sighting holds everybody",
            "NO-TRANSCRIPT" => "nothing to classify — never read as idle",
            "SKIP:draft-race" => "the row had unsent text; typing would have raced a person",
            "CLEANER" => "boot-material residue cleared before booting clean",
            "SELF-HEAL" => "the watcher repaired its own state",
            _ => "—",
        };
        md.push_str(&format!("| `{action}` | {count} | {last} | {meaning} |\n"));
    }
    md.push_str("\n---\n\n⛔ **These counts come from a log, not from ytrace, and that is a gap rather than a design.** Every other pane on Dash is ytrace-backed; the two watchdogs are the one supervision surface that emits no spans, so a wake cannot be correlated against the UI block or the render cost it caused. Until they do, this table is parsed prose and should be read as such.\n");
    md
}

/// Cold and heavy — the rows a reader is tempted to prompt and should harvest.
pub fn cold_md(report: &FleetRowsReport) -> String {
    let mut cold: Vec<_> = report.rows.iter().filter(|r| !r.is_alive).collect();
    cold.sort_by(|a, b| b.transcript_size_kb.cmp(&a.transcript_size_kb));

    let mut md = format!(
        "## Cold seats, heaviest first — {} of {} seat(s) hold no process\n\n",
        cold.len(),
        report.total_rows
    );
    if cold.is_empty() {
        md.push_str("> Every seat holds a live process right now.\n");
        return md;
    }
    md.push_str("| seat | campaign | context | lines | last moved | supervision |\n| :--- | :--- | ---: | ---: | :--- | :--- |\n");
    for r in cold.iter().take(12) {
        let mb = r.transcript_size_kb as f64 / 1024.0;
        let chip = if mb > 30.0 { "🚨" } else if mb > 10.0 { "⚠️" } else { "" };
        md.push_str(&format!(
            "| `{}` | {} | {chip} {:.1} MB | {} | {} | {} |\n",
            r.seat, r.campaign, mb, r.transcript_lines, r.last_active_mtime, r.supervision_state
        ));
    }
    if cold.len() > 12 {
        md.push_str(&format!("\n*{} more cold seat(s) not listed.*\n", cold.len() - 12));
    }
    md
}

/// The roll ledger — what version each host was last moved to, and by whom.
pub fn rolls_md() -> String {
    let relay = relay();
    let ledger = relay.join("deploy-ledger.txt");
    let text = tail_text(&ledger, 64 * 1024);
    let mut md = String::from("## The roll ledger — the last deploys the fleet actually took\n\n");
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        md.push_str("> ⛔ No deploy ledger. That is not \"nothing has rolled\" — it is this host having no record, which is a different and worse thing.\n");
    } else {
        md.push_str("| when | lane | hosts | build | version |\n| :--- | :--- | :--- | :--- | :--- |\n");
        for line in lines.iter().rev().take(8) {
            let f: Vec<&str> = line.split_whitespace().collect();
            md.push_str(&format!(
                "| {} | {} | {} | `{}` | {} |\n",
                f.first().unwrap_or(&"—"),
                f.get(1).unwrap_or(&"—"),
                f.get(2).unwrap_or(&"—"),
                f.get(3).map(|s| &s[..s.len().min(12)]).unwrap_or("—"),
                f.get(4).unwrap_or(&"—")
            ));
        }
    }
    // ⚠ THE LEDGER AND THE WATCHER CAN DISAGREE, and that disagreement is the
    //   interesting reading: a watcher that ticked an hour ago beside a ledger
    //   whose newest line is days old means every tick decided there was nothing
    //   to roll — or that the rolling half has quietly stopped.
    if let Some(age) = mtime_secs(&ledger).map(|m| now_secs() - m) {
        md.push_str(&format!("\nNewest ledger entry was written **{} ago**.\n", ago(age)));
    }
    let roll_tail = tail_text(&relay.join("roll-watch.log"), 32 * 1024);
    let last = last_nonempty_line(&roll_tail);
    if !last.is_empty() {
        md.push_str(&format!("\n**The roll watcher's last word:** `{last}`\n"));
    }
    md
}

/// The fold ledger — every row that was retired across all four planes.
pub fn folds_md() -> String {
    let dir = relay().join("folded");
    let mut entries: Vec<(String, String, String, String)> = Vec::new(); // when, seat, verdict, uuid
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) != Some("md") {
                continue;
            }
            let text = match std::fs::read_to_string(&path) {
                Ok(t) => t,
                Err(_) => continue,
            };
            let mut when = String::new();
            let mut verdict = String::new();
            let mut seat = String::new();
            for line in text.lines() {
                let t = line.trim();
                if let Some(rest) = t.strip_prefix("# folded ") {
                    seat = rest.split('—').next().unwrap_or("").trim().to_string();
                } else if let Some(rest) = t.strip_prefix("* when:") {
                    when = rest.trim().to_string();
                } else if let Some(rest) = t.strip_prefix("* verdict:") {
                    verdict = rest.trim().to_string();
                }
            }
            let uuid = path.file_stem().map(|s| short(&s.to_string_lossy())).unwrap_or_default();
            entries.push((when, seat, verdict, uuid));
        }
    }
    entries.sort_by(|a, b| b.0.cmp(&a.0));

    let mut md = format!("## The fold ledger — {} row(s) retired across all four planes\n\n", entries.len());
    md.push_str("> A fold is four planes, not one: the row is delisted, the monitor's subscribers are moved off it, the booter is disarmed for it, and the agent process is reaped. `session remove` reports the REQUEST rather than the effect and routinely delists a row whose agent keeps running — which is why the fourth step exists at all.\n\n");
    if entries.is_empty() {
        md.push_str("> Nothing folded on this host yet.\n");
    } else {
        let mut by_verdict: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for (_, _, v, _) in &entries {
            let key = v.split(' ').next().unwrap_or("—").to_string();
            *by_verdict.entry(key).or_insert(0) += 1;
        }
        md.push_str("| verdict | folds |\n| :--- | ---: |\n");
        for (v, n) in &by_verdict {
            md.push_str(&format!("| {v} | {n} |\n"));
        }
        md.push_str("\n### Most recent\n\n| when | seat | verdict | row |\n| :--- | :--- | :--- | :--- |\n");
        for (when, seat, verdict, uuid) in entries.iter().take(8) {
            md.push_str(&format!(
                "| {} | `{}` | {} | `{}` |\n",
                if when.is_empty() { "—" } else { when },
                if seat.is_empty() { "—" } else { seat },
                if verdict.is_empty() { "—" } else { verdict },
                uuid
            ));
        }
    }
    let roll_tail = tail_text(&relay().join("roll-watch.log"), 32 * 1024);
    let worktrees: Vec<&str> = roll_tail.lines().rev().filter(|l| l.contains("ygg-fold ·")).take(6).collect();
    if !worktrees.is_empty() {
        md.push_str("\n### The worktree half of the same sweep\n\n");
        for w in worktrees.iter().rev() {
            md.push_str(&format!("* `{}`\n", w.trim()));
        }
        md.push_str("\n⛔ A worktree with unpushed commits or a process standing in it is KEPT, whatever the row's state — a fold may never be the thing that loses work.\n");
    }
    md
}

/// ── CI plane — the single integration build (like booter/monitor, on dev) ───────

fn ci_subs_raw() -> Vec<Value> {
    let dir = relay().join("ci").join("subs");
    let mut out = Vec::new();
    let mut unreadable = Vec::new();
    for v in read_store(&dir, &mut unreadable) {
        out.push(v);
    }
    out.sort_by(|a, b| {
        let pa = s(a, "project");
        let pb = s(b, "project");
        pa.cmp(&pb).then(s(a, "lane").cmp(&s(b, "lane")))
    });
    out
}

fn ci_builds_raw(limit: usize) -> Vec<Value> {
    let dir = relay().join("ci").join("builds");
    let mut out = Vec::new();
    let mut unreadable = Vec::new();
    for v in read_store(&dir, &mut unreadable) {
        out.push(v);
    }
    out.sort_by(|a, b| f(b, "at").partial_cmp(&f(a, "at")).unwrap_or(std::cmp::Ordering::Equal));
    out.truncate(limit);
    out
}

pub fn ci_subs_md(_report: &FleetRowsReport) -> String {
    let subs = ci_subs_raw();
    let cfg_path = relay().join("ci").join("ci.json");
    let cfg: Option<Value> = std::fs::read_to_string(&cfg_path).ok().and_then(|t| serde_json::from_str(&t).ok());
    let mut md = String::from("## Enrolled lanes — who asked for the next build\n\n");
    if subs.is_empty() {
        md.push_str("> No lanes enrolled right now. `ygg-ci.py subscribe --lane lane/foo --project yggterm` (on `dev`, after `git push origin lane/foo`) is what puts one here. The watcher wakes every 300s (same cadence as booter/monitor) and merges `origin/main` + subs into `~/.yggterm/scratchpad/ci/<project>/integ-<ts>`.\n\n");
    } else {
        md.push_str("| project | lane | tip | age | want | by |\n| :--- | :--- | :--- | ---: | :--- | :--- |\n");
        let now = now_secs();
        for v in subs.iter().take(24) {
            let lane = s(v, "lane");
            let project = s(v, "project");
            let tip = s(v, "tip_at_enlist");
            let want = s(v, "want");
            let by = short(&s(v, "by"));
            let age = now - f(v, "enlisted_at");
            md.push_str(&format!(
                "| {} | `{}` | {} | {} | {} | `{}` |\n",
                if project.is_empty() { "—" } else { &project },
                if lane.is_empty() { "—" } else { &lane },
                if tip.is_empty() { "—" } else { &tip[..tip.len().min(12)] },
                ago(age),
                if want.is_empty() { "next" } else { &want },
                if by.is_empty() { "—" } else { &by }
            ));
        }
        if subs.len() > 24 {
            md.push_str(&format!("\n*{} more not listed.*\n", subs.len() - 24));
        }
    }
    md.push_str("\n### Project recipe — how each project builds\n\n");
    if let Some(c) = cfg {
        if let Some(projs) = c.get("projects").and_then(|v| v.as_object()) {
            md.push_str("| project | repo | host | interval | build | deploy |\n| :--- | :--- | :--- | ---: | :--- | :--- |\n");
            for (name, v) in projs {
                md.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} |\n",
                    name,
                    s(v, "repo"),
                    s(v, "host"),
                    ago(f(v, "interval")),
                    s(v, "build").chars().take(48).collect::<String>(),
                    s(v, "deploy").chars().take(32).collect::<String>()
                ));
            }
        } else {
            md.push_str("> No projects in `ci.json`.\n");
        }
    } else {
        md.push_str(&format!("> No `ci.json` at `{}` — defaults are `yggterm` on `dev` every 300s.\n", cfg_path.display()));
    }
    let hold_path = relay().join("ci").join("ci.hold");
    if let Ok(t) = std::fs::read_to_string(&hold_path) {
        if let Ok(v) = serde_json::from_str::<Value>(&t) {
            let reason = s(&v, "reason");
            let by = s(&v, "by");
            md.push_str(&format!("\n⏸ **CI hold active** — {} by `{}`.\n", if reason.is_empty() { "no reason" } else { &reason }, by));
        } else if !t.trim().is_empty() {
            md.push_str(&format!("\n⏸ **CI hold active** — `{}`.\n", t.lines().next().unwrap_or("").trim()));
        }
    }
    let disarm_path = relay().join("ci").join("ci.disarmed");
    if disarm_path.exists() {
        if let Ok(t) = std::fs::read_to_string(&disarm_path) {
            if let Ok(v) = serde_json::from_str::<Value>(&t) {
                md.push_str(&format!("\n⛔ **CI disarmed** — {}.\n", s(&v, "note")));
            } else {
                md.push_str("\n⛔ **CI disarmed**\n");
            }
        }
    }
    md.push_str("\n---\n\n*Enroll:* `ssh dev ygg-ci.py subscribe --lane lane/foo --project yggterm` after pushing. *Leave:* `unsubscribe` when done; `tick --dry-run` shows what would merge without building.\n");
    md
}

pub fn ci_builds_md(_report: &FleetRowsReport) -> String {
    let builds = ci_builds_raw(12);
    let mut md = String::from("## Builds — what the last integrations produced\n\n");
    if builds.is_empty() {
        md.push_str("> No builds yet. The first `tick` after a subscription merges `origin/main` + lanes into an ephemeral worktree and `cargo build --release` once, then `scripts/deploy-fleet.sh` (proves `md5sum /proc/<pid>/exe` fleet-wide).\n\n");
        return md;
    }
    md.push_str("| when | project | sha | lanes | conflicts | status | build |\n| :--- | :--- | :--- | ---: | ---: | :--- | :--- |\n");
    for v in builds.iter().take(8) {
        let at = f(v, "at");
        let project = s(v, "project");
        let sha = s(v, "sha");
        let status = s(v, "status");
        let build = if b(v, "build_ok") { "ok" } else if v.get("build_ok").is_none() { "—" } else { "fail" };
        let lanes = v.get("lanes").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0);
        let conflicts = v.get("conflicts").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0);
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            ago(now_secs() - at),
            if project.is_empty() { "—" } else { &project },
            if sha.is_empty() { "—" } else { &sha[..sha.len().min(8)] },
            lanes,
            conflicts,
            if status.is_empty() { "—" } else { &status },
            build
        ));
    }
    // sparkline of builds over last 6h, bucketed per hour
    let window_ms: u128 = 6 * 60 * 60 * 1000;
    let buckets = 12usize;
    let mut ts: Vec<u128> = builds.iter().filter_map(|v| {
        let at = f(v, "at");
        if at > 0.0 { Some((at * 1000.0) as u128) } else { None }
    }).collect();
    ts.sort_unstable();
    let series = bucket_counts(&ts, window_ms, buckets);
    let peak = series.iter().cloned().fold(0.0_f64, f64::max);
    md.push_str(&format!("\n**Builds last 6h ({} buckets):** `{} ` peak {} — each bucket {}m.\n", buckets, spark(&series), peak as i64, window_ms as u64 / buckets as u64 / 60_000));
    // show conflicts detail for most recent
    if let Some(latest) = builds.first() {
        if let Some(confs) = latest.get("conflicts").and_then(|v| v.as_array()) {
            if !confs.is_empty() {
                md.push_str("\n### Latest conflicts — excluded only, fleet still shipped the merged subset\n\n");
                md.push_str("| lane | reason |\n| :--- | :--- |\n");
                for c in confs.iter().take(6) {
                    md.push_str(&format!("| `{}` | {} |\n", s(c, "lane"), s(c, "reason")));
                }
                md.push_str("\n*Fix:* `git fetch && rebase origin/main && push --force-with-lease`; next tick retries — do not `unsubscribe`.\n");
            }
        }
    }
    md
}

pub fn ci_watchers_md(_report: &FleetRowsReport) -> String {
    let hb_path = relay().join("ci").join("ci.heartbeat");
    let log_path = relay().join("ci").join("ci.log");
    let cfg_path = relay().join("ci").join("ci.json");
    let mut md = String::from("## CI watcher — when it last fired\n\n");
    let now = now_secs();
    let hb: Option<Value> = std::fs::read_to_string(&hb_path).ok().and_then(|t| serde_json::from_str(&t).ok());
    let log_age = mtime_secs(&log_path).map(|m| now - m);
    let hb_age = hb.as_ref().map(|h| now - f(h, "ts"));
    let write_age = hb.as_ref().map(|h| now - f(h, "last_log_write_ts"));
    let cadence: Option<f64> = std::fs::read_to_string(&cfg_path).ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("projects").and_then(|p| p.get("yggterm")).and_then(|p| p.get("interval")).and_then(|x| x.as_f64()).or(Some(300.0)));
    match hb {
        Some(h) => {
            let loop_age = now - f(&h, "ts");
            let w_age = now - f(&h, "last_log_write_ts");
            let up = now - f(&h, "started_ts");
            let mute = w_age > loop_age.max(60.0) * 6.0;
            md.push_str(&format!(
                "| watcher | last loop | last log write | up | cadence | verdict |\n| :--- | ---: | ---: | ---: | ---: | :--- |\n| **ci watcher** | {} ago | {} ago | {} | {} | {} |\n",
                ago(loop_age),
                ago(w_age),
                ago(up),
                cadence.map(|c| ago(c)).unwrap_or_else(|| "300s".to_string()),
                if mute { format!("⛔ MUTE — loop {} ago but log {} ago", ago(loop_age), ago(w_age)) } else { verdict_for(Some(loop_age), cadence) }
            ));
            md.push_str(&format!("\n* pid {} · host {} · log {} ago*\n", i(&h, "pid"), s(&h, "host"), ago(w_age)));
        }
        None => {
            md.push_str(&format!(
                "| watcher | last log | cadence | verdict |\n| :--- | ---: | ---: | :--- |\n| **ci watcher** | {} | {} | {} |\n",
                log_age.map(|a| format!("{} ago", ago(a))).unwrap_or_else(|| "never".to_string()),
                cadence.map(|c| ago(c)).unwrap_or_else(|| "300s".to_string()),
                if log_age.is_none() { "NEVER SEEN — no log".to_string() } else { verdict_for(log_age, cadence) }
            ));
            md.push_str("\n⚠ No `ci.heartbeat` — the log's mtime is the only evidence. A ticking loop with a mute log looks healthy from outside.\n");
        }
    }
    if let Some(a) = hb_age { let _ = a; }
    if let Some(a) = write_age { let _ = a; }
    // last build word
    let builds = ci_builds_raw(1);
    if let Some(b) = builds.first() {
        md.push_str(&format!("\n**Last build:** `{}` {} — {} merged, {} conflicts — {}\n", s(b, "id"), s(b, "project"), b.get("lanes").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0), b.get("conflicts").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0), s(b, "status")));
    }
    md.push_str("\n---\n\n*Timer, not a burn:* `watch` sleeps 300s between `fetch+stat` ticks; a clean tick costs no build.\n");
    md
}

/// Dedicated booter view — armed rows only, not the combined armings map.
pub fn booter_subs_md(report: &FleetRowsReport) -> String {
    let p = planes();
    let mut md = String::from("## Armed rows — booter subscriptions only\n\n");
    let mut armed: Vec<&Armed> = p.armed.iter().collect();
    armed.sort_by(|a, b| a.campaign.cmp(&b.campaign).then(a.uuid.cmp(&b.uuid)));
    if armed.is_empty() {
        md.push_str("> No booter subscriptions. `ygg-booter.py subscribe --campaign <name>` arms a row; `list` shows who is armed. The booter is a dumb timer — it types `continue` when a row goes quiet too long.\n");
    } else {
        md.push_str("| row | campaign | kind | age / max | boots | state | purpose |\n| :--- | :--- | :--- | ---: | ---: | :--- | :--- |\n");
        for a in armed.iter().take(20) {
            let state = if a.lapsed { format!("lapsed — {}", if a.lapsed_reason.is_empty() { "no reason" } else { &a.lapsed_reason }) } else if a.gone_sightings > 0 { format!("gone ×{} — retiring", a.gone_sightings) } else if a.escalated { "escalated".to_string() } else if a.blind_skips > 0 { format!("blind×{}", a.blind_skips) } else { "—".to_string() };
            let note = if a.note.is_empty() { "—".to_string() } else { a.note.chars().take(48).collect::<String>() };
            md.push_str(&format!(
                "| `{}` @{} | {} | {} | {} / {:.0}h | {} | {} | {} |\n",
                short(&a.uuid), a.host, if a.campaign.is_empty() { "—" } else { &a.campaign }, if a.kind.is_empty() { "task" } else { &a.kind }, ago(a.age_secs), a.max_hours, a.boots, state, note
            ));
        }
        if armed.len() > 20 { md.push_str(&format!("\n*{} more not listed.*\n", armed.len() - 20)); }
    }
    if !p.unreadable.is_empty() {
        md.push_str(&format!("\n⛔ {} unreadable: {}\n", p.unreadable.len(), p.unreadable.join(", ")));
    }
    // quota hold banner like armings
    match &report.quota_hold {
        Some(h) => md.push_str(&format!("\n⏸ **Quota hold ACTIVE — {h}** — booter standing down.\n")),
        None => md.push_str("\n⚡ No quota hold.\n"),
    }
    md
}

pub fn booter_watcher_md(_report: &FleetRowsReport) -> String {
    let mut md = String::from("## Booter watcher — dumb timer, outside the session\n\n");
    for w in watchers().into_iter().filter(|w| w.name.contains("booter")) {
        md.push_str(&format!("| watcher | last fired | cadence | verdict |\n| :--- | ---: | ---: | :--- |\n| **{}** | {} | {} | {} |\n\n*{}*\n", w.name, w.last_fired_secs.map(|a| format!("{} ago", ago(a))).unwrap_or_else(|| "never".to_string()), w.cadence_secs.map(|c| format!("{} ({})", ago(c), w.cadence_source)).unwrap_or_else(|| w.cadence_source.to_string()), w.verdict, w.detail));
    }
    md.push_str("\n---\n\n*Mute check:* loop instant vs last log write — a ticking loop with a mute log supervises nobody.\n");
    md
}

/// Dedicated monitor view — watched rows only.
pub fn monitor_subs_md(_report: &FleetRowsReport) -> String {
    let p = planes();
    let mut md = String::from("## Watched rows — monitor supervision only\n\n");
    let mut watched: Vec<&Watched> = p.watched.iter().collect();
    watched.sort_by(|a, b| a.seat.cmp(&b.seat).then(a.uuid.cmp(&b.uuid)));
    if watched.is_empty() {
        md.push_str("> No monitor subscriptions. `ygg-monitor.py subscribe --role orchestrator --escalate-to <uuid>` attaches a row; `list` shows who is watched. The monitor judges *why* a row is quiet.\n");
    } else {
        md.push_str("| row | seat | campaign | role | escalates to | age | intent |\n| :--- | :--- | :--- | :--- | :--- | ---: | :--- |\n");
        for w in watched.iter().take(20) {
            let target = if w.owner_pinned { "🙋 pinned — owner holds".to_string() } else if w.escalate_to.is_empty() { "—".to_string() } else if w.escalate_host.is_empty() { short(&w.escalate_to) } else { format!("{} @{}", short(&w.escalate_to), w.escalate_host) };
            let intent = if w.intent.is_empty() { "—".to_string() } else { w.intent.chars().take(40).collect::<String>() };
            md.push_str(&format!(
                "| `{}` | {} | {} | {} | {} | {} | {} |\n",
                short(&w.uuid), if w.seat.is_empty() { "—" } else { &w.seat }, if w.campaign.is_empty() { "—" } else { &w.campaign }, if w.role.is_empty() { "—" } else { &w.role }, target, ago(w.age_secs), intent
            ));
        }
        if watched.len() > 20 { md.push_str(&format!("\n*{} more not listed.*\n", watched.len() - 20)); }
    }
    if !p.never_arm.is_empty() {
        md.push_str(&format!("\n🙋 **never-arm:** {} — a human types there, neither plane may touch it.\n", p.never_arm.join(", ")));
    }
    md
}

pub fn monitor_watcher_md(_report: &FleetRowsReport) -> String {
    let mut md = String::from("## Monitor watcher — judgement, not a timer\n\n");
    for w in watchers().into_iter().filter(|w| w.name.contains("monitor")) {
        md.push_str(&format!("| watcher | last fired | cadence | verdict |\n| :--- | ---: | ---: | :--- |\n| **{}** | {} | {} | {} |\n\n*{}*\n", w.name, w.last_fired_secs.map(|a| format!("{} ago", ago(a))).unwrap_or_else(|| "never".to_string()), w.cadence_secs.map(|c| format!("{} ({})", ago(c), w.cadence_source)).unwrap_or_else(|| w.cadence_source.to_string()), w.verdict, w.detail));
    }
    let episodes = std::fs::read_dir(relay().join("monitor-episodes")).map(|d| d.flatten().count()).unwrap_or(0);
    md.push_str(&format!("\n*{} escalation episode(s) on file.*\n", episodes));
    md
}

/// How far back the graph page looks, and how long a drawn page is reused.
const GRAPH_WINDOW_MS: u128 = 6 * 60 * 60 * 1000;
/// ⛔ THIS CAP IS A WINDOW TRUNCATION IN DISGUISE. `tail` returns the last N
/// records, so on a busy trace a small N silently narrows the window to minutes
/// — and every sample then lands in the final bucket, which draws as a dramatic
/// spike at "now" for a series that was in fact flat. It is generous on purpose,
/// and what actually came back is measured and printed rather than assumed.
const TAIL_CAP: usize = 250_000;
const GRAPH_TTL: std::time::Duration = std::time::Duration::from_secs(60);

struct GraphCache {
    at: std::time::Instant,
    md: String,
}

static GRAPHS: std::sync::OnceLock<std::sync::Mutex<Option<GraphCache>>> = std::sync::OnceLock::new();
static REFRESHING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn graph_cell() -> &'static std::sync::Mutex<Option<GraphCache>> {
    GRAPHS.get_or_init(|| std::sync::Mutex::new(None))
}

/// The graphs. Dash is exclusively ytrace, so every series here is a ytrace read.
///
/// ⛔ READING THE TRACE IS SECONDS, NOT MILLISECONDS, AND THIS RUNS ON A RENDER
/// PATH. The reader parses every generation file it can find whether or not the
/// window needs it, which on a live fleet is a hundred megabytes for a six-hour
/// question. So a drawn page is reused for a minute, and when it goes stale the
/// refresh happens on its own thread: a graph over six hours has no business
/// freezing the window of the person looking at it.
///
/// `blocking` is for callers with nowhere to put a thread — the CLI, which exits
/// before a background refresh could ever land.
pub fn graphs_md(blocking: bool) -> String {
    if let Ok(guard) = graph_cell().lock() {
        if let Some(c) = guard.as_ref() {
            if c.at.elapsed() < GRAPH_TTL {
                return c.md.clone();
            }
        }
    }
    if blocking {
        let md = graphs_render();
        if let Ok(mut guard) = graph_cell().lock() {
            *guard = Some(GraphCache { at: std::time::Instant::now(), md: md.clone() });
        }
        return md;
    }
    use std::sync::atomic::Ordering;
    if !REFRESHING.swap(true, Ordering::SeqCst) {
        std::thread::spawn(|| {
            let md = graphs_render();
            if let Ok(mut guard) = graph_cell().lock() {
                *guard = Some(GraphCache { at: std::time::Instant::now(), md });
            }
            REFRESHING.store(false, Ordering::SeqCst);
        });
    }
    match graph_cell().lock().ok().and_then(|g| g.as_ref().map(|c| (c.at.elapsed(), c.md.clone()))) {
        // ⭐ A STALE GRAPH IS SERVED WITH ITS AGE, NEVER SILENTLY. The number on
        //    screen is minutes old and the reader has to know that to trust it.
        Some((age, md)) => format!("> ⏳ *Drawn {} ago; a fresh reading is being collected and will appear on the next refresh.*\n\n{md}", ago(age.as_secs_f64())),
        None => "## The graphs\n\n> ⏳ **First reading in flight.** The trace is being collected on another thread — this page will fill in on the next refresh. It is deliberately not drawn from a partial read, because a graph missing its oldest half looks like a fleet that only just woke up.\n".to_string(),
    }
}

fn pct(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (((sorted.len() - 1) as f64) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn graphs_render() -> String {
    let homes = ytrace_homes("yggterm");
    let buckets = 24usize;
    let since = Some(now_ms().saturating_sub(GRAPH_WINDOW_MS));

    // ⭐ ONE PASS, NOT THREE. `incidents`, `tail` and `summarize` each re-read
    //    every file from scratch; asking all three of them the same question
    //    tripled the cost of this page for no extra information.
    let mut records: Vec<ytrace::YtraceRecord> = Vec::new();
    for home in &homes {
        records.extend(ytrace::query::tail(home, TAIL_CAP, since));
    }

    let mut md = format!(
        "## The graphs — {}h asked for, {} buckets\n\n",
        GRAPH_WINDOW_MS / 3_600_000,
        buckets
    );

    if records.is_empty() {
        if homes.iter().all(|h| !h.exists()) {
            md.push_str("> ⛔ **No ytrace home on this host.** Nothing below is a measurement — it is the absence of one, which is a different thing from a quiet fleet. Check `YTRACE_HOME`.\n");
        } else {
            md.push_str("> ⛔ **A ytrace home exists and returned no records in the window.** Either the app has not run recently or retention has already rolled the window away.\n");
        }
        return md;
    }
    if records.len() >= TAIL_CAP {
        md.push_str(&format!("⚠ **The reader's {TAIL_CAP}-record cap was reached**, so the oldest part of the window is missing from every series below. Each row states the span it actually covers.\n\n"));
    }

    // ── incidents over time ─────────────────────────────────────────────────
    let mut by_kind: std::collections::BTreeMap<String, Vec<u128>> = std::collections::BTreeMap::new();
    let mut severity: std::collections::BTreeMap<String, (usize, usize)> = std::collections::BTreeMap::new();
    for r in records.iter().filter(|r| r.payload.get("incident").and_then(|v| v.as_bool()).unwrap_or(false)) {
        let key = format!("{}/{}", r.category, r.name);
        by_kind.entry(key.clone()).or_default().push(r.ts_ms);
        let sev = r.payload.get("severity").and_then(|v| v.as_str()).unwrap_or("");
        let e = severity.entry(key).or_insert((0, 0));
        match sev {
            "error" => e.1 += 1,
            "warn" => e.0 += 1,
            _ => {}
        }
    }
    md.push_str("### Incidents over time\n\n| probe | count | warn / error | peak per bucket | covers | shape |\n| :--- | ---: | :--- | ---: | ---: | :--- |\n");
    let mut kinds: Vec<_> = by_kind.into_iter().collect();
    kinds.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    for (kind, ts) in kinds.iter().take(6) {
        // ⛔ EACH SERIES GETS ITS OWN AXIS, because a global one drawn from the
        //    single oldest record in the whole trace leaves most series pinned
        //    into the last few columns and reads as a fleet-wide spike.
        let span = window_of(ts);
        let series = bucket_counts(ts, span, buckets);
        let peak = series.iter().cloned().fold(0.0_f64, f64::max);
        let (w, e) = severity.get(kind).copied().unwrap_or((0, 0));
        md.push_str(&format!(
            "| `{kind}` | {} | {w} / {e} | {peak:.0} | {} | `{}` |\n",
            ts.len(),
            ago(span as f64 / 1000.0),
            spark(&series)
        ));
    }
    if kinds.is_empty() {
        md.push_str("| — | 0 | — | 0 | — | *no incidents in the window* |\n");
    }
    md.push_str("\n⚠ **Each sparkline is normalised against its own peak**, printed beside it, and drawn over its own span. The shapes say *when*; they never compare magnitudes across rows.\n");

    // ── latency over time ───────────────────────────────────────────────────
    md.push_str("\n### Latency over time — mean per bucket\n\n| probe | samples | mean | p95 | max | covers | shape |\n| :--- | ---: | ---: | ---: | ---: | ---: | :--- |\n");
    for (cat, name) in [
        ("daemon_request", "snapshot"),
        ("daemon_request", "status"),
        ("sidebar", "merge_rows"),
        ("ui", "block"),
    ] {
        let pts: Vec<(u128, f64)> = records
            .iter()
            .filter(|r| r.category == cat && r.name == name)
            .filter_map(|r| r.duration_ms.map(|d| (r.ts_ms, d)))
            .collect();
        if pts.is_empty() {
            md.push_str(&format!("| `{cat}/{name}` | 0 | — | — | — | — | *not seen in the window* |\n"));
            continue;
        }
        let ts: Vec<u128> = pts.iter().map(|(t, _)| *t).collect();
        let span = window_of(&ts);
        let series = bucket_means(&pts, span, buckets);
        let mut vals: Vec<f64> = pts.iter().map(|(_, v)| *v).collect();
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mean = vals.iter().sum::<f64>() / vals.len() as f64;
        md.push_str(&format!(
            "| `{cat}/{name}` | {} | {mean:.1} ms | {:.1} ms | {:.1} ms | {} | `{}` |\n",
            vals.len(),
            pct(&vals, 0.95),
            vals.last().copied().unwrap_or(0.0),
            ago(span as f64 / 1000.0),
            spark(&series)
        ));
    }
    md.push_str("\n⛔ **An empty bucket draws as the floor and means \"nothing happened\", never \"it got fast\".** Read the sample count before the shape.\n");

    // ── render cost ─────────────────────────────────────────────────────────
    let mut render: std::collections::BTreeMap<(String, String), Vec<f64>> = std::collections::BTreeMap::new();
    for r in records.iter().filter(|r| r.category == "render") {
        if let Some(d) = r.duration_ms {
            render.entry((r.name.clone(), r.clock.clone())).or_default().push(d);
        }
    }
    md.push_str("\n### Render cost\n\n| probe | clock | count | total | p50 | p95 | max |\n| :--- | :--- | ---: | ---: | ---: | ---: | ---: |\n");
    let mut rows: Vec<_> = render.into_iter().collect();
    rows.sort_by(|a, b| {
        b.1.iter().sum::<f64>().partial_cmp(&a.1.iter().sum::<f64>()).unwrap_or(std::cmp::Ordering::Equal)
    });
    for ((name, clock), mut vals) in rows.into_iter().take(5) {
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        md.push_str(&format!(
            "| `render/{name}` | {clock} | {} | {:.0} ms | {:.1} ms | {:.1} ms | {:.1} ms |\n",
            vals.len(),
            vals.iter().sum::<f64>(),
            pct(&vals, 0.50),
            pct(&vals, 0.95),
            vals.last().copied().unwrap_or(0.0)
        ));
    }
    md.push_str("\n⚠ `cpu` rows are CPU time and `wall` rows are elapsed — a render that *waited* is cheap on one and expensive on the other, and mixing them is how a busy GUI gets called idle.\n");
    md
}

/// The span a series actually covers, floored at a minute so a single sample
/// does not produce a zero-width axis.
fn window_of(ts: &[u128]) -> u128 {
    let now = now_ms();
    ts.iter()
        .min()
        .map(|e| now.saturating_sub(*e).max(60_000))
        .unwrap_or(GRAPH_WINDOW_MS)
        .min(GRAPH_WINDOW_MS)
}

/// Render the live block a page asked for. An unknown name is reported, never
/// silently dropped — a page that asks for a reading it does not get should say
/// so rather than look like a page with nothing to show.
pub fn live_widgets(kind: &str, page_id: &str, report: &FleetRowsReport, blocking: bool) -> Vec<Value> {
    let md = match kind {
        "armings" => armings_md(report),
        "census" => census_md(report),
        "watchers" => watchers_md(),
        "graphs" => graphs_md(blocking),
        "wakes" => wakes_md(),
        "cold" => cold_md(report),
        "rolls" => rolls_md(),
        "folds" => folds_md(),
        "ci_subs" => ci_subs_md(report),
        "ci_builds" => ci_builds_md(report),
        "ci_watchers" => ci_watchers_md(report),
        "booter_subs" => booter_subs_md(report),
        "booter_watcher" => booter_watcher_md(report),
        "monitor_subs" => monitor_subs_md(report),
        "monitor_watcher" => monitor_watcher_md(report),
        // The Legendary Bugs blocks live in their own module: they share this
        // dispatcher rather than a second one, so a page names a reading and
        // does not have to know which file serves it.
        //
        // ⛔ Matched with `if let`, not a guard — a guard calling `block()` to
        //    test it and again to use it would do every trace read twice.
        other => match crate::legendary::block(other, blocking) {
            Some(md) => md,
            None => format!(
                "> ⛔ This page asked for a live reading called `{other}`, and this build has no reader by that name. The prose above still stands; the numbers that belong here are missing, which is not the same as being zero."
            ),
        },
    };
    vec![json!({
        "kind": "markdown",
        "id": format!("live:{kind}:{page_id}"),
        "source": md,
    })]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ago_reads_without_arithmetic() {
        assert_eq!(ago(41.0), "41s");
        assert_eq!(ago(600.0), "10m");
        assert_eq!(ago(7200.0), "2h 00m");
        assert_eq!(ago(200_000.0), "2d 7h");
    }

    #[test]
    fn spark_is_flat_when_nothing_happened() {
        assert_eq!(spark(&[0.0, 0.0, 0.0]), "▁▁▁");
        assert_eq!(spark(&[]), "—");
    }

    #[test]
    fn spark_normalises_against_its_own_peak() {
        // The same shape at two scales must draw identically — which is exactly
        // why the peak has to be printed beside it.
        assert_eq!(spark(&[1.0, 2.0, 4.0]), spark(&[10.0, 20.0, 40.0]));
    }

    #[test]
    fn declared_interval_prefers_what_the_watcher_said() {
        assert_eq!(declared_interval("roll-watch up (interval 3600s, dry=0)"), Some(3600.0));
        assert_eq!(declared_interval("no cadence here"), None);
    }

    #[test]
    fn a_watcher_that_never_wrote_is_never_called_fresh() {
        // ⛔ The honesty rule that matters most on this pane: absence of a
        //    reading is never rendered as a healthy one.
        assert!(verdict_for(None, Some(300.0)).contains("NEVER SEEN"));
        assert!(verdict_for(Some(10_000.0), Some(300.0)).contains("SILENT"));
        assert_eq!(verdict_for(Some(100.0), Some(300.0)), "FRESH");
    }

    #[test]
    fn buckets_place_a_recent_event_at_the_end() {
        let now = now_ms();
        let series = bucket_counts(&[now - 1000], 3_600_000, 12);
        assert_eq!(series.last().copied(), Some(1.0));
        assert_eq!(series.iter().sum::<f64>(), 1.0);
    }

    #[test]
    fn an_unknown_live_block_says_so() {
        let report = FleetRowsReport::default();
        let w = live_widgets("no-such-reader", "p1", &report, true);
        assert!(w[0]["source"].as_str().unwrap().contains("no reader by that name"));
    }
}
