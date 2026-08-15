//! The widget schema — what yggterm paints in both Viewport and Rail surfaces.
//!
//! ⛔ ZERO RAW MARKDOWN DUMPS.
//! Viewport: SaaS-style GUI composition with native Dioxus cards, progress meters, and process trees.
//! Rail: Focused machine switcher (in Top mode) or supervision/jankbox control panel (in Dash mode).
//! Titlebar Switch: Dynamic app-driven Top ↔ Dash mode toggle.

use crate::fleet::{self, Machine};
use crate::rows::{FleetRowsReport, RowInfo};
use serde_json::{json, Value};

pub const MODE_TOP: &str = "top";
pub const MODE_DASH: &str = "dash";

#[derive(Debug, Clone)]
pub struct View {
    pub mode: String,
    pub selected_host: String,
    pub expanded_containers: Vec<String>,
    pub filter: String,
    pub notice: Option<String>,
    pub adding_machine: bool,
    pub new_machine_alias: String,
    pub new_machine_label: String,
    pub new_machine_is_yggdrasil: bool,
}

impl Default for View {
    fn default() -> Self {
        Self {
            mode: MODE_TOP.to_string(),
            selected_host: fleet::LOCAL.to_string(),
            expanded_containers: Vec::new(),
            filter: String::new(),
            notice: None,
            adding_machine: false,
            new_machine_alias: String::new(),
            new_machine_label: String::new(),
            new_machine_is_yggdrasil: false,
        }
    }
}

// ─── Formatting helpers ────────────────────────────────────────────────────────

pub fn gb(kb: i64) -> String {
    format!("{:.1} GB", kb as f64 / 1024.0 / 1024.0)
}

pub fn mb(kb: i64) -> String {
    if kb >= 1024 * 1024 {
        gb(kb)
    } else {
        format!("{} MB", kb / 1024)
    }
}

pub fn bytes_to_human(bytes: i64) -> String {
    let gb = bytes as f64 / 1024.0 / 1024.0 / 1024.0;
    if gb >= 1024.0 {
        format!("{:.1} TB", gb / 1024.0)
    } else if gb >= 1.0 {
        format!("{:.1} GB", gb)
    } else {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    }
}

