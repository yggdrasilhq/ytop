# Plan: ytop Dash-first AXIOM parity + jojo hot proof

**Status:** DRAFT for approval — no code land until you sign this  
**Branch:** main (ytop) + private `~/git/ytop-lore`  
**Date:** 2026-08-17  
**Choices locked by you:** Dash-first · ytop-lore private · jojo hot as proof

## 1. What we are building, in one paragraph

ytop becomes the **AXIOM for yggterm**: Dash is a blazing-fast, agent-first timeline that profiles the shit out of yggterm (200 agents + ychrome video on one machine, which you already hit at ~60), and Top is the htop/btop/eBPF/ZFS observability you expect but without the frankenstein. Both toggles live in one `libyggterm` document surface (no markdown tables), with the same Chrome the user already knows: `⚡ Top` | `📊 Dash`. The document view stays native shell DOM so `server app screenshot --backend os` is faithful and agents can drive it via `server app do/read/wait` without stealing your viewport.

## 2. Narrowest wedge that proves it

Do not rebuild Top fully then Dash fully. Build **one Dash slice end-to-end** that already cools jojo:

**Slice 1 (this plan):** Dash · Fleet Rows (grouped, searchable) + Resource Jankbox + Timeline (AXIOM-lite) + one Top probe card (system meters) to prove the fan-out. All gated by agent-first `server app do --session <path>` so an agent can filter/sort without moving your seat.

Jojo hot is the falsifier: `jojo %CPU ps` above shows `yggterm 40.9%` + `WebKitWebProcess 24.9%` + second `11.8%` — that is the 200-agent ceiling you feel while watching YouTube in ychrome. If this slice does not move that number, the architecture is wrong even if the pixels are pretty.

## 3. Dash = AXIOM rival (not a log dump)

AXIOM lays `timeline × trace × span`. ytop lays `timeline × probe × row`:

| AXIOM | ytop Dash | Probe (cheapest rung) |
|---|---|---|
| ingested logs | `~/.yggterm/event-trace.jsonl` + `server perf trio` + `server app state` | `api` — `yggterm-headless server trace tail 200` |
| dataset | per-agent row: UUID, title, token/context budget, stale lease, booter hold | `api` — `server snapshot tenants` + `session-titles.db` |
| query | filter by campaign prefix `6.x/2.x/9.x`, by `cost>30MB`, by `twin/leak` | `api` — schema `search-box` |
| timeline | scrollable 5-min window, span per row = CPU% + RSS + log volume | `proc` delta (`/proc/<pid>/stat` 400ms), not `ps %CPU` |
| alert | jankbox cards: spinning `until sleep` loops, twin duplicates, cold bloated transcripts, daemon PTY bloat, quota holds | `proc` + `api` |

**Timeline groundwork (AXIOM-like):** store `t0` + ring of `(t, row, cpu_ms, rss_mb, log_events)` in `ytop` daemon memory, 5-min TTL, downsample to 1s buckets for paint. No eBPF in Slice 1. eBPF is Slice 2 opt-in per host.

**Jankbox actions (one-click, auditable):** `Clean leaks & stale twins` reaps only safe orphans (`tenants` `age+cmd` + `booter never-arm` ledger check). Every `do` writes a trace row so panel + agent + system notification agree.

## 4. Top = htop + btop good parts + ZFS, not a fork

* CPU meter: **delta** over 400ms (`/proc/stat` twice), not `ps` lifetime avg (the trap `ytop dash` already fixed). Load 1/5/15 + core count.
* Memory/Swap gauge: `MemAvailable` + `SwapCached` honest, not `free`.
* ZFS: `zpool list -Hp` + `zpool iostat 1 1` per pool, health `ONLINE/DEGRADED/FAULTED`, frag %, `zfs list -Hp` dataset table. Yggdrasil hosts auto-detect `zfs` + `lxc`.
* LXC: collapsible tree (`depth/expanded/expand_action`) with per-container top procs (PID, CPU delta, RSS) on expand. No per-tab RSS invention — same caveat as web_surface.
* Process table: top N by `cpu%` or `rss`, searchable, `meters` widgets for proportion bars.

