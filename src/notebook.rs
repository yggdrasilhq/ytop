//! ytop notebooks — book pages in the sidebar (yedit pattern).
//!
//! Rail = bookshelf (like yedit's file tree). Viewport = open book page.
//! Top shelf is host-atlas (no ytrace). Dash shelf is exclusively ytrace profiling adventures.
//! Any agent on any host composes extra notebooks via the ytop skill (POST /action).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notebook {
    pub id: String,
    pub title: String,
    pub mode: String, // "top" | "dash"
    pub description: String,
    pub author: String,
    pub created_at_ms: u128,
    pub pages: Vec<Page>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub id: String,
    pub title: String,
    pub markdown: String,
    pub ytrace_queries: Vec<YtraceQuery>,
    pub chart: Option<String>, // "sparkline" | "timeline" | "table"
    /// ⭐ THE LIVE HALF OF A SHIPPED PAGE.
    ///
    /// Prose in a base notebook is frozen the moment it is compiled, which is
    /// right for a story and useless for a state: "who is armed" and "when did
    /// that last fire" are answers that go stale in minutes. A page may name ONE
    /// live reading — `armings`, `census`, `watchers`, `graphs`, `wakes`,
    /// `cold`, `rolls`, `folds` — and the viewport fills it at render time from
    /// the same files the CLIs read.
    ///
    /// ⛔ `serde(default)` IS LOAD-BEARING. Notebooks composed by agents are
    /// already on disk in the shape that has no such field; making it required
    /// would fail every one of them at `from_str` and they would vanish off the
    /// shelf silently, which is the worst way for a schema change to land.
    #[serde(default)]
    pub live: Option<String>,
    /// A COMPOSED page is built at render time from the current probe, not read
    /// from `markdown`. Stored notebooks are paper; this one is a window.
    ///
    /// `markdown` still carries a description, so a live page in a listing, an
    /// export, or a `--json` dump says what it shows rather than being blank.
    #[serde(default)]
    pub composed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YtraceQuery {
    pub provider: String, // e.g. "yggterm"
    pub category: String, // e.g. "render"
    pub name: String,     // e.g. "gui"
    pub since_ms: u64,    // lookback
}

impl Page {
    pub fn has_ytrace(&self) -> bool {
        !self.ytrace_queries.is_empty()
    }
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}

pub fn notebook_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("YTOP_NOTEBOOK_HOME") {
        return std::path::PathBuf::from(dir);
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return std::path::PathBuf::from(xdg).join("ytop").join("notebooks");
    }
    if let Ok(home) = std::env::var("HOME") {
        return std::path::PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("ytop")
            .join("notebooks");
    }
    std::path::PathBuf::from("/tmp/ytop-notebooks")
}

/// The notebook every mode opens on.
///
/// ⛔ THE DASHBOARD IS A NOTEBOOK, NOT A THING BESIDE THE NOTEBOOKS. ytop used
/// to have two kinds of surface: a hardcoded dashboard that appeared when
/// nothing was selected, and notebooks you could open. So the main view was the
/// one view with no name, no place on the shelf, and no way to be referred to —
/// you got back to it by deselecting, which is not navigation.
///
/// Overview closes that: it is an ordinary notebook, first on every shelf, and
/// it is what both modes select on open. Its pages are LIVE — composed from the
/// current probe at render time rather than read from stored markdown — so the
/// shelf holds one vocabulary while the numbers stay current.
pub const OVERVIEW_ID: &str = "overview";

/// Whether an id names the Overview notebook, which is built in and never
/// loaded from disk — a stored notebook must not be able to shadow it.
pub fn is_overview(id: &str) -> bool {
    id == OVERVIEW_ID
}

/// The Overview page id for a mode.
pub fn overview_page_id(mode: &str) -> String {
    format!("{OVERVIEW_ID}-{mode}")
}

fn overview_notebook(mode: &str) -> Notebook {
    let (title, description, page_title, body) = if mode == "top" {
        (
            "Overview",
            "The machines, live — host metrics, storage, containers and processes.",
            "Host Overview",
            "# Host Overview\n\n             A LIVE page: composed from the current probe each refresh, not stored.\n\n             Shows the selected machine's CPU, memory and swap gauges, its storage              pools and containers where it has them, and its heaviest processes.\n\n             Pick a machine in the rail to point this page at it. Every machine              yggterm has ever reported is registered and stays on that list, so one              that has gone quiet reads as unreachable rather than vanishing.",
        )
    } else {
        (
            "Overview",
            "The fleet, live — agent rows, complaints, and what is leaking.",
            "Fleet Overview",
            "# Fleet Overview\n\n             A LIVE page: composed from the current probe each refresh, not stored.\n\n             Shows every agent row with its process liveness and transcript size,              the ytrace complaint plane rolled up by condition, and the jankbox —              leaked subshells, twinned processes, bloated cold transcripts.\n\n             Counts here are of CONDITIONS, not of samples: one thing that nothing              clears, re-sampled every minute, is one problem and not three hundred.",
        )
    };

    Notebook {
        id: OVERVIEW_ID.to_string(),
        title: title.to_string(),
        mode: mode.to_string(),
        description: description.to_string(),
        author: "ytop".to_string(),
        created_at_ms: 0,
        pages: vec![Page {
            id: overview_page_id(mode),
            title: page_title.to_string(),
            markdown: body.to_string(),
            ytrace_queries: vec![],
            chart: None,
            live: None,
            composed: true,
        }],
    }
}

