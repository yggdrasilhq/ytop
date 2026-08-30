# Spec: `ytop` — Modern Fleet Infrastructure & Agent Cockpit

**Status:** ACTIVE (original 2026-08-15; notebook-first UX revision 2026-08-26)
**Target Repositories:** `ytop` (`~/gh/ytop`), `yggterm` (`~/gh/yggterm`), `libyggterm` (`~/gh/libyggterm`)

`DESIGN.md` is the canonical visual contract. `docs/spec-notebook-runtime.md`
and `docs/spec-observability-graphics.md` own live runtime and plot details.
Where older examples below disagree, those documents and the 2026-08-26
revision win.

---

## 1. Vision & Purpose

`ytop` replaces unstyled, terminal-dump telemetry with a **modern, rich, colorful desktop-class monitoring and operations console** inside Yggterm's `libyggterm` document-surface architecture.

It provides a unified, real-time control plane across two complementary operational modes:
1. **`Top` (Non-ytrace / Uninstrumented Hosts)**: Anything **without** a `ytrace` probe — raw kernel, third-party daemons, bare-metal host debugging, foreign app `strace`/`perf` without the `ytrace` SDK. This is the htop/ZFS/LXC/top fallback for hosts and subsystems that have not been instrumented. It ships no `ytrace` dependency by construction.
2. **`Dash` (Full-Trace Cockpit — NO LIMITATIONS)**: The canonical debugging surface for the fleet. **Dash has no cap** — it includes *everything Top has* **plus** the full `ytrace` bus: `host/cpu_delta`, `host/zfs` (`zpool iostat`/`arcstat`/`zil_commit`), `host/ebpf` (`sched_switch`/`io_uring`/`zfs_delay`), `daemon_request` hot paths, `render`/`xterm` storms, `attach`/`session`/`usability`, `ychrome`/`web` surfaces, and any app that links `ytrace` (`yggterm`, `ychrome`, `yedit`, `yggtopo`, `paper`, `cellulose`). Flamegraphs, cross-layer correlation (`web/policy × render × zfs_delay` in one query), and analytics live here. When a trace exists, Dash is where it is read.

```
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│  Y T O P   ·   [ ⚡ TOP ]   [ 📊 DASH ]                             [ 🔍 Filter ] [ 🔄 2s ] │
├──────────────────────────┬──────────────────────────────────────────────────────────────────┤
│ MACHINES & FLEET         │  ⚡ TOP VIEW: alpha (Local Host)                              │
│                          │                                                                  │
│ ● alpha (local)       │  ┌─ SYSTEM METRICS ────────────────────────────────────────────┐ │
│   37% CPU · 24.1/62.7 GB │  │ CPU: [████████████░░░░░░░░░░░░] 37.2% · 32 Cores · Load 9.78 │ │
│                          │  │ RAM: [████████████████░░░░░░░░] 320.9 / 503.6 GB (63.7%)    │ │
│ ● beta (ssh beta)          │  │ SWP: [░░░░░░░░░░░░░░░░░░░░░░░░] 0.0 MB / 0.0 MB              │ │
│   12% CPU · 8.4/32.0 GB  │  └─────────────────────────────────────────────────────────────┘ │
│                          │                                                                  │
│ ● delta (ssh delta)        │  ┌─ ZFS STORAGE & IOSTAT (Main Storage Pool) ──────────────────┐ │
│   5% CPU · 4.2/16.0 GB   │  │ zpool: rpool [ONLINE] · 1.8 TB / 3.6 TB Allocated (50%)     │ │
│                          │  │ Read:  1.2 MB/s (142 IOPS)  ·  Write: 8.4 MB/s (820 IOPS)   │ │
│ ● gamma (ssh gamma)        │  └─────────────────────────────────────────────────────────────┘ │
│   45% CPU · 94.2/128 GB  │                                                                  │
│                          │  ┌─ LXC CONTAINERS (with expandable process consumption) ──────┐ │
│ [ + Add SSH Machine ]    │  │ ▼ 📦 ct-alpha [RUNNING]  ·  24.5% CPU  ·  1.8 GB RAM     │ │
│                          │  │   ├─ PID 1235385  agy            24.7% CPU   336 MB RSS     │ │
│                          │  │   ├─ PID 1109226  claude          2.5% CPU   612 MB RSS     │ │
│                          │  │   └─ PID 3970214  claude          2.5% CPU   564 MB RSS     │ │
│                          │  │ ▶ 📦 ct-builder  [RUNNING]  ·   0.2% CPU  ·  256 MB RAM     │ │
│                          │  │ ▶ 📦 ct-postgres [RUNNING]  ·   1.1% CPU  ·  4.2 GB RAM     │ │
│                          │  └─────────────────────────────────────────────────────────────┘ │
└──────────────────────────┴──────────────────────────────────────────────────────────────────┘
```

