//! ytop's control server and action handler.
//!
//! `GET /ping` (liveness + change stamp), `GET /pane/<id>` (the rich Dioxus schema),
//! `POST /action` (all interactive actions: mode toggle, container uncollapse, jank cleanup, supervision).

use crate::{booter, fleet, probe, rows, schema, timeline};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TOPOLOGY_EVERY: Duration = Duration::from_secs(2);
const ROWS_EVERY: Duration = Duration::from_secs(4);
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

pub struct PaneState {
    pub view: schema::View,
    pub machines: Vec<fleet::Machine>,
    pub rows_report: rows::FleetRowsReport,
    pub timeline: timeline::Ring,
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

        std::thread::sleep(TOPOLOGY_EVERY);
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
            let pane = state.lock().unwrap();
            respond(stream, 200, &schema::viewport_view(&pane.view, &pane.machines, &pane.rows_report, &pane.timeline));
        }
        ("GET", "/pane/rail") => {
            let pane = state.lock().unwrap();
            respond(stream, 200, &schema::rail_view(&pane.view, &pane.machines, &pane.rows_report));
        }
        ("POST", "/action") => {
            let action = body["action"].as_str().unwrap_or("");
            let value = body["value"].as_str().unwrap_or("");
            let mut pane = state.lock().unwrap();

            if action == "mode" {
                pane.view.mode = value.to_string();
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

pub fn print_once(mode: &str, as_json: bool) -> Result<()> {
    if mode == schema::MODE_DASH {
        let report = rows::scan_all_hosts();
        if as_json {
            println!("{}", serde_json::to_string_pretty(&report)?);
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
