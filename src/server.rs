//! ytop's control server and action handler.
//!
//! `GET /ping` (liveness + change stamp), `GET /pane/<id>` (the rich Dioxus schema),
//! `POST /action` (all interactive actions: mode toggle, container uncollapse, jank cleanup, supervision).

use crate::{booter, fleet, notebook, probe, rows, schema, timeline};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TOPOLOGY_EVERY: Duration = Duration::from_secs(2);
const ROWS_EVERY: Duration = Duration::from_secs(4);
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

pub struct ZfsIoSample {
    pub t_ms: u64,
    pub read_bps: i64,
    pub write_bps: i64,
}

pub struct PaneState {
    pub view: schema::View,
    pub machines: Vec<fleet::Machine>,
    pub rows_report: rows::FleetRowsReport,
    pub timeline: timeline::Ring,
    pub zfs_history: std::collections::VecDeque<ZfsIoSample>,
    pub stamp: u64,
}

impl PaneState {
    fn touch(&mut self) {
        self.stamp = self.stamp.wrapping_add(1);
    }
}

pub struct Server {
    pub url: String,
    pub state: Arc<Mutex<PaneState>>,
}

pub fn spawn() -> Result<Server> {
    let listener = TcpListener::bind("127.0.0.1:0").context("binding the ytop control server")?;
    let port = listener.local_addr()?.port();
    let state = Arc::new(Mutex::new(PaneState {
        view: schema::View::default(),
        machines: Vec::new(),
        rows_report: rows::FleetRowsReport::default(),
        timeline: timeline::Ring::new(Duration::from_secs(300), Duration::from_secs(1)),
        zfs_history: std::collections::VecDeque::with_capacity(30),
        stamp: 0,
    }));
    {
        let state = Arc::clone(&state);
        std::thread::spawn(move || {
            for incoming in listener.incoming() {
                let Ok(stream) = incoming else { continue };
                let state = Arc::clone(&state);
                std::thread::spawn(move || handle_conn(stream, &state));
            }
        });
    }
    {
        let state = Arc::clone(&state);
        std::thread::spawn(move || sampler(state));
    }
    Ok(Server { url: format!("http://127.0.0.1:{port}"), state })
}

fn sampler(state: Arc<Mutex<PaneState>>) {
    let mut last_rows = Instant::now() - ROWS_EVERY;
    loop {
        let hosts = fleet::roster();
        let readings = fleet::read_all(&hosts, PROBE_TIMEOUT);
        let machines = fleet::group(readings);
        {
            let mut pane = state.lock().unwrap();
            // ZFS I/O spark history — 2s delta, keep 30 points (60s) like AXIOM ring
            if let Some(m) = machines.iter().find(|m| m.reachable()) {
                let p = m.principal();
                if let Some(io) = p["zfs"]["iostat"].as_object() {
                    let r = io.get("read_bytes_s").and_then(Value::as_i64).unwrap_or(0);
                    let w = io.get("write_bytes_s").and_then(Value::as_i64).unwrap_or(0);
                    let t_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or(Duration::from_secs(0)).as_millis() as u64;
                    pane.zfs_history.push_back(ZfsIoSample { t_ms, read_bps: r, write_bps: w });
                    while pane.zfs_history.len() > 30 {
                        pane.zfs_history.pop_front();
                    }
                }
            }
            pane.machines = machines;
            pane.touch();
        }

        if last_rows.elapsed() >= ROWS_EVERY {
            last_rows = Instant::now();
            let report = rows::scan_all_hosts();
            let mut pane = state.lock().unwrap();
            // AXIOM-like ring: one sample per live row per tick, downsampled to 1s
            for r in &report.rows {
                if r.is_alive {
                    pane.timeline.push(&r.seat, r.cpu_pct, r.rss_kb, 0);
                }
            }
            pane.rows_report = report;
            pane.touch();
        }

        // Adaptive backoff for hot hosts (common: ychrome YouTube @ 50%+ WebProcess).
        // When any host is busy (>120% total busy ≈ 7.5% avg on 16c) or ytop itself
        // shows video-driven WebProcess >50% CPU, stretch the fleet probe from 2s → 5s.
        // Saves ~60% SSH+proc wakeups while video plays, negligible dashboard lag.
        let hot = {
            let pane = state.lock().unwrap();
            pane.machines.iter().any(|m| {
                if !m.reachable() { return false; }
                let p = m.principal();
                let cpu_busy = p["cpu_busy_pct"].as_f64().unwrap_or(0.0);
                if cpu_busy > 120.0 { return true; }
                if let Some(tops) = p["top"].as_array() {
                    tops.iter().any(|t| t["comm"].as_str().unwrap_or("").contains("WebKitWebProces") && t["cpu_pct"].as_f64().unwrap_or(0.0) > 45.0)
                } else { false }
            })
        };
        if hot {
            std::thread::sleep(Duration::from_secs(5));
        } else {
            std::thread::sleep(TOPOLOGY_EVERY);
        }
    }
}

