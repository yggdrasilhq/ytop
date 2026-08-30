//! Legendary Bugs — the end-to-end chain, and where it has no probe.
//!
//! LEGENDARY is an owner-set PRIORITY, not a status: a defect that makes the
//! product unusable for the person using it. It outranks every other entry
//! however tractable those are, and it becomes or stops being LEGENDARY only on
//! the owner's word.
//!
//! ⭐ WHY THIS FILE EXISTS AT ALL. The chain a keystroke and a glyph travel is
//! kernel → daemon → client → xterm.js → pixel, and it is instrumented by five
//! different layers that have never been read in one place. So each session
//! re-derives it from a trace file, gets as far as its own layer, and files a
//! finding that the next session cannot build on.
//!
//! ⛔⛔ AND THE RULE THAT SHAPES EVERY BLOCK HERE: **A MISSING PROBE AND A QUIET
//! SYSTEM LOOK IDENTICAL.** A page that renders "0" for a link nobody ever
//! instrumented is not reporting health, it is reporting its own blindness in
//! the costume of health — and this campaign has paid for that mistake more than
//! once. So every block distinguishes three states and never collapses them:
//!
//!   ✅ **seen** — the probe exists and fired in this window, with a count
//!   ⚠ **named, not seen** — the probe exists and was silent, which may be good news
//!   ⛔ **no probe** — nothing would have recorded this even if it happened
//!
//! ⚠ THE TOP SHELF TAKES NO YTRACE. It reads `probe.rs` only — a 400 ms `/proc`
//! delta — so the kernel half of the chain is deliberately thinner here than the
//! yggterm half on Dash, and the pages say which of that thinness is the shelf's
//! rule and which is a genuinely absent instrument.

use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::sysinternals::{ago, spark};

/// How far back the Dash pages look.
const TRACE_WINDOW_MS: u128 = 6 * 60 * 60 * 1000;
/// See `sysinternals::TAIL_CAP` — the cap is a window truncation in disguise, so
/// it is generous and what actually came back is measured rather than assumed.
const TRACE_CAP: usize = 400_000;
const TRACE_TTL: Duration = Duration::from_secs(60);
const HOST_TTL: Duration = Duration::from_secs(30);

struct Cached<T> {
    at: Instant,
    v: T,
}

type Records = Arc<Vec<ytrace::YtraceRecord>>;

static TRACE: OnceLock<Mutex<Option<Cached<Records>>>> = OnceLock::new();
static TRACE_BUSY: AtomicBool = AtomicBool::new(false);
static HOST: OnceLock<Mutex<Option<Cached<Value>>>> = OnceLock::new();
static HOST_BUSY: AtomicBool = AtomicBool::new(false);

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}

fn ytrace_homes(provider: &str) -> Vec<std::path::PathBuf> {
    let mut homes = vec![ytrace::compat::resolve_home(provider)];
    if let Some(xdg) = dirs::home_dir().map(|h| h.join(".local").join("share").join("ytrace").join(provider)) {
        if !homes.contains(&xdg) && xdg.exists() {
            homes.push(xdg);
        }
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        let p = std::path::PathBuf::from(xdg).join("ytrace").join(provider);
        if !homes.contains(&p) && p.exists() {
            homes.push(p);
        }
    }
    homes
}

/// ⛔ ONE TRACE READ SERVES EVERY PAGE IN THIS NOTEBOOK. Reading the trace is
/// seconds, not milliseconds — the reader parses every generation file whether
/// or not the window needs it — and five pages each doing their own read would
/// make turning a page cost more than the bug does. Refreshed off-thread, so a
/// six-hour question never freezes the window of the person looking at it.
fn records(blocking: bool) -> Option<Records> {
    let cell = TRACE.get_or_init(|| Mutex::new(None));
    if let Ok(g) = cell.lock() {
        if let Some(c) = g.as_ref() {
            if c.at.elapsed() < TRACE_TTL {
                return Some(Arc::clone(&c.v));
            }
        }
    }
    let read = || -> Records {
        let since = Some(now_ms().saturating_sub(TRACE_WINDOW_MS));
        let mut out = Vec::new();
        for home in ytrace_homes("yggterm") {
            out.extend(ytrace::query::tail(&home, TRACE_CAP, since));
        }
        Arc::new(out)
    };
    if blocking {
        let v = read();
        if let Ok(mut g) = cell.lock() {
            *g = Some(Cached { at: Instant::now(), v: Arc::clone(&v) });
        }
        return Some(v);
    }
    if !TRACE_BUSY.swap(true, Ordering::SeqCst) {
        std::thread::spawn(move || {
            let v = read();
            if let Ok(mut g) = TRACE.get_or_init(|| Mutex::new(None)).lock() {
                *g = Some(Cached { at: Instant::now(), v });
            }
            TRACE_BUSY.store(false, Ordering::SeqCst);
        });
    }
    cell.lock().ok().and_then(|g| g.as_ref().map(|c| Arc::clone(&c.v)))
}