---

## 2. Core UI Philosophy & Aesthetics (DESIGN.md Alignment)

1. **One shared document engine**:
   - Notebook prose and scientific figures use `emd-renderer`, the shared
     libyggterm extended-markdown engine. Ytop does not carry private HTML,
     graph, or markdown rendering code.
   - Native shell widgets provide rail navigation, actions, forms, loading
     state, and structured live rows around the document.
   - Tables are appropriate for exact comparisons. ASCII gauges and raw trace
     dumps are transitional, not the final visualization language.
2. **Color-Coded Status & Health Tokens**:
   - **Emerald / Green (`durable`)**: Healthy processes, online ZFS pools, active normal agents.
   - **Sky Blue (`transient`)**: Running LXC containers, temporary tasks.
   - **Amber (`warning`)**: Context size bloat (>10MB), high CPU usage (>80%), degraded storage, parked agents.
   - **Rose (`danger`)**: Critical context size (>30MB), twin duplicate processes, spinning leaked subshell loops, dead processes.
   - **Indigo / Violet (`supervision`)**: Orchestrator seats, quota holds, rate limit states.
3. **Fluid Responsiveness**:
   - Multi-column card layout on wide desktop viewports; cleanly collapsing to single-column on narrower panes or sidebar rails.

---

## 2.5. Every operational surface is a notebook

There is no nameless dashboard beside the notebook product.

- Top opens the live `System Top` notebook. Its rail contains only notebook
  titles; a document-bar switcher inherits logical hosts from the launching
  Yggterm without collapsing guests that share one physical kernel.
- Dash also contains only notebooks and opens `Yggterm SysInternals`. Fleet totals,
  monitor/booter health, row costs, histories, problems, and overrides are
  pages in that book—not a second control partition above it.
- Shelf rows are flat, title-only entries. No book icons, folder glyphs, page
  children, page counts, descriptions, status dots, Compose button, or manual
  Refresh button.
- Base books ship in this repository. Agents compose additional books
  programmatically in the Ytop data directory.
- Live evidence refreshes independently of prose and navigation. Pane reads
  return cached or collecting state immediately and never hold the global state
  lock while walking trace files or notebook storage.

## 3. Top-Level Mode Architecture

The application titlebar hosts the primary mode toggle:
- `[ Top ]`: fast system reading—fleet selection, raw host/ZFS/LXC/process
  evidence, interval CPU, and explicit operational actions. It works without a
  ytrace emitter.
- `[ Dash ]`: the ytrace notebook shelf—cross-layer histories, flamegraphs,
  topology, app/browser/service correlation, alerts, and agent analysis.

> **Invariant (2026-08-23):** Dash is a strict superset of Top. Anything Top can show, Dash can show with trace context. Nothing Dash can trace is withheld to keep Top “distinct.”

---

## 4. `Top` View (Non-ytrace Fallback — Infrastructure & Host Topology for Uninstrumented Hosts)

> **Scope (2026-08-23):** Top exists for the **complement** of `ytrace`. When a host or app already emits `ytrace`, prefer Dash — it shows the same infra plus the trace. Top's value is *zero-dependency* visibility: no SDK, no daemon, just the probe over ssh on stdin.

### 4.1. Connected-host switcher & persistent machine registry

Machines are selected in `System Top`'s document bar. They are not a second
partition in the notebook rail; the entire rail remains available to the flat
book shelf.

- **Sources of Machines**:
  1. Local machine (`local` / `alpha`).
  2. Auto-discovered remote SSH sessions from **every** live Yggterm daemon
     (`server daemons` → `server snapshot --endpoint` per daemon). ⚠ Asking only
     the busiest daemon loses machines: a remote host is known to the daemon
     holding ITS sessions, which is not necessarily the busiest one.
  3. Stored user configurations from `~/.yggterm/config/machines.json`.

- **⛔ DISCOVERY REGISTERS; IT DOES NOT MERELY LIST.** Every discovered machine is
  merged into the registry and stays there. The roster is *registry ∪ today's
  discovery*, so a machine that has gone quiet is still probed and reports as
  **unreachable** rather than disappearing from the topology. A machine that is
  merely down and a machine that never existed must never look the same.

