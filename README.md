# ytop dash

**`lstopo` + `htop`, for a yggterm fleet.** The machines, the containers they
host, and what is actually burning them right now — as one view. Plus an off
switch for the fleet's watchdog.

It is a [libyggterm](https://github.com/yggdrasilhq/libyggterm) document-surface
app: it ships **no UI code**. It declares a widget schema over the terminal's own
control channel and yggterm paints it as native shell DOM — which is what keeps
it screenshot-faithful and drivable by the host's automation, instead of a
canvas nothing else can see into.

Outside yggterm it is a plain CLI that prints the same reading, because an app
that can only exist inside a GUI cannot be checked without one.

```
$ ytop dash --once

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
biography, not a live view. ytop dash samples `/proc/<pid>/stat` twice and reports
the delta, which is what htop actually does.

**Booter.** Who is armed, when they are due, and the switch that turns it off.
The fleet's booter is a watchdog that kicks stalled agent sessions; it could
always be stood down by someone with a shell on the right machine who knew the
verb, which is not an off switch but a rumour of one.

⛔ **ytop dash re-implements none of it.** Every read is `ygg-booter.py … --json`
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

## Building

```
cargo build --release
cargo test
```

## Configuration

| | |
|---|---|
| `~/.ytop/hosts` | extra ssh aliases, one per line. **Extends** the roster yggterm already knows; it never replaces it, so adding one line cannot silently hide the rest. Legacy `~/.yggtopo/hosts` still read. |
| `YTOP_REFRESH_SECS` | how often the machines are re-read (default 2). This is an ssh fan-out, not a local read — a bigger fleet on a slower link wants a bigger number. Legacy `YGGTOPO_REFRESH_SECS` still honoured. |
| `YTOP_BOOTER` | path to `ygg-booter.py`. Otherwise taken from the running watcher's own command line, and failing that from the conventional checkout path. Legacy `YGGTOPO_BOOTER` still honoured. |

No agent is installed on the machines it reads: the probe is sent over ssh on
stdin and run there, so a freshly added host works with no deployment.

## Licence

GPL-3.0-or-later. See [LICENSE](LICENSE).
