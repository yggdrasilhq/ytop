# Ytop agent contract

Ytop is the notebook workbench for end-to-end observability across hosts,
Yggdrasil systems, yggterm, libyggterm apps, browser surfaces, and webapps.
Before changing product behavior, read `DESIGN.md`,
`docs/spec-notebook-runtime.md`, and `docs/spec-observability-graphics.md`.

## Fleet memory

This repository has its own campaign memory. At the start of substantial work,
after a handover, or when another host may have learned something, run:

```text
ygg-memory status --harness codex
ygg-memory diff --harness codex
```

Open only the relevant door with `ygg-memory get --file <name>`. The canonical
door is `campaign-ytop.md`; it is linked from yggterm's campaign memory. Publish
durable findings with `ygg-memory publish`. Never synchronize credentials,
sessions, databases, indexes, or lock files between hosts.

## Product invariants

- The building block is a notebook. Every visible operational surface has a
  name, a purpose, and a place on a shelf.
- Top and Dash both begin directly with a flat notebook shelf. Connected-host
  selection is notebook chrome in `System Top`, never a second rail hierarchy.
- Dash's home notebook is
  `Yggterm SysInternals`.
- Notebook rows are title-only. No decorative icons, page-count noise, status
  dots, folders, or page children. Row groups are exceptional and require a
  genuinely complex book.
- Agents compose notebooks programmatically. Do not add a Compose button.
- Live readings never block pane GETs or notebook switching. Return cached data
  or an honest collecting state, refresh off the request path, and keep stale
  data visible while refreshing.
- A missing probe is not zero. Every reading distinguishes observed, silent,
  unavailable, stale, and uninstrumented states.
- Top process CPU is an interval delta, never `ps %CPU`. Destructive operations
  require an explicit verb chooser; a row click must never kill a process.
- Base notebooks live in this repository. Host-specific/composed notebooks live
  in the user's Ytop notebook directory and are never copied between hosts as a
  database.
- Human and agent readers are both first-class. Every page must state the
  question, window, source, freshness, units, and reproduction path in a form
  that can be parsed without pixels.

## Notebook quality gate

A notebook is not a telemetry dump. Before shipping one, be able to answer:

1. What operational decision is this book for?
2. What are the first three questions it answers?
3. Which blocks are live, how fresh are they, and what does absence mean?
4. What action can a reader safely take from the finding?
5. What does a beginner learn that lets them graduate to the next page?
6. Can an agent reproduce every plotted or summarized result from declared
   source, transform, window, and units?

Base books use concise pages, progressive disclosure, consistent row groups,
and professional prose. Tutorial prose teaches a tracing concept through a
real diagnostic task; it does not decorate an unrelated dashboard.

## Shared-platform boundary

Ytop owns notebook semantics, probe/query orchestration, and its pane schemas.
Yggterm owns generic libyggterm surfaces and interaction rendering.
`emd-renderer` in libyggterm owns extended-markdown parsing and document
visualization nodes. Never invent a Ytop-only plot renderer. Extend the shared
typed grammar with source-range and round-trip tests, then consume it here.

## Verification

- Run focused Rust tests, then the full `cargo test` suite.
- Validate the app protocol (`/ping`, rail, viewport, actions) independently of
  pixels.
- For visible work, use a yggterm shadow client and take iterative pixel
  screenshots on the live host. Compare the rail rhythm with ychrome and
  yggterm's native navigation. Do not claim a visual fix from JSON alone.
- Verify the running Ytop binary hash, the manifest text (`New Ytop`), and the
  externally rendered result on every deployed host.
- Preserve other agents' sessions and worktrees. Do not restart or kill a live
  GUI merely to obtain a clean test environment.
