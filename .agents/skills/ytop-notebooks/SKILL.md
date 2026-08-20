---
name: ytop-notebooks
description: Compose ytop profiling notebooks — book pages in the sidebar (yedit pattern) for Top (no ytrace) and Dash (exclusively ytrace). Any agent on any host can author a notebook via ytop skill.
---

# ytop notebooks — books in the sidebar

**Where:** Right Rail (sidebar) is the bookshelf, Viewport is the open book page. Like `yedit`’s file tree, but for profiling adventures. `Top` shelf has **no ytrace** (host atlas); `Dash` shelf is **exclusively ytrace** (profiling stories). Selected page renders as markdown book page with embedded `ytrace query` sparkline/timeline.

**Shipped base notebooks (code in `ytop/src/notebook.rs`):**

* `top-atlas-client` (Top) — *Host Atlas — a client host at 53 rows* · 2 pages: The Machine, Frag & Provisioning — no `ytrace_queries`, only `probe.rs` 400 ms delta.
* `dash-angry-gui` (Dash) — *The Angry GUI that wasn't — 50% vs 0.37 cores* · 3 pages: ps lied (ytrace `render/gui`), npm-cache 6.2G→146K (`daemon_request/status`), Fix & Verify
* `dash-idle-cost` (Dash) — *Idle Cost floor — 0.2 cores per daemon* · 1 page: `daemon_request/hot_restart`

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

**Verify:** `curl http://127.0.0.1:<port>/pane/rail | jq '.widgets[] | select(.id|startswith("notebook")) | .title'` — base 3 present; after compose `curl .../pane/rail` shows new book under Dash/Top separately. Viewport page `curl .../pane/viewport | jq '.widgets[] | select(.id|startswith("book_page"))'` must contain `ytrace queries` only for Dash.
