//! The widget schema — what yggterm paints in both Viewport and Rail surfaces.
//!
//! Viewport: SaaS-style GUI composition with rich multi-metric cards, progress meters, and process tables.
//! Rail: Focused machine switcher (in Top mode) or supervision/jankbox control panel (in Dash mode).
//! Titlebar Switch: Dynamic app-driven Top ↔ Dash mode toggle.

use crate::fleet::{self, Machine};
use crate::rows::FleetRowsReport;
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
    pub selected_notebook: Option<String>,
    pub selected_page: Option<String>,
    pub expanded_notebooks: Vec<String>,
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
            // ytop opens on Overview — the dashboard IS a notebook, so there
            // is no nameless view you reach by having selected nothing.
            selected_notebook: Some(crate::notebook::OVERVIEW_ID.to_string()),
            selected_page: Some(crate::notebook::overview_page_id(MODE_TOP)),
            expanded_notebooks: vec![crate::notebook::OVERVIEW_ID.to_string()],
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
    format!("`[{}{}]` **{:.1}%**", "█".repeat(filled), "░".repeat(empty), clamped)
}

pub fn plain_progress_bar(pct: f64, width: usize) -> String {
    let clamped = pct.clamp(0.0, 100.0);
    let filled = ((clamped / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}] {:.1}%", "█".repeat(filled), "░".repeat(empty), clamped)
}

pub fn spark_char(pct: f64) -> char {
    match (pct.clamp(0.0, 100.0) / 12.5) as usize {
        0 => '▁', 1 => '▂', 2 => '▃', 3 => '▄', 4 => '▅', 5 => '▆', 6 => '▇', _ => '█',
    }
}

fn label(text: impl Into<String>) -> Value {
    json!({"kind": "label", "text": text.into()})
}

fn chrono_like_now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
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