/// The local host reading — `probe.rs`, a 400 ms `/proc` delta, no ytrace.
fn host(blocking: bool) -> Option<Value> {
    let cell = HOST.get_or_init(|| Mutex::new(None));
    if let Ok(g) = cell.lock() {
        if let Some(c) = g.as_ref() {
            if c.at.elapsed() < HOST_TTL {
                return Some(c.v.clone());
            }
        }
    }
    let read = || crate::probe::read_host(None, Duration::from_secs(10));
    if blocking {
        let v = read();
        if let Ok(mut g) = cell.lock() {
            *g = Some(Cached { at: Instant::now(), v: v.clone() });
        }
        return Some(v);
    }
    if !HOST_BUSY.swap(true, Ordering::SeqCst) {
        std::thread::spawn(move || {
            let v = crate::probe::read_host(None, Duration::from_secs(10));
            if let Ok(mut g) = HOST.get_or_init(|| Mutex::new(None)).lock() {
                *g = Some(Cached { at: Instant::now(), v });
            }
            HOST_BUSY.store(false, Ordering::SeqCst);
        });
    }
    cell.lock().ok().and_then(|g| g.as_ref().map(|c| c.v.clone()))
}

fn collecting(what: &str) -> String {
    format!("> ⏳ **The {what} is being collected on another thread** — this page fills in on the next refresh. It is deliberately not drawn from a partial read: a chain missing its oldest half looks like a system that only just started, which is the exact confusion this notebook exists to remove.\n")
}

// ── counting helpers ─────────────────────────────────────────────────────────

fn count(recs: &[ytrace::YtraceRecord], cat: &str, name: &str) -> usize {
    recs.iter().filter(|r| r.category == cat && r.name == name).count()
}

fn stamps(recs: &[ytrace::YtraceRecord], cat: &str, name: &str) -> Vec<u128> {
    recs.iter().filter(|r| r.category == cat && r.name == name).map(|r| r.ts_ms).collect()
}

/// The span a set of stamps covers, so a shape is never drawn wider than its data.
fn span_of(ts: &[u128]) -> u128 {
    let now = now_ms();
    ts.iter().min().map(|e| now.saturating_sub(*e).max(60_000)).unwrap_or(TRACE_WINDOW_MS).min(TRACE_WINDOW_MS)
}

fn bucket(ts: &[u128], window: u128, n: usize) -> Vec<f64> {
    let start = now_ms().saturating_sub(window);
    let width = (window / n.max(1) as u128).max(1);
    let mut out = vec![0.0; n];
    for t in ts {
        if *t < start {
            continue;
        }
        out[(((*t - start) / width) as usize).min(n - 1)] += 1.0;
    }
    out
}

fn shape(ts: &[u128]) -> String {
    if ts.is_empty() {
        return "—".to_string();
    }
    let w = span_of(ts);
    format!("`{}` over {}", spark(&bucket(ts, w, 24)), ago(w as f64 / 1000.0))
}

/// `app_control` records its verb in two shapes — an object with `kind` on the
/// begin/end pair, a bare string on the stage events. Reading only one of them
/// silently halves the tally.
fn app_control_kind(r: &ytrace::YtraceRecord) -> Option<String> {
    match r.payload.get("command") {
        Some(Value::Object(o)) => o.get("kind").and_then(|k| k.as_str()).map(|s| s.to_string()),
        Some(Value::String(s)) => Some(s.clone()),
        _ => None,
    }
}

/// What an app-control verb does to the row set — the axis the narrowing is on.
///
/// ⛔ AN ATTRIBUTE CHANGE IS NOT A ROW-SET CHANGE, and the difference is the
/// whole narrowing. One `set_session_outline` into a quiet GUI, setting a seat
/// to the value it already held, produced no reset and no launch in the twelve
/// seconds after — so it is not "any app-control mutation" that costs a remount.
///
/// ⚠ And a READ is not a write. Lumping `describe_rows` in with `rename_session`
/// under one "no" would hide that the overwhelming majority of GUI traffic is
/// agents looking rather than changing anything — which is its own finding, on
/// the input-blocking page.
#[derive(PartialEq, Clone, Copy)]
enum Effect {
    RowSet,
    Attribute,
    ReadOnly,
}

fn effect_of(kind: &str) -> Effect {
    match kind {
        "create_terminal" | "remove_session" => Effect::RowSet,
        "set_session_outline" | "rename_session" | "submit_terminal_prompt" | "notify" => Effect::Attribute,
        _ => Effect::ReadOnly,
    }
}

fn changes_the_row_set(kind: &str) -> bool {
    effect_of(kind) == Effect::RowSet
}

// ── TOP shelf — the kernel half, probe.rs only, no ytrace ────────────────────

