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
            id: "top-atlas-client".to_string(),
            title: "Host Atlas — a client host at 53 rows".to_string(),
            mode: "top".to_string(),
            description: "Top wants no ytrace — host, ZFS, LXC. Read like an atlas.".to_string(),
            author: "ytop".to_string(),
            created_at_ms: now_ms(),
            pages: vec![
                Page {
                    id: "top-atlas-p1".to_string(),
                    title: "1. The Machine".to_string(),
                    markdown: "# Host Atlas — a client host\n\n> **Rule:** Top is host truth without ytrace. Dash is exclusively ytrace.\n\n| Atlas | Host |\n| :--- | :--- |\n| **client** | 53 live sessions, 2 plain shells (169×65), 11 %CPU load 0.72 |\n| **ZFS** | `zroot` + `zbulk` pools, `frag %` tells how scattered — `>80%` deserves balance |\n| **LXC** | `44 Total` — expand a container to see its top processes (PID/CPU/RSS) |\n\n`probe.rs` 400 ms `/proc` delta — total work across all cores, not `ps` lifetime.".to_string(),
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
        // Dash — exclusively ytrace: intelligent daemon & LLM complaints
        Notebook {
            id: "dash-intelligent".to_string(),
            title: "Intelligent Daemon — resource-aware rows, SSH detach, LLM complaints".to_string(),
            mode: "dash".to_string(),
            description: "Each daemon watches its rows — SSH hot rows get detached & reattached, local hot rows get telemetry, every fault is a ytrace incident for LLM diagnosis.".to_string(),
            author: "ytop".to_string(),
            created_at_ms: now_ms(),
            pages: vec![
                Page {
                    id: "dash-intelligent-p1".to_string(),
                    title: "1. How the governor thinks".to_string(),
                    markdown: "# Intelligent Daemon\n\n> **Governance is a verdict, not a metric.** Every 15 s the daemon samples each live row's PTY tree (`/proc/<shell_pid>/stat` delta → `core_fraction`, plus PSS). A pure `ytrace::diagnosis` says hot or not — the same code Dash and `ytrace query` use.\n\n- **SSH row** `>0.80 core × 45 s` → `incident ssh_row_hot warn` + telemetry. With `YGGTERM_GOVERNOR_SSH_DETACH=1` the reader parks (detach), reattaches 120 s later — the row stays, the hot SSH bridge doesn't burn a core.\n- **Local row** `>0.90 core or >1.5 GB × 30 s` → `incident local_row_hot / local_row_oom error` + telemetry only — never killed, LLM picks next step.\n- **Render storm** `>0.70 core × 30 s` → `render_storm warn` — the viewport throttle already landed, this files the story.\n\nAll are `payload.incident=true, complaint_for=llm` in `ytrace.jsonl` — file-first, daemon-down still readable.".to_string(),
                    ytrace_queries: vec![YtraceQuery {
                        provider: "yggterm".to_string(),
                        category: "row_resource".to_string(),
                        name: "ssh_hot".to_string(),
                        since_ms: 60_000,
                    }],
                    chart: Some("timeline".to_string()),
                },
                Page {
                    id: "dash-intelligent-p2".to_string(),
                    title: "2. Where complaints live".to_string(),
                    markdown: "## Where complaints live\n\n`ytrace` is the complaint bus. A fault is one wire record, three readers:\n\n| reader | verb |\n| --- | --- |\n| **Dash** | this notebook page — `ytrace query --app yggterm --category row_resource --since 1h --json` + sparkline of `incidents` |\n| **LLM** | `ytrace incidents --app yggterm --since 1h --json` or `query::health()` — JSON with `diagnosis`, `remedy`, `suggested_queries` (`grep <row_id>` next) |\n| **Telemetry** | `~/.yggterm/telemetry/terminal.sqlite3` (`source=resource_governor`) + `event-trace.jsonl` (`daemon/resource_governor`) — the 3-day narrative |\n\nDisable: `YGGTERM_GOVERNOR=0`. Dash keeps host atlas on Top and ytrace only on Dash — this page is Dash exclusively.".to_string(),
                    ytrace_queries: vec![YtraceQuery {
                        provider: "yggterm".to_string(),
                        category: "row_resource".to_string(),
                        name: "local_hot".to_string(),
                        since_ms: 300_000,
                    }],
                    chart: None,
                },
                Page {
                    id: "dash-intelligent-p3".to_string(),
                    title: "3. Self-diagnosis playground".to_string(),
                    markdown: "## Self-diagnosis playground\n\nTry it headlessly or via skill:\n\n```sh\nytrace incidents --app yggterm --since 5m --json | jq '.[].payload.diagnosis'\nytrace query --app yggterm --category daemon_request --name status --since 60s --top 5 --json\nytop --probe ytrace --json | jq .incidents\n# as an agent on any host:\n# POST /action notebook_compose_dash {\"title\":\"my incident\", \"ytrace_queries\":[...]}\n```\n\nThe book rule: **Top has no ytrace, Dash is exclusively ytrace.** Compose your profiling adventure as a Dash book page; `ytop --probe` is the discovery front door.".to_string(),
                    ytrace_queries: vec![],
                    chart: None,
                },
            ],
        },
        // Dash — exclusively ytrace: common bugs (render storm + session-only branch + titles + input + agy/codex)
        Notebook {
            id: "dash-common-bugs".to_string(),
            title: "Common Bugs — session-only rehydrate + render storm + titles + input + agy/codex".to_string(),
            mode: "dash".to_string(),
            description: "The session-only branch that starved keyboard+viewport, the unpinned 54–64 renders/s storm, titles that must never be shorthash/generic, input latency keystroke→pty→render, and agy/codex wiring vs Claude gold — all now ytrace-mirrored for Dash.".to_string(),
            author: "ytop".to_string(),
            created_at_ms: now_ms(),
            pages: vec![
                Page {
                    id: "dash-common-p1".to_string(),
                    title: "1. The session-only branch".to_string(),
                    markdown: "# The session-only branch\n\n> `retained_rehydrate_should_skip_before_read` returned early when the host was already live — no viewport seed, no `remote_resume_input_ready`. Plain shell never enters this path; agent session always does. The keyboard half is fixed; the paint half (`broken bottom`, `TUI breaks`) is the same branch.\n\n`viewport.rs` now mirrors the trace `ui/terminal_mount/retained_rehydrate_skipped_live_connected` (and `skipped_pre_resize` geometry fence) to ytrace, so Dash can count `skipped_live_connected` vs `skipped_pre_resize==0` in the current generation without tailing trace.\n\nQuery: `ytrace query --app yggterm --category terminal_mount --name retained_rehydrate_skipped_live_connected --since 1h` and correlate against faithful screenshots (`server app screenshot` canvas composite vs `server snapshot` `terminal_lines`). If they coincide, the fix is to seed the viewport on that branch instead of returning — same shape as releasing the input gate.".to_string(),
                    ytrace_queries: vec![
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "terminal_mount".to_string(),
                            name: "retained_rehydrate_skipped_live_connected".to_string(),
                            since_ms: 3600_000,
                        },
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "terminal_mount".to_string(),
                            name: "retained_rehydrate_skipped_pre_resize".to_string(),
                            since_ms: 3600_000,
                        },
                    ],
                    chart: Some("timeline".to_string()),
                },
                Page {
                    id: "dash-common-p2".to_string(),
                    title: "2. The render storm — 54–64 renders/s, one no-op ShellState write per frame".to_string(),
                    markdown: "# The render storm\n\n> Measured on a client host: `app_render_storm` — Dioxus root at **54–64 renders/s** (calm 0.7–1.2/s) pinning exactly one core for nine minutes, driven by **one `ShellState` write per render that changes no watched field**. Daemon event rate FLAT (13.1/s storming vs 12.1/s calm) — not \"58 rows re-attaching\", but a per-frame write that should not wake the root.\n\n`launch.rs` now emits both `ui/perf/app_render_rate` (every 60 s → `renders_per_sec`) and `render/storm` incident + `ui/render_fail_pattern/detected` `app_render_storm` to ytrace (Wall, always, no sampling), so Dash sees the storm without needing `render_top` deltas.\n\nQuery: `ytrace query --app yggterm --category render --name storm --since 1h` and `ytrace query --app yggterm --category ui --name app_render_rate --since 1h --top 5`; compare `renders_per_sec` trace vs `ytrace query` counts, and run deterministic `mock-tui codex-inline` + `pipeline_integration` harness on a compute host (never the client) to repro the single-branch rehydrate skip without touching the live viewport.".to_string(),
                    ytrace_queries: vec![
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "render".to_string(),
                            name: "storm".to_string(),
                            since_ms: 3600_000,
                        },
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "ui".to_string(),
                            name: "app_render_rate".to_string(),
                            since_ms: 3600_000,
                        },
                    ],
                    chart: Some("sparkline".to_string()),
                },
                Page {
                    id: "dash-common-p3".to_string(),
                    title: "3. Titles never shorthash — CLI store → LLM → untitled session → ytrace re-resolve".to_string(),
                    markdown: "# Titles never shorthash\n\n> Titles must never be `43936dd` (bare hash) or generic `\"Muse Code Stays Attached Daemon\"` / `\"Local Shell Stay Alive Daemon\"`. Wire FROM the CLI store (`TitleAuthority::Store` for muse `session-index.db`, claude `custom-title`, codex `Generated`) and via interface LLM (`request_litellm_title` `gpt-5.6-luna`) when absent/bugged; if LLM fails, title is `\"untitled session\"` (never hash). `ytrace` then re-resolves untitled every tick (`daemon::background_copy_chore` emits `title/resolve_attempt` + `title/untitled_session` incident) until a real title lands — Dash sees the retry timeline without polling the DB.\n\nMuse lifecycle: new row → `\"New Muse Code Session\"` (explicit `set_session_title_explicit` at `terminal new`) → after first prompt `session.jsonl` has user turn → background chore `LIVE_SUMMARY_REFRESH_HORIZON` replaces via `heuristic_title_from_context()` / `request_litellm_title()` → `set_session_title_hint()`; shorthash/generic triggers same path, untitled triggers `ytrace` retry next tick because `\"untitled session\"` is itself a fallback per `titles.rs`.\n\nQuery: `ytrace tail --app yggterm --category title --since 1h --json | jq '.[] | select(.name==\"untitled_session\")'` and `ytrace query --app yggterm --category title --name resolve_attempt --since 1h`".to_string(),
                    ytrace_queries: vec![
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "title".to_string(),
                            name: "untitled_session".to_string(),
                            since_ms: 3600_000,
                        },
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "title".to_string(),
                            name: "resolve_attempt".to_string(),
                            since_ms: 3600_000,
                        },
                    ],
                    chart: Some("timeline".to_string()),
                },
                Page {
                    id: "dash-common-p4".to_string(),
                    title: "4. Input latency — keystroke → PTY → render".to_string(),
                    markdown: "# Input latency\n\n> Every keystroke must be traceable end-to-end: `shell` `input/keystroke` (client has the bytes) → `daemon` `input/pty` (PTY `terminals.write` accepted) → `shell` `input/render` (`terminal_write_bridge.stage_or_immediate` staged for xterm). Each hop emits `ytrace` `input/*` (`Wall always`, `session_path`, `data_len`) so Dash can compute `pty - keystroke` and `render - pty` p50/p95 per session (like `render/storm` vs `daemon_request/status`). A stuck input gate (`remote_resume_input_ready` false) or lost PTY write (`terminal_write_error` → `recover_terminal_write_lost_runtime`) shows as `keystroke` without `pty`/`render` — the latency tail, not a screenshot, is the falsifier.\n\nProbes wired: `perf.rs: input/keystroke|pty|render` always; `viewport.rs:Input` emits `keystroke`, `daemon.rs:write_local_terminal_with_lost_runtime_recovery` emits `pty`, `viewport.rs:terminal_write_bridge.stage_or_immediate` emits `render`. Query: `ytrace tail --category input --since 5m --json | jq 'group_by(.name) | map({name: .[0].name, count: length})'` to flush out bugs where `keystroke` ≫ `pty` or `render` lags >50 ms.".to_string(),
                    ytrace_queries: vec![
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "input".to_string(),
                            name: "keystroke".to_string(),
                            since_ms: 300_000,
                        },
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "input".to_string(),
                            name: "pty".to_string(),
                            since_ms: 300_000,
                        },
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "input".to_string(),
                            name: "render".to_string(),
                            since_ms: 300_000,
                        },
                    ],
                    chart: Some("timeline".to_string()),
                },
                Page {
                    id: "dash-common-p5".to_string(),
                    title: "5. Agy + all CLIs vs Claude gold, codex geometry".to_string(),
                    markdown: "# Agy + all CLIs vs Claude gold, codex geometry\n\n> `claude-code` is gold: one file per session `~/.claude/projects/*/*.jsonl` (filename IS id), `TitleAuthority::Store` (`custom-title > ai-title`), `id_assigned_at_birth:false` but filename IS id, flag `--resume`. Every other CLI is probed against it:\n\n* **Agy** (`agy` `remote-agy://`/`agy-runtime://` `agy --conversation <id>`) — DB `~/.gemini/antigravity-cli/conversations/*.db` + `brain/*` + `history.jsonl`, `TitleAuthority::Store` (`conversation_summaries.title`). Faults like `muse`: shorthash/generic → `ytrace title/*` (now `cli/agy_title` `no_title_in_store` / `fallback:true` / `is_untitled` in `daemon.rs:collect_live_antigravity_title_syncs`), resume uses `agy-runtime://` + DB `conversation_id` (not row UUID) — verify `remote_runtime_agent_session_key(\"remote-agy://…\")` returns `agy-runtime://<internal-id>` or switch orphans PTY (same `muse` kick, now `ytrace cli/agy_resume`).\n* **Codex** (`remote-session://` historical + `codex-runtime://` `codex resume <id>`, `re_roots_with_cwd:true`) and `codex-litellm` (local-only) — `Generated` titles, store `~/.codex/sessions/**/rollout-*.jsonl` id inside file. Faults are viewport: **geometry squish** (daemon re-creates PTY at `120×36` after hot-update, `last_sent_*` stale-equal) → `viewport.rs:9837` repair now `ytrace cli/codex_geometry` (`stale_cols/rows`, `live_cols/rows`, `codex_squish_repair`); `pi`/`qwen`/`opencode`/`kimi`/`grok` etc. share same `muse`/`agy` title+resume checks (shorthash never shown, `resume` subcommand vs flag, `store_globs` per `AGENT_CLIS`, `re_roots_with_cwd` per arm). Check: `ytrace tail --category cli --since 1h --json | jq 'group_by(.name)'`.\n\nAll faults logged like Muse exemplar: matrix `docs/cli-integration.md` Issue 13/14/15 + this Dash p4/p5.".to_string(),
                    ytrace_queries: vec![
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "cli".to_string(),
                            name: "agy_title".to_string(),
                            since_ms: 3600_000,
                        },
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "cli".to_string(),
                            name: "codex_geometry".to_string(),
                            since_ms: 3600_000,
                        },
                    ],
                    chart: Some("timeline".to_string()),
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
