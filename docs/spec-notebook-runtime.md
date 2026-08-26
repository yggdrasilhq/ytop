# Spec: Ytop notebook runtime

**Status:** core navigation, stale-while-revalidate, connected-host inheritance,
and EMD v1 components are implemented. Alert evaluation and cross-host notebook
distribution remain planned contracts.

## 1. Notebook as the unit of observability

A notebook is a versioned, human-readable document plus declared live evidence
and actions. It is simultaneously:

- a book a person can learn and operate from;
- a reproducible analysis an agent can inspect without screenshots;
- a live view whose readings update independently of its prose;
- a portable composition that can join kernel, host, service, yggterm, app,
  browser, and webapp evidence.

Every notebook declares a purpose, mode, provenance, pages, and evidence. A
page is not accepted merely because it renders. It must answer an operational
question and say what to do with the answer.

## 2. Storage and ownership

Base notebooks are source-controlled in this repository and ship with Ytop.
The initial shelf is deliberately small:

| Mode | Base notebook | Purpose |
|---|---|---|
| Top | System Top | immediate system pressure and process action |
| Top | Yggdrasil System | host, ZFS, LXC, process and kernel workbench |
| Top | Linux Performance Clinic | guided CPU, memory, storage, network, and handoff decisions |
| Top | Legendary Bugs — kernel half | known end-to-end blind spots at host/kernel layers |
| Dash | Yggterm SysInternals | daemon/rows/supervision/history/operator home |
| Dash | Tracing Field Guide | guided windows, distributions, correlation, absence, and interventions |
| Dash | Fleet Overview | live aggregate fleet reading |
| Dash | Legendary Bugs — yggterm half | known daemon-to-pixel failure chain |

Agent-composed notebooks are JSON documents under the Ytop data directory
(`$YTOP_NOTEBOOK_HOME`, then `$XDG_DATA_HOME/ytop/notebooks`, then
`~/.local/share/ytop/notebooks`). Agents write them programmatically through the
notebook API/skill. There is no compose button in the GUI.

The shelf deduplicates normalized titles within a mode, keeping the newest
composition. A stored notebook cannot shadow a shipped id. Host-specific books
remain on their host unless explicitly promoted to a source-controlled base
book; database or index synchronization is forbidden.

## 3. Navigation

`Top` opens `System Top`. `Dash` opens `Yggterm SysInternals`, page 1. Shelf
rows are flat and open page 1. Page turns are footer actions. The rail remains
visible, so a second in-document back button is unnecessary.

Neither mode has a pre-shelf partition. In Top, a compact host switcher inside
the document bar is populated from every live Yggterm daemon snapshot. Selecting
a logical host repoints the live System Top reading even when several guests
share one physical-machine group. Dash has no host switcher.

## 4. Live evidence state machine

Every live block is one of:

```text
collecting -> observed -> refreshing -> observed
                  |             |
                  +-> stale <----+
                  +-> unreadable

uninstrumented is a separate terminal capability state, never observed(0)
```

Each block exposes, in machine-readable form:

- `source`: probe/query identity and host/application scope;
- `window`: start/end or lookback plus sampling cadence;
- `updated_at` and `age`;
- `coverage`: expected/seen emitters or probes;
- `units` and aggregation;
- `state`: collecting, observed, refreshing, stale, unreadable, or
  uninstrumented;
- `reproduce`: structured command/query description;
- `data`: the typed result and optional tabular fallback.

The current Rust `Page.live` key selects a reader, and `Page.ytrace_queries`
declares trace sources. This is the compatibility shape. The next schema
revision should replace stringly live readers with a versioned `evidence[]`
array while keeping old notebooks loadable through `serde(default)`.

## 5. Scheduling and latency

Pane GETs may hold the shared state mutex only long enough to clone a snapshot.
All filesystem walks, ytrace aggregation, SSH calls, probe execution, plot
transforms, and LLM work happen after the lock or on background workers.

Readers use stale-while-revalidate caches keyed by evidence specification and
scope. Coalesce identical in-flight work. A missed deadline returns cached or
collecting state; it does not extend an interaction. Results publish a new
document version when complete. Notebook switching should target a warm-frame
budget of 100 ms and a cold-frame budget of 250 ms before placeholders.

Sampling cadence is evidence-specific and can back off under load. The UI must
display the effective cadence and age rather than promising a false fixed
refresh rate.

## 6. Actions

An action declares id, label, target, risk, preconditions, execution backend,
timeout, and result schema. Read-only actions may execute directly. Mutating
actions must be explicit and narrowly targeted. Destructive actions require a
choice or confirmation and emit an audit result.

System Top's first action is process signalling. `Kill…` discloses TERM, INT,
and KILL. PID 1 and the Ytop process are rejected before command execution.
Local execution bypasses a shell; remote execution uses fixed signal tokens,
numeric PIDs, BatchMode SSH, and the existing multiplexed connection.

## 7. Alerts and the agent harness

Notebooks may eventually declare `alerts[]` over evidence, not arbitrary prose:

```text
id, evidence_id, predicate, for, cooldown, severity,
scripted_action?, agent_policy?, notify_policy
```

Evaluation is deterministic first: typed comparisons, rates, absence, age,
coverage, and change detection. A scripted action may run only from an explicit
allowlist. The LLM is woken when interpretation is required, evidence conflicts,
or the notebook's policy asks for synthesis. It receives the notebook purpose,
alert, bounded evidence window, recent actions, and available safe verbs—not a
raw home directory.

The harness uses the interface LLM configured by the host platform. Ytop does
not carry a second model/key configuration. Default authority is diagnose and
notify. Autonomous mutations require per-notebook policy, bounded targets,
idempotence, cooldown, audit history, and an operator-visible override.

Uptime Kuma is not deprecated until a Ytop status notebook proves equivalent
or better checks, retention, notification delivery, maintenance windows,
public-status needs, and outage survivability. The migration is evidence-led;
the existing monitor remains the fallback until parity is measured.

## 8. Planned composability examples

- **Ychrome DevTools:** choose profile, then tab; correlate network/policy,
  console, DOM/render, process, ytrace history, client kernel, and server spans.
- **REST/API explorer:** request composition, auth-safe redaction, timings,
  response/schema history, server traces, persistence effects, and replay.
- **End-to-end webapp:** one book from client kernel and browser through yggui
  layers, yggterm transport, service traces, database/storage, and server
  kernel.
- **Status notebook:** service checks plus host/container/process cause, change
  history, maintenance policy, notification evidence, and agent triage.

These are notebook compositions over shared evidence and rendering contracts,
not hardcoded product modes.