/// Where each shelf sits in the chain, and what this host can actually see.
pub fn chain_map_md(blocking: bool) -> String {
    let mut md = String::from("## The chain, and who owns which link\n\n");
    md.push_str("| # | link | layer | which shelf carries it |\n| ---: | :--- | :--- | :--- |\n");
    for (n, link, layer, shelf) in [
        (1, "the PTY child is forked and exec'd", "kernel", "**Top** — `probe.rs` process census"),
        (2, "the scheduler and the disk under that work", "kernel", "**Top** — pressure, ZFS iostat"),
        (3, "the session is asked to launch a terminal", "daemon", "Dash — `session/request_terminal_launch*`"),
        (4, "PTY bytes are accepted", "daemon", "Dash — `input/pty`"),
        (5, "the mount begins, resets, or is skipped", "client", "Dash — `terminal_mount/*`"),
        (6, "the JS terminal is created and reports ready", "client → js", "Dash — `terminal_mount/js_*`"),
        (7, "bytes are enqueued and flushed into xterm.js", "xterm.js", "Dash — `xterm_write/*`, `terminal_js/*`"),
        (8, "a frame is painted, wholly or partly", "xterm.js", "Dash — `xterm_render/*`"),
        (9, "the glyphs are on the screen", "pixel", "⛔ **neither** — no probe crosses this"),
    ] {
        md.push_str(&format!("| {n} | {link} | `{layer}` | {shelf} |\n"));
    }
    md.push_str("\n⛔ **Link 9 is the one nobody can close from inside the app.** Everything above it says the software believed it painted; only a camera or a screenshot correlated to a frame id says a person saw it.\n");

    md.push_str("\n### What this host can see right now\n\n");
    let Some(h) = host(blocking) else {
        md.push_str(&collecting("host reading"));
        return md;
    };
    if let Some(err) = h.get("error").and_then(|e| e.as_str()) {
        md.push_str(&format!("⛔ **The host probe failed: {err}.** Nothing below this line is a measurement — it is the absence of one.\n"));
        return md;
    }
    let tools: Vec<String> = h["ebpf_tools"].as_array().map(|a| a.iter().filter_map(|t| t.as_str().map(String::from)).collect()).unwrap_or_default();
    md.push_str(&format!(
        "* **{} cores**, load {:.2}, {} processes, host busy **{:.1}%** — the floor a mount lands on.\n",
        h["cpu_count"].as_i64().unwrap_or(0),
        h["load"][0].as_f64().unwrap_or(0.0),
        h["procs_total"].as_i64().unwrap_or(0),
        h["cpu_busy_pct"].as_f64().unwrap_or(0.0)
    ));
    md.push_str(&format!(
        "* **ytrace on this host:** {}\n",
        if h["ytrace"]["has_ytrace"].as_bool().unwrap_or(false) { "present — the Dash half of this book has data" } else { "⛔ absent — every Dash page in this book will be blank, and blank is not calm" }
    ));
    md.push_str(&format!(
        "* **kernel-call tooling:** {}\n",
        if tools.is_empty() { "⛔ none installed — links 1 and 2 cannot be measured here at all".to_string() } else { format!("`{}` installed — ⚠ detected, never run (see the eBPF page)", tools.join("`, `")) }
    ));
    md
}

/// The kernel half of a mount: what actually runs, and what it costs.
pub fn kernel_half_md(blocking: bool) -> String {
    let mut md = String::from("## The kernel half of a mount\n\n");
    let Some(h) = host(blocking) else {
        md.push_str(&collecting("host reading"));
        return md;
    };
    if let Some(err) = h.get("error").and_then(|e| e.as_str()) {
        md.push_str(&format!("⛔ **The host probe failed: {err}.** \"I could not look\" is never rendered as \"it is idle\".\n"));
        return md;
    }

    let mem_total = h["mem_total_kb"].as_f64().unwrap_or(0.0);
    let mem_avail = h["mem_available_kb"].as_f64().unwrap_or(0.0);
    let swap_total = h["swap_total_kb"].as_f64().unwrap_or(0.0);
    let swap_free = h["swap_free_kb"].as_f64().unwrap_or(0.0);
    md.push_str("### Pressure — the conditions a re-mount has to complete under\n\n");
    md.push_str(&format!(
        "| busy | load 1m/5m/15m | memory | swap used | processes | sample |\n| ---: | :--- | :--- | ---: | ---: | ---: |\n| {:.1}% | {:.2} / {:.2} / {:.2} | {:.1} of {:.1} GB free | {:.1} GB | {} | {} ms |\n",
        h["cpu_busy_pct"].as_f64().unwrap_or(0.0),
        h["load"][0].as_f64().unwrap_or(0.0),
        h["load"][1].as_f64().unwrap_or(0.0),
        h["load"][2].as_f64().unwrap_or(0.0),
        mem_avail / 1024.0 / 1024.0,
        mem_total / 1024.0 / 1024.0,
        (swap_total - swap_free) / 1024.0 / 1024.0,
        h["procs_total"].as_i64().unwrap_or(0),
        h["sample_ms"].as_i64().unwrap_or(0)
    ));
    md.push_str("\n⛔ **`cpu_busy` here is a 400 ms delta, not a `ps` lifetime average.** A process that burned a core an hour ago and has slept since reads calm here and busy in `ps` — the delta is the live view, `ps` is a biography.\n");

    // The processes a mount actually creates: the shell/PTY tenants.
    let empty = Vec::new();
    let top = h["top"].as_array().unwrap_or(&empty);
    md.push_str("\n### The processes under the surface\n\n| pid | command | cpu | rss |\n| ---: | :--- | ---: | ---: |\n");
    let mut shown = 0;
    for p in top.iter().take(10) {
        shown += 1;
        md.push_str(&format!(
            "| {} | `{}` | {:.1}% | {} MB |\n",
            p["pid"].as_i64().unwrap_or(0),
            p["comm"].as_str().unwrap_or("?"),
            p["cpu_pct"].as_f64().unwrap_or(0.0),
            p["rss_kb"].as_i64().unwrap_or(0) / 1024
        ));
    }
    if shown == 0 {
        md.push_str("| — | *no process sample* | — | — |\n");
    }

    if h["zfs"]["has_zfs"].as_bool().unwrap_or(false) {
        md.push_str("\n### The disk a mount reads from\n\n");
        md.push_str("A mount that replays a row's screen reads it from disk, so a pool under load is a slow mount.\n\n");
        let io = &h["zfs"]["iostat"];
        md.push_str(&format!(
            "| read | write | read ops | write ops |\n| ---: | ---: | ---: | ---: |\n| {:.1} MB/s | {:.1} MB/s | {} | {} |\n",
            io["read_bytes_s"].as_f64().unwrap_or(0.0) / 1_048_576.0,
            io["write_bytes_s"].as_f64().unwrap_or(0.0) / 1_048_576.0,
            io["read_ops"].as_i64().unwrap_or(0),
            io["write_ops"].as_i64().unwrap_or(0)
        ));
    }

    md.push_str("\n---\n\n⛔ **What this page CANNOT tell you, and it is the important half.** A `/proc` sample every 400 ms cannot see a process that lives less than 400 ms — and the fork/exec of a PTY child is exactly that. So a mount storm is visible here only as *pressure*, never as the individual mounts. The instrument that would see them is on the next page, and it is not installed.\n");
    md
}