- **⛔ MERGING NEVER CLOBBERS.** The operator's `label` and their `is_yggdrasil`
  flag survive every rediscovery — they carry knowledge discovery does not have.
  Auto-detection may **fill** an unset `is_yggdrasil` (a reading showing ZFS pools
  or containers), but may **never clear** one: a probe that timed out once must not
  demote a machine that is a hypervisor.
- Explicit machines may still be registered programmatically in
  `~/.yggterm/config/machines.json`; discovery and stored configuration are
  unioned. Ytop does not add a second machine-management UI to the rail.
- The switcher names every logical host. Selecting one opens its live reading;
  unreachable and collecting states are expressed in the document instead of
  shrinking or decorating the shelf.

### 4.2. Host System Metrics Card
- Hostname, CPU Model, Physical/Logical Cores, Kernel, Uptime.
- **Visual Progress Gauges**:
  - **CPU Utilization**: `[██████░░░░] 37%` (real-time delta sampled over 400ms).
  - **Memory Utilization**: `[████████░░] 320.9 GB / 503.6 GB (63%)`.
  - **Swap Space**: `[░░░░░░░░░░] 0 MB / 0 MB`.
  - **Load Averages**: 1m, 5m, 15m indicators.

### 4.3. Yggdrasil Host Special Features (e.g. `main` / server)
When a machine is designated as a Yggdrasil node (or has ZFS / LXC installed):
1. **ZFS Storage & IOSTAT Card**:
   - Pools overview: Pool name, health state (`ONLINE`, `DEGRADED`, `FAULTED`), capacity, allocation, fragmentation %.
   - **Real-Time `zpool iostat`**:
     - Read IOPS & Read Bandwidth (MB/s).
     - Write IOPS & Write Bandwidth (MB/s).
   - Dataset list with mountpoint and space usage (`zfs list -Hp`).
2. **LXC Container Consumptions & Expandable Process Drill-Down**:
   - Container summary table: Name, Status (`RUNNING`, `STOPPED`), Memory RAM, CPU%.
   - **Collapsible Disclosure Tree**:
     - Each container row can be uncollapsed (`expanded: true/false`).
     - When expanded, shows the top CPU and Memory processes running **inside that specific LXC container** (PID, user, CPU%, RSS MB, process name, command).
     - Enables immediate isolation of which container or process is consuming host capacity.

### 4.4. Host Top Processes Card
- Top 10-20 processes across the entire machine.
- Sortable by CPU% or Memory RSS.
- Interactive search/filter box.
- Each eligible process row exposes `Kill…`; it opens an explicit `TERM`,
  `INT`, and `KILL` chooser. TERM is the graceful default recommendation, but
  no signal is sent until a choice is made. PID 1 and Ytop itself are protected.

---

## 5. `Dash` View (Full-Trace Fleet Cockpit — NO LIMITATIONS; kernel → ytrace → app → flamegraph)

> **Scope (2026-08-23):** Dash has **no limitations**. It is the one place where kernel, daemon, fleet, and app traces meet. This enables true cross-layer work: a hitch in `web/policy fetch` × `render/gui` storm × `host/zfs_delay` outlier correlated in one notebook, one query, one timeline. Flamegraphs and analytics are Dash-only — Top never synthesises them from `ps`.

### 5.0 Full-trace capabilities (kernel + ytrace + app)

- **Kernel / Host (via `ytop::probe` + `host/*` ytrace):** `host/cpu_delta` (400 ms delta, never `ps`), `host/zfs` (`zpool iostat` / `arcstat` / `zil_commit` / `zfs list`), `host/ebpf` (`sched_switch` latency, `io_uring` queue depth, `zfs_delay` under `tracefs` where available, `perf` sampling for flamegraphs). Sampling interval is `YTOP_SAMPLE_MS` (default 400 ms); `YTOP_EBPF` gates `ebpf` collection and requires `tracefs` mount.
- **Daemon / Fleet (via `ytrace` + `server daemons/snapshot/perf`):** `daemon_request` p50/p95/p99, `attach`/`terminal_mount`/`xterm_paint` ladder, `daemon/pty_handoff`, `session/activation` (user gesture vs internal), `render`/`xterm` storms, `resource/jankbox` (twin/leak/bloat/daemon footprint).
- **App surfaces (via `ytrace` SDK):** `ychrome` (`web/policy` / `SurfacePolicyGate::Pending` / `ssh -L` vs `ssh -D`), `yedit`/`paper`/`cellulose`/`yggtopo` document-surface traces, `libyggterm` `emit_trace!` in viewport/rails/cwd-tree.
- **Analytics:** Per-notebook `ytrace query --app <app> --category <cat> --since <window> --top N` tables, `tail` timeline, `incidents` ranked by `trigger`, and flamegraphs (`host/ebpf` stack folding). All queries are file-first (`ytrace query|tail|incidents|health|registry --json`), never raw `~/.local/share/ytrace` globs.