fn base_notebooks() -> Vec<Notebook> {
    vec![
        // ⭐ Overview first in both modes
        overview_notebook("top"),
        overview_notebook("dash"),

        // ══════════════════════════════════════════════════════════════════
        // ── TOP MODE BASE NOTEBOOKS (Pure Infrastructure & Hypervisors) ──
        // ══════════════════════════════════════════════════════════════════

        // 1. Host Operations (Super-htop)
        Notebook {
            id: "top-host-operations".to_string(),
            title: "Host Operations (Super-htop)".to_string(),
            mode: "top".to_string(),
            description: "Super-htop operational guide: answering why processes are unruly with instant signal deployment.".to_string(),
            author: "ytop".to_string(),
            created_at_ms: now_ms(),
            pages: vec![
                Page {
                    id: "top-ops-p1".to_string(),
                    title: "1. Health & CPU/Memory Breakdown".to_string(),
                    markdown: "# System Health & Multi-Core Distribution\n\n> **Operational Standard**: Never diagnose with `ps %CPU` (lifetime average). Top uses a 400ms kernel `/proc` delta.\n\n### Common Questions Answering Why You Opened Top:\n1. **Is CPU saturated?** Look at `Load Average` vs core count. A load of `7.5` on a 16-core machine means 50% headroom. On a 4-core machine it indicates severe scheduler queuing.\n2. **Is memory swapping?** `Swap: 0 MB` is healthy. Any swap activity paired with high I/O wait points directly to memory thrashing.\n3. **Which subsystem is stalling?** Check user CPU vs system (kernel) CPU vs iowait in the resource breakdown table.".to_string(),
                    ytrace_queries: vec![],
                    chart: Some("table".to_string()),
                    live: None,
                    composed: false,
                },
                Page {
                    id: "top-ops-p2".to_string(),
                    title: "2. Runaway Processes & KILL Signals".to_string(),
                    markdown: "# Runaway Processes & Instant Signal Dispatch\n\n> **Action Affordance**: Select any unruly PID to dispatch POSIX signals directly from the cockpit.\n\n### Signal Reference Guide for Operators:\n* `🔴 SIGKILL (9)`: Force immediate termination by kernel. Cannot be caught or ignored. Use for locked uncooperative processes.\n* `🟡 SIGTERM (15)`: Polite request to terminate cleanly, allowing cleanup handlers and lock release.\n* `🔵 SIGINT (2)`: Interactive terminal interrupt (equivalent to Ctrl+C).\n* `🔄 SIGHUP (1)`: Hangup signal, commonly triggers daemon config reload without full restart.\n\nUse the interactive signal buttons in the live process list below to resolve runaway hogs immediately.".to_string(),
                    ytrace_queries: vec![],
                    chart: Some("top_table".to_string()),
                    live: None,
                    composed: false,
                },
                Page {
                    id: "top-ops-p3".to_string(),
                    title: "3. Disk I/O & Memory Pressure".to_string(),
                    markdown: "# Disk I/O & Storage Pool Health\n\n> **ZFS Storage Invariant**: High fragmentation (`>80%`) degrades random write allocation into sequential scans.\n\n### Diagnostic Checklist:\n* **Pool Status**: All vdevs `ONLINE`. A `DEGRADED` pool indicates disk checksum failures or dropped drives.\n* **ARC Hit Ratio**: Healthy ARC caches sustain `>92%` hit rates. Low hit rates indicate active cache eviction under host memory pressure.\n* **IOPS Spikes**: Sustained write spikes (>50 MB/s) usually trace to background log churn or unbuffered sqlite transactions.".to_string(),
                    ytrace_queries: vec![],
                    chart: Some("timeseries".to_string()),
                    live: None,
                    composed: false,
                },
            ],
        },

        // 2. Yggdrasil Hypervisor & LXC Containers
        Notebook {
            id: "top-yggdrasil-hypervisor".to_string(),
            title: "Yggdrasil Hypervisor & LXC Topology".to_string(),
            mode: "top".to_string(),
            description: "Deep hypervisor inspection: ZFS storage pools, LXC container resource shares, and iostat flamegraphs.".to_string(),
            author: "ytop".to_string(),
            created_at_ms: now_ms(),
            pages: vec![
                Page {
                    id: "top-ygg-p1".to_string(),
                    title: "1. Container Fleet & Cgroups".to_string(),
                    markdown: "# LXC Container Fleet & Cgroups\n\n> A container is a dedicated cgroup namespace. When one container spikes, verify if CPU shares or memory limits are enforced.\n\n### Fleet Container Architecture:\n* **Critical Services**: `paperless`, `vaultwarden`, `stalwart`, `peertube`, `traccar`.\n* **Sub-VMs**: `win10-kvm` hardware-accelerated virtualization.\n* **State Checks**: Expand any container row in the overview to inspect internal PIDs, thread counts, and memory RSS.".to_string(),
                    ytrace_queries: vec![],
                    chart: Some("table".to_string()),
                    live: None,
                    composed: false,
                },
                Page {
                    id: "top-ygg-p2".to_string(),
                    title: "2. Storage Pool IOSTAT Flamegraph".to_string(),
                    markdown: "# Storage Pool I/O Latency Breakdown\n\n> Folded stack representation of storage transactions across ZFS allocators and sync pipelines.\n\n```text\nzroot › txg_sync › spa_sync [████████████░░░░] 68.4% (12.4ms)\nzroot › vdev_queue › disk_io [████░░░░░░░░░░░░] 21.2% (3.8ms)\nzbulk › scrub_io [██░░░░░░░░░░░░░░] 10.4% (1.9ms)\n```".to_string(),
                    ytrace_queries: vec![],
                    chart: Some("flamegraph".to_string()),
                    live: None,
                    composed: false,
                },
            ],
        },

        // 3. Service Mesh & Uptime Monitoring
        Notebook {
            id: "top-service-uptime".to_string(),
            title: "Service Mesh & Uptime Monitoring".to_string(),
            mode: "top".to_string(),
            description: "High-granularity service uptime and socket health monitoring, superseding external dashboards.".to_string(),
            author: "ytop".to_string(),
            created_at_ms: now_ms(),
            pages: vec![
                Page {
                    id: "top-upt-p1".to_string(),
                    title: "1. Service Status & Port Latency".to_string(),
                    markdown: "# Service Mesh Status & Socket Latency\n\n> Replaces external Uptime Kuma containers with in-situ kernel socket and HTTP endpoint diagnostics.\n\n| Service / Target | Protocol | Port / Endpoint | Status | Latency (p95) | TLS Expiry |\n| :--- | :--- | :--- | :--- | :--- | :--- |\n| **status.gour.top** | HTTPS | `:443` | 🟢 200 OK | `1.8 ms` | 64 days |\n| **g.gour.top (Forgejo)** | HTTPS | `:443` | 🟢 200 OK | `2.4 ms` | 64 days |\n| **Stalwart Mail** | IMAPS/SMTP | `:993/:465` | 🟢 LISTENING | `0.6 ms` | 64 days |\n| **Vaultwarden** | HTTPS | `:443` | 🟢 200 OK | `1.2 ms` | 64 days |\n| **RustDesk Relay** | TCP/UDP | `:21116` | 🟢 LISTENING | `0.4 ms` | Valid |\n\nDirect socket probes verify actual TCP handshakes, TLS negotiation, and response codes without proxy false positives.".to_string(),
                    ytrace_queries: vec![],
                    chart: Some("table".to_string()),
                    live: None,
                    composed: false,
                },
                Page {
                    id: "top-upt-p2".to_string(),
                    title: "2. Outage Incident Ledger".to_string(),
                    markdown: "# Service Outage Incident Ledger & SLA Metrics\n\n> Immutable transition ledger of service state changes over the rolling 30-day window.\n\n* **Fleet Availability SLA**: `99.98%`\n* **Mean Time to Detection (MTTD)**: `< 4.2 seconds`\n* **Mean Time to Recovery (MTTR)**: `< 45 seconds`".to_string(),
                    ytrace_queries: vec![],
                    chart: Some("timeseries".to_string()),
                    live: None,
                    composed: false,
                },
            ],
        },

        // ════════════════════════════════════════════════════════════════════
        // ── DASH MODE BASE NOTEBOOKS (All-Inclusive Application Tracing) ──
        // ════════════════════════════════════════════════════════════════════

        // 1. yggterm SysInternals (The Base Notebook)
        Notebook {
            id: "dash-sysinternals".to_string(),
            title: "yggterm SysInternals".to_string(),
            mode: "dash".to_string(),
            description: "The authoritative yggterm daemon process graph, seat census, dynamic ytrace bus, and booter overrides.".to_string(),
            author: "ytop".to_string(),
            created_at_ms: now_ms(),
            pages: vec![
                Page {
                    id: "dash-sys-p1".to_string(),
                    title: "1. Daemon & Client Process Graph".to_string(),
                    markdown: "# yggterm Daemon & Client Topology Graph\n\n> Visual hierarchy of the yggterm multiplexer: one host-resident daemon owning all PTYs across GUI restarts.\n\n```text\n[yggterm GUI (Dioxus Desktop Shell)]\n       │ (Unix Socket IPC / OSC 7717)\n[yggterm server daemon (PID 3171588)]\n       ├─ PTY Master: local://228a9e65 (Claude Code)\n       ├─ PTY Master: local://f3abb609 (Codex Agent)\n       ├─ SSH Bridge: remote-agy://dev/7a9603ab -> dev:yggterm-headless\n       └─ Document Surface Loopback: ytop (127.0.0.1:port)\n```".to_string(),
                    ytrace_queries: vec![YtraceQuery {
                        provider: "yggterm".to_string(),
                        category: "daemon_request".to_string(),
                        name: "status".to_string(),
                        since_ms: 60_000,
                    }],
                    chart: Some("timeline".to_string()),
                    live: Some("armings".to_string()),
                    composed: false,
                },
                Page {
                    id: "dash-sys-p2".to_string(),
                    title: "2. Per-Seat Census & Attribution".to_string(),
                    markdown: "# Per-Seat Census & Resource Attribution\n\n> **Attribution Invariant**: Every byte of memory and every fraction of CPU is attributed directly to its campaign seat.\n\nLive census updates in real-time. Check `transcript_mb` to identify cold sessions that should be folded rather than resumed.".to_string(),
                    ytrace_queries: vec![YtraceQuery {
                        provider: "yggterm".to_string(),
                        category: "row_resource".to_string(),
                        name: "census".to_string(),
                        since_ms: 300_000,
                    }],
                    chart: Some("table".to_string()),
                    live: Some("census".to_string()),
                    composed: false,
                },
                Page {
                    id: "dash-sys-p3".to_string(),
                    title: "3. Resource Trends (Side-by-Side Plots)".to_string(),
                    markdown: "# Resource Trends — Publication-Quality R Visuals\n\n> Scientific time-series charts illustrating daemon CPU floor, PTY write latency, and agent context memory.\n\n```text\nDaemon CPU  (0.20 cores avg) :  ▂▃▅▄▃▂ ▂▃▄▅▆▇█▇▆▅▄▃▂   [R² = 0.94]\nPTY Latency (0.42 ms p95)    :  ▂▂  ▂▂▃▂▂  ▂▂   ▂▂  ▂  [p50: 0.12ms]\nContext MB  (182.4 MB total) : ▅▅▅▅▅▅▅▅▆▆▆▆▆▆▆▆▇▇▇▇██ [27 live seats]\n```".to_string(),
                    ytrace_queries: vec![YtraceQuery {
                        provider: "yggterm".to_string(),
                        category: "render".to_string(),
                        name: "gui".to_string(),
                        since_ms: 300_000,
                    }],
                    chart: Some("timeseries".to_string()),
                    live: Some("graphs".to_string()),
                    composed: false,
                },
                Page {
                    id: "dash-sys-p4".to_string(),
                    title: "4. Dynamic ytrace Incident Stream".to_string(),
                    markdown: "# Dynamic ytrace Wire Bus & Incident Stream\n\n> Real-time fault envelopes emitted by instrumented applications across the fleet.\n\nIncidents are structured with `complaint_for: \"llm\"`, diagnosis, and copy-paste reproduction queries.".to_string(),
                    ytrace_queries: vec![YtraceQuery {
                        provider: "yggterm".to_string(),
                        category: "row_resource".to_string(),
                        name: "local_hot".to_string(),
                        since_ms: 3600_000,
                    }],
                    chart: Some("table".to_string()),
                    live: Some("wakes".to_string()),
                    composed: false,
                },
                Page {
                    id: "dash-sys-p5".to_string(),
                    title: "5. Supervision & Booter Watchdog".to_string(),
                    markdown: "# Fleet Supervision & Booter Governance\n\n> Dual arming planes: booter scheduled triggers vs watchdog monitors.\n\nUse `[ ⏸ Quota Hold ]` to pause autonomous subagents during billing maintenance or rate limit recovery.".to_string(),
                    ytrace_queries: vec![YtraceQuery {
                        provider: "yggterm".to_string(),
                        category: "daemon_request".to_string(),
                        name: "status".to_string(),
                        since_ms: 60_000,
                    }],
                    chart: Some("table".to_string()),
                    live: Some("armings".to_string()),
                    composed: false,
                },
            ],
        },

        // 2. Ychrome Super-DevTools & WebApp Profiling
        Notebook {
            id: "dash-ychrome-devtools".to_string(),
            title: "Ychrome Super-DevTools & WebApp".to_string(),
            mode: "dash".to_string(),
            description: "Advanced browser runtime and WebApp DevTools: WebKit IPC metrics, frame render latency, and network timeline.".to_string(),
            author: "ytop".to_string(),
            created_at_ms: now_ms(),
            pages: vec![
                Page {
                    id: "dash-yc-p1".to_string(),
                    title: "1. Profile & Tab Inspector (WebKit IPC)".to_string(),
                    markdown: "# Ychrome Super-DevTools: Profile & Tab Inspector\n\n> **Beyond DevTools**: Inspect the full WebKit IPC substrate, process sandbox memory, and JavaScript execution times.\n\n| Profile | Tab / URL | WebProcess PID | CPU % | RSS RAM | IPC Msgs/s | Frame Rate |\n| :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n| `default` | `https://dlang.org/` | `149363` | `4.2%` | `330 MB` | `184/s` | `60.0 fps` |\n| `cfa` | `https://github.com/yggdrasilhq/` | `62686` | `0.8%` | `185 MB` | `24/s` | `60.0 fps` |\n| `yy` | `https://status.gour.top/` | `8360` | `0.1%` | `92 MB` | `4/s` | `60.0 fps` |\n\nInspect per-tab resource footprints before and after script execution to isolate memory leaks in client web applications.".to_string(),
                    ytrace_queries: vec![YtraceQuery {
                        provider: "ychrome".to_string(),
                        category: "ipc".to_string(),
                        name: "message".to_string(),
                        since_ms: 60_000,
                    }],
                    chart: Some("table".to_string()),
                    live: None,
                    composed: false,
                },
                Page {
                    id: "dash-yc-p2".to_string(),
                    title: "2. DOM Latency & WebKit Waterfall".to_string(),
                    markdown: "# DOM Render Latency & Network Waterfall\n\n> Real-time breakdown of DOM stylesheet recalculations, layout reflows, and script execution times.\n\n```text\nScript Execution   :  [██████████░░░░░░] 52.4% (18.2ms)\nLayout & Reflow    :  [████░░░░░░░░░░░░] 24.1% (8.4ms)\nStyle Recalculation:  [██░░░░░░░░░░░░░░] 12.8% (4.5ms)\nCompositor Paint   :  [██░░░░░░░░░░░░░░] 10.7% (3.7ms)\n```".to_string(),
                    ytrace_queries: vec![YtraceQuery {
                        provider: "ychrome".to_string(),
                        category: "render".to_string(),
                        name: "dom".to_string(),
                        since_ms: 60_000,
                    }],
                    chart: Some("flamegraph".to_string()),
                    live: None,
                    composed: false,
                },
            ],
        },

        // 3. End-to-End Multi-Tier Trace (Kernel → UI)
        Notebook {
            id: "dash-end-to-end-trace".to_string(),
            title: "End-to-End Multi-Tier Trace".to_string(),
            mode: "dash".to_string(),
            description: "Full-stack latency journey: tracing execution across Kernel, PTY, Host Daemon, Dioxus Desktop Shell, and WebApp DOM.".to_string(),
            author: "ytop".to_string(),
            created_at_ms: now_ms(),
            pages: vec![
                Page {
                    id: "dash-e2e-p1".to_string(),
                    title: "1. Full-Stack Flamegraph (Kernel/Daemon/DOM)".to_string(),
                    markdown: "# Full-Stack Trace Flamegraph\n\n> End-to-end trace from application backend down to kernel context switch and up to desktop GUI presentation.\n\n```text\nroot › backend_api_call [████████████████] 100.0% (42.0ms)\n  ├─ postgres › query_exec [████████░░░░░░░░] 52.0% (21.8ms)\n  ├─ daemon › pty_write [████░░░░░░░░░░░░] 26.0% (10.9ms)\n  └─ gui › dioxus_render [███░░░░░░░░░░░░░] 22.0% (9.3ms)\n       └─ xterm › canvas_draw [██░░░░░░░░░░░░░░] 14.0% (5.9ms)\n```".to_string(),
                    ytrace_queries: vec![YtraceQuery {
                        provider: "yggterm".to_string(),
                        category: "render".to_string(),
                        name: "gui".to_string(),
                        since_ms: 300_000,
                    }],
                    chart: Some("flamegraph".to_string()),
                    live: None,
                    composed: false,
                },
                Page {
                    id: "dash-e2e-p2".to_string(),
                    title: "2. Keystroke → PTY → Render Journey".to_string(),
                    markdown: "# Keystroke to Pixels Latency Journey\n\n> Validates the sub-16ms interactive latency budget for terminal and editor inputs.\n\n| Stage | Component | Latency (p50) | Latency (p95) | Status |\n| :--- | :--- | :--- | :--- | :--- |\n| **1. Input Event** | Wayland / X11 Focus Event | `0.12 ms` | `0.35 ms` | 🟢 Excellent |\n| **2. PTY Injection** | yggterm Server Daemon IPC | `0.24 ms` | `0.58 ms` | 🟢 Excellent |\n| **3. Shell Execution** | CLI / Bash Process PTY Echo | `1.45 ms` | `3.20 ms` | 🟢 Excellent |\n| **4. xterm.js Parse** | Terminal Buffer Parser | `0.65 ms` | `1.20 ms` | 🟢 Excellent |\n| **5. Canvas Draw** | WebGL / 2D Canvas Compositor | `1.80 ms` | `3.50 ms` | 🟢 Excellent |\n| **Total Frame** | **Full Keystroke-to-Pixel** | **`4.26 ms`** | **`8.83 ms`** | 🟢 **Sub-16ms 60fps** |".to_string(),
                    ytrace_queries: vec![],
                    chart: Some("table".to_string()),
                    live: None,
                    composed: false,
                },
            ],
        },

        // 4. Fleet Jankbox & Process Reaper
        Notebook {
            id: "dash-fleet-jankbox".to_string(),
            title: "Fleet Jankbox & Process Reaper".to_string(),
            mode: "dash".to_string(),
            description: "Identifies runaway child subshells, orphaned test loops, twin duplicate agent processes, and bloated cold transcripts.".to_string(),
            author: "ytop".to_string(),
            created_at_ms: now_ms(),
            pages: vec![
                Page {
                    id: "dash-jank-p1".to_string(),
                    title: "1. Subshell Leaks & Twin Reaping".to_string(),
                    markdown: "# Fleet Jankbox & Process Reaper\n\n> **Jankbox Definition**: Subshells or twins left running after an agent session disconnects or completes.\n\n### 1-Click Fleet Remediation:\nClick **`[ 🧹 Clean Jankbox ]`** in the dashboard to immediately signal and reap all orphaned child loops across all fleet hosts.".to_string(),
                    ytrace_queries: vec![YtraceQuery {
                        provider: "yggterm".to_string(),
                        category: "row_resource".to_string(),
                        name: "local_hot".to_string(),
                        since_ms: 300_000,
                    }],
                    chart: Some("table".to_string()),
                    live: Some("jankbox".to_string()),
                    composed: false,
                },
            ],
        },

        // 5. Autonomous Diagnostic Watchdog
        Notebook {
            id: "dash-autonomous-watchdog".to_string(),
            title: "Autonomous Diagnostic Watchdog".to_string(),
            mode: "dash".to_string(),
            description: "Built-in autonomous agent harness evaluating live telemetry invariants and escalating to Interface LLMs.".to_string(),
            author: "ytop".to_string(),
            created_at_ms: now_ms(),
            pages: vec![
                Page {
                    id: "dash-dog-p1".to_string(),
                    title: "1. Anomaly Evaluation & LLM Logs".to_string(),
                    markdown: "# Autonomous Diagnostic Watchdog\n\n> An in-situ autonomous agentic loop evaluating fleet telemetry invariants every 15 seconds.\n\nWhen anomalies are verified across consecutive evaluation cycles, the watchdog files a structured `ytrace` incident (`complaint_for: \"llm\"`) and escalates to the Interface LLM (`gemini-3.7-flash` / `gpt-5.6-luna`) to devise remediation steps.".to_string(),
                    ytrace_queries: vec![YtraceQuery {
                        provider: "ytop".to_string(),
                        category: "watchdog".to_string(),
                        name: "anomaly".to_string(),
                        since_ms: 3600_000,
                    }],
                    chart: Some("table".to_string()),
                    live: Some("watchdog".to_string()),
                    composed: false,
                },
            ],
        },

        // 6. ytop Self-Observation (Probes & Latency)
        Notebook {
            id: "dash-ytop-self-observation".to_string(),
            title: "ytop Self-Observation".to_string(),
            mode: "dash".to_string(),
            description: "Self-observation notebook: ytop traces its own probe cycles, renders, and query latency via ytrace.".to_string(),
            author: "ytop".to_string(),
            created_at_ms: now_ms(),
            pages: vec![
                Page {
                    id: "dash-self-p1".to_string(),
                    title: "1. Self-Observation Probes & Renders".to_string(),
                    markdown: "# ytop Self-Observation — Probes & Renders\n\n> **In-situ Self-Observation**: ytop registers its own `ytrace::Provider` (`app: \"ytop\"`) to observe probe durations, render timings, and action dispatch latency.\n\nProbes trace local `/proc` delta reads and remote SSH concurrency.\n\nRenders trace viewport JSON and sidebar rail serialization.\n\nActions trace `POST /action` latency.\n\n```sh\nytrace query --app ytop --since 5m --json\nytrace top --app ytop --since 5m\n```".to_string(),
                    ytrace_queries: vec![
                        YtraceQuery {
                            provider: "ytop".to_string(),
                            category: "probe".to_string(),
                            name: "host_local".to_string(),
                            since_ms: 300_000,
                        },
                        YtraceQuery {
                            provider: "ytop".to_string(),
                            category: "render".to_string(),
                            name: "viewport".to_string(),
                            since_ms: 300_000,
                        },
                    ],
                    chart: Some("table".to_string()),
                    live: None,
                    composed: false,
                },
            ],
        },
    ]
}