/// ⛔ The kernel-call probe: detected, never run.
pub fn ebpf_gap_md(blocking: bool) -> String {
    let mut md = String::from("## ⛔ The kernel-call half is DECLARED, not MEASURED\n\n");
    md.push_str("The end-to-end tracing this notebook is for begins at a kernel call. This shelf does not make one.\n\n");
    let Some(h) = host(blocking) else {
        md.push_str(&collecting("host reading"));
        return md;
    };
    let tools: Vec<String> = h["ebpf_tools"].as_array().map(|a| a.iter().filter_map(|t| t.as_str().map(String::from)).collect()).unwrap_or_default();
    let available = h["ebpf_available"].as_bool().unwrap_or(false);

    md.push_str(&format!(
        "| what ytop does | what it reports |\n| :--- | :--- |\n| `which bpftrace perf bpftool` | **{}** |\n| runs any of them | ⛔ **never** |\n\n",
        if tools.is_empty() { "none found".to_string() } else { format!("`{}`", tools.join("`, `")) }
    ));
    if available {
        md.push_str("⚠ **The tools are installed and that is all that has been established.** `ebpf_available: true` means a binary is on `PATH`. It does not mean a probe was attached, that anything was sampled, or that a single kernel event has ever been recorded by ytop. A reader who takes the green tick for a measurement has been misled by this pane, which is why it is spelled out rather than shown as a status chip.\n\n");
    } else {
        md.push_str("⛔ **Nothing is installed, so links 1 and 2 of the chain are unmeasurable on this host** — not quiet, unmeasurable.\n\n");
    }
    md.push_str("### What would have to be measured, and what it would settle\n\n");
    md.push_str("| kernel event | the question it answers |\n| :--- | :--- |\n");
    md.push_str("| `sched_switch` on the client's UI thread | is a freeze the app blocking, or the kernel not scheduling it? Right now `ui/block` records the gap and cannot say which |\n");
    md.push_str("| `sched_process_fork` / `exec` for PTY children | how many processes a mount storm actually creates — the `/proc` sampler cannot see short-lived ones |\n");
    md.push_str("| `zfs_delay` / block-layer wait | is a slow mount waiting on the pool, or on the daemon? |\n");
    md.push_str("| `io_uring` submission and completion | where a stalled write is parked |\n");
    md.push_str("\n⛔ **Until one of those runs, every \"0\" on this shelf about kernel behaviour is ytop's blindness and not the kernel's silence.** That distinction is the whole reason this page exists rather than a tidy green panel.\n");
    md
}

// ── DASH shelf — the yggterm half, exclusively ytrace ────────────────────────

/// The churn: rows nobody is looking at, torn down and re-mounted.
pub fn churn_md(blocking: bool) -> String {
    let Some(recs) = records(blocking) else {
        return format!("## The churn\n\n{}", collecting("trace"));
    };
    let mut md = String::from("## The churn — what is re-mounting, and what sets it off\n\n");
    if recs.is_empty() {
        md.push_str("⛔ **No ytrace records in the window.** That is not a calm fleet — it is an absent instrument. Check `YTRACE_HOME` before reading a zero here as good news.\n");
        return md;
    }

    let begin = stamps(&recs, "terminal_mount", "begin");
    let reset = stamps(&recs, "terminal_mount", "bootstrap_reset");
    let skipped = stamps(&recs, "terminal_mount", "bootstrap_spawn_skipped_inactive_retained_host");
    let launch = stamps(&recs, "session", "request_terminal_launch_for_active_begin");

    md.push_str("| event | count | shape |\n| :--- | ---: | :--- |\n");
    for (label, ts) in [
        ("`terminal_mount/begin` — a full mount", &begin),
        ("`terminal_mount/bootstrap_reset` — the surface reset under a mount", &reset),
        ("…`bootstrap_spawn_skipped_inactive_retained_host` — on a row nobody was looking at", &skipped),
        ("`session/request_terminal_launch_for_active_begin`", &launch),
    ] {
        md.push_str(&format!("| {label} | {} | {} |\n", ts.len(), shape(ts)));
    }
    if !begin.is_empty() {
        md.push_str(&format!(
            "\n**{:.1} resets per mount.** A reset per mount would be the mount doing its job; more than one means the surface is being torn down again after it was built.\n",
            reset.len() as f64 / begin.len() as f64
        ));
    }

    // ── the narrowing: row-set changes, not time ────────────────────────────
    let mut verbs: std::collections::BTreeMap<String, Vec<u128>> = std::collections::BTreeMap::new();
    for r in recs.iter().filter(|r| r.category == "app_control") {
        if let Some(k) = app_control_kind(r) {
            verbs.entry(k).or_default().push(r.ts_ms);
        }
    }
    let mut rowset: Vec<u128> = Vec::new();
    md.push_str("\n### What the GUI was asked to do\n\n| app-control verb | calls | changes the row set? |\n| :--- | ---: | :--- |\n");
    let mut rows: Vec<_> = verbs.into_iter().collect();
    rows.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    for (kind, ts) in &rows {
        let effect = effect_of(kind);
        if effect == Effect::RowSet {
            rowset.extend(ts.iter().copied());
        }
        md.push_str(&format!(
            "| `{kind}` | {} | {} |\n",
            ts.len(),
            match effect {
                Effect::RowSet => "⛔ **yes — this is the one that costs a re-mount**",
                Effect::Attribute => "no — an attribute write, measured as free",
                Effect::ReadOnly => "no — a read; it changes nothing and still costs UI-thread time",
            }
        ));
    }
    if rows.is_empty() {
        md.push_str("| — | 0 | *no app-control calls in the window* |\n");
    }

    // Correlation, computed here rather than quoted from the entry.
    if !reset.is_empty() {
        let window_ms = 6_000u128;
        rowset.sort();
        let near = reset
            .iter()
            .filter(|t| rowset.iter().any(|c| **t >= *c && **t - *c <= window_ms))
            .count();
        md.push_str(&format!(
            "\n### Do the resets follow the row set changing?\n\n**{near} of {} resets fell within 6 s of a `create_terminal` or `remove_session`** ({:.0}%), over {} row-set changes in the window.\n\n",
            reset.len(),
            (near as f64 / reset.len() as f64) * 100.0,
            rowset.len()
        ));
        md.push_str("⚠ **This is a correlation this page computes, not a verdict it inherits.** A high share supports the narrowing that housekeeping — folding corpses, spawning lanes, reseating rows — is what makes the window flash. A low share means something else is driving the resets in this window and the narrowing does not generalise. Either way the number is measured here rather than quoted.\n");
        md.push_str("\n⛔ **And it can never prove causation on its own,** because an orchestrator that is busy changing the row set is also busy doing everything else. The falsifier is the one the entry names: leave the GUI untouched for ten minutes with several rows producing output, and count the resets whose target is not the active row. It must be zero.\n");
    }
    md
}

