//! The widget schema — what yggterm paints in both Viewport and Rail surfaces.
//!
//! Viewport: SaaS-style GUI composition with rich multi-metric cards, progress meters, and process tables.
//! Rail: A flat notebook shelf in both modes. Operational selectors belong to
//! the active notebook, not in a second navigation hierarchy above the books.
//! Titlebar Switch: Dynamic app-driven Top ↔ Dash mode toggle.

use crate::fleet::{self, Machine};
use crate::rows::FleetRowsReport;
use serde_json::{Value, json};

pub const MODE_TOP: &str = "top";
pub const MODE_DASH: &str = "dash";

#[derive(Debug, Clone)]
pub struct View {
    pub mode: String,
    pub selected_host: String,
    pub expanded_containers: Vec<String>,
    pub filter: String,
    pub notice: Option<String>,
    pub selected_notebook: Option<String>,
    pub selected_page: Option<String>,
    pub expanded_notebooks: Vec<String>,
    /// The process whose signal chooser is open in System Top.
    pub process_signal_target: Option<i64>,
}

impl Default for View {
    fn default() -> Self {
        Self {
            mode: MODE_TOP.to_string(),
            selected_host: fleet::LOCAL.to_string(),
            expanded_containers: Vec::new(),
            filter: String::new(),
            notice: None,
            // ytop opens on Overview — the dashboard IS a notebook, so there
            // is no nameless view you reach by having selected nothing.
            selected_notebook: Some(crate::notebook::OVERVIEW_ID.to_string()),
            selected_page: Some(crate::notebook::overview_page_id(MODE_TOP)),
            expanded_notebooks: vec![crate::notebook::OVERVIEW_ID.to_string()],
            process_signal_target: None,
        }
    }
}

impl View {
    /// Move to the mode's actual home notebook. Dash is a notebook product,
    /// so its home is SysInternals rather than a parallel dashboard.
    pub fn select_mode(&mut self, mode: &str) -> bool {
        if !matches!(mode, MODE_TOP | MODE_DASH) {
            return false;
        }
        self.mode = mode.to_string();
        let (notebook, page) = crate::notebook::home_for_mode(mode);
        self.selected_notebook = Some(notebook.to_string());
        self.selected_page = Some(page);
        self.process_signal_target = None;
        self.notice = None;
        true
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
    format!(
        "`[{}{}]` **{:.1}%**",
        "█".repeat(filled),
        "░".repeat(empty),
        clamped
    )
}

pub fn plain_progress_bar(pct: f64, width: usize) -> String {
    let clamped = pct.clamp(0.0, 100.0);
    let filled = ((clamped / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!(
        "[{}{}] {:.1}%",
        "█".repeat(filled),
        "░".repeat(empty),
        clamped
    )
}

pub fn spark_char(pct: f64) -> char {
    match (pct.clamp(0.0, 100.0) / 12.5) as usize {
        0 => '▁',
        1 => '▂',
        2 => '▃',
        3 => '▄',
        4 => '▅',
        5 => '▆',
        6 => '▇',
        _ => '█',
    }
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
            {"id": MODE_TOP, "label": "Top", "title": "Infrastructure & Machine Topology"},
            {"id": MODE_DASH, "label": "Dash", "title": "Agent Fleet & Jankbox Cockpit"},
        ]
    })
}

// ─── RAIL VIEW (Sidebar in Top vs Dash modes) ──────────────────────────────────

pub fn rail_view(view: &View, _machines: &[Machine], report: &FleetRowsReport) -> Value {
    let mut widgets = Vec::new();

    if let Some(notice) = &view.notice {
        widgets.push(json!({"kind": "label", "text": notice, "muted": true}));
    }

    // Both modes begin here. A rail is navigation, not a miniature dashboard;
    // connected machines and operational controls live in System Top itself.
    widgets.push(section("Notebooks", false));
    for notebook in crate::notebook::list_notebooks(Some(&view.mode)) {
        let selected = view.selected_notebook.as_deref() == Some(&notebook.id);
        widgets.push(json!({
            "kind": "list-row",
            "id": format!("notebook:{}", notebook.id),
            "title": notebook.title,
            "selected": selected,
            "row_action": format!("page_open:{}:0", notebook.id),
        }));
    }

    let live_note = if view.mode == MODE_TOP {
        "Live · systems refresh every 2–5 s"
    } else if report.quota_hold.is_some() {
        "Live · supervision hold active"
    } else {
        "Live · fleet evidence refreshes every 4 s"
    };
    json!({
        "title": "Ytop",
        "titlebar_switch": titlebar_switch_spec(&view.mode),
        "widgets": widgets,
        "footer": [json!({"kind": "label", "text": live_note, "muted": true})]
    })
}

// ─── VIEWPORT VIEW (SaaS Dashboard in Top vs Dash modes) ───────────────────────