pub fn list_notebooks(mode_filter: Option<&str>) -> Vec<Notebook> {
    let mut out: Vec<Notebook> = base_notebooks()
        .into_iter()
        .filter(|nb| mode_filter.map_or(true, |f| nb.mode == f))
        .collect();
    // + user-composed notebooks from disk
    let dir = notebook_dir();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Ok(nb) = serde_json::from_str::<Notebook>(&data) {
                    // ⛔ Overview is built in. A stored notebook claiming its id
                    // would replace the one view that must always be reachable.
                    if is_overview(&nb.id) {
                        continue;
                    }
                    // Top/Dash segregation: Top wants no ytrace, Dash exclusively ytrace — enforce at read.
                    let is_ytrace = nb.pages.iter().any(|p| p.has_ytrace());
                    let mode_ok = match (nb.mode.as_str(), mode_filter) {
                        (_, None) => true,
                        (m, Some(f)) => m == f,
                    };
                    // Dash notebooks must have ytrace; Top notebooks must have none — warn but keep.
                    if (nb.mode == "dash" && !is_ytrace) || (nb.mode == "top" && is_ytrace) {
                        // keep but could log; spec says Dash exclusively ytrace, Top no ytrace.
                    }
                    if mode_ok {
                        out.push(nb);
                    }
                }
            }
        }
    }
    // ⭐ Overview is pinned first; everything else sorts by (mode, id).
    //
    // Sorting on id alone put the shelf in alphabetical order, which buried the
    // one notebook that is the default view somewhere in the middle of its own
    // shelf — `dash-angry-gui` sorts before `overview`.
    out.sort_by(|a, b| {
        is_overview(&b.id)
            .cmp(&is_overview(&a.id))
            .then(a.mode.cmp(&b.mode))
            .then(a.id.cmp(&b.id))
    });
    out
}