/// The mount ladder — where a mount stops, rung by rung.
pub fn mount_ladder_md(blocking: bool) -> String {
    let Some(recs) = records(blocking) else {
        return format!("## The mount ladder\n\n{}", collecting("trace"));
    };
    let mut md = String::from("## The ladder a mount climbs, and the rung it stops on\n\n");
    md.push_str("> **A mount begins with an EMPTY surface.** Nothing has to fail for the screen to be blank — the mount simply starts that way. So the question is never \"did it error\", it is **how far up did it get**.\n\n");

    // ⭐ The ladder in order. A rung that drops sharply is where blank lives.
    let ladder: [(&str, &str, &str); 9] = [
        ("terminal_mount", "begin", "the mount starts — surface empty by construction"),
        ("terminal_mount", "ensure_begin", "the terminal is ensured to exist"),
        ("terminal_mount", "bootstrap_spawn_scheduled", "the bootstrap is scheduled"),
        ("terminal_mount", "js_eval_created", "the JS terminal object is created"),
        ("terminal_mount", "js_ready", "xterm.js reports itself ready"),
        ("terminal_mount", "attach_ready", "the stream is attached"),
        ("terminal_mount", "first_output", "the first bytes arrive"),
        ("terminal_mount", "first_meaningful_output", "⭐ the first bytes that are not protocol noise"),
        ("reveal", "reveal_ready", "the surface is revealed to the eye"),
    ];
    let mut prev: Option<usize> = None;
    md.push_str("| rung | what it means | count | lost here |\n| :--- | :--- | ---: | ---: |\n");
    for (cat, name, meaning) in ladder {
        let n = count(&recs, cat, name);
        // ⛔ `reveal_ready` COUNTS REVEALS, NOT MOUNTS — a surface can be
        //    revealed many times over one mount, so subtracting it from the rung
        //    above would invent a negative loss out of a different population.
        let lost = if cat != "terminal_mount" {
            "*a different denominator — reveals, not mounts*".to_string()
        } else {
            match prev {
                Some(p) if p >= n => format!("{}", p - n),
                // Not an error: the ladder is not strictly nested, because some
                // rungs fire on retries and recoveries that skipped earlier ones.
                Some(_) => "— *more than the rung above; retries reach it too*".to_string(),
                None => "—".to_string(),
            }
        };
        md.push_str(&format!("| `{cat}/{name}` | {meaning} | {n} | {lost} |\n"));
        // reveal_ready counts reveals rather than mounts, so it does not
        // continue the same denominator — the ladder stops comparing there.
        if cat == "terminal_mount" {
            prev = Some(n);
        }
    }

    let begin = count(&recs, "terminal_mount", "begin");
    let meaningful = count(&recs, "terminal_mount", "first_meaningful_output");
    if begin > 0 {
        md.push_str(&format!(
            "\n⛔ **{meaningful} of {begin} mounts reached meaningful output** — {:.0}% never showed the person anything but an empty surface in this window.\n",
            (meaningful as f64 / begin as f64) * 100.0
        ));
    }

    md.push_str("\n### The recovery ladder — firing, and losing\n\n| probe | count |\n| :--- | ---: |\n");
    for (cat, name) in [
        ("terminal_mount", "ensure_retry"),
        ("terminal_mount", "resume_recovery_begin"),
        ("terminal_mount", "read_error_after_attach"),
        ("terminal_mount", "reveal_screen_reconcile"),
        ("terminal_mount", "placeholder_stage"),
        ("reveal", "reveal_failed"),
        ("reveal", "reveal_cancelled"),
    ] {
        md.push_str(&format!("| `{cat}/{name}` | {} |\n", count(&recs, cat, name)));
    }
    md.push_str("\n⭐ **A busy recovery ladder is not reassurance.** It means the blank surface is being detected and fought over, repeatedly, and the fight is being lost often enough to matter. Curing the churn removes the need for most of this; curing the ladder alone leaves the churn paying for it forever.\n");
    md
}