pub fn viewport_view(
    view: &View,
    machines: &[Machine],
    report: &FleetRowsReport,
    timeline: &crate::timeline::Ring,
    host_timeline: &crate::timeline::Ring,
    zfs_history: &std::collections::VecDeque<crate::server::ZfsIoSample>,
) -> Value {
    // If a notebook page is selected, render the book page (paper) — like turning a page.
    // Both Top and Dash have books; Top pages have NO ytrace, Dash pages exclusively ytrace.
    // ⚠ Overview is a LIVE notebook: it is selected like any other, but its page
    // is composed below from the current probe rather than read from `markdown`.
    // Falling through is what makes "every view is a notebook" true without
    // freezing the dashboard into stored text.
    if let Some(nb_id) = view
        .selected_notebook
        .as_ref()
        .filter(|id| !crate::notebook::is_overview(id))
    {
        if let Some(nb) = crate::notebook::get_notebook(nb_id) {
            if let Some(page_id) = &view.selected_page {
                if let Some(page) = nb.pages.iter().find(|p| &p.id == page_id && !p.composed) {
                    let mut widgets = Vec::new();
                    widgets.push(json!({
                        "kind": "markdown",
                        "id": format!("book_page:{}", page.id),
                        "source": page.markdown,
                    }));
                    if let Some(markdown) = crate::notebook::ytrace_preview(page) {
                        widgets.push(json!({
                            "kind": "markdown",
                            "id": format!("ytrace_preview:{}", page.id),
                            "source": markdown
                        }));
                    }
                    // ── The live half of the page ────────────────────────────
                    // ⭐ A shipped page's prose is frozen at build time. When it
                    //    names a live reading, ytop fills it here from the same
                    //    files the CLIs read — so a supervision page shows the
                    //    fleet as it is now rather than as it was when written.
                    if let Some(kind) = page.live.as_deref() {
                        for w in crate::sysinternals::live_widgets(kind, &page.id, report, false) {
                            widgets.push(w);
                        }
                    }
                    // Page turns remain in the footer. The shelf is always in
                    // the rail, so a duplicate "back" button is navigation
                    // chrome pretending to be content.
                    let page_idx = nb.pages.iter().position(|p| &p.id == page_id).unwrap_or(0);
                    let mut footer = Vec::new();
                    if page_idx > 0 {
                        footer.push(json!({"kind": "button", "id": format!("page_prev:{}", nb.id), "action": format!("page_open:{}:{}", nb.id, page_idx - 1), "label": "← Prev"}));
                    }
                    if page_idx + 1 < nb.pages.len() {
                        footer.push(json!({"kind": "button", "id": format!("page_next:{}", nb.id), "action": format!("page_open:{}:{}", nb.id, page_idx + 1), "label": "Next →"}));
                    }
                    footer.push(json!({
                        "kind": "label",
                        "text": format!("Page {} of {} · live", page_idx + 1, nb.pages.len()),
                        "muted": true
                    }));
                    return json!({
                        "title": format!("{} — {}", nb.title, page.title),
                        "titlebar_switch": titlebar_switch_spec(&view.mode),
                        "widgets": widgets,
                        "footer": footer
                    });
                }
            }
        }
    }

    let mut widgets = Vec::new();

    let mut rendered_host = view.selected_host.clone();
    if view.mode == MODE_TOP {
        let mut tabs = Vec::new();
        for reading in machines.iter().flat_map(|machine| machine.readings.iter()) {
            let Some(host) = reading["host"].as_str() else {
                continue;
            };
            let label = reading["label"].as_str().unwrap_or(host);
            // Discovery can describe this process' own host twice: once as
            // `local` and once through the yggterm connection alias (for
            // example `dev`). Two identical tabs do not represent two useful
            // choices, so retain the first reading while keeping genuinely
            // distinct logical guests whose labels differ.
            if tabs.iter().any(|tab: &Value| {
                tab["id"] == host || tab["label"].as_str() == Some(label)
            }) {
                continue;
            }
            tabs.push(json!({
                "id": host,
                "label": label,
            }));
        }
        if !tabs.is_empty() {
            let active = if tabs.iter().any(|tab| tab["id"] == view.selected_host) {
                view.selected_host.as_str()
            } else {
                tabs[0]["id"].as_str().unwrap_or(fleet::LOCAL)
            };
            rendered_host = active.to_string();
            widgets.push(json!({
                "kind": "tabs",
                "id": "connected_host",
                "action": "select_host",
                "active": active,
                "tabs": tabs,
            }));
        }
    }

    // Top filter search box in bar
    widgets.push(json!({
        "kind": "search-box",
        "id": "filter",
        "action": "filter",
        "value": view.filter,
        "placeholder": if view.mode == MODE_TOP { "Search processes, containers, PID..." } else { "Search campaign, seat, role, UUID..." },
    }));

    if view.mode == MODE_TOP {
        // TOP MODE: Infrastructure & Host Health Dashboard
        // Select the LOGICAL host reading, not the physical group principal.
        // Several logical guests may intentionally share a kernel; Top must
        // still let the operator inspect each connected host independently.
        let target_reading = machines
            .iter()
            .flat_map(|machine| machine.readings.iter())
            .find(|reading| reading["host"].as_str() == Some(rendered_host.as_str()))
            .or_else(|| {
                machines
                    .iter()
                    .flat_map(|machine| machine.readings.iter())
                    .next()
            });

        if let Some(p) = target_reading {
            let host = p["host"].as_str().unwrap_or("?");
            let shown = p["label"].as_str().unwrap_or(host);

            if p["ok"].as_bool() != Some(true) {
                let error_msg = p["error"]
                    .as_str()
                    .unwrap_or("Host unreachable or connection timed out.");
                let md = format!(
                    "# ⚠️ Host Offline: {shown}\n\n\
                    > **Connection Error**: `{error_msg}`\n\n\
                    Please verify the machine is powered on, network is reachable, and SSH credentials are configured."
                );
                widgets.push(json!({
                    "kind": "markdown",
                    "id": "offline_doc",
                    "source": md,
                }));
            } else {
                let total_kb = p["mem_total_kb"].as_i64().unwrap_or(0);
                let avail_kb = p["mem_available_kb"].as_i64().unwrap_or(0);
                let used_kb = (total_kb - avail_kb).max(0);
                let mem_pct = if total_kb > 0 {
                    used_kb as f64 * 100.0 / total_kb as f64
                } else {
                    0.0
                };
                let swap_total_kb = p["swap_total_kb"].as_i64().unwrap_or(0);
                let swap_free_kb = p["swap_free_kb"].as_i64().unwrap_or(0);
                let swap_used_kb = (swap_total_kb - swap_free_kb).max(0);
                let swap_pct = if swap_total_kb > 0 {
                    swap_used_kb as f64 * 100.0 / swap_total_kb as f64
                } else {
                    0.0
                };
                let cpu_busy = p["cpu_busy_pct"].as_f64().unwrap_or(0.0);
                let cpu_cores = p["cpu_count"].as_i64().unwrap_or(1).max(1);
                let cpu_normalized = (cpu_busy / cpu_cores as f64).clamp(0.0, 100.0);
                let cpu_model = p["cpu_model"].as_str().unwrap_or("AMD/Intel Processor");
                let kernel = p["kernel"].as_str().unwrap_or("Linux");
                let arch = p["arch"].as_str().unwrap_or("x86_64");
                let uptime = duration(p["uptime_s"].as_f64().unwrap_or(0.0));
                let procs_total = p["procs_total"].as_i64().unwrap_or(0);
                let load = p["load"]
                    .as_array()
                    .map(|l| {
                        l.iter()
                            .filter_map(|v| v.as_f64())
                            .map(|v| format!("{v:.2}"))
                            .collect::<Vec<_>>()
                            .join(" · ")
                    })
                    .unwrap_or_else(|| "0.00 · 0.00 · 0.00".to_string());

                let mut md = String::new();

                // 1. Host Header Banner
                md.push_str(&format!(
                    "# {shown}\n\n\
                    | Attribute | Value | Attribute | Value |\n\
                    | :--- | :--- | :--- | :--- |\n\
                    | **Kernel** | `{kernel}` | **Uptime** | `{uptime}` |\n\
                    | **Architecture** | `{arch}` | **Total Tasks** | `{procs_total} procs` |\n\
                    | **CPU Cores** | `{cpu_cores} Cores ({cpu_model})` | **Load Average** | `{load}` |\n\n"
                ));

                // First-class EMD component: the source is replaced on each
                // document-version refresh, while parsing/layout stay inside
                // the shared renderer. Missing samples remain JSON null gaps.
                let history = host_timeline.since_ms(300_000);
                let host_samples: Vec<_> =
                    history.iter().filter(|sample| sample.row == host).collect();
                let cpu_values: Vec<Value> = host_samples
                    .iter()
                    .map(|sample| json!({
                        "x": format!("-{}s", history.last().map(|last| last.t_ms.saturating_sub(sample.t_ms) / 1000).unwrap_or(0)),
                        "y": sample.cpu_pct,
                    }))
                    .collect();
                let ram_values: Vec<Value> = host_samples
                    .iter()
                    .map(|sample| json!({
                        "x": format!("-{}s", history.last().map(|last| last.t_ms.saturating_sub(sample.t_ms) / 1000).unwrap_or(0)),
                        "y": if total_kb > 0 { Some(sample.rss_kb as f64 * 100.0 / total_kb as f64) } else { None },
                    }))
                    .collect();
                let component_state = if host_samples.is_empty() {
                    "collecting"
                } else {
                    "observed"
                };
                let resource_plot = json!({
                    "version": 1,
                    "kind": "plot",
                    "spec": {
                        "title": "Resource pressure over five minutes",
                        "subtitle": "Per-core CPU work and RAM occupancy; hover a point for its exact reading.",
                        "mark": "line",
                        "x_label": "time",
                        "y_label": "percent",
                        "include_zero": true,
                        "height": 280,
                        "legend": true,
                        "series": [
                            {"name": "CPU average", "units": "percent", "values": cpu_values},
                            {"name": "RAM used", "units": "percent", "values": ram_values}
                        ],
                        "evidence": {
                            "question": "Is this host under sustained CPU or memory pressure?",
                            "source": "/proc/stat + /proc/meminfo",
                            "window": "last 5 min",
                            "freshness": "2–5 s",
                            "units": "percent",
                            "state": component_state,
                            "reproduction": "ytop --json"
                        }
                    }
                });
                md.push_str("## Pressure history\n\n");
                md.push_str("```emd\n");
                md.push_str(&resource_plot.to_string());
                md.push_str("\n```\n\n");

                // 2. Hardware Resource Gauges — professional, example-driven (beginner → expert)
                md.push_str("## Resource Meters\n\n");
                md.push_str("> How hard is this machine working *right now*? `CPU Busy` is total work across all cores; `avg` is per-core share. `RAM` is memory in use. `Swap` is overflow when RAM is full — `0 MB` is healthy.\n\n");
                md.push_str(&format!(
                    "| Resource | Usage | Used | Total | What it means |\n\
                    | :--- | :--- | :--- | :--- | :--- |\n\
                    | **CPU Busy** | {} | `{cpu_busy:.1}%` (`{cpu_normalized:.1}% avg`) | `{cpu_cores} Cores` | Total work; e.g. `22% avg` on 32 cores ≈ `7` cores busy — plenty of headroom |\n\
                    | **RAM Memory** | {} | `{}` | `{}` | Memory in use; e.g. `309 GB / 503 GB` ≈ half free — `>90%` deserves a look |\n",
                    progress_bar(cpu_normalized, 20),
                    progress_bar(mem_pct, 20),
                    gb(used_kb),
                    gb(total_kb)
                ));

                if swap_total_kb > 0 {
                    md.push_str(&format!(
                        "| **Swap Space** | {} | `{}` | `{}` | Overflow if RAM full; `0 MB` is ideal |\n\n",
                        progress_bar(swap_pct, 20),
                        mb(swap_used_kb),
                        mb(swap_total_kb)
                    ));
                } else {
                    md.push_str("| **Swap Space** | `[░░░░░░░░░░░░░░░░░░░░]` **0.0%** | `0 MB` | `0 MB` | No overflow — healthy |\n\n");
                }

                // 3. ZFS Storage & Real-Time IOSTAT — plain English + example (avoids overly technical)
                let zfs = &p["zfs"];
                if zfs["has_zfs"].as_bool().unwrap_or(false) {
                    md.push_str("## ZFS Storage Pools & Real-Time IOSTAT\n\n");
                    md.push_str("> Pool = a group of disks acting as one. `Health` is safety, `Used/Total` is how full, `Frag %` is how scattered the data is (high frag can slow you). Example: `zroot 65% used, 54% frag` → still safe, but balance if frag keeps climbing.\n\n");
                    if let Some(pools) = zfs["pools"].as_array() {
                        md.push_str(
                            "| Pool | Health | Allocation Meter | Used / Total | Frag % |\n",
                        );
                        md.push_str("| :--- | :--- | :--- | :--- | :--- |\n");
                        for pool in pools {
                            let name = pool["name"].as_str().unwrap_or("?");
                            let health = pool["health"].as_str().unwrap_or("UNKNOWN");
                            let cap_pct = pool["cap_pct"].as_f64().unwrap_or(0.0);
                            let size_b = pool["size_bytes"].as_i64().unwrap_or(0);
                            let alloc_b = pool["alloc_bytes"].as_i64().unwrap_or(0);
                            let frag_pct = pool["frag_pct"].as_i64().unwrap_or(0);
                            let health_badge = if health == "ONLINE" {
                                "🟢 ONLINE"
                            } else {
                                "🟡 DEGRADED"
                            };

                            md.push_str(&format!(
                                "| **{name}** | {health_badge} | {} | `{} / {}` | `{frag_pct}%` |\n",
                                progress_bar(cap_pct, 16),
                                bytes_to_human(alloc_b),
                                bytes_to_human(size_b)
                            ));
                        }
                        md.push('\n');
                    }

                    if let Some(io) = zfs["iostat"].as_object() {
                        let r_ops = io.get("read_ops").and_then(Value::as_i64).unwrap_or(0);
                        let w_ops = io.get("write_ops").and_then(Value::as_i64).unwrap_or(0);
                        let r_bytes = io.get("read_bytes_s").and_then(Value::as_i64).unwrap_or(0);
                        let w_bytes = io.get("write_bytes_s").and_then(Value::as_i64).unwrap_or(0);
                        md.push_str(&format!(
                            "> Live I/O (now): Read **{}/s** ({} IOPS)  ·  Write **{}/s** ({} IOPS)\n\n",
                            bytes_to_human(r_bytes),
                            r_ops,
                            bytes_to_human(w_bytes),
                            w_ops
                        ));
                        if zfs_history.len() > 1 {
                            let max_r = zfs_history
                                .iter()
                                .map(|s| s.read_bps)
                                .max()
                                .unwrap_or(1)
                                .max(1) as f64;
                            let max_w = zfs_history
                                .iter()
                                .map(|s| s.write_bps)
                                .max()
                                .unwrap_or(1)
                                .max(1) as f64;
                            let spark_r: String = zfs_history
                                .iter()
                                .map(|s| spark_char(s.read_bps as f64 / max_r * 100.0))
                                .collect();
                            let spark_w: String = zfs_history
                                .iter()
                                .map(|s| spark_char(s.write_bps as f64 / max_w * 100.0))
                                .collect();
                            md.push_str(&format!("> Last 60s (2s buckets, 30 samples): read spark `{spark_r}` · write spark `{spark_w}` — e.g. `▁▃█▁` spiked then cooled. Flat high `████` means sustained load.\n\n"));
                        } else if !zfs_history.is_empty() {
                            md.push_str("> Gathering 60s history — one sample per 2s (wait ~6s for spark).\n\n");
                        }
                    }
                }

                // 3b. Daemon Cost — beginner-friendly example, expert model preserved (not overly technical)
                md.push_str("### Daemon Cost — when you run many agents, share one manager\n\n");
                md.push_str("> **In plain words:** a *daemon* is the background manager that holds your sessions. One manager for 200 agents costs less than many managers each holding a few. Example: one shared daemon for 23 sessions ≈ `0.45` cores; 14 separate daemons for 34 sessions ≈ `3.0` cores — **4.5× more per session when split**. Most cost is kernel work (`2.58` cores), your window is tiny (`0.01`).\n\n");
                md.push_str(&format!(
                    "> **For experts — model** `cores = 0.116 + 0.0104·owned + 0.000337·rows` (R² 0.939), measured 25s `/proc` delta on 14 daemons. Single shared daemon ≈ `0.45` cores / 23 sessions vs `3.0` cores / 34 sessions on 14 daemons.\n\n\
                    > *Probe tip for agents:* `yggterm-headless server perf-summary --category render --top 5 --json` + `server perf-incidents --list` (never `ps %CPU` — lifetime avg, not current). Ytop fan-out reuses `ControlMaster` (45s) so 3-host read <1s. `eBPF` ring (`bpftrace`/`perf`) is Slice 2 opt-in per Yggdrasil host.\n\n"
                ));

                // 3c. eBPF Live Probes — collapsed by default, only when tools present (opt-in)
                let ebpf_avail = p["ebpf_available"].as_bool().unwrap_or(false);
                let ebpf_tools = p["ebpf_tools"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                if ebpf_avail {
                    md.push_str("### eBPF Live Probes — opt-in, no overhead until you need it\n\n");
                    md.push_str(&format!("> Tools found: `{ebpf_tools}`. Use for deep dives when Top's 400ms delta isn't enough. Example: `sudo bpftrace -e 'tracepoint:sched:sched_switch {{ @[comm] = count(); }}'` → hot task histogram. Or `perf top -a` for kernel hotspots. Keep off by default — zero cost when idle.\n\n"));
                } else {
                    md.push_str("> eBPF probes hidden — install `bpftrace` or `perf` to enable live kernel tracing (zero overhead when not installed).\n\n");
                }

                // 4. LXC Containers — subtle, example-driven
                let containers = p["containers"].as_array().cloned().unwrap_or_default();
                if !containers.is_empty() {
                    md.push_str(&format!(
                        "## LXC Containers ({} Total)\n\n",
                        containers.len()
                    ));
                    md.push_str("> A container is a lightweight machine inside your machine. `Status` is running or stopped, `Top Internal Process` is the busiest thing inside it. Example: `android-kvm RUNNING 0.0% 41 MB` → powered on but idle.\n\n");
                    md.push_str(
                        "| Container | Status | CPU % | RAM RSS | Tasks | Top Internal Process |\n",
                    );
                    md.push_str("| :--- | :--- | :--- | :--- | :--- | :--- |\n");

                    for c in &containers {
                        let c_name = c["name"].as_str().unwrap_or("?");
                        let state = c["state"].as_str().unwrap_or("?");
                        let c_cpu = c["cpu_busy_pct"].as_f64().unwrap_or(0.0);
                        let c_rss = c["mem_rss_kb"].as_i64().unwrap_or(0);
                        let procs_count = c["procs_count"].as_i64().unwrap_or(0);
                        let state_badge = if state == "RUNNING" {
                            "🟢 RUNNING"
                        } else {
                            "⚪ STOPPED"
                        };

                        let top_proc_str = c["top_procs"]
                            .as_array()
                            .and_then(|tps| tps.first())
                            .map(|tp| {
                                let pid = tp["pid"].as_i64().unwrap_or(0);
                                let comm = tp["comm"].as_str().unwrap_or("?");
                                let cpu_p = tp["cpu_pct"].as_f64().unwrap_or(0.0);
                                format!("`{comm}` ({cpu_p:.1}% CPU, PID {pid})")
                            })
                            .unwrap_or_else(|| "—".to_string());

                        let is_expanded = view.expanded_containers.contains(&c_name.to_string());
                        let expand_hint = if is_expanded { "▼" } else { "▶" };
                        if matches_filter(&view.filter, &[c_name, state, &top_proc_str]) {
                            md.push_str(&format!(
                                "| **{expand_hint} {c_name}** | {state_badge} | `{c_cpu:.1}%` | `{}` | `{} procs` | {} |\n",
                                mb(c_rss),
                                procs_count,
                                top_proc_str
                            ));
                            if is_expanded {
                                if let Some(tps) = c["top_procs"].as_array() {
                                    md.push_str(&format!("\n> **{c_name}** — top processes (tap again to collapse):\n\n"));
                                    md.push_str("| PID | User | CPU % | RSS | Command |\n");
                                    md.push_str("| :--- | :--- | :--- | :--- | :--- |\n");
                                    for tp in tps.iter().take(6) {
                                        let pid = tp["pid"].as_i64().unwrap_or(0);
                                        let comm = tp["comm"].as_str().unwrap_or("?");
                                        let cpu_p = tp["cpu_pct"].as_f64().unwrap_or(0.0);
                                        let rss = tp["rss_kb"].as_i64().unwrap_or(0);
                                        let user = tp["user"].as_str().unwrap_or("?");
                                        md.push_str(&format!(
                                            "| `{pid}` | `{user}` | `{cpu_p:.1}%` | `{}` | `{comm}` |\n",
                                            mb(rss)
                                        ));
                                    }
                                    md.push_str("\n");
                                }
                            }
                        }
                    }
                    md.push('\n');
                }

                let processes = p["top"].as_array().cloned().unwrap_or_default();
                if !processes.is_empty() {
                    widgets.push(section("Processes · live 400 ms sample", false));
                    widgets.push(json!({
                        "kind": "label",
                        "text": "CPU is interval work, not a lifetime average. Kill… opens an explicit signal chooser.",
                        "muted": true
                    }));
                    for process in processes
                        .into_iter()
                        .filter(|process| {
                            let pid = process["pid"].as_i64().unwrap_or(0).to_string();
                            matches_filter(
                                &view.filter,
                                &[
                                    process["comm"].as_str().unwrap_or(""),
                                    process["cmd"].as_str().unwrap_or(""),
                                    process["user"].as_str().unwrap_or(""),
                                    &pid,
                                ],
                            )
                        })
                        .take(20)
                    {
                        let pid = process["pid"].as_i64().unwrap_or(0);
                        let command = process["comm"].as_str().unwrap_or("?");
                        let cpu = process["cpu_pct"].as_f64().unwrap_or(0.0);
                        let rss = process["rss_kb"].as_i64().unwrap_or(0);
                        let user = process["user"].as_str().unwrap_or("?");
                        let scope = process
                            .get("container")
                            .and_then(Value::as_str)
                            .map(|name| format!("container {name}"))
                            .unwrap_or_else(|| "host".to_string());
                        let chooser_open = view.process_signal_target == Some(pid);
                        let signalable = pid > 1 && pid != std::process::id() as i64;
                        let actions = if chooser_open && signalable {
                            vec![
                                json!({"label": "TERM", "action": format!("process_signal:TERM:{pid}"), "title": "Request a graceful termination"}),
                                json!({"label": "INT", "action": format!("process_signal:INT:{pid}"), "title": "Send an interrupt"}),
                                json!({"label": "KILL", "action": format!("process_signal:KILL:{pid}"), "title": "Force termination immediately"}),
                                json!({"label": "×", "action": "process_signal_cancel", "title": "Close signal chooser"}),
                            ]
                        } else if signalable {
                            vec![json!({
                                "label": "Kill…",
                                "action": format!("process_signal_menu:{pid}"),
                                "title": format!("Choose a signal for PID {pid}")
                            })]
                        } else {
                            Vec::new()
                        };
                        let menu = if signalable {
                            vec![
                                json!({"label": "SIGTERM", "action": format!("process_signal:TERM:{pid}"), "title": "Request graceful termination"}),
                                json!({"label": "SIGINT", "action": format!("process_signal:INT:{pid}"), "title": "Send an interrupt"}),
                                json!({"label": "SIGKILL", "action": format!("process_signal:KILL:{pid}"), "title": "Force termination immediately"}),
                            ]
                        } else {
                            Vec::new()
                        };
                        widgets.push(json!({
                            "kind": "list-row",
                            "id": format!("process:{pid}"),
                            "title": format!("{command} · PID {pid}"),
                            "subtitle": format!("{cpu:.1}% CPU · {} · {user} · {scope}", mb(rss)),
                            "actions": actions,
                            "menu": menu
                        }));
                    }
                }

                // Process controls are the reason people open Top under
                // pressure. Keep them before the explanatory system paper so
                // the first viewport is actionable instead of making the user
                // scroll through a long report to reach Kill….
                widgets.push(json!({
                    "kind": "markdown",
                    "id": "top_dashboard_doc",
                    "source": md,
                }));
            }
        }
    } else {
        // DASH MODE: Agent Fleet Cockpit & Jankbox Matrix
        let mut md = String::new();

        md.push_str("# Agent Fleet — Rows & Jankbox Cockpit\n\n");
        md.push_str("> All your agents in one place. `Total Seats` is slots, `Live Agents` is running now, `Fleet Agent CPU/RAM` is total work. Example: `54 live / 54 total, 1.9% CPU` → all idle, healthy.\n\n");

        // Overview KPI Cards
        md.push_str(&format!(
            "| Total Seats | Live Agents | Fleet Agent CPU | Fleet Agent RAM | Total Context Window |\n\
            | :--- | :--- | :--- | :--- | :--- |\n\
            | **`{}`** | **`{}`** | **`{:.1}%`** | **`{:.1} MB`** | **`{:.1} MB`** |\n\n",
            report.total_rows,
            report.live_count,
            report.total_agent_cpu_pct,
            report.total_agent_rss_mb,
            report.total_transcript_mb
        ));

        // Supervision Status Alert
        if let Some(hold) = &report.quota_hold {
            md.push_str(&format!(
                "> ⏸ **SUPERVISION QUOTA HOLD ACTIVE**: `{hold}`\n\
                > All automated subagents and booter loops are temporarily on hold to conserve quota.\n\n"
            ));
        } else {
            md.push_str(
                "> 🟢 **SUPERVISION ACTIVE**: Fleet orchestrators running normal scheduling.\n\n",
            );
        }

        // Jankbox Warnings (if any) — interactive, not just a badge
        if report.leak_count > 0 || report.twin_count > 0 {
            md.push_str(&format!(
                "> ⚠️ **JANKBOX ANOMALIES DETECTED**: **{}** orphaned spinning loops, **{}** twin duplicate sessions.\n",
                report.leak_count, report.twin_count
            ));
            if !report.jankbox.leaked_subshell_pids.is_empty() {
                md.push_str(&format!(
                    "> Leaked: `{}`\n",
                    report
                        .jankbox
                        .leaked_subshell_pids
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if !report.jankbox.twin_stale_pids.is_empty() {
                md.push_str(&format!(
                    "> Twins: `{}`\n",
                    report
                        .jankbox
                        .twin_stale_pids
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if !report.jankbox.bloated_transcripts_mb.is_empty() {
                md.push_str(&format!(
                    "> Bloated: `{}`\n",
                    report
                        .jankbox
                        .bloated_transcripts_mb
                        .iter()
                        .map(|(s, mb)| format!("{s} {mb:.1}MB"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            md.push_str("\n");
        }

        // Timeline — plain English, example-driven (beginner → expert, AXIOM-lite)
        md.push_str("## Timeline — per-row CPU, memory and log volume\n\n");
        md.push_str("> What did each agent do *just now*? `CPU` is work in the last 400ms (real-time, like htop), `RSS` is memory, `log volume` is how much it wrote. Example: `12%` with spark `▃▅█▁` → spiked then idled — flat `████` means stuck hot. `Slice 2` keeps a 5-min history (1s buckets) below.\n\n");
        md.push_str("> **For experts:** `proc` delta `400ms /proc/<pid>/stat` twice; ring `t0 + (t,row,cpu,rss,log_events)` downsampled to 1s, 5-min TTL.\n\n");
        md.push_str("| Row | CPU | RSS | Span |\n");
        md.push_str("| :--- | :--- | :--- | :--- |\n");
        for row in report.rows.iter().filter(|r| r.is_alive).take(12) {
            let span = format!("{}·{}KB·{}L", row.cpu_pct, row.rss_kb, row.transcript_lines);
            md.push_str(&format!(
                "| `{}` | {} | `{}` | `{}` |\n",
                row.seat,
                progress_bar(row.cpu_pct.clamp(0.0, 100.0), 10),
                mb(row.rss_kb),
                span
            ));
        }
        if report.rows.iter().filter(|r| r.is_alive).count() == 0 {
            md.push_str("| — | `idle` | — | `no live rows` |\n");
        }
        md.push_str(&format!(
            "\n> **Fleet rollup:** `total_cpu {:.1}%` · `total_rss {:.1} MB` · `{} live / {} total` — probe fan-out `1 ssh / host / 2s`, agent-first `server app do --session` without stealing your viewport.\n\n",
            report.total_agent_cpu_pct, report.total_agent_rss_mb, report.live_count, report.total_rows
        ));

        // AXIOM-like ring history (last 60s, per-row spark)
        let recent = timeline.since_ms(60_000);
        if !recent.is_empty() {
            use std::collections::BTreeMap;
            let mut by_row: BTreeMap<String, Vec<&crate::timeline::Sample>> = BTreeMap::new();
            for s in &recent {
                by_row.entry(s.row.clone()).or_default().push(s);
            }
            md.push_str("### Last 60s — per-row history (1s buckets, 5-min window)\n\n");
            md.push_str("> History for the last minute, one point per second. `Spark` is a mini-chart of CPU — e.g. `▁▃█▁` spiked. High flat `████` → hot loop.\n\n");
            md.push_str("| Row | Samples | Avg CPU | Peak | Spark | Last RSS |\n");
            md.push_str("| :--- | :--- | :--- | :--- | :--- | :--- |\n");
            for (row, samples) in by_row.iter().take(12) {
                let avg =
                    samples.iter().map(|s| s.cpu_pct as f64).sum::<f64>() / samples.len() as f64;
                let peak = samples.iter().map(|s| s.cpu_pct as f64).fold(0.0, f64::max);
                let last_rss = samples.last().map(|s| s.rss_kb).unwrap_or(0);
                let spark: String = samples
                    .iter()
                    .map(|s| {
                        let p = s.cpu_pct.clamp(0.0, 100.0) as f64;
                        match (p / 12.5) as usize {
                            0 => '▁',
                            1 => '▂',
                            2 => '▃',
                            3 => '▄',
                            4 => '▅',
                            5 => '▆',
                            6 => '▇',
                            _ => '█',
                        }
                    })
                    .collect();
                md.push_str(&format!(
                    "| `{}` | `{}` | ` {:.1}%` | ` {:.1}%` | `{} ` | `{}` |\n",
                    row,
                    samples.len(),
                    avg,
                    peak,
                    spark,
                    mb(last_rss)
                ));
            }
            md.push_str("\n");
        } else {
            md.push_str(
                "> *Timeline ring filling — 5-min TTL, 1s bucket. Run 2 ticks to see spark.*\n\n",
            );
        }

        // Fleet Rows Matrix
        md.push_str("## Fleet Agent Matrix\n\n");
        md.push_str("> One row per agent. `Seat` is position, `Role` is job, `Campaign` is project. `Status` is `Live` or `Dead`, `Context` is transcript size. Example: `019da16a ytop verify 0% Live` → your verification agent, idle.\n\n");
        md.push_str("| Seat | Role | Campaign | UUID | Status | Context | Supervision |\n");
        md.push_str("| :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n");

        for row in &report.rows {
            let seat = if row.seat.is_empty() { "?" } else { &row.seat };
            let role = if row.role.is_empty() {
                "agent"
            } else {
                &row.role
            };
            let camp = if row.campaign.is_empty() {
                "unregistered"
            } else {
                &row.campaign
            };
            let uuid = if row.uuid.is_empty() {
                "—"
            } else {
                &row.uuid
            };
            let short_uuid = if uuid.len() > 8 { &uuid[..8] } else { uuid };

            let status_badge = if row.is_alive {
                let cpu = row.cpu_pct;
                if row.twin_alert {
                    format!("⛔ TWIN (`{cpu:.0}%` CPU)")
                } else {
                    format!("🟢 LIVE (`{cpu:.0}%` CPU)")
                }
            } else {
                "💀 DEAD".to_string()
            };

            let ctx_str = format!(
                "`{} KB` ({}L)",
                row.transcript_size_kb, row.transcript_lines
            );
            let sup_str = if !row.supervision_state.is_empty() {
                &row.supervision_state
            } else if report.quota_hold.is_some() {
                "⏸ Quota Held"
            } else {
                "🟢 Active"
            };

            if matches_filter(&view.filter, &[seat, role, camp, uuid, &status_badge]) {
                md.push_str(&format!(
                    "| **`{seat}`** | `{role}` | **`{camp}`** | `{short_uuid}` | {status_badge} | {ctx_str} | {sup_str} |\n",
                ));
            }
        }
        md.push('\n');

        widgets.push(json!({
            "kind": "markdown",
            "id": "dash_dashboard_doc",
            "source": md,
        }));
    }

    json!({
        "title": if view.mode == MODE_TOP { "System Top" } else { "Fleet Overview" },
        "titlebar_switch": titlebar_switch_spec(&view.mode),
        "widgets": widgets,
        "footer": [json!({
            "kind": "label",
            "text": if view.mode == MODE_TOP { "Live · sampled every 2–5 s" } else { "Live · sampled every 4 s" },
            "muted": true
        })]
    })
}

#[cfg(test)]
mod overview_view_tests {
    use super::*;
    use serde_json::json;

    fn machine(host: &str) -> Machine {
        Machine {
            key: Some(host.to_string()),
            readings: vec![json!({
                "ok": true,
                "host": host,
                "label": host,
                "cpu_busy_pct": 12.0,
                "cpu_count": 8,
                "cpu_model": "Test CPU",
                "kernel": "test",
                "arch": "x86_64",
                "uptime_s": 3600.0,
                "procs_total": 100,
                "load": [0.5, 0.4, 0.3],
                "mem_total_kb": 8_000_000,
                "mem_available_kb": 4_000_000,
                "swap_total_kb": 0,
                "swap_free_kb": 0,
                "containers": [],
                "zfs": {"has_zfs": false, "pools": [], "datasets": [], "iostat": null},
                "top": [],
            })],
        }
    }

    fn render(view: &View) -> String {
        let report = FleetRowsReport::default();
        let ring = crate::timeline::Ring::default();
        let zfs = std::collections::VecDeque::new();
        let host_ring = crate::timeline::Ring::default();
        let out = viewport_view(view, &[machine("alpha")], &report, &ring, &host_ring, &zfs);
        serde_json::to_string(&out).unwrap()
    }

    #[test]
    fn top_lists_and_renders_each_logical_host_even_when_they_share_one_machine() {
        let grouped = Machine {
            key: Some("one-physical-system".to_string()),
            readings: vec![
                machine("alpha").readings[0].clone(),
                {
                    let mut beta = machine("beta").readings[0].clone();
                    beta["label"] = json!("Beta LXC");
                    beta["cpu_model"] = json!("BETA LOGICAL HOST");
                    beta
                },
                {
                    let mut gamma = machine("gamma").readings[0].clone();
                    gamma["label"] = json!("Gamma LXC");
                    gamma
                },
            ],
        };
        let view = View {
            selected_host: "beta".to_string(),
            ..View::default()
        };
        let report = FleetRowsReport::default();
        let rows = crate::timeline::Ring::default();
        let hosts = crate::timeline::Ring::default();
        let zfs = std::collections::VecDeque::new();
        let rendered = viewport_view(&view, &[grouped], &report, &rows, &hosts, &zfs);
        let tabs = rendered["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|widget| widget["id"] == "connected_host")
            .expect("the connected host switcher is notebook chrome");
        let ids: Vec<_> = tabs["tabs"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|tab| tab["id"].as_str())
            .collect();
        assert_eq!(ids, ["alpha", "beta", "gamma"]);
        assert_eq!(tabs["active"], "beta");
        let text = rendered.to_string();
        assert!(
            text.contains("BETA LOGICAL HOST"),
            "Top rendered the physical principal instead of beta"
        );
    }

    #[test]
    fn top_collapses_the_local_host_and_its_same_named_connection_alias() {
        let grouped = Machine {
            key: Some("one-physical-system".to_string()),
            readings: vec![
                machine("alpha").readings[0].clone(),
                {
                    let mut local = machine(fleet::LOCAL).readings[0].clone();
                    local["label"] = json!("beta");
                    local["cpu_model"] = json!("LOCAL BETA READING");
                    local
                },
                machine("beta").readings[0].clone(),
                machine("gamma").readings[0].clone(),
            ],
        };
        let view = View::default();
        let report = FleetRowsReport::default();
        let rows = crate::timeline::Ring::default();
        let hosts = crate::timeline::Ring::default();
        let zfs = std::collections::VecDeque::new();
        let rendered = viewport_view(&view, &[grouped], &report, &rows, &hosts, &zfs);
        let tabs = rendered["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|widget| widget["id"] == "connected_host")
            .unwrap();
        let labels: Vec<_> = tabs["tabs"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|tab| tab["label"].as_str())
            .collect();
        assert_eq!(labels, ["alpha", "beta", "gamma"]);
        assert_eq!(tabs["active"], fleet::LOCAL);
        assert!(rendered.to_string().contains("LOCAL BETA READING"));
    }

    #[test]
    fn system_top_emits_a_typed_dynamic_pressure_plot() {
        let text = render(&View::default());
        assert!(text.contains("Resource pressure over five minutes"));
        assert!(
            text.contains("\\\"kind\\\":\\\"plot\\\""),
            "the plot must be an EMD component in markdown source"
        );
        assert!(text.contains("/proc/stat + /proc/meminfo"));
        assert!(
            text.contains("collecting"),
            "an empty history must not render as zero"
        );
    }

    #[test]
    fn top_rail_is_only_the_flat_notebook_shelf() {
        let rail = rail_view(
            &View::default(),
            &[machine("alpha")],
            &FleetRowsReport::default(),
        );
        let widgets = rail["widgets"].as_array().unwrap();
        let sections: Vec<_> = widgets
            .iter()
            .filter(|widget| widget["kind"] == "section")
            .filter_map(|widget| widget["text"].as_str())
            .collect();
        assert_eq!(sections, ["Notebooks"]);
        assert!(!rail.to_string().contains("host:alpha"));
        assert!(!rail.to_string().contains("Compose Notebook"));
        assert!(!rail.to_string().contains("Refresh"));

        let notebook_rows: Vec<_> = widgets
            .iter()
            .filter(|widget| {
                widget["id"]
                    .as_str()
                    .is_some_and(|id| id.starts_with("notebook:"))
            })
            .collect();
        assert_eq!(notebook_rows[0]["title"], "System Top");
        assert!(notebook_rows.iter().all(|row| row.get("icon").is_none()));
        assert!(notebook_rows.iter().all(|row| row.get("status").is_none()));
        assert!(notebook_rows.iter().all(|row| row.get("depth").is_none()));
    }

    #[test]
    fn dash_rail_contains_only_the_notebook_shelf() {
        let mut view = View::default();
        view.select_mode(MODE_DASH);
        let rail = rail_view(&view, &[machine("alpha")], &FleetRowsReport::default());
        let widgets = rail["widgets"].as_array().unwrap();
        let sections: Vec<_> = widgets
            .iter()
            .filter(|widget| widget["kind"] == "section")
            .filter_map(|widget| widget["text"].as_str())
            .collect();
        assert_eq!(sections, ["Notebooks"]);
        let first_row = widgets
            .iter()
            .find(|widget| widget["kind"] == "list-row")
            .unwrap();
        assert_eq!(first_row["title"], "Yggterm SysInternals");
    }

    #[test]
    fn system_top_processes_offer_an_explicit_signal_chooser() {
        let mut host = machine("alpha");
        host.readings[0]["top"] = json!([{
            "pid": 4242,
            "comm": "runaway",
            "cmd": "runaway --busy",
            "cpu_pct": 99.0,
            "rss_kb": 2048,
            "user": "test"
        }]);
        let report = FleetRowsReport::default();
        let ring = crate::timeline::Ring::default();
        let zfs = std::collections::VecDeque::new();

        let host_ring = crate::timeline::Ring::default();
        let closed = viewport_view(
            &View::default(),
            &[host.clone()],
            &report,
            &ring,
            &host_ring,
            &zfs,
        );
        let closed_widgets = closed["widgets"].as_array().unwrap();
        let process_index = closed_widgets
            .iter()
            .position(|widget| widget["id"] == "process:4242")
            .unwrap();
        let paper_index = closed_widgets
            .iter()
            .position(|widget| widget["id"] == "top_dashboard_doc")
            .unwrap();
        assert!(
            process_index < paper_index,
            "process actions must lead the Top page"
        );
        let closed_row = closed_widgets
            .iter()
            .find(|widget| widget["id"] == "process:4242")
            .unwrap();
        assert_eq!(closed_row["actions"][0]["label"], "Kill…");

        let open = View {
            process_signal_target: Some(4242),
            ..View::default()
        };
        let open = viewport_view(&open, &[host], &report, &ring, &host_ring, &zfs);
        let open_row = open["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|widget| widget["id"] == "process:4242")
            .unwrap();
        let labels: Vec<_> = open_row["actions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|action| action["label"].as_str())
            .collect();
        assert_eq!(labels, ["TERM", "INT", "KILL", "×"]);
    }

    /// ⭐ ytop opens ON Overview — there is no nameless view you reach by having
    /// selected nothing.
    #[test]
    fn the_default_view_opens_on_overview() {
        let view = View::default();
        assert_eq!(
            view.selected_notebook.as_deref(),
            Some(crate::notebook::OVERVIEW_ID)
        );
        assert_eq!(
            view.selected_page.as_deref(),
            Some(crate::notebook::overview_page_id(MODE_TOP).as_str())
        );
    }

    /// ...and selecting it renders the LIVE dashboard, not stored markdown.
    /// This is the whole claim: a notebook that is nonetheless a window.
    #[test]
    fn overview_renders_the_live_dashboard_not_a_stored_page() {
        let text = render(&View::default());
        // The live host card, composed from the reading above.
        assert!(text.contains("Test CPU"), "live probe data is missing");
        // ...and NOT the stored-page chrome that every paper notebook carries.
        assert!(
            !text.contains("Back to Overview"),
            "Overview rendered as a stored page instead of the live view"
        );
    }

    /// An ordinary notebook renders as clean paper. Navigation stays in the
    /// rail rather than being repeated inside the document.
    #[test]
    fn a_paper_notebook_still_renders_as_a_page() {
        let nb = crate::notebook::list_notebooks(Some(MODE_TOP))
            .into_iter()
            .find(|n| !crate::notebook::is_overview(&n.id))
            .expect("a paper notebook must exist on the Top shelf");
        let view = View {
            selected_notebook: Some(nb.id.clone()),
            selected_page: Some(nb.pages[0].id.clone()),
            ..View::default()
        };
        let text = render(&view);
        assert!(text.contains(&nb.title), "notebook title is missing");
        assert!(
            !text.contains("Back to Overview"),
            "document contains obsolete navigation chrome"
        );
    }

    /// Each mode opens on its named home. Dash is notebook-only and therefore
    /// opens on SysInternals, not a parallel nameless dashboard.
    #[test]
    fn each_mode_has_its_own_home() {
        let mut dash = View::default();
        dash.select_mode(MODE_DASH);
        assert_eq!(
            dash.selected_notebook.as_deref(),
            Some(crate::notebook::DASH_HOME_ID)
        );
        assert_eq!(dash.selected_page.as_deref(), Some("dash-sysint-p1"));
        assert!(render(&dash).contains("Two planes"));
    }

    #[test]
    fn invalid_mode_is_ignored_without_losing_the_current_notebook() {
        let mut view = View::default();
        let notebook = view.selected_notebook.clone();
        let page = view.selected_page.clone();

        assert!(!view.select_mode(""));
        assert_eq!(view.mode, MODE_TOP);
        assert_eq!(view.selected_notebook, notebook);
        assert_eq!(view.selected_page, page);
    }
}
