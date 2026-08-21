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
                    live: None,
                },
                Page {
                    id: "top-atlas-p2".to_string(),
                    title: "2. Frag & Provisioning".to_string(),
                    markdown: "## Frag & Provisioning\n\n> `npm-cache/_cacache 6.2G → 146K` — reclaimed 99.98% with `npm cache clean --force`.\n\nTop pages tell host stories without ytrace: how full, how scattered, how many daemons own the floor.".to_string(),
                    ytrace_queries: vec![],
                    chart: None,
                    live: None,
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
                    live: None,
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
                    live: None,
                },
                Page {
                    id: "dash-angry-p3".to_string(),
                    title: "3. Fix & Verify".to_string(),
                    markdown: "## Fix & Verify\n\n> `server app session remove local://a64c6ce9…` → `row_still_listed:false verified:false processes_survived` → `pgrep agy` empty → `tenants row_count 54→53`.\n\nClose the 0.85-core `agy --dangerously-skip-permissions` (`ytop verification` proof already landed `55e374a`). Verify with `viewport_force_log` + `ps delta`, not `ps` lifetime.\n\nNext profiling adventure: why `status` poll is 1.6% not N² — use ytrace to compose page 1.".to_string(),
                    ytrace_queries: vec![],
                    chart: None,
                    live: None,
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
                    live: None,
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
                    live: None,
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
                    live: None,
                },
                Page {
                    id: "dash-intelligent-p3".to_string(),
                    title: "3. Self-diagnosis playground".to_string(),
                    markdown: "## Self-diagnosis playground\n\nTry it headlessly or via skill:\n\n```sh\nytrace incidents --app yggterm --since 5m --json | jq '.[].payload.diagnosis'\nytrace query --app yggterm --category daemon_request --name status --since 60s --top 5 --json\nytop --probe ytrace --json | jq .incidents\n# as an agent on any host:\n# POST /action notebook_compose_dash {\"title\":\"my incident\", \"ytrace_queries\":[...]}\n```\n\nThe book rule: **Top has no ytrace, Dash is exclusively ytrace.** Compose your profiling adventure as a Dash book page; `ytop --probe` is the discovery front door.".to_string(),
                    ytrace_queries: vec![],
                    chart: None,
                    live: None,
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
                    live: None,
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
                    live: None,
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
                    live: None,
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
                    live: None,
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
                    live: None,
                },
            ],
        },
        // Dash — exclusively ytrace: the supervision system itself, explained then shown live.
        Notebook {
            id: "dash-sysinternals".to_string(),
            title: "yggterm SysInternals".to_string(),
            mode: "dash".to_string(),
            description: "The supervision system as a book: the two arming planes, the seat census, when each watcher last fired, the ytrace graphs — and four dream-mode walkthroughs of what the machinery is FOR, each with the live numbers beside it.".to_string(),
            author: "ytop".to_string(),
            created_at_ms: now_ms(),
            pages: vec![
                Page {
                    id: "dash-sysint-p1".to_string(),
                    title: "1. The armings — two planes, and a row can be on one".to_string(),
                    markdown: "# Two planes, and a row can be on exactly one\n\nThere are two watchdogs over this fleet, and they are **separate stores** rather than two views of\none. A row can be on either, both, or neither, and the four cases behave completely differently\nwhen something goes wrong.\n\n**The booter is a dumb timer, and that is its virtue.** A session SUBSCRIBES to it, and a detached\nwatcher — one that outlives the session — types `continue` when it goes quiet. It has to be\noutside: **a stalled session cannot boot itself**, because the stall *is* the turn ending early, so\nanything scheduled inside the turn is dead in exactly the case that matters.\n\n**The monitor is the judgement.** A timer can ask \"has this been quiet too long\"; it cannot ask\n*why*, and the why decides the action:\n\n* mid-turn and **thinking** — leave it alone\n* mid-turn and **abandoned** — wake it, and from the outside it looks identical to thinking\n* **out of context** — it cannot be woken at all; it has to be relayed to a successor\n* **taken back by a person** — nothing may touch it\n\nThe discriminator between the first two is CPU. A thinking agent burns some; an abandoned one does\nnot. Without that, both collapse into \"do not touch\" — and the abandoned case is precisely the one\nthat needs touching.\n\n## The failure this page exists for\n\nSubscribing to one plane is not subscribing to the other, and one chip cannot say which:\n\n| state | what actually happens when it stalls |\n| :--- | :--- |\n| **both** | it is woken; if the wake does not take, somebody hears about it |\n| **booter only** | it is woken — and if the wake does not take, **the escalation rings into an empty room** |\n| **monitor only** | somebody would hear — but nothing wakes it first, so nothing ever escalates |\n| **neither** | it sits |\n\nBooter-only is the common one, because subscribing is one verb and attaching is another, and a lane\nin a hurry does the first and forgets the second. It is invisible from any pane that renders\nsupervision as a single word.\n\n## Reading the block below\n\n`gone ×N` is **not** a gap — the booter is counting a retired row down and will drop it by itself,\nbecause a corpse must not be booted forever. `lapsed` has already expired on its own. A row on\n**never-arm** is not unsupervised either: that file asserts *a human types at this address*, and the\nbooter's only remedy is to type, so arming one would type into a person.\n\n⛔ **Do not bulk-arm what this page shows.** No probe separates \"nobody ever attached it\" from \"it\nstood itself down deliberately\", and guessing wrong types into somebody. Decide per row.\n\n## Why `ui/block` is the trace on this page\n\nSupervision is not free. Every classification a watchdog makes probes a row, and a probe crosses the\nUI thread of the machine a person is typing on. A rising `ui/block` density with no user-facing\ncause is worth reading against how many rows are armed — the cost of watching is paid in the\nkeyboard latency of whoever is watching.".to_string(),
                    ytrace_queries: vec![
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "ui".to_string(),
                            name: "block".to_string(),
                            since_ms: 3600000,
                        },
                    ],
                    chart: Some("timeline".to_string()),
                    live: Some("armings".to_string()),
                },
                Page {
                    id: "dash-sysint-p2".to_string(),
                    title: "2. The rows — the seat census".to_string(),
                    markdown: "# The seat census — a seat is an address, not a process\n\nA row is a **seat** in the sidebar; the agent process is a **tenant** of it. The two come apart\nconstantly, and most fleet mistakes live in that gap:\n\n* a seat with **no process** is cold — often perfectly correct, because a lane that finished its\n  work stands down and waits to be folded\n* a **process with no seat** is an orphan, still burning CPU somewhere nobody will ever read\n* **two processes on one seat** is a twin, usually a resume that landed twice; it doubles the cost\n  of the same conversation and neither half knows about the other\n* a **child loop** left behind by a test harness spins forever at the machine's expense\n\n## Context size is not a curiosity, it is the price of the next turn\n\nResuming a cold seat means re-reading its entire context before it can produce a sentence. Past\nroughly 10 MB that read costs more than the work it enables, which is why the right verb for a heavy\ncold seat is *harvest*, not *continue* — page 6.\n\n## What the columns mean\n\n`LIVE ×N` counts **processes, not health** — `×2` is a twin, not twice the work. `cpu` is a sampled\ndelta rather than a `ps` lifetime average, so a seat that burned a core an hour ago and has slept\nsince reads calm here and busy in `ps`; the delta is the live view and `ps` is a biography.\n`last moved` is the transcript's mtime, which a working row touches every few seconds.\n\n`supervision` is the **collapsed, single-plane chip** that the fleet pane shows. Page 1 is the honest\nversion of that same column — the two are on the same shelf deliberately, so the difference between\n\"⚡ Armed\" and *armed on which plane* is one page turn away.\n\n## Why `sidebar/merge_rows` is the trace on this page\n\nIt is what drawing this census costs. It runs on every snapshot, so its p95 is an early warning: a\nmerge that has drifted from single-digit milliseconds into tens means the row plane itself has become\nthe jank, and the seat count is the first thing to look at.".to_string(),
                    ytrace_queries: vec![
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "sidebar".to_string(),
                            name: "merge_rows".to_string(),
                            since_ms: 3600000,
                        },
                    ],
                    chart: Some("sparkline".to_string()),
                    live: Some("census".to_string()),
                },
                Page {
                    id: "dash-sysint-p3".to_string(),
                    title: "3. When each last fired — and silence has no symptom".to_string(),
                    markdown: "# When each last fired\n\nEvery watcher here fails the same way: **it stops, and stopping looks exactly like a calm fleet.**\nNo error, no alert, no missing pane — just nothing, which is also what a healthy quiet hour produces.\nThe only instrument that separates them is a clock, which is why this page is a table of timestamps\nrather than a table of statuses.\n\n## The four cadences\n\n| watcher | the question it asks | cadence |\n| :--- | :--- | :--- |\n| **booter** | has this row been quiet too long | a pass every few minutes, per subscriber |\n| **monitor** | *why* is it quiet, and who should hear | the same order, plus a deliberately long escalation window |\n| **roll watcher** | has `main` moved past the daemon that is running | hourly |\n| **fold sweep** | which rows are finished, stalled or dead | rides the roll watcher's tick |\n\nThe monitor's escalation window is long **on purpose**. A finished relay row idles by design;\nescalating it after four minutes produced three false alarms inside one minute, so the window is\nfifteen. A watchdog that cries at every rest is uninstalled within a day, which is a worse outcome\nthan a slow one.\n\n## ⛔ Alive is not audible\n\nThe booter reports **two** instants: when its loop last ticked, and when it last managed to write to\nits log. A process ticking into a file nobody can read supervises nobody, and from outside it is\nindistinguishable from a healthy one — same process, same CPU, same uptime. The pair is the only\nthing that catches it.\n\n**The other three have no heartbeat file.** Their row below is the mtime of a log, which says the log\nmoved — not that the loop is well. That is weaker evidence and it is labelled as such rather than\nrounded up to a green tick.\n\n## Why `heartbeat/panic` is the trace on this page\n\nIt is the daemon's own host-health complaint: sustained memory, sustained cores, UI-block density,\nruntime tmpfs growth. It belongs here because **a watcher that has stopped and a host that has fallen\nover produce the same silence**, and this is the series that tells the two apart.".to_string(),
                    ytrace_queries: vec![
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "heartbeat".to_string(),
                            name: "panic".to_string(),
                            since_ms: 21600000,
                        },
                    ],
                    chart: Some("timeline".to_string()),
                    live: Some("watchers".to_string()),
                },
                Page {
                    id: "dash-sysint-p4".to_string(),
                    title: "4. The graphs — what the fleet costs, over time".to_string(),
                    markdown: "# The graphs\n\nDash is exclusively ytrace, so every series here is a file-first read of `ytrace.jsonl`. Nothing is\nsampled from `ps` and nothing is asked of the daemon — the trace survives the daemon being down,\nwhich is exactly the moment a person most wants to know what happened.\n\n## What to look at, and in what order\n\n1. **`ui/block` over time.** A block is the UI thread stalling long enough to be felt. **Density\n   matters more than any single spike** — a rising tail precedes a freeze. Blocks caused by an agent\n   probing rows are indistinguishable from blocks caused by the app itself, so read the shape against\n   page 1's arming count before blaming the app.\n2. **`daemon_request` latency.** The request path everything else rides on. `snapshot` is polled\n   continuously, so its p95 is the fleet's floor: when it moves, everything moves.\n3. **`render` cost.** Split by clock. `cpu` rows are CPU time, `wall` rows are elapsed — a render\n   that *waited* is cheap on one and expensive on the other, and mixing them is how a busy GUI gets\n   called idle.\n4. **`heartbeat/panic`.** Host level, not app level. When this is firing, everything above it is a\n   symptom rather than a cause.\n\n## How to read a sparkline honestly\n\nEach is normalised against **its own** peak, and the peak is printed beside it. Two lines of equal\nheight are not equal magnitudes. An empty bucket draws as the floor and means *nothing happened* —\nnever *it got faster* — so read the sample count before the shape.\n\n## ⛔ What is missing here, and it is a real gap rather than a design\n\n**The two watchdogs emit no ytrace at all.** Their wakes, their escalations and their rate-limit\nholds exist only as lines in a log, which is why page 5 is parsed prose instead of a series. Until\nthey emit spans, a wake cannot be correlated against the `ui/block` it caused — and that correlation\nis the entire reason for putting supervision and profiling on one Dash.".to_string(),
                    ytrace_queries: vec![
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "ui".to_string(),
                            name: "block".to_string(),
                            since_ms: 21600000,
                        },
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "daemon_request".to_string(),
                            name: "snapshot".to_string(),
                            since_ms: 7200000,
                        },
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "render".to_string(),
                            name: "gui".to_string(),
                            since_ms: 21600000,
                        },
                    ],
                    chart: Some("sparkline".to_string()),
                    live: Some("graphs".to_string()),
                },
                Page {
                    id: "dash-sysint-p5".to_string(),
                    title: "5. Dream — a lane stalls, and something outside it wakes it".to_string(),
                    markdown: "# Dream 1 — a lane stalls, and something outside it types `continue`\n\n**The story.** A lane on seat `4.3` is halfway through a build. Its turn ends early — no error, no\ncrash, the model simply stopped. Nothing inside the session can help, because everything inside the\nsession ended when the turn did. The detached watcher notices the transcript has not moved for longer\nthan its window, classifies the row, and writes one `continue` **to the PTY, not to the composer** —\nthe composer belongs to whoever is typing, and a write there races the agent's own input. Sessions\nhave refused a composer submit for thirty seconds each and taken a PTY write instantly.\n\nIf the wake takes, the lane resumes and nobody was ever involved. If it does not, the second plane\nearns its keep: the monitor escalates to the campaign's orchestrator, which can probe, read and\ndecide — or to a person when there is no orchestrator to carry it.\n\n**What makes it work:** the watcher is outside the thing it watches.\n**What makes it fail:** the lane was armed on the booter and attached to nothing, so the escalation\nhad nowhere to go — page 1.\n\n## The verdicts, and why each has its own remedy\n\n| verdict | remedy |\n| :--- | :--- |\n| `WORKING` | nothing |\n| `IDLE` | wake once; escalate if it stays idle past the window |\n| `STUCK` | mid-turn and **not** burning CPU — abandoned, so wake it |\n| `RATE_LIMITED` | ⛔ **do not wake.** The account cannot spend; the session is fine. A boot here burns a refused turn and teaches nothing. One sighting holds the **whole fleet**, because a rate limit is account-wide while detection can only ever be per-row. |\n| `NO_TRANSCRIPT` | ⛔ not \"idle\" — nothing could be read, and \"I could not look\" is not a measurement |\n| `SKIP:draft-race` | the row had unsent text; typing would have raced a person mid-sentence |\n\n**An escalation is a fact, not a failure.** A row that escalated is one a person or an orchestrator\nnow owns, and the booter deliberately stops booting it — two watchdogs and a human all typing into\none row is worse than none of them.\n\n## Why `ui/block` is the trace on this page\n\nA wake is a write into a live terminal on the machine somebody is using. The blocks around a burst of\nwakes are the cost of the safety net, and they are the honest argument for why the watchers should\nnot be armed over rows nobody is supervising.".to_string(),
                    ytrace_queries: vec![
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "ui".to_string(),
                            name: "block".to_string(),
                            since_ms: 21600000,
                        },
                    ],
                    chart: Some("timeline".to_string()),
                    live: Some("wakes".to_string()),
                },
                Page {
                    id: "dash-sysint-p6".to_string(),
                    title: "6. Dream — a lane goes cold, and is harvested rather than prompted".to_string(),
                    markdown: "# Dream 2 — a lane goes cold, and is harvested rather than prompted\n\n**The story.** Seat `4.7` has been quiet for hours. Its process is gone. Its transcript is 34 MB. The\ntempting move is to resume it and ask what it was doing.\n\n⛔ **That question is the most expensive thing on this page.** Resuming a cold seat re-reads its\nentire context before it can produce a single sentence, and what comes back is a self-report that\ncannot be verified anyway. **The asking IS the expense** — what you are buying is the wake, not the\nanswer.\n\n**The cheaper path, in order, and it is cheapest-first on purpose:**\n\n1. **mtime** — how cold is it actually\n2. **size** — what would a wake cost\n3. **what it was TOLD** — the instructions in the transcript are the highest signal per byte in the\n   file\n4. **its last prose turn** — a working lane's own status report, already written down\n5. **what it DID** — the files it wrote, the commits it made\n\nA transcript says what a session *believed*; a commit says what it *did*. **The artefact wins.** Two\nlanes have been told apart, their roles established and one safely retired, from six extracted lines\nout of megabytes of transcript.\n\nThen the seat is folded (page 8) and a successor is claimed with a brief distilled from those\nartefacts — so the successor needs no history at all, which is the whole point of harvesting rather\nthan resuming.\n\n## ⛔ And swallowing it whole is the other failure\n\nReading the entire transcript instead of asking it moves the same cost into *your* context and\ncarries it for the rest of the session, when the signal you wanted was in the last one per cent.\nBoth mistakes look like diligence. **Extract, do not ingest.**\n\n## The chips\n\n`⚠️` above 10 MB, `🚨` above 30 MB. They are not health warnings about the lane — the lane may have\ndone excellent work. They are the price of the decision you are about to make about it.".to_string(),
                    ytrace_queries: vec![
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "daemon_request".to_string(),
                            name: "status".to_string(),
                            since_ms: 3600000,
                        },
                    ],
                    chart: Some("table".to_string()),
                    live: Some("cold".to_string()),
                },
                Page {
                    id: "dash-sysint-p7".to_string(),
                    title: "7. Dream — a roll lands, and the client restarts under it".to_string(),
                    markdown: "# Dream 3 — a roll lands, and the client is restarted under it\n\n**The story.** `main` moves ahead of the daemon that is actually running. Nobody notices, because\n**a stale daemon works perfectly** — it simply works like last week. Rows keep being served by a\nbinary whose bugs are fixed upstream and whose fixes are not present, and every report filed against\nit is a report about a version nobody is developing any more.\n\nThe roll watcher compares the two hashes on its own cadence, builds, deploys to every host, and\nrecords what landed. The restart is the part that needs care, because it lands on the machine a\nperson is typing at — so it is a scheduled event with a ledger, not a surprise.\n\n## Why a ledger, rather than \"just check the version\"\n\nA version string says what a binary *claims*. The ledger says **when, which lane, which hosts, which\nbuild, which version** — so a bug report can be pinned to the build that was live when it was filed.\nWith several hosts each holding several checkouts, an unpushed commit is not \"not yet shared\", it is\na **divergence somebody reconciles by hand later**, usually without knowing which side is newer.\n\n⛔ **A roll must never type into a row a human attends.** The never-arm list is consulted before\nanything is restarted, and a graceful handover types nothing at all — it lets the sessions land on\nthe new binary at their own next turn.\n\n## Why `daemon_request/hot_restart` is the trace on this page\n\nIt is the restart itself, timed. It is measured in **seconds, not milliseconds**, and that number is\nthe fleet's whole tolerance for rolling: while it is small the roll is invisible, and when it grows\nthe roll stops being maintenance and becomes an interruption somebody will start avoiding.".to_string(),
                    ytrace_queries: vec![
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "daemon_request".to_string(),
                            name: "hot_restart".to_string(),
                            since_ms: 86400000,
                        },
                    ],
                    chart: Some("timeline".to_string()),
                    live: Some("rolls".to_string()),
                },
                Page {
                    id: "dash-sysint-p8".to_string(),
                    title: "8. Dream — a row is folded, and its worktree with it".to_string(),
                    markdown: "# Dream 4 — a row is folded, and its worktree with it\n\n**The story.** A lane finishes, announces that it is done, and stands down with nobody coming after\nit. Its seat stays in the sidebar. Its process stays resident. The booter goes on arming a corpse.\nNothing anywhere says so — and within an hour the sidebar has refilled with quiet corpses that all\nlook like working lanes.\n\n**Retiring a row is four planes, not one:**\n\n1. the **row** is delisted\n2. the **monitor**'s subscribers are moved off it\n3. the **booter** is disarmed for it\n4. the **agent process** is reaped\n\nThe fourth is not defensive programming. `session remove` reports the **request**, not the effect,\nand routinely delists a row whose agent keeps running — so a fold that skips step four produces\nexactly the orphan it was meant to prevent.\n\nAll four steps existed for a long time in exactly one place: as a side effect of a *successor*\nclaiming a seat. So the fleet had a `replace` and no `fold`, and a lane that finished with nobody\ncoming after it had no path to being retired at all.\n\n## ⛔ A stall is not a fold\n\nA lane that has merely paused, with work still assigned, needs **one `continue`** — folding it throws\naway a lane that was fine. So STALLED is its own verdict with its own remedy, and it is acted on once\nper stall rather than once per sweep.\n\nAnd **a lane that simply stops, announcing nothing, is the common case.** An early version of this\nsweep required an announcement before it would call anything finished, and therefore classified every\nsilent corpse as WORKING forever.\n\n## The worktree half of the same sweep\n\nA folded lane usually leaves a git worktree behind. It is removed only when it is genuinely spent:\n⛔ **unpushed commits, or a live process standing in it, means KEEP** — whatever the row's state.\n**A fold may never be the thing that loses work.**\n\n## Dry by default\n\nFolding kills somebody's agent. It may never be the accidental outcome of a mistyped flag, so the\nsweep classifies and changes nothing until it is told a second time.".to_string(),
                    ytrace_queries: vec![
                        YtraceQuery {
                            provider: "yggterm".to_string(),
                            category: "sidebar".to_string(),
                            name: "merge_rows".to_string(),
                            since_ms: 21600000,
                        },
                    ],
                    chart: Some("table".to_string()),
                    live: Some("folds".to_string()),
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
        let mut seen = std::collections::BTreeSet::new();
        let mut pages = std::collections::BTreeSet::new();
        for nb in base_notebooks() {
            assert!(seen.insert(nb.id.clone()), "duplicate notebook id `{}`", nb.id);
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
    }
}