/// Ghost frames and broken paint — the xterm.js half.
pub fn paint_chain_md(blocking: bool) -> String {
    let Some(recs) = records(blocking) else {
        return format!("## The paint chain\n\n{}", collecting("trace"));
    };
    let mut md = String::from("## Ghost frames and broken paint — the xterm.js half\n\n");

    // ⛔ frame_gap is THRESHOLD-GATED. Reading its count as "how often frames
    //    are late" is fine; reading its distribution as "the frame rate" is a
    //    mistake that was made once already on this data.
    let gaps: Vec<f64> = recs
        .iter()
        .filter(|r| r.category == "xterm_render" && r.name == "frame_gap")
        .filter_map(|r| r.payload.get("gap_ms").and_then(|v| v.as_f64()))
        .collect();
    let partial = recs
        .iter()
        .filter(|r| r.category == "xterm_render" && r.name == "frame_gap")
        .filter(|r| {
            let painted = r.payload.get("rows_painted").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let rows = r.payload.get("rows").and_then(|v| v.as_f64()).unwrap_or(0.0);
            rows > 0.0 && painted < rows
        })
        .count();

    md.push_str("### Late frames — the ghost\n\n");
    if gaps.is_empty() {
        md.push_str("⚠ `xterm_render/frame_gap` did not fire in this window. Because the probe is threshold-gated, that is genuinely good news here — but only if the trace has other xterm records, which the table below settles.\n\n");
    } else {
        let mut sorted = gaps.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let min = sorted[0];
        let p50 = sorted[sorted.len() / 2];
        let max = sorted[sorted.len() - 1];
        md.push_str(&format!(
            "**{} late frames.** Gap min **{min:.0} ms**, median **{p50:.0} ms**, worst **{max:.0} ms**.\n\n",
            gaps.len()
        ));
        md.push_str(&format!(
            "⛔ **The minimum is {min:.0} ms because the probe only fires above a threshold — it is not a frame-rate meter.** So this counts *late* frames and can say nothing about how many frames were on time; dividing it by anything to get a percentage produces a number with no meaning. A ghost frame is the previous row's surface still on screen while the new mount has not painted, and a median of half a second is long enough for a person to see one.\n\n"
        ));
    }

    md.push_str("### Partial paints — the broken TUI\n\n");
    let framewins = recs.iter().filter(|r| r.category == "xterm_render" && r.name == "frame_window").count();
    md.push_str(&format!(
        "`frame_gap` records carry `rows_painted` against `rows`, so a half-drawn frame IS distinguishable from a whole one: **{partial} of {} late frames painted fewer rows than the terminal has.**\n\n",
        gaps.len()
    ));
    if partial == 0 && !gaps.is_empty() {
        md.push_str("⭐ **None — and that is a real result, not a missing probe.** The broken TUI paint reported from the seat is therefore probably NOT \"a frame drew half its rows\". The remaining candidates are a frame painted from a stale buffer, or a paint that never happened at all and left the previous surface up — which is the ghost above, not a partial draw. ⇒ The next measurement is a frame id carried from the write that caused it, so a painted frame can be matched to the bytes it was supposed to show.\n\n");
    }

    md.push_str("### The write path underneath\n\n| probe | count | what it is |\n| :--- | ---: | :--- |\n");
    for (cat, name, what) in [
        ("xterm_write", "enqueue", "bytes queued for the terminal"),
        ("xterm_write", "flush", "bytes handed to xterm.js"),
        ("xterm_write", "flush_window", "the periodic flush rollup"),
        ("terminal_js", "xterm_write_flush", "the JS side of the same flush, with repair state"),
        ("terminal_js", "xterm_forced_refresh", "⚠ a refresh the code had to force"),
        ("xterm_screen", "reset", "⚠ the screen thrown away and redrawn"),
        ("xterm_render", "frame_window", "the periodic paint rollup — the honest denominator"),
    ] {
        md.push_str(&format!("| `{cat}/{name}` | {} | {what} |\n", count(&recs, cat, name)));
    }

    // paint_repair lives one level down inside the JS payload.
    let repairs: Vec<String> = recs
        .iter()
        .filter(|r| r.name == "xterm_write_flush")
        .filter_map(|r| {
            let p = r.payload.get("payload").unwrap_or(&r.payload);
            if p.get("paint_repair").and_then(|v| v.as_bool()).unwrap_or(false) {
                Some(p.get("paint_repair_reason").and_then(|v| v.as_str()).unwrap_or("(no reason)").to_string())
            } else {
                None
            }
        })
        .collect();
    md.push_str(&format!("\n### Paint repairs — {} in the window\n\n", repairs.len()));
    if repairs.is_empty() {
        md.push_str("None fired. `paint_repair` is the JS layer noticing its own canvas is wrong and redrawing it, so a zero here is genuinely quiet — the probe is not threshold-gated.\n");
    } else {
        let mut by_reason: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for r in &repairs {
            *by_reason.entry(r.clone()).or_insert(0) += 1;
        }
        md.push_str("| reason | times |\n| :--- | ---: |\n");
        for (r, n) in by_reason.iter().take(8) {
            md.push_str(&format!("| `{r}` | {n} |\n"));
        }
        md.push_str("\n⭐ **Every repair is a frame that was wrong until the code noticed.** Between the moment it went wrong and the moment it was repaired, the person was looking at it.\n");
    }
    let _ = framewins;
    md
}