fn handle_conn(stream: TcpStream, state: &Mutex<PaneState>) {
    let Ok(peek) = stream.try_clone() else { return };
    let mut reader = BufReader::new(peek);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let (path, _query) = target.split_once('?').unwrap_or((target, ""));

    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
    }
    let body: Value = if content_length > 0 {
        let mut raw = vec![0u8; content_length];
        if reader.read_exact(&mut raw).is_err() {
            return;
        }
        serde_json::from_slice(&raw).unwrap_or(Value::Null)
    } else {
        Value::Null
    };

    match (method, path) {
        ("GET", "/ping") => {
            let pane = state.lock().unwrap();
            respond(stream, 200, &json!({
                "ok": true,
                "app_name": "Ytop",
                "document_version": pane.stamp.to_string(),
            }));
        }
        ("GET", "/pane/topo") => {
            let _span = crate::trace::span("render", "viewport");
            let pane = state.lock().unwrap();
            respond(stream, 200, &schema::viewport_view(&pane.view, &pane.machines, &pane.rows_report, &pane.timeline, &pane.zfs_history));
        }
        ("GET", "/pane/rail") => {
            let _span = crate::trace::span("render", "rail");
            let pane = state.lock().unwrap();
            respond(stream, 200, &schema::rail_view(&pane.view, &pane.machines, &pane.rows_report));
        }
        ("POST", "/action") => {
            let action = body["action"].as_str().unwrap_or("");
            let value = body["value"].as_str().unwrap_or("");
            let _span = crate::trace::span_with("action", "dispatch", json!({ "action": action, "value": value }));
            let mut pane = state.lock().unwrap();

            if action == "mode" {
                pane.view.mode = value.to_string();
                // Each mode opens on ITS Overview. Carrying the other mode's
                // selection across would leave the viewport on a page that is
                // not on the shelf you are now looking at.
                pane.view.selected_notebook = Some(crate::notebook::OVERVIEW_ID.to_string());
                pane.view.selected_page = Some(crate::notebook::overview_page_id(value));
                if !pane
                    .view
                    .expanded_notebooks
                    .iter()
                    .any(|id| crate::notebook::is_overview(id))
                {
                    pane.view
                        .expanded_notebooks
                        .push(crate::notebook::OVERVIEW_ID.to_string());
                }
                pane.touch();
            } else if let Some(host) = action.strip_prefix("select_host:") {
                pane.view.selected_host = host.to_string();
                pane.touch();
            } else if action == "select_host" {
                pane.view.selected_host = value.to_string();
                pane.touch();
            } else if action == "filter" {
                pane.view.filter = value.to_string();
                pane.touch();
            } else if let Some(cont) = action.strip_prefix("toggle_container:") {
                let cont_name = cont.to_string();
                if let Some(pos) = pane.view.expanded_containers.iter().position(|c| c == &cont_name) {
                    pane.view.expanded_containers.remove(pos);
                } else {
                    pane.view.expanded_containers.push(cont_name);
                }
                pane.touch();
            } else if action == "add_machine_prompt" {
                pane.view.adding_machine = true;
                pane.touch();
            } else if action == "add_machine_cancel" {
                pane.view.adding_machine = false;
                pane.touch();
            } else if action == "add_machine_save" {
                let alias = body.get("values").and_then(|v| v.get("new_machine_alias")).and_then(Value::as_str).unwrap_or("");
                let label = body.get("values").and_then(|v| v.get("new_machine_label")).and_then(Value::as_str).unwrap_or("");
                let is_ygg = body.get("values").and_then(|v| v.get("new_machine_yggdrasil")).and_then(Value::as_bool).unwrap_or(false);
                if !alias.is_empty() {
                    let _ = fleet::add_machine_to_config(alias, label, is_ygg);
                    pane.view.notice = Some(format!("✅ Added machine: {alias}"));
                }
                pane.view.adding_machine = false;
                pane.touch();
            } else if action == "clean_jankbox" {
                match rows::clean_jankbox_on_dev() {
                    Ok(killed) => {
                        pane.view.notice = Some(format!("🧹 Cleaned {killed} leaked/twin processes!"));
                    }
                    Err(e) => {
                        pane.view.notice = Some(format!("⛔ Clean failed: {e}"));
                    }
                }
                pane.touch();
            } else if action == "quota_hold" {
                let _ = booter::set_rate_limit_hold(None, "indefinite", "manual hold from ytop");
                pane.view.notice = Some("⏸ Quota hold activated".to_string());
                pane.touch();
            } else if action == "quota_release" {
                let _ = booter::release_rate_limit_hold(None);
                pane.view.notice = Some("▶ Quota hold released".to_string());
                pane.touch();
            } else if let Some(id) = action.strip_prefix("notebook_toggle:") {
                let nb_id = id.to_string();
                if let Some(pos) = pane.view.expanded_notebooks.iter().position(|x| x == &nb_id) {
                    pane.view.expanded_notebooks.remove(pos);
                } else {
                    pane.view.expanded_notebooks.push(nb_id.clone());
                    pane.view.selected_notebook = Some(nb_id);
                }
                pane.touch();
            } else if let Some(rest) = action.strip_prefix("page_open:") {
                if let Some((nb_id, idx_str)) = rest.split_once(':') {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        pane.view.selected_notebook = Some(nb_id.to_string());
                        if crate::notebook::is_overview(nb_id) {
                            // ⚠ Overview exists once per MODE and both carry the
                            // same id, so resolving it by id alone would open the
                            // Top page while standing in Dash. The mode decides.
                            let mode = pane.view.mode.clone();
                            pane.view.selected_page =
                                Some(crate::notebook::overview_page_id(&mode));
                            pane.view.notice = None;
                        } else if let Some(nb) = crate::notebook::get_notebook(nb_id) {
                            if let Some(page) = nb.pages.get(idx) {
                                pane.view.selected_page = Some(page.id.clone());
                                pane.view.notice = Some(format!("📖 {} — {}", nb.title, page.title));
                            }
                        }
                    }
                }
                pane.touch();
            } else if action == "notebook_compose_dash" || action == "notebook_compose_top" {
                // Skill composition: body["payload"]["title"] + ["pages"] JSON → write notebook file.
                let mode = if action.ends_with("_dash") { "dash" } else { "top" };
                let title = body.get("payload").and_then(|p| p.get("title")).and_then(Value::as_str).unwrap_or("Untitled Notebook").to_string();
                let desc = body.get("payload").and_then(|p| p.get("description")).and_then(Value::as_str).unwrap_or("Composed via ytop skill").to_string();
                let author = body.get("payload").and_then(|p| p.get("author")).and_then(Value::as_str).unwrap_or("agent").to_string();
                let id = format!("{}-{}", mode, title.to_lowercase().replace(|c: char| !c.is_alphanumeric(), "-").trim_matches('-'));
                let pages_raw = body.get("payload").and_then(|p| p.get("pages")).and_then(|v| v.as_array()).cloned().unwrap_or_default();
                let pages: Vec<crate::notebook::Page> = pages_raw.into_iter().filter_map(|pv| {
                    let title = pv.get("title").and_then(Value::as_str)?.to_string();
                    let markdown = pv.get("markdown").and_then(Value::as_str).unwrap_or("").to_string();
                    if let Ok(page) = serde_json::from_value::<crate::notebook::Page>(pv.clone()) {
                        return Some(page);
                    }
                    // Fallback when id/queries are caller-supplied without strict schema: preserve queries/chart.
                    let ytrace_queries = pv
                        .get("ytrace_queries")
                        .and_then(|v| serde_json::from_value::<Vec<crate::notebook::YtraceQuery>>(v.clone()).ok())
                        .unwrap_or_default();
                    let chart = pv.get("chart").and_then(Value::as_str).map(|s| s.to_string());
                    let page_id = pv
                        .get("id")
                        .and_then(Value::as_str)
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("{}-{}", id, title.to_lowercase().replace(|c: char| !c.is_alphanumeric(), "-")));
                    Some(crate::notebook::Page {
                        id: page_id,
                        title,
                        markdown,
                        ytrace_queries,
                        chart,
                        live: pv.get("live").and_then(Value::as_str).map(|s| s.to_string()),
                        // Agent-composed pages are paper, never a composed window.
                        composed: false,
                    })
                }).collect();
                let nb = crate::notebook::Notebook {
                    id: id.clone(),
                    title: title.clone(),
                    mode: mode.to_string(),
                    description: desc,
                    author,
                    created_at_ms: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0),
                    pages: if pages.is_empty() {
                        vec![crate::notebook::Page {
                            id: format!("{id}-p1"),
                            title: "Page 1".to_string(),
                            markdown: format!("# {title}\n\nComposed via ytop skill on host `{}`.", pane.view.selected_host),
                            ytrace_queries: vec![],
                            chart: None,
                            live: None,
                            composed: false,
                        }]
                    } else { pages },
                };
                match crate::notebook::write_notebook(&nb) {
                    Ok(path) => pane.view.notice = Some(format!("✏️ Notebook '{}' written to {}", nb.title, path.display())),
                    Err(e) => pane.view.notice = Some(format!("⛔ Notebook write failed: {e}")),
                }
                pane.touch();
            } else if action == "refresh" {
                pane.touch();
            }

            respond(stream, 200, &json!({"ok": true}));
        }
        _ => {
            respond(stream, 404, &json!({"error": "not found"}));
        }
    }
}

