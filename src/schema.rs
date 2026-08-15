//! The widget schema — what yggterm paints in both Viewport and Rail surfaces.
//!
//! ⛔ ZERO RAW MARKDOWN DUMPS. Viewport and rail both paint rich, native Dioxus DOM widgets
//! (cards, gauges, meters, collapsible trees, and interactive buttons).

use crate::fleet::{self, Machine};
use crate::rows::{FleetRowsReport, RowInfo};
use serde_json::{json, Value};

pub const MODE_TOP: &str = "top";
pub const MODE_DASH: &str = "dash";

pub const TAB_ROWS: &str = "rows";
pub const TAB_JANKBOX: &str = "jankbox";
pub const TAB_SUPERVISION: &str = "supervision";

#[derive(Debug, Clone)]
pub struct View {
    pub mode: String,
    pub dash_tab: String,
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
            dash_tab: TAB_ROWS.to_string(),
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

fn matches(filter: &str, haystack: &[&str]) -> bool {
    if filter.trim().is_empty() {
        return true;
    }
    let needle = filter.to_lowercase();
    haystack.iter().any(|h| h.to_lowercase().contains(&needle))
}

// ─── Header & Mode Switcher ───────────────────────────────────────────────────

fn header(view: &View) -> Vec<Value> {
    let mut widgets = vec![
        json!({
            "kind": "tabs",
            "id": "mode_switch",
            "action": "mode",
            "active": view.mode,
            "tabs": [
                {"id": MODE_TOP, "label": "⚡ Top (Infrastructure)"},
                {"id": MODE_DASH, "label": "📊 Dash (Agent Fleet & Jankbox)"},
            ],
        }),
    ];

    if let Some(notice) = &view.notice {
        widgets.push(label(notice.clone()));
    }
    widgets
}

// ─── TOP VIEW (Infrastructure, Machines, ZFS, LXC) ───────────────────────────

pub fn top_view(view: &View, machines: &[Machine]) -> Value {
    let mut widgets = header(view);

    widgets.push(json!({
        "kind": "search-box",
        "id": "filter",
        "action": "filter",
        "value": view.filter,
        "placeholder": "filter processes, containers, or pools",
    }));

    // 1. Machines Selector Bar Card
    widgets.push(section("Connected Machines & Fleet", true));
    let mut machine_tabs = Vec::new();
    for m in machines {
        let p = m.principal();
        let host = p["host"].as_str().unwrap_or("?");
        let shown = p["label"].as_str().unwrap_or(host);
        let cpu = p["cpu_busy_pct"].as_f64().unwrap_or(0.0);
        let mem_total = p["mem_total_kb"].as_i64().unwrap_or(0);
        let mem_avail = p["mem_available_kb"].as_i64().unwrap_or(0);
        let mem_used = (mem_total - mem_avail).max(0);
        let status_mark = if m.reachable() { "●" } else { "○" };
        let tab_label = if m.reachable() {
            format!("{status_mark} {shown} ({cpu:.0}% CPU, {})", gb(mem_used))
        } else {
            format!("{status_mark} {shown} (unreachable)")
        };
        machine_tabs.push(json!({
            "id": host,
            "label": tab_label,
        }));
    }

    widgets.push(json!({
        "kind": "tabs",
        "id": "machine_selector",
        "action": "select_host",
        "active": view.selected_host,
        "tabs": machine_tabs,
    }));

    // Add Machine Button / Form
    if view.adding_machine {
        widgets.push(section("➕ Add New SSH Machine", true));
        widgets.push(json!({
            "kind": "text-input",
            "id": "new_machine_alias",
            "label": "SSH Alias / Hostname (e.g. main or user@server)",
            "value": view.new_machine_alias,
            "placeholder": "ssh-alias",
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
            "label": "Is Yggdrasil Node (ZFS Storage / LXC Host)",
            "value": view.new_machine_is_yggdrasil,
        }));
        widgets.push(json!({
            "kind": "button",
            "id": "save_machine_btn",
            "label": "Save Machine to Registry",
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
            "label": "➕ Add SSH Machine to Registry",
            "action": "add_machine_prompt",
        }));
    }

    // 2. Selected Machine Detail Cards
    let target_machine = machines
        .iter()
        .find(|m| {
            m.readings
                .iter()
                .any(|r| r["host"].as_str() == Some(&view.selected_host))
        })
        .or_else(|| machines.first());

    if let Some(m) = target_machine {
        let p = m.principal();
        let host = p["host"].as_str().unwrap_or("?");
        let shown = p["label"].as_str().unwrap_or(host);

        if !m.reachable() {
            widgets.push(section(format!("⚠ Host {shown} Unreachable"), true));
            widgets.push(label(format!(
                "Error: {}",
                p["error"].as_str().unwrap_or("connection timed out")
            )));
        } else {
            // Card A: System Health & Hardware Gauges
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

            widgets.push(section(format!("🖥️ System Health: {shown}"), true));
            widgets.push(label(format!(
                "Kernel: {} · Arch: {} · Uptime: {} · Load: [ {load} ] · Procs: {}",
                p["kernel"].as_str().unwrap_or("?"),
                p["arch"].as_str().unwrap_or("?"),
                duration(p["uptime_s"].as_f64().unwrap_or(0.0)),
                p["procs_total"].as_i64().unwrap_or(0)
            )));
            widgets.push(label(format!(
                "CPU Usage:  {}  ({} Cores · {})",
                progress_bar(cpu_busy, 20),
                p["cpu_count"].as_i64().unwrap_or(0),
                p["cpu_model"].as_str().unwrap_or("unknown")
            )));
            widgets.push(label(format!(
                "Memory RAM: {}  ({} / {})",
                progress_bar(mem_pct, 20),
                gb(used_kb),
                gb(total_kb)
            )));
            if swap_total_kb > 0 {
                widgets.push(label(format!(
                    "Swap Space: {}  ({} / {})",
                    progress_bar(swap_pct, 20),
                    mb(swap_used_kb),
                    mb(swap_total_kb)
                )));
            }

            // Card B: ZFS Storage & Real-Time IOSTAT
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
                            "Pool [{name}]  Health: {health}  Allocation: {}  ({} / {}, frag {frag_pct}%)",
                            progress_bar(cap_pct, 16),
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
                        "Live IOSTAT: Read {}/s ({} IOPS)  ·  Write {}/s ({} IOPS)",
                        bytes_to_human(r_bytes),
                        r_ops,
                        bytes_to_human(w_bytes),
                        w_ops
                    )));
                }
            }

            // Card C: LXC Containers with Collapsible Process Trees
            let containers = p["containers"].as_array().cloned().unwrap_or_default();
            if !containers.is_empty() {
                widgets.push(section(format!("📦 LXC Containers & Guest Process Drill-Down ({} containers)", containers.len()), true));
                for c in &containers {
                    let c_name = c["name"].as_str().unwrap_or("?");
                    let state = c["state"].as_str().unwrap_or("?");
                    let c_cpu = c["cpu_busy_pct"].as_f64().unwrap_or(0.0);
                    let c_rss = c["mem_rss_kb"].as_i64().unwrap_or(0);
                    let procs_count = c["procs_count"].as_i64().unwrap_or(0);
                    let is_expanded = view.expanded_containers.contains(&c_name.to_string());

                    let title_text = format!(
                        "📦 {c_name} [{state}] · {c_cpu:.1}% CPU · {} RAM ({procs_count} processes)",
                        mb(c_rss)
                    );

                    widgets.push(json!({
                        "kind": "list-row",
                        "id": format!("container:{c_name}"),
                        "title": title_text,
                        "subtitle": if is_expanded { "Click to collapse process breakdown" } else { "Click to expand internal process consumption tree" },
                        "status": if state == "RUNNING" { "transient" } else { "" },
                        "expanded": is_expanded,
                        "expand_action": format!("toggle_container:{c_name}"),
                        "row_action": format!("toggle_container:{c_name}"),
                    }));

                    if is_expanded {
                        let top_procs = c["top_procs"].as_array().cloned().unwrap_or_default();
                        if top_procs.is_empty() {
                            widgets.push(json!({
                                "kind": "list-row",
                                "id": format!("proc:{c_name}:empty"),
                                "title": "  (no high resource processes active inside container)",
                                "depth": 1,
                            }));
                        } else {
                            for tp in top_procs {
                                let pid = tp["pid"].as_i64().unwrap_or(0);
                                let comm = tp["comm"].as_str().unwrap_or("?");
                                let cpu_p = tp["cpu_pct"].as_f64().unwrap_or(0.0);
                                let rss = tp["rss_kb"].as_i64().unwrap_or(0);
                                let user = tp["user"].as_str().unwrap_or("?");
                                let cmd = tp["cmd"].as_str().unwrap_or("");
                                widgets.push(json!({
                                    "kind": "list-row",
                                    "id": format!("proc:{c_name}:{pid}"),
                                    "title": format!("  PID {pid:<7} {cpu_p:>5.1}% CPU   {:>8}   {comm}", mb(rss)),
                                    "subtitle": format!("  User: {user} · Cmd: {cmd}"),
                                    "depth": 1,
                                }));
                            }
                        }
                    }
                }
            }

            // Card D: Host Top Processes
            let top_procs = p["top"].as_array().cloned().unwrap_or_default();
            if !top_procs.is_empty() {
                widgets.push(section(format!("⚡ Top Active Processes: {shown}"), true));
                for proc in top_procs {
                    let pid = proc["pid"].as_i64().unwrap_or(0);
                    let comm = proc["comm"].as_str().unwrap_or("?");
                    let cpu_pct = proc["cpu_pct"].as_f64().unwrap_or(0.0);
                    let rss_kb = proc["rss_kb"].as_i64().unwrap_or(0);
                    let user = proc["user"].as_str().unwrap_or("?");
                    let cmd = proc["cmd"].as_str().unwrap_or("");
                    let cont = proc.get("container").and_then(Value::as_str);

                    if !matches(&view.filter, &[comm, cmd, user, &pid.to_string()]) {
                        continue;
                    }

                    let scope = if let Some(cn) = cont {
                        format!("📦 ct:{cn}")
                    } else {
                        "host".to_string()
                    };

                    widgets.push(json!({
                        "kind": "list-row",
                        "id": format!("top_proc:{host}:{pid}"),
                        "title": format!("{cpu_pct:>5.1}% CPU   {:>8}   {comm}  ({scope})", mb(rss_kb)),
                        "subtitle": format!("PID {pid} · User: {user} · Cmd: {cmd}"),
                        "status": if cpu_pct > 50.0 { "transient" } else { "" },
                    }));
                }
            }
        }
    }

    json!({
        "title": "ytop — infrastructure",
        "widgets": widgets,
        "footer": [
            label(format!("{} machines monitored", machines.len())),
            json!({"kind": "button", "id": "refresh_top", "action": "refresh", "label": "Refresh"}),
        ]
    })
}