/// Input blocking — the class both symptoms were filed under.
pub fn input_chain_md(blocking: bool) -> String {
    let Some(recs) = records(blocking) else {
        return format!("## Input blocking\n\n{}", collecting("trace"));
    };
    let mut md = String::from("## Input blocking — the class both symptoms were filed under\n\n");
    md.push_str("> Ghost frames, freezes and broken TUI paint are all filed as **input-blocking** defects, and that is not loose language: while the surface is wrong, typing into it is either impossible or unsafe, and a person who cannot trust what is on screen stops typing.\n\n");

    md.push_str("| probe | count | shape |\n| :--- | ---: | :--- |\n");
    for (cat, name) in [
        ("input", "keystroke"),
        ("input", "pty"),
        ("input", "render"),
        ("input", "loop_block"),
        ("input", "unconsumed"),
    ] {
        let ts = stamps(&recs, cat, name);
        md.push_str(&format!("| `{cat}/{name}` | {} | {} |\n", ts.len(), shape(&ts)));
    }

    let keystroke = count(&recs, "input", "keystroke");
    let pty = count(&recs, "input", "pty");
    if pty > keystroke.saturating_mul(10) {
        md.push_str(&format!(
            "\n⛔ **`pty` ({pty}) dwarfs `keystroke` ({keystroke}), and that is NOT a lost-keystroke measurement.** `input/pty` counts every write into a PTY, and on a fleet almost all of those are an agent's own output rather than a person's fingers. ⇒ The end-to-end keystroke latency this notebook wants — key → pty → render — cannot be computed by dividing these, because they do not count the same population. The probe that would close it is a keystroke id carried through to the render that displays it.\n"
        ));
    }

    // ui/block by subject — what is actually holding the thread.
    let mut subjects: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut severe = 0usize;
    for r in recs.iter().filter(|r| r.category == "ui" && r.name == "block") {
        let s = r.payload.get("subject").and_then(|v| v.as_str()).unwrap_or("(not recorded)").to_string();
        *subjects.entry(s).or_insert(0) += 1;
        if r.payload.get("severity").and_then(|v| v.as_str()) == Some("error") {
            severe += 1;
        }
    }
    let total: usize = subjects.values().sum();
    md.push_str(&format!("\n### What is holding the UI thread — {total} blocks, {severe} severe\n\n| subject | blocks | share |\n| :--- | ---: | ---: |\n"));
    let mut rows: Vec<_> = subjects.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1));
    for (s, n) in rows.iter().take(8) {
        md.push_str(&format!("| `{s}` | {n} | {:.0}% |\n", if total > 0 { (*n as f64 / total as f64) * 100.0 } else { 0.0 }));
    }
    let agent_probes: usize = rows.iter().filter(|(s, _)| s.starts_with("app_control/")).map(|(_, n)| *n).sum();
    if total > 0 {
        md.push_str(&format!(
            "\n⚠ **{:.0}% of these blocks are `app_control/*` — agent probes, none of them the user's.** Every read an agent makes of the GUI is paid for in the typing latency of whoever is at the keyboard, which is why the standing instruction is to batch reads and prefer the trace file to a live probe.\n",
            (agent_probes as f64 / total as f64) * 100.0
        ));
    }
    md.push_str("\n⛔ **A block with no `subject` is not an unimportant one.** It is one where the last activity before the gap was not recorded, so it is precisely the block nobody can chase — and it is usually the largest bucket.\n");
    md
}