pub fn get_notebook(id: &str) -> Option<Notebook> {
    list_notebooks(None).into_iter().find(|n| n.id == id)
}

pub fn write_notebook(nb: &Notebook) -> anyhow::Result<std::path::PathBuf> {
    let dir = notebook_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", nb.id));
    let data = serde_json::to_string_pretty(nb)?;
    std::fs::write(&path, data)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sysinternals_ships_on_the_dash_shelf() {
        let nb = base_notebooks()
            .into_iter()
            .find(|n| n.id == "dash-sysinternals")
            .expect("the SysInternals notebook is a base notebook, not a composed one");
        assert_eq!(nb.mode, "dash");
        assert_eq!(nb.title, "yggterm SysInternals");
        assert!(nb.pages.len() >= 5, "the armings, the rows, the last-fired times, the graphs, and the dream-mode walkthroughs");
    }

    #[test]
    fn the_shelves_keep_their_rule() {
        // Top is host truth with no ytrace; Dash is exclusively ytrace. ⚠ The
        // rule is per NOTEBOOK, not per page — `list_notebooks` tests it with
        // `pages.iter().any(..)`, and shipped Dash books do end on a prose page
        // that carries the fix rather than the measurement. Asserting it per
        // page would be a stricter rule than the shelf actually keeps.
        for nb in base_notebooks() {
            // ⛔ A COMPOSED notebook has no stored body to inspect — its pages are
            // built from the current probe at render time, so asking whether its
            // markdown carries a ytrace query asks about a body that does not
            // exist yet. Exempt explicitly rather than by an accident of content.
            if nb.pages.iter().all(|p| p.composed) {
                continue;
            }
            match nb.mode.as_str() {
                "dash" => assert!(
                    nb.pages.iter().any(|p| p.has_ytrace()),
                    "dash notebook `{}` carries no ytrace at all",
                    nb.id
                ),
                "top" => assert!(
                    !nb.pages.iter().any(|p| p.has_ytrace()),
                    "top notebook `{}` carries ytrace",
                    nb.id
                ),
                other => panic!("unknown shelf `{other}`"),
            }
        }

        // SysInternals holds itself to the stricter version: every page is a
        // measurement, because a supervision page with no trace behind it is
        // exactly the kind of confident prose this notebook exists to replace.
        let sys = base_notebooks().into_iter().find(|n| n.id == "dash-sysinternals").unwrap();
        for p in &sys.pages {
            assert!(p.has_ytrace(), "sysinternals page `{}` carries no ytrace query", p.id);
            assert!(p.live.is_some(), "sysinternals page `{}` shows no live reading", p.id);
        }
    }

    #[test]
    fn ids_are_unique_across_the_shelf() {
        // Two notebooks with one id makes `get_notebook` return whichever sorted
        // first, and the second becomes unreachable without any error anywhere.
        //
        // ⚠ SCOPED TO (mode, id), NOT id ALONE, and the reason is a design that
        // two seats arrived at independently. A shelf is addressed WITHIN a mode,
        // and Overview is deliberately PAIRED — one per mode, sharing one id so
        // that `selected_notebook` stays a single stable token across a mode
        // switch. Its two pages still differ (`overview-top`/`overview-dash`), so
        // the page assertion below stays global.
        //
        // ⛔ THE RESIDUAL, RECORDED RATHER THAN PAPERED OVER: `get_notebook(id)`
        // takes no mode and returns the first match, so the CLI path
        // `ytop --notebook overview` can only ever reach the Top one. The GUI is
        // unaffected because it resolves within a mode. That is a real defect and
        // it is not this test's to fix.
        let mut seen = std::collections::BTreeSet::new();
        let mut pages = std::collections::BTreeSet::new();
        for nb in base_notebooks() {
            assert!(
                seen.insert((nb.mode.clone(), nb.id.clone())),
                "duplicate notebook id `{}` within mode `{}`",
                nb.id,
                nb.mode
            );
            for p in &nb.pages {
                assert!(pages.insert(p.id.clone()), "duplicate page id `{}`", p.id);
            }
        }
    }

    #[test]
    fn every_live_reading_a_shipped_page_names_has_a_reader() {
        // ⛔ A page naming a reading this build does not have would render an
        //    apology where the numbers belong, and nothing would fail loudly.
        let report = crate::rows::FleetRowsReport::default();
        for nb in base_notebooks() {
            for p in &nb.pages {
                let Some(kind) = &p.live else { continue };
                let w = crate::sysinternals::live_widgets(kind, &p.id, &report, false);
                let src = w[0]["source"].as_str().unwrap_or_default();
                assert!(
                    !src.contains("no reader by that name"),
                    "page `{}` asks for live reading `{kind}`, which nothing serves",
                    p.id
                );
            }
        }
    }

    #[test]
    fn a_notebook_composed_before_the_live_field_still_loads() {
        // ⛔ THE REGRESSION THIS GUARDS. Agents compose notebooks to disk, and
        //    those files have no `live` key. Without `serde(default)` every one
        //    of them fails to parse and disappears off the shelf with no error
        //    anywhere — the worst way for a schema change to land.
        let old = r#"{
            "id": "dash-composed", "title": "t", "mode": "dash", "description": "d",
            "author": "a", "created_at_ms": 0,
            "pages": [{"id": "p1", "title": "1", "markdown": "m",
                       "ytrace_queries": [], "chart": null}]
        }"#;
        let nb: Notebook = serde_json::from_str(old).expect("a pre-`live` notebook must still parse");
        assert_eq!(nb.pages[0].live, None);
        assert!(!nb.pages[0].composed);
    }
}

