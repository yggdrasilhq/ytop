# AGENTS.md — ytop Cockpit & Observability Substrate

## Mission & Core Value Proposition
`ytop` is the modern infrastructure, fleet agent, and end-to-end application observability cockpit for the Yggdrasil platform. Built on the **libyggterm document surface** protocol (OSC 7717), `ytop` turns complex system observability into an interactive, high-aesthetic book of **diagnostic notebooks**.

### The Inspiration & The Moat
* **Inspiration:** Inspired by Axiom (the modern logging and observability SaaS), `ytop` is designed as the "Emacs of Observability" for terminal and agent workspaces.
* **The Moat:** Deep interactive UX for tracing, eBPF, Dtrace, and application-layer telemetry. While tools like `htop` or raw Linux CLI probes dump static counters, `ytop` answers operational questions with hand-held guidance, scientific ggplot2/R-quality visual charts, and instant action affordances (e.g. signal dispatch, subshell reaping, quota holds).
* **Dual Readability:** Every notebook is designed to be **first-class for humans and first-class for autonomous agents**. Agents programmatically compose, query, and inspect notebooks without UI friction.

---

## Dual Mode Contract

| Dimension | `Top` Mode (Infrastructure & Hypervisor) | `Dash` Mode (Application & Fleet Telemetry) |
| :--- | :--- | :--- |
| **Scope** | Pure host infrastructure: `/proc` 400ms delta, ZFS pools, LXC containers, systemd service mesh. **Zero ytrace application noise.** | All-inclusive: Level-0 host metrics up through deep application-layer `ytrace` spans, daemon internals, agent seats, and WebApp DevTools. |
| **Sidebar Layout** | **Two partitions**: Top partition (≤30% height) for connected fleet hosts; Bottom partition for the Bookshelf (Notebook rows). | **Single partition**: 100% focused on the Bookshelf (Notebook rows). No top partition. |
| **Base Notebook** | **Host Operations (Super-htop)** with instant process actions (SIGKILL, SIGTERM, SIGINT, SIGHUP). | **yggterm SysInternals** with daemon topology graph, seat resource census, live ytrace incidents, and booter controls. |

---

## The Bookshelf & Row Engine Architecture
* **Shared List-Row Engine:** Consumes Yggterm's shared `list-row` component vocabulary matching `Live Sessions` and `ychrome`.
* **Group Headers (`depth: 0`):** Use `icon: "icon:folder"` or `"icon:archive"` or `"📖"`, with disclosure chevrons (`expanded: Some(bool)`) and page count subtitles.
* **Child Pages (`depth: 1`):** Use context-specific glyphs (`"🔥"` flamegraph, `"📈"` timeseries, `"📊"` metrics/top table, `"file:md"` narrative/guide, `"⚡"` incidents/signals).
* **Zero Custom Chrome:** Pure document-surface schema emitted over loopback HTTP; Dioxus renders native shell DOM.

---

## Non-Blocking & Performance Invariants
1. **Zero UI Thread Stalls:** Schema assembly in `viewport_view` and `rail_view` must execute in <1ms.
2. **Asynchronous Background Probing:** Heavy `/proc` walks, remote SSH probes, ZFS/LXC scans, and `ytrace` queries run in dedicated background worker threads and query caches.
3. **Instant Selection:** Clicking a notebook or page switches views with 0ms perceptibility; no synchronous filesystem or network blocking on the request thread.

---

## Autonomous Agent Harness
`ytop` includes a built-in telemetry watchdog:
* Monitors background metrics, evaluates alert invariants in notebooks (e.g. CPU > 95% for 30s, quota hold tripped, subshell leaks, twin agents).
* Issues structured `ytrace` complaints and escalates to the Interface LLM (`gemini-3.7-flash` / `gpt-5.6-luna`) or notifies the campaign orchestrator.