All reads over **one ssh fan-out per host**, 2s cadence, typed JSON. No agent installed on remote — probe sent over stdin.

## 5. Decision log — what we are NOT doing

* No blank-table formatter, no `rustfmt --all` (fleet rule).
* No Exhibit B on `libyggterm` (MPL plain stays agent-linkable).
* No per-surface RSS fabrication (WebKit shares one web process).
* No `GDK_BACKEND=x11` on Wayland jojo (must stay `wayland-native` or screenshots lie).
* No `vendor/` rewrite for Apache — reuse remains GPL-compatible via dual `Apache-2.0 OR MIT` arms.

## 6. File map for Slice 1

```
~/gh/ytop/src/
  main.rs         — `--mode top|dash --tab rows|jankbox|supervision` (exists)
  probe.rs        — add: `perf_trio`, `trace_tail`, `zpool iostat` parsers
  rows.rs         — FleetRowsReport grouping by campaign prefix, cost chips
  schema.rs       — Dash cards: timeline (meter strip), jankbox list-row + actions, search-box
  fleet.rs        — Machine registry + LXC/ZFS topology (derived via btime)
  server.rs       — spawn + `print_once --json` + `probe_once` fan-out
~/gh/ytop/docs/spec-ytop-design.md — stays SSOT, this plan amends it
~/git/ytop-lore/lore/yggterm-jojo-hot.md — first WORKS entry = cooled jojo with cost
```

## 7. Roles

| Repo | Licence | What goes there |
|---|---|---|
| `~/gh/ytop` | `GPL-3.0-or-later` | UI, probe, schema |
| `~/gh/libyggterm` | `MPL-2.0` | new widget if second app needs it (Tier C) |
| `~/git/ytop-lore` | private | proven hot-cool patterns, per-host costs |

## 8. Verification gates (no hand-wave)

* `cargo test -p ytop` before/after — no count can fail on absence (jankbox must name survivors, not just empties).
* `ytop --once --json | jq` on `openclaw` + `ssh jojo -- ytpoprobe` (local) must match `cargo run -p ytop -- --probe`.
* Live: `server app screenshot /tmp/ytop.png --backend os` faithful before declaring Dash done (ytop is under-glass webview — same trap as web_surface).
* Jojo hot falsifier: `ssh jojo "ps -o %cpu,comm --sort=-%cpu | head"` `yggterm` 40.9% → after slice, with 60 agents + ychrome video, `load < Ncores` and perf trio `span_cpu_hot` not firing on idle.

## 9. Next 3 moves if you approve

1. Extend `probe.rs` with `trace_tail` + `perf trio` + delta-CPU reader, behind `api` rung, ~120LOC.
2. Build `schema.rs` Dash timeline strip (meter + list-row tree) + jankbox card, wire `search-box` to fleet rows, document-surface test harness.
3. Cool jojo: add `Clean leaks` to jankbox, run `server tenants` to reap spinning `until sleep` loops, measure before/after `cpu%`, log `WORKS` to `ytop-lore/lore/yggterm-jojo-hot.md` with `cost: 40.9% → X% · probe: tenants+perf trio @ 2026-08-17`.

## 10. Open question for you (one)

Is the timeline's first data source only the existing `~/.yggterm/event-trace.jsonl` + `server snapshot`, or should Slice 1 also tail `journalctl --user -u yggterm --since 5min` as a second lane? The former is free; the latter needs `ssh` + `journal` parse but catches watchdog churn that never writes a trace row.

---
**Approval:** reply "approve" or name changes. On approve, Slice 1 lands as two commits: (1) `ytop-lore` first pattern stub + `probe.rs` extension, (2) `schema.rs` Dash timeline+jankbox document surface.