pub fn duration(secs: f64) -> String {
    let s = secs.max(0.0) as i64;
    let (d, h, m) = (s / 86400, (s % 86400) / 3600, (s % 3600) / 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}

pub fn progress_bar(pct: f64, width: usize) -> String {
    let clamped = pct.clamp(0.0, 100.0);
    let filled = ((clamped / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}] {:.1}%", "█".repeat(filled), "░".repeat(empty), clamped)
}

fn label(text: impl Into<String>) -> Value {
    json!({"kind": "label", "text": text.into()})
}

fn section(text: impl Into<String>, card: bool) -> Value {
    json!({"kind": "section", "text": text.into(), "card": card})
}

fn matches_filter(filter: &str, haystack: &[&str]) -> bool {
    if filter.trim().is_empty() {
        return true;
    }
    let needle = filter.to_lowercase();
    haystack.iter().any(|h| h.to_lowercase().contains(&needle))
}

fn titlebar_switch_spec(active_mode: &str) -> Value {
    json!({
        "active": active_mode,
        "action": "mode",
        "segments": [
            {"id": MODE_TOP, "label": "⚡ Top", "title": "Infrastructure & Machine Topology"},
            {"id": MODE_DASH, "label": "📊 Dash", "title": "Agent Fleet & Jankbox Cockpit"},
        ]
    })
}

// ─── RAIL VIEW (Sidebar in Top vs Dash modes) ──────────────────────────────────

pub fn rail_view(view: &View, machines: &[Machine], report: &FleetRowsReport) -> Value {
    let mut widgets = Vec::new();

    if let Some(notice) = &view.notice {
        widgets.push(label(notice.clone()));
    }

    if view.mode == MODE_TOP {
        // TOP MODE RAIL: Only Machines and Add Button
        widgets.push(section("Connected Machines", true));

        for m in machines {
            let p = m.principal();
            let host = p["host"].as_str().unwrap_or("?");
            let shown = p["label"].as_str().unwrap_or(host);
            let cpu = p["cpu_busy_pct"].as_f64().unwrap_or(0.0);
            let mem_total = p["mem_total_kb"].as_i64().unwrap_or(0);
            let mem_avail = p["mem_available_kb"].as_i64().unwrap_or(0);
            let mem_used = (mem_total - mem_avail).max(0);
            let is_selected = host == view.selected_host || (view.selected_host == fleet::LOCAL && host == "local");

            let title = if m.reachable() {
                format!("{shown}  ·  {cpu:.0}% CPU  ·  {}", gb(mem_used))
            } else {
                format!("{shown}  (unreachable)")
            };

            widgets.push(json!({
                "kind": "list-row",
                "id": format!("host:{host}"),
                "title": title,
                "status": if m.reachable() { "durable" } else { "transient" },
                "selected": is_selected,
                "row_action": format!("select_host:{host}"),
            }));
        }

        if view.adding_machine {
            widgets.push(section("➕ Add SSH Machine", true));
            widgets.push(json!({
                "kind": "text-input",
                "id": "new_machine_alias",
                "label": "SSH Alias / Host",
                "value": view.new_machine_alias,
                "placeholder": "main",
            }));
            widgets.push(json!({
                "kind": "text-input",
                "id": "new_machine_label",
                "label": "Display Label",
                "value": view.new_machine_label,
                "placeholder": "Main Server",
            }));
            widgets.push(json!({
                "kind": "toggle",
                "id": "new_machine_yggdrasil",
                "label": "Yggdrasil Hypervisor (ZFS/LXC)",
                "value": view.new_machine_is_yggdrasil,
            }));
            widgets.push(json!({
                "kind": "button",
                "id": "save_machine_btn",
                "label": "Save Machine",
                "action": "add_machine_save",
                "primary": true,
            }));
            widgets.push(json!({
                "kind": "button",
                "id": "cancel_machine_btn",
                "label": "Cancel",
                "action": "add_machine_cancel",
            }));
        } else {
            widgets.push(json!({
                "kind": "button",
                "id": "add_machine_prompt_btn",
                "label": "➕ Add SSH Machine",
                "action": "add_machine_prompt",
            }));
        }
    } else {
        // DASH MODE RAIL: Fleet Overview, Supervision & Jankbox Controls
        widgets.push(section("Fleet Overview", true));
        widgets.push(label(format!(
            "Total Seats: {}  ·  Live: {}",
            report.total_rows, report.live_count
        )));
        widgets.push(label(format!(
            "Agent CPU: {:.1}%  ·  Agent RAM: {:.1} MB",
            report.total_agent_cpu_pct, report.total_agent_rss_mb
        )));
        widgets.push(label(format!(
            "Total Context: {:.1} MB",
            report.total_transcript_mb
        )));

        // Supervision
        widgets.push(section("Supervision Controls", true));
        if let Some(hold) = &report.quota_hold {
            widgets.push(label(format!("⏸ Quota Hold Active: {hold}")));
            widgets.push(json!({
                "kind": "button",
                "id": "release_hold_btn",
                "label": "▶ Release Quota Hold",
                "action": "quota_release",
                "primary": true,
            }));
        } else {
            widgets.push(label("Supervision: Active"));
            widgets.push(json!({
                "kind": "button",
                "id": "set_hold_btn",
                "label": "⏸ Set Quota Hold",
                "action": "quota_hold",
            }));
        }

        // Jankbox
        widgets.push(section("Jankbox Diagnostics", true));
        widgets.push(label(format!(
            "Orphaned Leaks: {} loops  ·  Twins: {}",
            report.leak_count, report.twin_count
        )));
        if report.jankbox.total_jank_procs > 0 {
            widgets.push(json!({
                "kind": "button",
                "id": "clean_jankbox_btn",
                "label": "🧹 Clean Leaks & Twins",
                "action": "clean_jankbox",
                "primary": true,
                "danger": true,
            }));
        }
    }

    json!({
        "title": if view.mode == MODE_TOP { "Machines" } else { "Fleet Control" },
        "titlebar_switch": titlebar_switch_spec(&view.mode),
        "widgets": widgets,
        "footer": [
            json!({"kind": "button", "id": "refresh_rail", "action": "refresh", "label": "Refresh"}),
        ]
    })
}

// ─── VIEWPORT VIEW (SaaS Dashboard in Top vs Dash modes) ───────────────────────

pub fn viewport_view(view: &View, machines: &[Machine], report: &FleetRowsReport) -> Value {
    let mut widgets = Vec::new();

    if let Some(notice) = &view.notice {
        widgets.push(label(notice.clone()));
    }

    if view.mode == MODE_TOP {
        // TOP MODE VIEWPORT: Complete GUI htop view for selected machine
        let target_machine = machines
            .iter()
            .find(|m| {
                m.readings
                    .iter()
                    .any(|r| r["host"].as_str() == Some(&view.selected_host) || (view.selected_host == "local" && r["host"].as_str() == Some(fleet::LOCAL)))
            })
            .or_else(|| machines.first());

        if let Some(m) = target_machine {
            let p = m.principal();
            let host = p["host"].as_str().unwrap_or("?");
            let shown = p["label"].as_str().unwrap_or(host);

            if !m.reachable() {
                widgets.push(section(format!("Host {shown} Unreachable"), true));
                widgets.push(label(format!(
                    "Connection failed: {}",
                    p["error"].as_str().unwrap_or("timed out")
                )));
            } else {
                let total_kb = p["mem_total_kb"].as_i64().unwrap_or(0);
                let avail_kb = p["mem_available_kb"].as_i64().unwrap_or(0);
                let used_kb = (total_kb - avail_kb).max(0);
                let mem_pct = if total_kb > 0 { used_kb as f64 * 100.0 / total_kb as f64 } else { 0.0 };
                let swap_total_kb = p["swap_total_kb"].as_i64().unwrap_or(0);
                let swap_free_kb = p["swap_free_kb"].as_i64().unwrap_or(0);
                let swap_used_kb = (swap_total_kb - swap_free_kb).max(0);
                let swap_pct = if swap_total_kb > 0 { swap_used_kb as f64 * 100.0 / swap_total_kb as f64 } else { 0.0 };
                let cpu_busy = p["cpu_busy_pct"].as_f64().unwrap_or(0.0);
                let load = p["load"]
                    .as_array()
                    .map(|l| {
                        l.iter().filter_map(|v| v.as_f64()).map(|v| format!("{v:.2}")).collect::<Vec<_>>().join(" ")
                    })
                    .unwrap_or_default();

                // Card 1: System Hardware & Resource Gauges
                widgets.push(section(format!("🖥️ System Health: {shown}"), true));
                widgets.push(label(format!(
                    "Kernel {} · Arch {} · Uptime {} · Load [ {load} ] · Procs: {}",
                    p["kernel"].as_str().unwrap_or("?"),
                    p["arch"].as_str().unwrap_or("?"),
                    duration(p["uptime_s"].as_f64().unwrap_or(0.0)),
                    p["procs_total"].as_i64().unwrap_or(0)
                )));
                widgets.push(label(format!(
                    "CPU Usage:  {}  ({} Cores · {})",
                    progress_bar(cpu_busy, 24),
                    p["cpu_count"].as_i64().unwrap_or(0),
                    p["cpu_model"].as_str().unwrap_or("unknown")
                )));
                widgets.push(label(format!(
                    "Memory RAM: {}  ({} / {})",
                    progress_bar(mem_pct, 24),
                    gb(used_kb),
                    gb(total_kb)
                )));
                if swap_total_kb > 0 {
                    widgets.push(label(format!(
                        "Swap Space: {}  ({} / {})",
                        progress_bar(swap_pct, 24),
                        mb(swap_used_kb),
                        mb(swap_total_kb)
                    )));
                }

                // Card 2: ZFS Storage & IOSTAT (if present)
                let zfs = &p["zfs"];
                if zfs["has_zfs"].as_bool().unwrap_or(false) {
                    widgets.push(section("💾 ZFS Storage Pools & Real-Time IOSTAT", true));
                    if let Some(pools) = zfs["pools"].as_array() {
                        for pool in pools {
                            let name = pool["name"].as_str().unwrap_or("?");
                            let health = pool["health"].as_str().unwrap_or("UNKNOWN");
                            let cap_pct = pool["cap_pct"].as_f64().unwrap_or(0.0);
                            let size_b = pool["size_bytes"].as_i64().unwrap_or(0);
                            let alloc_b = pool["alloc_bytes"].as_i64().unwrap_or(0);
                            let frag_pct = pool["frag_pct"].as_i64().unwrap_or(0);

                            widgets.push(label(format!(
                                "Pool [{name}]  {health}  ·  Allocation {}  ({} / {}, frag {frag_pct}%)",
                                progress_bar(cap_pct, 20),
                                bytes_to_human(alloc_b),
                                bytes_to_human(size_b)
                            )));
                        }
                    }
                    if let Some(io) = zfs["iostat"].as_object() {
                        let r_ops = io.get("read_ops").and_then(Value::as_i64).unwrap_or(0);
                        let w_ops = io.get("write_ops").and_then(Value::as_i64).unwrap_or(0);
                        let r_bytes = io.get("read_bytes_s").and_then(Value::as_i64).unwrap_or(0);
                        let w_bytes = io.get("write_bytes_s").and_then(Value::as_i64).unwrap_or(0);
                        widgets.push(label(format!(
                            "Throughput: Read {}/s ({} IOPS)  ·  Write {}/s ({} IOPS)",
                            bytes_to_human(r_bytes),
                            r_ops,
                            bytes_to_human(w_bytes),
                            w_ops
                        )));
                    }
                }

                // Card 3: LXC Containers & Process Trees
                let containers = p["containers"].as_array().cloned().unwrap_or_default();
                if !containers.is_empty() {
                    widgets.push(section(format!("📦 LXC Containers ({} total)", containers.len()), true));
                    for c in &containers {
                        let c_name = c["name"].as_str().unwrap_or("?");
                        let state = c["state"].as_str().unwrap_or("?");
                        let c_cpu = c["cpu_busy_pct"].as_f64().unwrap_or(0.0);
                        let c_rss = c["mem_rss_kb"].as_i64().unwrap_or(0);
                        let procs_count = c["procs_count"].as_i64().unwrap_or(0);
                        let is_expanded = view.expanded_containers.contains(&c_name.to_string());

                        let title_text = format!(
                            "{} {} [{state}] · {c_cpu:.1}% CPU · {} RAM ({procs_count} procs)",
                            if is_expanded { "▼" } else { "▶" },
                            c_name,
                            mb(c_rss)
                        );

                        widgets.push(json!({
                            "kind": "list-row",
                            "id": format!("container:{c_name}"),
                            "title": title_text,
                            "status": if state == "RUNNING" { "transient" } else { "" },
                            "row_action": format!("toggle_container:{c_name}"),
                        }));

                        if is_expanded {
                            let top_procs = c["top_procs"].as_array().cloned().unwrap_or_default();
                            for tp in top_procs {
                                let pid = tp["pid"].as_i64().unwrap_or(0);
                                let comm = tp["comm"].as_str().unwrap_or("?");
                                let cpu_p = tp["cpu_pct"].as_f64().unwrap_or(0.0);
                                let rss = tp["rss_kb"].as_i64().unwrap_or(0);
                                let user = tp["user"].as_str().unwrap_or("?");
                                widgets.push(json!({
                                    "kind": "list-row",
                                    "id": format!("proc:{c_name}:{pid}"),
                                    "title": format!("    PID {pid:<7} {cpu_p:>5.1}% CPU   {:>8}   {comm} ({user})", mb(rss)),
                                }));
                            }
                        }
                    }
                }

                // Card 4: Host Top Processes
                let top_procs = p["top"].as_array().cloned().unwrap_or_default();
                if !top_procs.is_empty() {
                    widgets.push(section(format!("⚡ Top Processes: {shown}"), true));
                    widgets.push(json!({
                        "kind": "search-box",
                        "id": "filter",
                        "action": "filter",
                        "value": view.filter,
                        "placeholder": "filter processes",
                    }));

                    for proc in top_procs {
                        let pid = proc["pid"].as_i64().unwrap_or(0);
                        let comm = proc["comm"].as_str().unwrap_or("?");
                        let cpu_pct = proc["cpu_pct"].as_f64().unwrap_or(0.0);
                        let rss_kb = proc["rss_kb"].as_i64().unwrap_or(0);
                        let user = proc["user"].as_str().unwrap_or("?");
                        let cmd = proc["cmd"].as_str().unwrap_or("");
                        let cont = proc.get("container").and_then(Value::as_str);

                        if !matches_filter(&view.filter, &[comm, cmd, user, &pid.to_string()]) {
                            continue;
                        }

                        let scope = if let Some(cn) = cont {
                            format!("ct:{cn}")
                        } else {
                            "host".to_string()
                        };

                        widgets.push(json!({
                            "kind": "list-row",
                            "id": format!("top_proc:{host}:{pid}"),
                            "title": format!("PID {pid:<7} {cpu_pct:>5.1}% CPU   {:>8}   {comm}  ({scope})", mb(rss_kb)),
                            "status": if cpu_pct > 50.0 { "transient" } else { "" },
                        }));
                    }
                }
            }
        }
    } else {
        // DASH MODE VIEWPORT: Full Agent Fleet Cockpit
        widgets.push(section("Agent Fleet Rows Census", true));
        widgets.push(json!({
            "kind": "search-box",
            "id": "filter",
            "action": "filter",
            "value": view.filter,
            "placeholder": "filter rows by seat, campaign, role, or uuid",
        }));

        for (campaign, rows) in &report.campaigns {
            let matching_rows: Vec<&RowInfo> = rows
                .iter()
                .filter(|r| {
                    matches_filter(
                        &view.filter,
                        &[&r.seat, &r.campaign, &r.role, &r.uuid, &r.title, &r.supervision_state],
                    )
                })
                .collect();

            if matching_rows.is_empty() {
                continue;
            }

            widgets.push(section(format!("Campaign: {campaign} ({} rows)", rows.len()), true));

            for r in matching_rows {
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

                let short_uuid = if r.uuid.len() >= 8 { &r.uuid[..8] } else { &r.uuid };
                let title_line = format!(
                    "Seat {:<6} {:<12} [{}]  ·  {proc_str}  ·  {:.1}% CPU  ·  {} RAM  ·  {} KB Context",
                    r.seat,
                    r.title,
                    short_uuid,
                    r.cpu_pct,
                    mb(r.rss_kb),
                    r.transcript_size_kb
                );

                let status_dot = if r.twin_alert || r.leaked_child_loops > 0 {
                    "transient"
                } else if r.is_alive {
                    "durable"
                } else {
                    ""
                };

                widgets.push(json!({
                    "kind": "list-row",
                    "id": format!("row:{}", r.uuid),
                    "title": title_line,
                    "status": status_dot,
                }));
            }
        }
    }

    json!({
        "title": if view.mode == MODE_TOP { "ytop — Top" } else { "ytop — Dash" },
        "titlebar_switch": titlebar_switch_spec(&view.mode),
        "widgets": widgets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_reading(host: &str, ok: bool) -> Value {
        json!({
            "ok": ok,
            "host": host,
            "hostname": host,
            "kernel": "6.8.0",
            "arch": "x86_64",
            "btime": 1700000000,
            "cpu_model": "Test CPU",
            "cpu_count": 16,
            "virt": "none",
            "containers": [],
            "zfs": {"has_zfs": false, "pools": [], "iostat": null, "datasets": []},
            "uptime_s": 3600.0,
            "load": [1.0, 0.8, 0.5],
            "mem_total_kb": 16_000_000,
            "mem_available_kb": 8_000_000,
            "swap_total_kb": 0,
            "swap_free_kb": 0,
            "procs_total": 150,
            "cpu_busy_pct": 25.0,
            "top": [],
        })
    }

    #[test]
    fn test_viewport_and_rail_generation() {
        let view = View::default();
        let machines = vec![Machine {
            key: Some("k1".into()),
            readings: vec![sample_reading("openclaw", true)],
        }];
        let report = FleetRowsReport::default();
        let viewport = viewport_view(&view, &machines, &report);
        let rail = rail_view(&view, &machines, &report);

        assert!(viewport["widgets"].is_array());
        assert!(rail["widgets"].is_array());
        assert!(viewport["titlebar_switch"].is_object());
    }
}