### 5.0.1 Dash vs Top — when to use which

| Question | Use Top | Use Dash |
|---|---|---|
| Host has `ytrace`? | — | **Dash** (superset) |
| Bare host, no SDK, just need ZFS/htop? | **Top** | — (Dash would work but adds no trace) |
| Need flamegraph / eBPF / cross-layer `web×render×zfs`? | — | **Dash** |
| No daemon running / offline? | Top probes over ssh on stdin still work | Dash still works — `ytrace` tail reads history |

### 5.1. Agent Fleet Rows Table Card
- Grouped by Campaign / Outline Prefix:
  - `6.x (widgets: refactor)`
  - `2.x (graph-demo)`
  - `3.x (finance-demo)`
  - `9.x (social-demo)`
  - `7.x (practice)`
  - `Unregistered / Custom Sessions`
- **For Each Row**:
  - **Seat & Role Badge**: `2.0 [orchestrator]`, `6.6 [relay]`.
  - **Title / Intent Label**: Clear task synopsis.
  - **UUID**: 8-char display with full copy affordance.
  - **Process Liveness**:
    - `LIVE [PIDs]`
    - `⛔ TWIN DUPLICATE [PIDs]` (Alert!)
    - `⚠️ LEAKS [N child loops]` (Alert!)
    - `💀 DEAD / RETIRED`
  - **Resource Consumption**: Real-time CPU% and RAM MB of the agent CLI and its child subshells.
  - **Context Budget & Transcript**:
    - Size in KB/MB.
    - Line count & last active timestamp.
    - Warning chips: `Normal (<5MB)`, `⚠️ HEAVY (>10MB)`, `🚨 CRITICAL (>30MB)`.
  - **Supervision State**: `⏸ Quota Held`, `🛡️ Never-Arm`, `⏸ Parked`, `⚡ Armed (Booter/Monitor)`, `Unsupervised`.

### 5.2. Resource "Jankbox" Profiling & Diagnostics Card
Dedicated diagnostic engine to explain and eliminate host lag:
- **Spinning Subshell Leaks**: Detects orphaned `until ... sleep` test loops or dangling bash child processes.
- **Twin Duplicate Instances**: Identifies dual `claude` / `codex` processes attached to identical session IDs from daemon bumps.
- **Bloated Cold Transcripts**: Flags inactive rows with massive transcripts that would waste tokens if resumed.
- **Daemon Footprint**: Measures PTY buffer memory and server daemon consumption.
- **One-Click Action**: `[ 🧹 Clean Leaks & Stale Twins ]` button to instantly reap safe orphaned processes without disturbing active agents.

### 5.3. Supervision & Watchdog Control Card
- Live status of Booter and Monitor watchdogs across the fleet.
- **Quota Hold Banner**: Shows active rate limit countdown with `[ ⏸ Set Quota Hold ]` and `[ ▶ Release Hold ]` triggers.
- **Arm / Disarm Toggles**: Per-row or fleet-wide booter and monitor subscription controls.
- **Never-Arm Ledger Editor**: View and toggle rows that must never be auto-woken.

---

## 5.4. Row Titles — owned by yggterm, not by ytop

Rows born from a launcher follow the birth-title convention
**`New {Machine} {App}`**, implemented once in yggterm
(`yggterm-core/src/birth_title.rs`). ⛔ ytop must NOT compose its own titles: the
convention is one builder precisely because it was once two, and an app that
re-derived it would restore the split.

⚠ **Birth titles stamp at BIRTH.** Rows created before a build carrying the
convention keep their old titles indefinitely — nothing re-titles an existing
row. A screenshot showing stale titles is therefore not evidence that the
convention is broken; check when the row was born and which build was deployed.

## 6. Widget Schema & YggUI Component Contract

`ytop` declares widgets following the `libyggterm` Tier A/C specification.