// ─── DASH VIEW (Fleet Rows, Jankbox Diagnostics, Supervision) ─────────────────

pub fn dash_view(view: &View, report: &FleetRowsReport) -> Value {
    let mut widgets = header(view);

    widgets.push(json!({
        "kind": "tabs",
        "id": "dash_subtabs",
        "action": "tab",
        "active": view.dash_tab,
        "tabs": [
            {"id": TAB_ROWS, "label": "👥 Agent Fleet Rows"},
            {"id": TAB_JANKBOX, "label": "🩺 Jankbox Diagnostics"},
            {"id": TAB_SUPERVISION, "label": "⚡ Supervision & Controls"},
        ],
    }));

    widgets.push(json!({
        "kind": "search-box",
        "id": "filter",
        "action": "filter",
        "value": view.filter,
        "placeholder": "filter rows by seat, campaign, role, or uuid",
    }));

    if let Some(hold) = &report.quota_hold {
        widgets.push(section("⏸ Fleet-Wide Quota Hold Active", true));
        widgets.push(label(format!("Hold Reason: {hold} — All agent wakes & escalations are SUPPRESSED.")));
    }

    match view.dash_tab.as_str() {
        TAB_JANKBOX => {
            widgets.push(section("🩺 Host Lag & Jankbox Bottlenecks", true));
            widgets.push(label(format!(
                "Total Orphaned Jank Processes: {}  ·  Twin Duplicate Alarms: {}  ·  Leaked Subshells: {}",
                report.jankbox.total_jank_procs, report.twin_count, report.leak_count
            )));

            if report.jankbox.total_jank_procs > 0 {
                widgets.push(json!({
                    "kind": "button",
                    "id": "clean_jankbox_btn",
                    "label": "🧹 Clean All Leaked Subshells & Stale Twins",
                    "action": "clean_jankbox",
                    "primary": true,
                    "danger": true,
                }));
            } else {
                widgets.push(label("✅ Clean fleet! No spinning test subshells or stale twins detected."));
            }

            if !report.jankbox.bloated_transcripts_mb.is_empty() {
                widgets.push(section("⚠️ Bloated Transcripts (>10MB Context Burn Risk)", true));
                for (seat, size_mb) in &report.jankbox.bloated_transcripts_mb {
                    widgets.push(json!({
                        "kind": "list-row",
                        "id": format!("bloat:{seat}"),
                        "title": format!("Seat {seat}  —  {size_mb:.1} MB Transcript"),
                        "subtitle": "High token burn hazard if booted without compacting",
                        "status": "transient",
                    }));
                }
            }
        }
        TAB_SUPERVISION => {
            widgets.push(section("⚡ Fleet Supervision Controls", true));
            widgets.push(label(format!(
                "Active Supervision: {} rows armed  ·  Quota Hold: {}",
                report.rows.iter().filter(|r| r.supervision_state.contains("Armed")).count(),
                report.quota_hold.as_deref().unwrap_or("Inactive")
            )));
            if report.quota_hold.is_some() {
                widgets.push(json!({
                    "kind": "button",
                    "id": "release_hold_btn",
                    "label": "▶ Release Fleet Quota Hold",
                    "action": "quota_release",
                    "primary": true,
                }));
            } else {
                widgets.push(json!({
                    "kind": "button",
                    "id": "set_hold_btn",
                    "label": "⏸ Set Indefinite Quota Hold",
                    "action": "quota_hold",
                }));
            }
        }
        _ => {
            // TAB_ROWS (default)
            widgets.push(section("Agent Fleet Overview", true));
            widgets.push(label(format!(
                "Seats: {}  ·  Live: {}  ·  Total Agent CPU: {:.1}%  ·  Agent RAM: {:.1} MB  ·  Context: {:.1} MB",
                report.total_rows, report.live_count, report.total_agent_cpu_pct, report.total_agent_rss_mb, report.total_transcript_mb
            )));

            for (campaign, rows) in &report.campaigns {
                let matching_rows: Vec<&RowInfo> = rows
                    .iter()
                    .filter(|r| {
                        matches(
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
                        format!("⛔ TWIN DUPLICATE ({:?})", r.pids)
                    } else if !r.pids.is_empty() {
                        if r.leaked_child_loops > 0 {
                            format!("LIVE PID {:?} + ⚠️ {} leaked subshell(s)", r.pids, r.leaked_child_loops)
                        } else {
                            format!("LIVE PID {:?}", r.pids)
                        }
                    } else {
                        "💀 DEAD / NO PROCESS".to_string()
                    };

                    let burn_warn = if r.transcript_size_kb > 30 * 1024 {
                        " 🚨 CRITICAL CONTEXT (>30MB)"
                    } else if r.transcript_size_kb > 10 * 1024 {
                        " ⚠️ HEAVY CONTEXT (>10MB)"
                    } else {
                        ""
                    };

                    let title_line = format!("Seat {} · {} [{}]", r.seat, r.title, r.role);
                    let subtitle_line = format!(
                        "UUID: {}  ·  Host: {}  ·  Supervision: {}",
                        if r.uuid.len() >= 8 { &r.uuid[..8] } else { &r.uuid },
                        r.host,
                        r.supervision_state
                    );
                    let detail_line = format!(
                        "Process: {proc_str} ({:.1}% CPU, {} RAM) | Transcript: {} KB ({}L, mtime {}){burn_warn}",
                        r.cpu_pct,
                        mb(r.rss_kb),
                        r.transcript_size_kb,
                        r.transcript_lines,
                        r.last_active_mtime
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
                        "subtitle": subtitle_line,
                        "detail": detail_line,
                        "status": status_dot,
                    }));
                }
            }
        }
    }

    json!({
        "title": "ytop — fleet dashboard",
        "widgets": widgets,
        "footer": [
            label(format!("{} seats total · {} live · {:.1} MB context", report.total_rows, report.live_count, report.total_transcript_mb)),
            json!({"kind": "button", "id": "refresh_dash", "action": "refresh", "label": "Refresh"}),
        ]
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
    fn test_top_view_generation() {
        let view = View::default();
        let machines = vec![Machine {
            key: Some("k1".into()),
            readings: vec![sample_reading("openclaw", true)],
        }];
        let schema = top_view(&view, &machines);
        assert!(schema["widgets"].is_array());
        assert_eq!(schema["title"], "ytop — infrastructure");
    }

    #[test]
    fn test_dash_view_generation() {
        let view = View { mode: MODE_DASH.to_string(), ..View::default() };
        let report = FleetRowsReport::default();
        let schema = dash_view(&view, &report);
        assert!(schema["widgets"].is_array());
        assert_eq!(schema["title"], "ytop — fleet dashboard");
    }
}
