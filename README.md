# ytop

**`lstopo` + `htop` + DTrace notebooks, for a yggterm fleet.** The machines, the containers they
host, and what is actually burning them right now — plus notebooks that read **ytrace** probes from `yggterm`, `ychrome`, and any app that embeds `ytrace`. Formerly `yggtopo` — that name now resolves to `ytop`.

> **One `ytop` sees five planes at once:** the server machine(s), the client machine you’re looking from, the `yggterm` terminal fleet, the `ychrome` browser surface, and the webapp in the viewport. A hitch in the app’s `fetch` span, a `render/gui` storm, a `zfs_delay` outlier, and a `media_capture` prompt all land as the same `ytrace` record kind, queried the same way, in the same notebook — so a frontend jank is correlated to a ZFS commit without switching tools.

It ships a curated base shelf. Both rails contain notebook titles only. **Top**
opens `System Top`: its document bar inherits every host connected to the
launching Yggterm, and its pages carry host/ZFS/LXC/process evidence and safe
operational actions. **Dash** opens `Yggterm SysInternals`: daemon and row
costs, monitor/booter health, histories, trace trouble, and operator overrides
live in its pages. Additional notebooks are composed programmatically by
agents; the GUI intentionally has no Compose button.

It is a [libyggterm](https://github.com/yggdrasilhq/libyggterm) document-surface
app: it declares its surfaces over the terminal's control channel and yggterm
paints the shell interaction widgets. Notebook prose uses libyggterm's shared
`emd-renderer`; its versioned `emd` blocks now provide plots, sparklines,
metrics, data grids, query panes, agent findings, and nested workbench layout.
The renderer owns scales and SVG pixels; Ytop only emits typed evidence. This
keeps the result screenshot-faithful, agent-readable, and reusable outside
observability.

Outside yggterm it is a plain CLI that prints the same reading, because an app
that can only exist inside a GUI cannot be checked without one.

```
$ ytop --once

alpha · 16 × Example CPU E1 · kernel 9.9.9-invented
  ├─ alpha          55.2% cpu    659 procs  (none)
  ├─ 📦 sandbox     RUNNING
       12.0%    533584 KB  example-renderer
        7.2%    207448 KB  example-shell

beta · 32 × Example CPU E2 · kernel 9.9.9-invented
  ├─ beta         2772.0% cpu    530 procs  (lxc)
  ├─ gamma          22.1% cpu     96 procs  (lxc)
```

## What it shows

**Topology.** Machines, and the yggterm hosts that live on each. Two containers
on one kernel collapse into one machine; a separate box stands alone.

⭐ **That grouping is DERIVED, not configured.** A hand-maintained "these two
are the same box" list is a second source of truth about the topology and is
wrong the first time a guest moves. The key is the kernel's own boot instant
(`btime` in `/proc/stat`) — shared by every container on that kernel and not
virtualised by lxcfs — carried with the CPU model and core count so that two
distinct machines booting in the same second cannot collide.

**Live processes.** htop's half.

⛔ **`ps %CPU` is a lifetime average** — total CPU over total age — so a process
that burned a core for an hour and has slept since reads as busy forever, and
one that started spinning ten seconds ago reads as idle. A view built on it is a
biography, not a live view. ytop samples `/proc/<pid>/stat` twice and reports
the delta, which is what htop actually does.

Each eligible process row has **Kill…**, which opens an explicit `TERM`, `INT`,
or `KILL` chooser. Nothing is sent by selecting the row. PID 1 and Ytop itself
are protected; TERM is the normal graceful choice and KILL is immediate last
resort.

**Booter.** Who is armed, when they are due, and the switch that turns it off.
The fleet's booter is a watchdog that kicks stalled agent sessions; it could
always be stood down by someone with a shell on the right machine who knew the
verb, which is not an off switch but a rumour of one.

⛔ **ytop re-implements none of it.** Every read is `ygg-booter.py … --json`
and every write is one of its verbs. Two answers to "is the booter on right now"
is the defect this pane exists to remove, not to double.

## Honesty rules it keeps

These are the ones that changed the code, not just the prose:

- **"I could not look" is never rendered as "it is idle."** An unreachable host
  says so, names the failure, and states that nothing under it is a measurement.
- **An empty container list is not "no containers."** Only a container host can
  enumerate guests — from inside one the answer is unknowable — so the footer
  reports how many machines could be *asked*, not a bare zero.
- **A subscriber that was never classified reads as "due unknown",** never as
  safe.
- **A watcher that is alive but ticking into a closed log is never drawn as
  healthy.** Alive is not audible.

## ytrace notebooks

`ytop` in **Top mode** is the Linux-tooling workbench: interval host/process
sampling, ZFS, LXC, and the Yggdrasil System notebook. It remains useful when no
application trace exists and explicitly reports the kernel probes it has not
yet attached.

`ytop` in **Dash mode** is the DTrace notebook — `ytrace query --app <app> --category <cat> --since <window> --top N` tables + `tail` timeline + `incidents` ranked by `trigger`. It goes beyond Chrome DevTools because it sees cross-process, cross-host spans (`web/policy` fetch, `SurfacePolicyGate::Pending`, `ssh -L` vs `ssh -D`).

**A page can have a live half.** Shipped prose is frozen the moment it compiles, which is right for
a story and useless for a state — "who is armed" and "when did that last fire" go stale in minutes.
So a page may name one **live reading** that the viewport fills at render time from the same files
the CLIs read. `Yggterm SysInternals` is the notebook built on it: the two supervision planes joined
row by row, the seat census, every watcher's last-fired time against its cadence, the graphs, and
four walkthroughs of what the machinery is *for*.

Live trace summaries use stale-while-revalidate caching. Opening a page never
waits for a trace directory walk: the first frame shows cached evidence or an
honest collecting state, and later document-version polls deliver the result.
The pane state lock is released before notebooks render, so a slow page cannot
freeze sampling or other actions.

The design and runtime contracts are in [DESIGN.md](DESIGN.md),
[docs/spec-notebook-runtime.md](docs/spec-notebook-runtime.md), and
[docs/spec-observability-graphics.md](docs/spec-observability-graphics.md).

⛔ **Membership is a fact; dueness is a judgement.** A live block renders which rows are in which
store and the fields those stores wrote about themselves. Whether a one-plane row is a *gap*, a
deliberate stand-down, or a corpse mid-countdown belongs to the watchdogs' own verbs — a second copy
of that reasoning here would drift, and then disagree about a live row on the day it mattered.

Notebooks are readable with no GUI at all, for the same reason the rest of ytop is:

```
$ ytop --notebook                            # the shelf
$ ytop --notebook dash-sysinternals          # its pages
$ ytop --notebook dash-sysinternals --page 3 # one page, live blocks filled in
```

## Building

```
cargo build --release
cargo test
```

## Configuration

| | |
|---|---|
| `~/.ytop/hosts` (compat `~/.yggtopo/hosts`) | extra ssh aliases, one per line. **Extends** the roster yggterm already knows; it never replaces it, so adding one line cannot silently hide the rest. |
| `YTOP_REFRESH_SECS` (compat `YGGTOPO_REFRESH_SECS`) | how often the machines are re-read (default 2). This is an ssh fan-out, not a local read — a bigger fleet on a slower link wants a bigger number. |
| `YTOP_BOOTER` (compat `YGGTOPO_BOOTER`) | path to `ygg-booter.py`. Otherwise taken from the running watcher's own command line, and failing that from the conventional checkout path. |

No agent is installed on the machines it reads: the probe is sent over ssh on
stdin and run there, so a freshly added host works with no deployment.

## Licence

GPL-3.0-or-later. See [LICENSE](LICENSE).