pub fn rail_view(view: &View, machines: &[Machine], report: &FleetRowsReport) -> Value {
    let mut widgets = Vec::new();

    if let Some(notice) = &view.notice {
        widgets.push(label(notice.clone()));
    }

    // ── 1. TOP MODE: RESERVED TOP PARTITION (≤30% AREA) FOR CONNECTED FLEET ──
    if view.mode == MODE_TOP {
        widgets.push(section("🌐 Connected Fleet", false));

        for m in machines {
            let p = m.principal();
            let host = p["host"].as_str().unwrap_or("?");
            let shown = p["label"].as_str().unwrap_or(host);
            let cpu = p["cpu_busy_pct"].as_f64().unwrap_or(0.0);
            let mem_total = p["mem_total_kb"].as_i64().unwrap_or(0);
            let mem_avail = p["mem_available_kb"].as_i64().unwrap_or(0);
            let mem_used = (mem_total - mem_avail).max(0);
            let is_selected = host == view.selected_host || (view.selected_host == fleet::LOCAL && host == "local");

            let subtitle = if m.reachable() {
                format!("{cpu:.0}% CPU · {} RAM", gb(mem_used))
            } else {
                "unreachable".to_string()
            };

            widgets.push(json!({
                "kind": "list-row",
                "id": format!("host:{host}"),
                "title": shown,
                "subtitle": subtitle,
                "icon": "🖥",
                "status": if m.reachable() { "durable" } else { "danger" },
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
    }

    // ── 2. NOTEBOOK ROWS (BOOKSHELF IN LIVE-SESSIONS VOCABULARY) ──
    widgets.push(section(
        if view.mode == MODE_TOP { "📚 Operational Notebooks" } else { "📚 Application Observability Notebooks" },
        false,
    ));

    for nb in crate::notebook::list_notebooks(Some(if view.mode == MODE_TOP { "top" } else { "dash" })) {
        let expanded = view.expanded_notebooks.contains(&nb.id);
        let is_selected = view.selected_notebook.as_deref() == Some(&nb.id);
        widgets.push(json!({
            "kind": "list-row",
            "id": format!("notebook:{}", nb.id),
            "title": nb.title.clone(),
            "subtitle": format!("{} pages · {}", nb.pages.len(), nb.author),
            "icon": "icon:folder",
            "status": if is_selected { "durable" } else { "transient" },
            "selected": is_selected,
            "depth": 0,
            "expanded": Some(expanded),
            "expand_action": format!("notebook_toggle:{}", nb.id),
            "row_action": format!("notebook_toggle:{}", nb.id),
        }));
        if expanded {
            for (idx, page) in nb.pages.iter().enumerate() {
                let selected_page = is_selected && view.selected_page.as_deref() == Some(&page.id);
                let page_icon = match page.chart.as_deref() {
                    Some("flamegraph") => "🔥",
                    Some("timeseries") | Some("timeline") => "📈",
                    Some("sparkline") => "📊",
                    Some("table") | Some("top_table") => "⚡",
                    _ => "file:md",
                };
                widgets.push(json!({
                    "kind": "list-row",
                    "id": format!("page:{}:{}", nb.id, page.id),
                    "title": page.title.clone(),
                    "subtitle": if page.has_ytrace() { "🔬 ytrace" } else { "host-only" },
                    "icon": page_icon,
                    "status": if selected_page { "durable" } else { "muted" },
                    "selected": selected_page,
                    "depth": 1,
                    "row_action": format!("page_open:{}:{}", nb.id, idx),
                }));
            }
        }
    }

    json!({
        "title": if view.mode == MODE_TOP { "Machines & Atlas" } else { "Fleet Cockpit" },
        "titlebar_switch": titlebar_switch_spec(&view.mode),
        "widgets": widgets,
        "footer": [
            json!({"kind": "button", "id": "refresh_rail", "action": "refresh", "label": "Refresh"}),
        ]
    })
}

// ─── VIEWPORT VIEW (SaaS Dashboard in Top vs Dash modes) ───────────────────────

pub fn viewport_view(view: &View, machines: &[Machine], report: &FleetRowsReport, timeline: &crate::timeline::Ring, zfs_history: &std::collections::VecDeque<crate::server::ZfsIoSample>) -> Value {
    // If a notebook page is selected, render the rich multi-card document surface
    if let Some(nb_id) = view
        .selected_notebook
        .as_ref()
        .filter(|id| !crate::notebook::is_overview(id))
    {
        if let Some(nb) = crate::notebook::get_notebook(nb_id) {
            if let Some(page_id) = &view.selected_page {
                if let Some(page) = nb.pages.iter().find(|p| &p.id == page_id && !p.composed) {
                    let mut widgets = Vec::new();
                    let page_idx = nb.pages.iter().position(|p| &p.id == page_id).unwrap_or(0);

                    // ── CARD 1: BREADCRUMB & METADATA HEADER ──
                    widgets.push(section(&format!("📖 {}", nb.title), true));
                    let created_dt = chrono::DateTime::from_timestamp_millis(nb.created_at_ms as i64)
                        .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                        .unwrap_or_else(|| "recent".to_string());

                    widgets.push(label(format!(
                        "**Page {} of {}: {}**  ·  Author: `{}`  ·  Created: `{}`",
                        page_idx + 1, nb.pages.len(), page.title, nb.author, created_dt
                    )));
                    widgets.push(label(format!(
                        "🔬 **Scope**: {}  ·  **Mode**: `{}`  ·  **ID**: `{}`",
                        if page.has_ytrace() { "Level-0 Host Metrics + ytrace Full Application Observability" } else { "Level-0 Host Infrastructure Metrics" },
                        nb.mode, page.id
                    )));

                    // ── CARD 2: CASE INVESTIGATION & HYPOTHESES ──
                    widgets.push(section("📝 Case Investigation & Hypotheses", true));
                    widgets.push(json!({
                        "kind": "markdown",
                        "id": format!("narrative:{}", page.id),
                        "source": page.markdown.clone(),
                    }));

                    // ── CARD 3: REAL-TIME OBSERVABILITY & VISUAL DIAGNOSTICS ──
                    if page.has_ytrace() {
                        let _span = crate::trace::span("notebook", "query");
                        widgets.push(section("📊 Real-Time Observability & Visual Diagnostics", true));
                        if let Some(q) = page.ytrace_queries.first() {
                            let homes = {
                                let mut hs = vec![ytrace::compat::resolve_home(&q.provider)];
                                if q.provider == "yggterm" {
                                    if let Some(xdg) = dirs::home_dir().map(|h| h.join(".local").join("share").join("ytrace").join("yggterm")) {
                                        if xdg != hs[0] && xdg.exists() {
                                            hs.push(xdg);
                                        }
                                    }
                                    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
                                        let p = std::path::PathBuf::from(xdg).join("ytrace").join("yggterm");
                                        if !hs.contains(&p) && p.exists() {
                                            hs.push(p);
                                        }
                                    }
                                }
                                hs
                            };
                            let since = Some(chrono_like_now_ms().saturating_sub(q.since_ms as u128));
                            let chart_type = page.chart.as_deref().unwrap_or("table");

                            match chart_type {
                                "flamegraph" => {
                                    let mut stacks = Vec::new();
                                    for home in &homes {
                                        stacks.extend(ytrace::query::flamegraph_folded(home, since, true));
                                    }
                                    stacks.sort_by(|a, b| b.1.cmp(&a.1));
                                    stacks.truncate(15);
                                    let max_dur = stacks.first().map(|s| s.1 as f64).unwrap_or(1.0).max(1.0);

                                    let mut md = String::from("## 🔥 Latency Flamegraph (Application Hierarchy)\n\n| Probe Hierarchy | Time Share | Latency |\n| :--- | :--- | :--- |\n");
                                    for (stack, dur_ns) in &stacks {
                                        let dur_ms = *dur_ns as f64 / 1_000_000.0;
                                        let pct = (*dur_ns as f64 / max_dur) * 100.0;
                                        let bar_len = ((pct / 100.0) * 16.0).round().clamp(0.0, 16.0) as usize;
                                        let bar = format!("`[{}{}]`", "█".repeat(bar_len), "░".repeat(16usize.saturating_sub(bar_len)));
                                        let clean_stack = stack.replace(';', " › ");
                                        md.push_str(&format!("| `{}` | {} | **{:.1} ms** |\n", clean_stack, bar, dur_ms));
                                    }
                                    if stacks.is_empty() {
                                        md.push_str("| — | — | — |\n> *No span records in window.*\n");
                                    }
                                    widgets.push(json!({ "kind": "markdown", "id": format!("ytrace_preview:{}", page.id), "source": md }));
                                }
                                "timeline" | "timeseries" => {
                                    let mut series = Vec::new();
                                    let bucket_ms = 60_000; // 1 min bucket
                                    for home in &homes {
                                        series.extend(ytrace::query::timeseries(home, bucket_ms, since));
                                    }
                                    series.sort_by_key(|b| b.bucket_start_ms);
                                    let mut md = String::from("## 📈 Time-Series Trend & Incident Rollup\n\n| Time (UTC) | Events | Spans | Total ms | p95 ms | Incidents |\n| :--- | :--- | :--- | :--- | :--- | :--- |\n");
                                    for b in series.iter().rev().take(12) {
                                        let dt = chrono::DateTime::from_timestamp_millis(b.bucket_start_ms as i64)
                                            .map(|d| d.format("%H:%M:%S").to_string())
                                            .unwrap_or_else(|| b.bucket_start_ms.to_string());
                                        let inc_badge = if b.incident_count > 0 {
                                            format!("🚨 **{}**", b.incident_count)
                                        } else {
                                            "0".to_string()
                                        };
                                        md.push_str(&format!("| `{}` | {} | {} | {:.1} ms | {:.1} ms | {} |\n", dt, b.count, b.span_count, b.total_duration_ms, b.p95_ms, inc_badge));
                                    }
                                    if series.is_empty() {
                                        md.push_str("| — | — | — | — | — | — |\n> *No timeseries activity in window.*\n");
                                    }
                                    widgets.push(json!({ "kind": "markdown", "id": format!("ytrace_preview:{}", page.id), "source": md }));
                                }
                                _ => {
                                    // Default: Top Probe Table
                                    let mut sums = Vec::new();
                                    for home in &homes {
                                        sums.extend(ytrace::query::summarize(home, Some(&q.category), since));
                                    }
                                    sums.sort_by(|a, b| b.total_ms.partial_cmp(&a.total_ms).unwrap());
                                    sums.truncate(10);
                                    let mut md = String::from("## 🔬 Top Probes (Application-Layer Latency & Calls)\n\n| Probe (category/name) | Clock | Count | Total ms | p50 ms | p95 ms | Max ms |\n| :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n");
                                    for s in &sums {
                                        md.push_str(&format!("| `{}/{}` | `{}` | {} | **{:.1} ms** | {:.1} ms | {:.1} ms | {:.1} ms |\n", s.category, s.name, s.clock, s.count, s.total_ms, s.p50_ms, s.p95_ms, s.max_ms));
                                    }
                                    if sums.is_empty() {
                                        md.push_str("| — | — | — | — | — | — | — |\n> *No ytrace records yet in window.*\n");
                                    }
                                    widgets.push(json!({ "kind": "markdown", "id": format!("ytrace_preview:{}", page.id), "source": md }));
                                }
                            }
                        }
                    }

                    // ── Live half of the page (host supervision/jankbox widgets) ──
                    if let Some(kind) = page.live.as_deref() {
                        widgets.push(section("⚡ Live Host & Supervision State", true));
                        for w in crate::sysinternals::live_widgets(kind, &page.id, report, false) {
                            widgets.push(w);
                        }
                    }

                    // ── CARD 4: REPRODUCIBILITY & ASSERTIONS ──
                    widgets.push(section("🔍 Reproducibility & Assertions", true));
                    let verify_source = if page.has_ytrace() {
                        let q = page.ytrace_queries.first();
                        let app = q.map(|x| x.provider.as_str()).unwrap_or("yggterm");
                        let cat = q.map(|x| x.category.as_str()).unwrap_or("render");
                        let since = q.map(|x| x.since_ms).unwrap_or(7200000);
                        format!(
                            "```bash\nytrace top --app {app} --category {cat} --since {since}ms\nytrace flame --app {app} --category {cat} --since {since}ms\nytop --mode dash\n```\n\n> ✅ **Invariants Checked**: Validated against live `/proc` 400ms deltas and `ytrace.jsonl` wire bus."
                        )
                    } else {
                        "```bash\nytop --mode top\n```\n\n> ✅ **Invariants Checked**: Validated against host `/proc` 400ms CPU deltas and ZFS storage pools.".to_string()
                    };
                    widgets.push(json!({
                        "kind": "markdown",
                        "id": format!("verify:{}", page.id),
                        "source": verify_source,
                    }));

                    // ── FOOTER PAGINATION CHROME ──
                    let mut footer = Vec::new();
                    if page_idx > 0 {
                        footer.push(json!({"kind": "button", "id": format!("page_prev:{}", nb.id), "action": format!("page_open:{}:{}", nb.id, page_idx - 1), "label": "← Prev Page"}));
                    }
                    if page_idx + 1 < nb.pages.len() {
                        footer.push(json!({"kind": "button", "id": format!("page_next:{}", nb.id), "action": format!("page_open:{}:{}", nb.id, page_idx + 1), "label": "Next Page →"}));
                    }
                    footer.push(json!({"kind": "button", "id": "book_back", "action": format!("page_open:{}:0", crate::notebook::OVERVIEW_ID), "label": "📚 Back to Overview"}));

                    return json!({
                        "title": format!("📖 {} — {}  ({} / {})", nb.title, page.title, page_idx + 1, nb.pages.len()),
                        "titlebar_switch": titlebar_switch_spec(&view.mode),
                        "widgets": widgets,
                        "footer": footer,
                    });
                }
            }
        }
    }

    let mut widgets = Vec::new();

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
                let error_msg = p["error"].as_str().unwrap_or("Host unreachable or connection timed out.");
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
                let mem_pct = if total_kb > 0 { used_kb as f64 * 100.0 / total_kb as f64 } else { 0.0 };
                let swap_total_kb = p["swap_total_kb"].as_i64().unwrap_or(0);
                let swap_free_kb = p["swap_free_kb"].as_i64().unwrap_or(0);
                let swap_used_kb = (swap_total_kb - swap_free_kb).max(0);
                let swap_pct = if swap_total_kb > 0 { swap_used_kb as f64 * 100.0 / swap_total_kb as f64 } else { 0.0 };
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
                        l.iter().filter_map(|v| v.as_f64()).map(|v| format!("{v:.2}")).collect::<Vec<_>>().join(" · ")
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
                        md.push_str("| Pool | Health | Allocation Meter | Used / Total | Frag % |\n");
                        md.push_str("| :--- | :--- | :--- | :--- | :--- |\n");
                        for pool in pools {
                            let name = pool["name"].as_str().unwrap_or("?");
                            let health = pool["health"].as_str().unwrap_or("UNKNOWN");
                            let cap_pct = pool["cap_pct"].as_f64().unwrap_or(0.0);
                            let size_b = pool["size_bytes"].as_i64().unwrap_or(0);
                            let alloc_b = pool["alloc_bytes"].as_i64().unwrap_or(0);
                            let frag_pct = pool["frag_pct"].as_i64().unwrap_or(0);
                            let health_badge = if health == "ONLINE" { "🟢 ONLINE" } else { "🟡 DEGRADED" };

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
                            let max_r = zfs_history.iter().map(|s| s.read_bps).max().unwrap_or(1).max(1) as f64;
                            let max_w = zfs_history.iter().map(|s| s.write_bps).max().unwrap_or(1).max(1) as f64;
                            let spark_r: String = zfs_history.iter().map(|s| spark_char(s.read_bps as f64 / max_r * 100.0)).collect();
                            let spark_w: String = zfs_history.iter().map(|s| spark_char(s.write_bps as f64 / max_w * 100.0)).collect();
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
                let ebpf_tools = p["ebpf_tools"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default();
                if ebpf_avail {
                    md.push_str("### eBPF Live Probes — opt-in, no overhead until you need it\n\n");
                    md.push_str(&format!("> Tools found: `{ebpf_tools}`. Use for deep dives when Top's 400ms delta isn't enough. Example: `sudo bpftrace -e 'tracepoint:sched:sched_switch {{ @[comm] = count(); }}'` → hot task histogram. Or `perf top -a` for kernel hotspots. Keep off by default — zero cost when idle.\n\n"));
                } else {
                    md.push_str("> eBPF probes hidden — install `bpftrace` or `perf` to enable live kernel tracing (zero overhead when not installed).\n\n");
                }

                // 4. LXC Containers — subtle, example-driven
                let containers = p["containers"].as_array().cloned().unwrap_or_default();
                if !containers.is_empty() {
                    md.push_str(&format!("## LXC Containers ({} Total)\n\n", containers.len()));
                    md.push_str("> A container is a lightweight machine inside your machine. `Status` is running or stopped, `Top Internal Process` is the busiest thing inside it. Example: `android-kvm RUNNING 0.0% 41 MB` → powered on but idle.\n\n");
                    md.push_str("| Container | Status | CPU % | RAM RSS | Tasks | Top Internal Process |\n");
                    md.push_str("| :--- | :--- | :--- | :--- | :--- | :--- |\n");

                    for c in &containers {
                        let c_name = c["name"].as_str().unwrap_or("?");
                        let state = c["state"].as_str().unwrap_or("?");
                        let c_cpu = c["cpu_busy_pct"].as_f64().unwrap_or(0.0);
                        let c_rss = c["mem_rss_kb"].as_i64().unwrap_or(0);
                        let procs_count = c["procs_count"].as_i64().unwrap_or(0);
                        let state_badge = if state == "RUNNING" { "🟢 RUNNING" } else { "⚪ STOPPED" };

                        let top_proc_str = c["top_procs"].as_array().and_then(|tps| tps.first()).map(|tp| {
                            let pid = tp["pid"].as_i64().unwrap_or(0);
                            let comm = tp["comm"].as_str().unwrap_or("?");
                            let cpu_p = tp["cpu_pct"].as_f64().unwrap_or(0.0);
                            format!("`{comm}` ({cpu_p:.1}% CPU, PID {pid})")
                        }).unwrap_or_else(|| "—".to_string());

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

                // 5. Host Top Processes Data Table
                let top_procs = p["top"].as_array().cloned().unwrap_or_default();
                if !top_procs.is_empty() {
                    md.push_str("## ⚡ Top Consuming Processes\n\n");
                    md.push_str("| PID | Signals | User | CPU % | RAM RSS | Scope | Command |\n");
                    md.push_str("| :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n");

                    let mut rendered_procs = 0;
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

                        let scope_badge = if let Some(cn) = cont {
                            format!("`ct:{cn}`")
                        } else {
                            "`host`".to_string()
                        };

                        let display_cmd = if !cmd.is_empty() {
                            let truncated = if cmd.len() > 60 { &cmd[..60] } else { cmd };
                            format!("`{comm}` <span style=\"color:#888\">{truncated}</span>")
                        } else {
                            format!("`{comm}`")
                        };

                        let signal_buttons = format!(
                            "[`🔴 KILL`](action:signal_process:{pid}:9) [`🟡 TERM`](action:signal_process:{pid}:15) [`🔵 INT`](action:signal_process:{pid}:2) [`🔄 HUP`](action:signal_process:{pid}:1)"
                        );

                        md.push_str(&format!(
                            "| `{pid}` | {signal_buttons} | `{user}` | **`{cpu_pct:.1}%`** | `{}` | {} | {} |\n",
                            mb(rss_kb),
                            scope_badge,
                            display_cmd
                        ));

                        rendered_procs += 1;
                        if rendered_procs >= 30 {
                            break;
                        }
                    }
                    md.push('\n');
                }

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
            md.push_str("> 🟢 **SUPERVISION ACTIVE**: Fleet orchestrators running normal scheduling.\n\n");
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
                let avg = samples.iter().map(|s| s.cpu_pct as f64).sum::<f64>() / samples.len() as f64;
                let peak = samples.iter().map(|s| s.cpu_pct as f64).fold(0.0, f64::max);
                let last_rss = samples.last().map(|s| s.rss_kb).unwrap_or(0);
                let spark: String = samples.iter().map(|s| {
                    let p = s.cpu_pct.clamp(0.0, 100.0) as f64;
                    match (p / 12.5) as usize {
                        0 => '▁', 1 => '▂', 2 => '▃', 3 => '▄', 4 => '▅', 5 => '▆', 6 => '▇', _ => '█',
                    }
                }).collect();
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
            md.push_str("> *Timeline ring filling — 5-min TTL, 1s bucket. Run 2 ticks to see spark.*\n\n");
        }

        // Fleet Rows Matrix
        md.push_str("## Fleet Agent Matrix\n\n");
        md.push_str("> One row per agent. `Seat` is position, `Role` is job, `Campaign` is project. `Status` is `Live` or `Dead`, `Context` is transcript size. Example: `019da16a ytop verify 0% Live` → your verification agent, idle.\n\n");
        md.push_str("| Seat | Role | Campaign | UUID | Status | Context | Supervision |\n");
        md.push_str("| :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n");

        for row in &report.rows {
            let seat = if row.seat.is_empty() { "?" } else { &row.seat };
            let role = if row.role.is_empty() { "agent" } else { &row.role };
            let camp = if row.campaign.is_empty() { "unregistered" } else { &row.campaign };
            let uuid = if row.uuid.is_empty() { "—" } else { &row.uuid };
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

            let ctx_str = format!("`{} KB` ({}L)", row.transcript_size_kb, row.transcript_lines);
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
        "title": if view.mode == MODE_TOP { "Infrastructure Monitor" } else { "Fleet Cockpit" },
        "titlebar_switch": titlebar_switch_spec(&view.mode),
        "widgets": widgets,
        "footer": [
            json!({"kind": "button", "id": "refresh_viewport", "action": "refresh", "label": "Refresh Dashboard"}),
        ]
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
        let out = viewport_view(view, &[machine("alpha")], &report, &ring, &zfs);
        serde_json::to_string(&out).unwrap()
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

    /// An ordinary notebook still renders as paper, with its way back.
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
        assert!(text.contains("Back to Overview"), "no way back to Overview");
    }

    /// ⚠ Each mode opens on ITS Overview. Carrying Top's page into Dash left the
    /// viewport on a page that is not on the shelf being looked at.
    #[test]
    fn each_mode_has_its_own_overview_page() {
        assert_ne!(
            crate::notebook::overview_page_id(MODE_TOP),
            crate::notebook::overview_page_id(MODE_DASH)
        );
        let dash = View {
            mode: MODE_DASH.to_string(),
            selected_page: Some(crate::notebook::overview_page_id(MODE_DASH)),
            ..View::default()
        };
        // Still the live view, not a stored page.
        assert!(!render(&dash).contains("Back to Overview"));
    }
}