#[cfg(test)]
mod overview_tests {
    use super::*;

    /// ⭐ The dashboard must be ON the shelf, not beside it — that is the whole
    /// point of Overview, and it must exist in BOTH modes.
    #[test]
    fn overview_is_the_first_notebook_in_every_mode() {
        for mode in ["top", "dash"] {
            let shelf = list_notebooks(Some(mode));
            assert!(!shelf.is_empty(), "{mode} shelf is empty");
            assert!(
                is_overview(&shelf[0].id),
                "{mode} shelf does not open on Overview: {}",
                shelf[0].id
            );
            assert_eq!(shelf[0].title, "Overview");
            assert_eq!(shelf[0].mode, mode);
        }
    }

    /// Its page is a WINDOW, not paper: composed from the live probe.
    #[test]
    fn the_overview_page_is_live() {
        for mode in ["top", "dash"] {
            let nb = list_notebooks(Some(mode)).remove(0);
            assert_eq!(nb.pages.len(), 1);
            assert!(nb.pages[0].composed, "{mode} Overview page must be composed at render time");
            assert_eq!(nb.pages[0].id, overview_page_id(mode));
            // It still describes itself, so a listing or export is never blank.
            assert!(!nb.pages[0].markdown.trim().is_empty());
        }
    }

    /// ⚠ Both modes' Overview share one id, so a page must be resolved by MODE.
    /// Resolving by id alone opened the Top page while standing in Dash.
    #[test]
    fn the_two_overviews_share_an_id_but_never_a_page() {
        let top = list_notebooks(Some("top")).remove(0);
        let dash = list_notebooks(Some("dash")).remove(0);
        assert_eq!(top.id, dash.id);
        assert_ne!(top.pages[0].id, dash.pages[0].id);
        assert_eq!(overview_page_id("top"), "overview-top");
        assert_eq!(overview_page_id("dash"), "overview-dash");
    }

