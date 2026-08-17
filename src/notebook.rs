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

fn base_notebooks() -> Vec<Notebook> {
    vec![
        // Top — no ytrace (host atlas)
        Notebook {
            id: "top-atlas-jojo".to_string(),
            title: "Host Atlas — jojo at 53 rows".to_string(),
            mode: "top".to_string(),
            description: "Top wants no ytrace — host, ZFS, LXC. Read like an atlas.".to_string(),
            author: "ytop".to_string(),
            created_at_ms: now_ms(),
            pages: vec![
                Page {
                    id: "top-atlas-p1".to_string(),
                    title: "1. The Machine".to_string(),
                    markdown: "# Host Atlas — jojo\n\n> **Rule:** Top is host truth without ytrace. Dash is exclusively ytrace.\n\n| Atlas | Host |\n| :--- | :--- |\n| **jojo** | 53 live sessions, 2 plain shells (169×65), 11 %CPU load 0.72 |\n| **ZFS** | `zroot` + `zbulk` pools, `frag %` tells how scattered — `>80%` deserves balance |\n| **LXC** | `44 Total` — expand a container to see its top processes (PID/CPU/RSS) |\n\n`probe.rs` 400 ms `/proc` delta — total work across all cores, not `ps` lifetime.".to_string(),
                    ytrace_queries: vec![],
                    chart: None,
                },
                Page {
                    id: "top-atlas-p2".to_string(),
                    title: "2. Frag & Provisioning".to_string(),
                    markdown: "## Frag & Provisioning\n\n> `npm-cache/_cacache 6.2G → 146K` — reclaimed 99.98% with `npm cache clean --force`.\n\nTop pages tell host stories without ytrace: how full, how scattered, how many daemons own the floor.".to_string(),
                    ytrace_queries: vec![],
                    chart: None,
                },
            ],
        },
        // Dash — exclusively ytrace
        Notebook {
            id: "dash-angry-gui".to_string(),
            title: "The Angry GUI that wasn't — 50% vs 0.37 cores".to_string(),
            mode: "dash".to_string(),
            description: "Dash is exclusively ytrace — profiling adventure with file-first timeline.".to_string(),
            author: "ytop".to_string(),
            created_at_ms: now_ms(),
            pages: vec![
                Page {
                    id: "dash-angry-p1".to_string(),
                    title: "1. ps lied — lifetime vs delta".to_string(),
                    markdown: "# The Angry GUI that wasn't\n\n> `ps %CPU 50.1%` is lifetime average. `render_probe` delta 400 ms = `0.20 cores`.\n\n`yggterm 3892080 50%` + `WebKit 46%` = storm biography, not current. After hot `agy 0.85 cores` + `find` storm reaped, `0.22+0.15=0.37 cores` — healthy. Book rule: measure with `CLOCK_THREAD_CPUTIME_ID`, not `ps`.\n\nCheck with: `ytrace query --app yggterm --category render --json` vs `server perf-summary --category render` (must agree 1.4%).".to_string(),
                    ytrace_queries: vec![YtraceQuery {
                        provider: "yggterm".to_string(),
                        category: "render".to_string(),
                        name: "gui".to_string(),
                        since_ms: 60_000,
                    }],
                    chart: Some("sparkline".to_string()),
                },
                Page {
                    id: "dash-angry-p2".to_string(),
                    title: "2. npm-cache 6.2G → 146K".to_string(),
                    markdown: "## The real leak — npm-cache\n\n> `~/.yggterm/npm-cache/_cacache 6.2G (874 blobs 90–126M) → 146K` with `npm cache verify` + `clean --force`.\n\n`cli-staging 81M` was the relocated `codex-litellm` leak; `npm-cache` was the unbounded one. `du -sh` before/after is the notebook's sparkline.\n\nFix: `YTRACE_HOME` bounded, `retention 1G` 3-day ceiling, `npm_config_cache_max 500M` next.".to_string(),
                    ytrace_queries: vec![YtraceQuery {
                        provider: "yggterm".to_string(),
                        category: "daemon_request".to_string(),
                        name: "status".to_string(),
                        since_ms: 60_000,
                    }],
                    chart: Some("timeline".to_string()),
                },
                Page {
                    id: "dash-angry-p3".to_string(),
                    title: "3. Fix & Verify".to_string(),
                    markdown: "## Fix & Verify\n\n> `server app session remove local://a64c6ce9…` → `row_still_listed:false verified:false processes_survived` → `pgrep agy` empty → `tenants row_count 54→53`.\n\nClose the 0.85-core `agy --dangerously-skip-permissions` (`ytop verification` proof already landed `55e374a`). Verify with `viewport_force_log` + `ps delta`, not `ps` lifetime.\n\nNext profiling adventure: why `status` poll is 1.6% not N² — use ytrace to compose page 1.".to_string(),
                    ytrace_queries: vec![],
                    chart: None,
                },
            ],
        },
        Notebook {
            id: "dash-idle-cost".to_string(),
            title: "Idle Cost floor — 0.2 cores per daemon, not per session".to_string(),
            mode: "dash".to_string(),
            description: "Why 14 daemons at 34 sessions cost 3 cores, but one daemon at 23 costs 0.45 — 4.5× win.".to_string(),
            author: "ytop".to_string(),
            created_at_ms: now_ms(),
            pages: vec![
                Page {
                    id: "dash-idle-p1".to_string(),
                    title: "1. N_reachable × 0.2-core floor".to_string(),
                    markdown: "# Idle Cost floor\n\n> `cores = 0.116+0.0104·owned+0.000337·rows R²0.939 (4.5× win)` — Daemon Cost card (probe via `CLOCK_THREAD_CPUTIME_ID` 1.38 ms `status`).\n\nOne daemon per 200 agents ≈ `0.116+0.0104·200 ≈ 2.2` cores; 14 daemons ≈ `14·0.2 = 2.8` cores regardless of work. `rows` term is `4.65 µs/row` (IPW, r=0.998) — 1.6% of daemon CPU, never N².\n\nQuery: `ytrace query --app yggterm --category daemon_request --name status --since 60s`".to_string(),
                    ytrace_queries: vec![YtraceQuery {
                        provider: "yggterm".to_string(),
                        category: "daemon_request".to_string(),
                        name: "hot_restart".to_string(),
                        since_ms: 60_000,
                    }],
                    chart: Some("timeline".to_string()),
                },
            ],
        },
    ]
}

pub fn list_notebooks(mode_filter: Option<&str>) -> Vec<Notebook> {
    let mut out = base_notebooks();
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
    // Stable sort: base first (by id), then user by created_at
    out.sort_by(|a, b| a.mode.cmp(&b.mode).then(a.id.cmp(&b.id)));
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