/// ⭐ THE PAGE THE WHOLE NOTEBOOK IS FOR: where the chain has no probe.
pub fn probe_gaps_md(blocking: bool) -> String {
    let Some(recs) = records(blocking) else {
        return format!("## Where the chain has no probe\n\n{}", collecting("trace"));
    };
    let present: std::collections::BTreeSet<String> =
        recs.iter().map(|r| format!("{}/{}", r.category, r.name)).collect();

    let mut md = String::from("## Where the chain has no probe\n\n");
    md.push_str("> ⛔ **A missing probe and a quiet system look identical.** Everything else in this book is a count; this page is the map of what those counts can and cannot mean. Three states, never collapsed into two:\n>\n> * ✅ **seen** — the probe exists and fired here\n> * ⚠ **named, not seen** — the probe exists in code and was silent, which may be good news\n> * ⛔ **no probe** — nothing would have recorded this even if it happened\n\n");

    // (link, probe or None, what it would settle)
    let chain: [(&str, Option<&str>, &str); 12] = [
        ("1 · kernel — a PTY child is forked and exec'd", None, "how many processes a mount storm really creates; the 400 ms `/proc` sampler cannot see one that lives 40 ms"),
        ("2 · kernel — the UI thread is or is not scheduled", None, "whether a freeze is the app blocking or the kernel not running it. `ui/block` records the gap and cannot say which"),
        ("3 · daemon — a launch is requested", Some("session/request_terminal_launch_for_active_begin"), "that the daemon was asked at all"),
        ("4 · daemon — a session is born", Some("session/live_session_birth"), "a new row entering the set"),
        ("5 · daemon — PTY bytes accepted", Some("input/pty"), "the write reached the pty"),
        ("6 · client — the mount begins", Some("terminal_mount/begin"), "the surface exists and is empty"),
        ("7 · client — the surface is reset under it", Some("terminal_mount/bootstrap_reset"), "the churn itself"),
        ("8 · client → js — the JS terminal reports ready", Some("terminal_mount/js_ready"), "xterm.js is alive"),
        ("9 · xterm.js — bytes are flushed in", Some("terminal_js/xterm_write_flush"), "the glyphs were handed over"),
        ("10 · xterm.js — a frame is painted", Some("xterm_render/frame_window"), "how many frames, and how many painted the whole canvas"),
        ("11 · pixel — the glyphs are on the screen", None, "that a PERSON saw it. Everything above says only that the software believed it painted"),
        ("12 · origin — WHO caused this switch", None, "user gesture vs internal. Until this exists, a person clicking between rows and an app-driven switch produce an identical trace, and every session re-argues it from the same ambiguous evidence"),
    ];

    md.push_str("| link in the chain | state | what it settles |\n| :--- | :--- | :--- |\n");
    let mut gaps = 0;
    for (link, probe, settles) in chain {
        let state = match probe {
            None => {
                gaps += 1;
                "⛔ **no probe**".to_string()
            }
            Some(p) => {
                let n = recs.iter().filter(|r| format!("{}/{}", r.category, r.name) == p).count();
                if n > 0 {
                    format!("✅ `{p}` — {n}")
                } else if present.iter().any(|k| k.starts_with(p.split('/').next().unwrap_or(p))) {
                    format!("⚠ `{p}` named, not seen")
                } else {
                    format!("⚠ `{p}` — no record of this category at all in the window")
                }
            }
        };
        md.push_str(&format!("| {link} | {state} | {settles} |\n"));
    }

    md.push_str(&format!("\n**{gaps} of {} links have no probe at all.**\n\n", chain.len()));
    md.push_str("⭐ **And two of the four gaps are the cheap ones.** An origin field on an activation event and a frame id carried from a write to the paint it produced are both small changes, and between them they would settle the two questions this LEGENDARY entry currently cannot answer: whether the active row switches by itself, and whether a frame showed the bytes it was given.\n\n");
    md.push_str("⚠ **Correction this page carries against the written entry.** The entry records that the xterm.js half is the gap and that a partially-painted frame and a fully-painted one are indistinguishable. On this host that is not so: `xterm_render/frame_gap` carries `rows_painted` against `rows`, and `frame_window` carries `full_canvas_frames` against `count`. The paint layer IS instrumented. What is genuinely missing is the link from a frame back to the bytes that caused it, and the link from a painted frame to a person's eye.\n");

    md.push_str("\n### Everything the trace actually holds, for the record\n\n");
    let mut cats: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for r in recs.iter() {
        *cats.entry(r.category.clone()).or_insert(0) += 1;
    }
    md.push_str(&format!("{} distinct probes across {} categories in the window: ", present.len(), cats.len()));
    let mut names: Vec<String> = cats.keys().cloned().collect();
    names.sort();
    md.push_str(&format!("`{}`.\n", names.join("`, `")));
    md.push_str("\n⛔ **A category absent from that list is not a category that was quiet.** It is one this window never saw, which may mean the feature never ran, the probe was compiled out, or retention already rolled it away — three very different things that a zero cannot tell apart.\n");
    md
}

/// Dispatch for the Legendary Bugs blocks. Returns `None` for a name this
/// module does not serve, so the caller can report it rather than guess.
pub fn block(kind: &str, blocking: bool) -> Option<String> {
    Some(match kind {
        "chain_map" => chain_map_md(blocking),
        "kernel_half" => kernel_half_md(blocking),
        "ebpf_gap" => ebpf_gap_md(blocking),
        "churn" => churn_md(blocking),
        "mount_ladder" => mount_ladder_md(blocking),
        "paint_chain" => paint_chain_md(blocking),
        "input_chain" => input_chain_md(blocking),
        "probe_gaps" => probe_gaps_md(blocking),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_attribute_change_is_not_a_row_set_change() {
        // ⛔ The narrowing that cost real measurement time: one
        //    `set_session_outline` into a quiet GUI produced no reset at all.
        assert!(changes_the_row_set("create_terminal"));
        assert!(changes_the_row_set("remove_session"));
        assert!(!changes_the_row_set("set_session_outline"));
        assert!(!changes_the_row_set("rename_session"));
        assert!(!changes_the_row_set("describe_rows"));
    }

    #[test]
    fn the_app_control_verb_is_read_in_both_shapes() {
        // Reading only the object shape silently halved the tally.
        let mk = |payload: Value| ytrace::YtraceRecord {
            v: 1,
            ts_ms: 0,
            pid: 0,
            app: "yggterm".into(),
            app_version: String::new(),
            component: "ui".into(),
            category: "app_control".into(),
            name: "request_begin".into(),
            clock: "wall".into(),
            duration_ms: None,
            payload,
        };
        assert_eq!(app_control_kind(&mk(json!({"command": {"kind": "create_terminal"}}))), Some("create_terminal".into()));
        assert_eq!(app_control_kind(&mk(json!({"command": "describe_rows"}))), Some("describe_rows".into()));
        assert_eq!(app_control_kind(&mk(json!({}))), None);
    }

    #[test]
    fn an_unserved_block_is_reported_rather_than_guessed() {
        assert!(block("no-such-block", true).is_none());
    }

    #[test]
    fn a_shape_is_never_drawn_wider_than_its_data() {
        assert_eq!(shape(&[]), "—");
        let recent = now_ms() - 5_000;
        assert!(shape(&[recent]).contains("over"));
    }
}