fn respond(mut stream: TcpStream, status: u16, body: &Value) {
    let payload = body.to_string();
    let head = format!(
        "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(payload.as_bytes());
    let _ = stream.flush();
}

/// Print a notebook, or one of its pages, with its live blocks filled in.
///
/// ⛔ THE REPORT IS READ FROM THIS HOST ONLY, and the page says so. The GUI's
/// census fans out over ssh; doing that from a CLI check would make a cheap
/// verification into a fleet-wide one, and a slow instrument gets run less
/// often than a fast one — which is the failure that matters here.
pub fn print_notebook(id: &str, page: Option<usize>) -> Result<()> {
    if id.is_empty() {
        for nb in notebook::list_notebooks(None) {
            println!("{:<22} [{:<4}] {:<2} page(s)  {}", nb.id, nb.mode, nb.pages.len(), nb.title);
        }
        return Ok(());
    }
    let Some(nb) = notebook::get_notebook(id) else {
        anyhow::bail!("no notebook `{id}` — run `ytop --notebook` to list the shelf");
    };

    let Some(n) = page else {
        println!("📖 {}  [{}]\n{}\n", nb.title, nb.mode, nb.description);
        for (idx, p) in nb.pages.iter().enumerate() {
            let mut marks = Vec::new();
            if p.has_ytrace() {
                marks.push("🔬 ytrace".to_string());
            }
            if let Some(l) = &p.live {
                marks.push(format!("🔴 live:{l}"));
            }
            println!("  {}. {}   {}", idx + 1, p.title, marks.join(" · "));
        }
        println!("\nOne page: ytop --notebook {} --page 1", nb.id);
        return Ok(());
    };
    let Some(p) = n.checked_sub(1).and_then(|i| nb.pages.get(i)) else {
        anyhow::bail!("notebook `{}` has {} page(s); asked for {n}", nb.id, nb.pages.len());
    };

    println!("{}\n", p.markdown);
    if p.has_ytrace() {
        let qs: Vec<String> = p
            .ytrace_queries
            .iter()
            .map(|q| format!("ytrace query --app {} --category {} --name {} --since {}s", q.provider, q.category, q.name, q.since_ms / 1000))
            .collect();
        println!("---\n\n**ytrace on this page** (chart `{}`):\n", p.chart.as_deref().unwrap_or("—"));
        for q in qs {
            println!("    {q}");
        }
        println!();
    }
    if let Some(kind) = &p.live {
        // ⛔ `unwrap_or_default()` here is a real risk and the blocks are written
        //    for it: a probe that failed becomes an EMPTY report, which every
        //    live block must render as "nothing could be read" rather than as a
        //    fleet with no seats in it.
        let report = rows::probe_rows(None, std::time::Duration::from_secs(20)).unwrap_or_default();
        println!("---\n");
        for w in crate::sysinternals::live_widgets(kind, &p.id, &report, true) {
            if let Some(src) = w["source"].as_str() {
                println!("{src}");
            }
        }
        println!("\n> This block was read on this host alone. The GUI's census fans out across the fleet;\n> a seat that lives on another machine is absent here, which is not the same as absent.");
    }
    Ok(())
}

/// The window every `--once` complaint view is read over.
///
/// Stated, bounded, and printed beside every number it produces. An unstated
/// window is how a lifetime tally passes for a rate.
const COMPLAINT_WINDOW: std::time::Duration = std::time::Duration::from_secs(3600);

pub fn print_once(mode: &str, tab: &str, as_json: bool) -> Result<()> {
    if mode == schema::MODE_DASH {
        let report = rows::scan_all_hosts();
        let (conditions, total_records) =
            crate::complaints::read_live("yggterm", COMPLAINT_WINDOW);
        if as_json {
            println!("{}", serde_json::to_string_pretty(&json!({
                "tab": tab,
                "rows": report,
                "complaints": {
                    "window_secs": COMPLAINT_WINDOW.as_secs(),
                    "records": total_records,
                    "conditions": conditions.iter().map(|c| json!({
                        "incident_id": c.incident_id,
                        "severity": c.severity,
                        "samples": c.samples,
                        "span_secs": c.span_secs,
                        "emitters": c.emitters.iter().map(|(pid, ver, age)| json!({
                            "pid": pid, "version": ver, "age_secs": age,
                        })).collect::<Vec<_>>(),
                        "diagnosis": c.diagnosis,
                        "untrustworthy_fields": c.untrustworthy_fields().iter()
                            .map(|(f, v)| json!({"field": f, "caveat": v.caveat()}))
                            .collect::<Vec<_>>(),
                        "climbing_levels": c.cumulative_fields(),
                    })).collect::<Vec<_>>(),
                },
            }))?);
            return Ok(());
        }
        // The complaint plane is what Dash is for; the tab chooses the lens.
        if tab == "jankbox" {
            println!("{}", crate::complaints::render(&conditions, total_records, COMPLAINT_WINDOW));
            let j = &report.jankbox;
            println!("  ── JANKBOX ────────────────────────────────────────────────────────────");
            println!("  spinning subshells : {:?}", j.leaked_subshell_pids);
            println!("  stale twin pids    : {:?}", j.twin_stale_pids);
            println!("  total jank procs   : {}", j.total_jank_procs);
            if j.bloated_transcripts_mb.is_empty() {
                println!("  bloated transcripts: none");
            } else {
                println!("  bloated transcripts:");
                for (uuid, mb) in &j.bloated_transcripts_mb {
                    println!("      {uuid}  {mb:.1} MB");
                }
            }
            return Ok(());
        }
        if tab == "supervision" {
            println!("  ── SUPERVISION ────────────────────────────────────────────────────────");
            match &report.quota_hold {
                Some(h) => println!("  ⏸ QUOTA HOLD ACTIVE: {h}"),
                None => println!("  quota hold: none"),
            }
            let mut by_state: std::collections::BTreeMap<&str, Vec<&str>> =
                std::collections::BTreeMap::new();
            for r in &report.rows {
                by_state.entry(r.supervision_state.as_str()).or_default().push(&r.seat);
            }
            for (state, seats) in by_state {
                println!("  {:<22} {:>3} row(s)  {}", state, seats.len(), seats.join(" "));
            }
            return Ok(());
        }
        println!("==========================================================================================================");
        println!("  Y T O P   ·   F L E E T   A G E N T   R O W S   &   J A N K B O X   D A S H B O A R D");
        println!("==========================================================================================================");
        if let Some(hold) = &report.quota_hold {
            println!("  ⏸ QUOTA HOLD ACTIVE: {hold}\n");
        }
        println!("  {:<6} {:<13} {:<14} {:<10} {:<30} {:<20} {}",
            "SEAT", "ROLE", "CAMPAIGN", "UUID", "PROCESS STATUS", "TRANSCRIPT", "SUPERVISION");
        println!("  {}", "-".repeat(110));

        for r in &report.rows {
            let proc_str = if r.twin_alert {
                format!("⛔ TWIN ({:?})", r.pids)
            } else if !r.pids.is_empty() {
                if r.leaked_child_loops > 0 {
                    format!("LIVE {:?} + ⚠️{} leaks", r.pids, r.leaked_child_loops)
                } else {
                    format!("LIVE {:?}", r.pids)
                }
            } else {
                "💀 DEAD".to_string()
            };

            let t_str = format!("{} KB ({}L)", r.transcript_size_kb, r.transcript_lines);
            let short_u = if r.uuid.len() >= 8 { &r.uuid[..8] } else { &r.uuid };

            println!("  {:<6} {:<13} {:<14} {:<10} {:<30} {:<20} {}",
                r.seat, r.role, r.campaign, short_u, proc_str, t_str, r.supervision_state);
        }
        println!("  {}", "=".repeat(110));
        println!("  Total Seats: {}  ·  Live: {}  ·  Agent CPU: {:.1}%  ·  Agent RAM: {:.1} MB  ·  Context: {:.1} MB",
            report.total_rows, report.live_count, report.total_agent_cpu_pct, report.total_agent_rss_mb, report.total_transcript_mb);
        println!("  Jankbox Leaks: {} spinning subshells  ·  Twin Duplicate Alarms: {}",
            report.leak_count, report.twin_count);
        println!("{}", crate::complaints::render(&conditions, total_records, COMPLAINT_WINDOW));
        return Ok(());
    }

    // Default: TOP mode
    let hosts = fleet::roster();
    let readings = fleet::read_all(&hosts, PROBE_TIMEOUT);
    let machines = fleet::group(readings);
    if as_json {
        println!("{}", json!({
            "hosts": hosts,
            "machines": machines.iter().map(|m| json!({
                "key": m.key,
                "reachable": m.reachable(),
                "hosts": m.readings.iter().map(|r| r["host"].clone()).collect::<Vec<_>>(),
                "readings": m.readings,
            })).collect::<Vec<_>>(),
        }));
        return Ok(());
    }
    for machine in machines {
        let principal = machine.principal();
        if !machine.reachable() {
            println!(
                "⚠ {} — could not be read: {}",
                principal["label"].as_str().or(principal["host"].as_str()).unwrap_or("?"),
                principal["error"].as_str().unwrap_or("no reason given")
            );
            continue;
        }
        println!(
            "\n🖥️  {} · {} × {} · kernel {}",
            principal["hostname"].as_str().unwrap_or("?"),
            principal["cpu_count"].as_i64().unwrap_or(0),
            principal["cpu_model"].as_str().unwrap_or("?"),
            principal["kernel"].as_str().unwrap_or("?"),
        );
        for reading in &machine.readings {
            println!(
                "  ├─ {:<12} {:>6.1}% cpu  {:>5} procs  ({})",
                reading["label"].as_str().or(reading["host"].as_str()).unwrap_or("?"),
                reading["cpu_busy_pct"].as_f64().unwrap_or(0.0),
                reading["procs_total"].as_i64().unwrap_or(0),
                reading["virt"].as_str().unwrap_or("?"),
            );
        }

        // ZFS Pool info if available
        let zfs = &principal["zfs"];
        if zfs["has_zfs"].as_bool().unwrap_or(false) {
            for pool in zfs["pools"].as_array().cloned().unwrap_or_default() {
                println!(
                    "  ├─ 💾 zpool: {:<8} [{}] · {} alloc / {} total (frag {}%)",
                    pool["name"].as_str().unwrap_or("?"),
                    pool["health"].as_str().unwrap_or("?"),
                    schema::bytes_to_human(pool["alloc_bytes"].as_i64().unwrap_or(0)),
                    schema::bytes_to_human(pool["size_bytes"].as_i64().unwrap_or(0)),
                    pool["frag_pct"].as_i64().unwrap_or(0),
                );
            }
        }

        // LXC Containers with process details
        for container in principal["containers"].as_array().cloned().unwrap_or_default() {
            let c_name = container["name"].as_str().unwrap_or("?");
            let c_state = container["state"].as_str().unwrap_or("?");
            let c_cpu = container["cpu_busy_pct"].as_f64().unwrap_or(0.0);
            let c_rss = container["mem_rss_kb"].as_i64().unwrap_or(0);
            println!(
                "  ├─ 📦 {:<12} [{}] · {:>5.1}% cpu  {:>8} ram",
                c_name, c_state, c_cpu, schema::mb(c_rss)
            );
            for p in container["top_procs"].as_array().cloned().unwrap_or_default() {
                println!(
                    "  │   └─ PID {:<7} {:>5.1}% cpu  {:>8}   {}",
                    p["pid"].as_i64().unwrap_or(0),
                    p["cpu_pct"].as_f64().unwrap_or(0.0),
                    schema::mb(p["rss_kb"].as_i64().unwrap_or(0)),
                    p["comm"].as_str().unwrap_or("?")
                );
            }
        }

        for p in principal["top"].as_array().cloned().unwrap_or_default().iter().take(5) {
            println!(
                "     {:>6.1}%  {:>8} KB  {}",
                p["cpu_pct"].as_f64().unwrap_or(0.0),
                p["rss_kb"].as_i64().unwrap_or(0),
                p["comm"].as_str().unwrap_or("?"),
            );
        }
    }
    Ok(())
}

pub fn probe_once(host: Option<&str>) -> Result<()> {
    println!("{}", probe::read_host(host, PROBE_TIMEOUT));
    Ok(())
}