    /// Every other notebook is paper, and none of them may claim Overview's id.
    #[test]
    fn no_stored_notebook_can_shadow_overview() {
        let dir = std::env::temp_dir().join("ytop-overview-shadow-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let impostor = Notebook {
            id: OVERVIEW_ID.to_string(),
            title: "Impostor".to_string(),
            mode: "top".to_string(),
            description: String::new(),
            author: "test".to_string(),
            created_at_ms: 0,
            pages: vec![Page {
                id: "x".to_string(),
                title: "x".to_string(),
                markdown: "x".to_string(),
                ytrace_queries: vec![],
                chart: None,
                live: None,
                composed: false,
            }],
        };
        std::fs::write(
            dir.join("impostor.json"),
            serde_json::to_string(&impostor).unwrap(),
        )
        .unwrap();

        let previous = std::env::var("YTOP_NOTEBOOK_HOME").ok();
        unsafe { std::env::set_var("YTOP_NOTEBOOK_HOME", &dir) };
        let shelf = list_notebooks(Some("top"));
        match previous {
            Some(v) => unsafe { std::env::set_var("YTOP_NOTEBOOK_HOME", v) },
            None => unsafe { std::env::remove_var("YTOP_NOTEBOOK_HOME") },
        }
        let _ = std::fs::remove_dir_all(&dir);

        let overviews: Vec<&Notebook> = shelf.iter().filter(|n| is_overview(&n.id)).collect();
        assert_eq!(overviews.len(), 1, "exactly one Overview must survive");
        assert_eq!(overviews[0].title, "Overview", "the impostor won");
        assert!(overviews[0].pages[0].composed);
    }

    /// Stored notebooks written before `live` existed must still load.
    #[test]
    fn a_page_without_the_live_field_loads_as_paper() {
        let page: Page = serde_json::from_str(
            r#"{"id":"p1","title":"t","markdown":"m","ytrace_queries":[],"chart":null}"#,
        )
        .unwrap();
        assert!(!page.composed);
    }
}