### 6.1. Reusable Widget Vocabulary
| Widget Kind | Role in `ytop` |
|---|---|
| `section` | Flat semantic band inside a notebook; it never creates a second rail hierarchy. |
| `titlebar_switch` | Top-level mode switch (`Top` <-> `Dash`). |
| `list-row` | Flat machine, notebook, process, and evidence rows. Notebook rows use no icon, status, or nesting. |
| `search-box` | Live filtering of processes, containers, and agent rows. |
| `button` / row action | Explicit actions (`Kill…`, page turns, safe operator overrides). Live refresh is automatic. |
| `toggle` | Supervision flags and auto-refresh intervals. |
| `markdown` | Stable notebook prose rendered by the shared extended-markdown engine; typed plots land there. |

---

## 7. Configuration & State Storage

- **Machine Registry**: `~/.yggterm/config/machines.json`
  ```json
  {
    "machines": [
      {
        "alias": "beta",
        "label": "beta (Backend Compute)",
        "is_yggdrasil": true
      },
      {
        "alias": "gamma",
        "label": "gamma (Server & Storage)",
        "is_yggdrasil": true
      }
    ]
  }
  ```
- **Never-Arm Ledger**: `~/.yggterm/relay/never-arm.tsv`
- **Quota Hold File**: `~/.yggterm/relay/booter.rate-limit-hold`
- **Booter State**: `~/.yggterm/relay/booter/<uuid>.json`
- **Monitor State**: `~/.yggterm/relay/monitor/<uuid>.json`

---

## 8. Implementation Plan

1. **Phase 1: Probing & Backend Extensions (`ytop::probe`, `ytop::rows`, `ytop::fleet`)**:
   - Add ZFS pool status and `zpool iostat` metrics collection to host probe.
   - Add LXC container process hierarchy extraction (cgroup-aware per-container process tree).
   - Add persistent `~/.yggterm/config/machines.json` loader and writer.
   - Add agent row resource footprint summation (CPU% and RSS RAM per seat).
   - **Add `host/ebpf` probe (opt-in via `YTOP_EBPF=1`): `sched_switch`/`io_uring`/`zfs_delay` via `tracefs`, flamegraph `perf` sampling folded into Dash `ytrace` — Top deliberately does not collect this.**
2. **Phase 2: Notebook-first shell (`ytop::schema`)**:
   - Keep both modes as flat title-only shelves with no preamble partition;
     place Top's inherited connected-host selector in notebook chrome.
   - Ship purposeful `System Top`, `Yggdrasil System`, and `Yggterm
     SysInternals` base books with structured live rows and actions.
   - Move notebook and trace reads off the pane lock; refresh automatically with
     stale-while-revalidate state.
3. **Phase 3: Interactive Server & Actions (`ytop::server`, `ytop::osc`)**:
   - Handle tab switching (`top` <-> `dash`), machine selection, container expansion toggles, and `+ Add Machine` form submissions.
   - Handle `Clean Leaks & Stale Twins` and supervision actions.
   - **Wire Dash `ytrace query|tail|incidents` notebooks and `host/ebpf` toggles to the same `filter`/`action` plumbing.**
4. **Phase 4: CLI & Automation Verification**:
   - Support `ytop --once --tab top`, `ytop --once --tab dash`, and `--json` (both now emit `host/*` via probe; Dash additionally emits `ytrace` incidents/flamegraph JSON when `YTOP_EBPF` and `ytrace` are present).
   - Validate on live GUI with `yggterm` and test suites.
5. **Phase 5: Libyggterm SDK — ytrace in every surface app (2026-08-23):**
   - `yggterm`, `ychrome`, `yedit`, `paper`, `cellulose`, `yggtopo`, `yggdrasil-maker` each emit `ytrace` spans at viewport/rails/cwd-tree boundaries via `libyggterm` `emit_trace!`. Required probes: `viewport/mount`, `rails/select`, `cwd_tree/navigate`, `daemon_request`, `render/gui`, `web/policy`. Tracked in `libyggterm/docs/spec-app-architecture.md` — a surface without a probe is a `Dash` blind spot.
6. **Phase 6: Retention policy:**
   - Development hosts may retain larger trace generations than production
     hosts, but notebooks query through ytrace interfaces rather than assuming
     a private path or fixed quota. Retention, coverage, and oldest/newest event
     time are evidence displayed beside every historical result.
7. **Phase 7: Shared publication graphics and agent alerts:**
   - Extend the implemented EMD v1 component tree toward full
     grammar-of-graphics transforms, export, app-routed controls, accessibility,
     and agent-readable analysis records.
   - Add deterministic notebook alert predicates, safe scripted verbs, and a
     bounded interface-LLM harness. Uptime Kuma remains until a status notebook
     proves operational parity.
