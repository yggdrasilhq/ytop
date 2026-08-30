---
name: ytop-notebooks
description: Compose ytop profiling notebooks — book pages in the sidebar (yedit pattern) for Top (no ytrace) and Dash (exclusively ytrace). Any agent on any host can author a notebook via ytop skill.
---

# ytop notebooks — books in the sidebar

**Where:** Right Rail (sidebar) is the bookshelf, Viewport is the open book page. Like `yedit`’s file tree, but for profiling adventures. `Top` shelf has **no ytrace** (host atlas); `Dash` shelf is **exclusively ytrace** (profiling stories). Selected page renders as markdown book page with embedded `ytrace query` sparkline/timeline.

**Shipped base notebooks (code in `ytop/src/notebook.rs`)** — ⚠ this list goes stale the moment one is added; `ytop --notebook` prints the live shelf and is the answer that cannot drift:

* `top-atlas-client` (Top) — *Host Atlas — a client host at 53 rows* · 2 pages: The Machine, Frag & Provisioning — no `ytrace_queries`, only `probe.rs` 400 ms delta.
* `dash-angry-gui` (Dash) — *The Angry GUI that wasn't — 50% vs 0.37 cores* · 3 pages: ps lied (ytrace `render/gui`), npm-cache 6.2G→146K (`daemon_request/status`), Fix & Verify
* `dash-idle-cost` (Dash) — *Idle Cost floor — 0.2 cores per daemon* · 1 page: `daemon_request/hot_restart`
* `dash-intelligent` (Dash) — *Intelligent Daemon* · 3 pages: the governor's verdict, where complaints live, self-diagnosis
* `dash-common-bugs` (Dash) — *Common Bugs* · 5 pages: session-only rehydrate, render storm, titles, input latency, CLI matrix
* `top-legendary-bugs` (Top) — *Legendary Bugs — the kernel half* · 3 pages: what LEGENDARY means and the chain, the kernel half of a mount, ⛔ the kernel-call probe that is detected and never run. No `ytrace_queries` — Top's rule — so its live blocks read `probe.rs` only.
* `dash-legendary-bugs` (Dash) — *Legendary Bugs — the yggterm half* · 5 pages: the mount churn, the ladder a mount stops on, ghost frames and broken paint, input blocking, and ⛔ the map of where the chain has no probe.
* `dash-sysinternals` (Dash) — *yggterm SysInternals* · 8 pages: the two arming planes, the seat census, when each watcher last fired, the graphs, and four dream-mode walkthroughs — **the first notebook with live blocks** (below).

**Live blocks — the half of a page that is not frozen at build time.** A `Page` may carry
`"live": "<reader>"` beside its markdown, and the viewport fills it at render time from the same
files the CLIs read. Readers in `ytop/src/sysinternals.rs`: `armings` (both subscription stores,
joined by row id) · `census` (seats) · `watchers` (last-fired + cadence) · `graphs` (ytrace,
bucketed) · `wakes` (the booter's own action column) · `cold` · `rolls` · `folds`.
Readers in `ytop/src/legendary.rs`, reached through the same dispatcher: `chain_map` ·
`kernel_half` · `ebpf_gap` (Top, `probe.rs` only) · `churn` · `mount_ladder` · `paint_chain` ·
`input_chain` · `probe_gaps` (Dash, ytrace).

⛔ **A MISSING PROBE AND A QUIET SYSTEM LOOK IDENTICAL** — the rule every Legendary page keeps, and
the one worth copying into any new notebook. Three states, never two: ✅ **seen** (fired here, with
a count) · ⚠ **named, not seen** (exists in code, silent — which may be good news) · ⛔ **no probe**
(nothing would have recorded it even if it happened). A page rendering `0` for a link nobody
instrumented is reporting its own blindness in the costume of health.

⚠ **Two shelves, two notebooks, one subject.** `Legendary Bugs` is a pair — `top-legendary-bugs`
and `dash-legendary-bugs` — because a `Notebook` carries one `mode` and the shelves are separate
lists. Each page cross-references its twin. Nothing in the mechanism needed changing for it.

⛔ **Membership is a fact, dueness is a judgement.** A live block may render which rows are in which
store and the fields those stores wrote; whether a one-plane row is a *gap* belongs to
`ygg-monitor.py list` and `ygg-booter.py list --due`, and a second copy of that reasoning inside ytop
would drift and then disagree on the day it mattered.

⚠ `graphs` is cached for 60 s and refreshed off-thread — the ytrace reader parses every generation
file whether or not the window needs it, so drawing it inline froze the render path for seconds.

**Read a notebook without a GUI** — the pages are readings, so they are checkable like one, and
checking them must never mean interrupting whoever is using the window:

```sh
ytop --notebook                                  # the shelf
ytop --notebook dash-sysinternals                # its pages, with 🔬 ytrace / 🔴 live badges
ytop --notebook dash-sysinternals --page 1       # one page, live blocks filled in
```

**Agent composition (any host, host-aware):**

```sh
# Dash notebook with ytrace queries (file-first, stdin-fed — never argv-joined)
curl -s -X POST http://127.0.0.1:<ytop-port>/action \
  -H 'Content-Type: application/json' \
  -d '{
    "action":"notebook_compose_dash",
    "payload":{
      "title":"Why status poll is 1.6%",
      "description":"Dash exclusively ytrace — per-row 4.65 µs",
      "author":"a06f8497",
      "pages":[
        {"title":"1. The 4.65 µs slope","markdown":"# ...","ytrace_queries":[{"provider":"yggterm","category":"daemon_request","name":"status","since_ms":60000}],"chart":"timeline"},
        {"title":"2. No N²","markdown":"## ...","ytrace_queries":[],"chart":null}
      ]
    }
  }' | jq .

# Top notebook (no ytrace — must not contain ytrace_queries)
curl -s -X POST http://127.0.0.1:<port>/action \
  -d '{"action":"notebook_compose_top","payload":{"title":"ZFS frag atlas","pages":[{"title":"Pool zroot","markdown":"..."}]}}' | jq .
```

Stored to `~/.local/share/ytop/notebooks/<mode>-<slug>.json` (over ssh: `ssh <host> "python3 - \"$NOTEBOOK_ID\""` stdin-fed, not `python3 -c`). `ytrace` itself stays `ytrace/src/lib.rs` + `registry::heartbeat` + `query::summarize`; `YTOP_NOTEBOOK_HOME` overrides path.

**Rail interaction:** `notebook_toggle:<id>` expands book → `page_open:<id>:<idx>` opens page in viewport. `Viewport` shows `📖 <Notebook> — <Page>` with `🔬 ytrace` badge + inline `ytrace query` preview table (`provider/category/name since`) when `has_ytrace`.

**Verify:** `ytop --notebook` lists every base notebook without a GUI at all; in the GUI, `curl http://127.0.0.1:<port>/pane/rail | jq '.widgets[] | select(.id|startswith("notebook")) | .title'` — every base book present; after compose `curl .../pane/rail` shows new book under Dash/Top separately. Viewport page `curl .../pane/viewport | jq '.widgets[] | select(.id|startswith("book_page"))'` must contain `ytrace queries` only for Dash.
