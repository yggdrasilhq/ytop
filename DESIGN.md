# Ytop design

Ytop is an observability notebook environment: the composability of Emacs, the
analytical finish of R/ggplot2, and the immediacy of a good system monitor. Its
moat is not another dashboard. It is one notebook that can follow a symptom
from a browser gesture through an app, yggterm, a server, storage, and the
kernel while keeping every claim reproducible for a person or an agent.

## Information architecture

The titlebar switch selects two reading modes:

- **Top** is the fast system reading. Its rail is a notebook shelf. `System
  Top` is the home and should answer the
  questions people reach for htop to answer. `Yggdrasil System` joins the host,
  ZFS, LXCs, process pressure, and the kernel tracing workbench. The notebook's
  compact host switcher inherits every logical machine connected to the
  launching Yggterm, including guests sharing one physical kernel.
- **Dash** is the ytrace notebook shelf. Nothing sits above it. `Yggterm
  SysInternals` is home: daemon graph, row costs, monitor/booter health, trace
  problems, histories, wake decisions, and operator overrides belong in its
  pages rather than a parallel control panel.

The shelf is a table of contents. A row gets the full horizontal width for its
title. It has no icon, folder glyph, page count, description paragraph, or
decorative status dot. Selecting a book opens its first page; previous/next
turns pages in the document footer. A complex book may earn groups only when
the group names materially help navigation.

## Rail rhythm

Both modes begin directly with `Notebooks`. Consecutive rows use yggterm's native tight
list rhythm, selected tint, typography, and spacing. The reference quality bar
is ychrome's Tabs rail and yggterm's Live Sessions navigation: quiet chrome,
obvious hierarchy, and no stack of cards inside cards.

Connected hosts, fleet totals, holds, watcher health, jank, and overrides are
notebook content or notebook chrome; putting them above the books turns the
notebook product into an afterthought.

## Document composition

Each page has one job. It opens with the diagnostic question and enough
orientation for a beginner, then places live evidence next to the explanation.
Use this order when it fits:

1. question and decision;
2. current finding with freshness and coverage;
3. visualization or ranked evidence;
4. interpretation and common failure shapes;
5. safe action or drill-down;
6. reproducibility metadata for an agent.

Do not lead with internal ids, raw query syntax, or a metadata wall. Stable
prose is source markdown rendered through `emd-renderer`. Live blocks are typed
readings inserted between prose blocks. Navigation and refresh state belong to
shell chrome, not inside the essay.

Tutorial writing is professional and specific. Define unfamiliar terms where
they first influence a decision. Prefer a measured example over a slogan. Name
the limit of an instrument beside the number it cannot establish.

## Live behavior

The notebook must feel alive without flicker:

- system samples: normally 2 s, adaptively 5 s under high host pressure;
- fleet/row samples: 4 s;
- interaction response: the first frame should be available without waiting
  for disk walks, SSH fan-out, ytrace aggregation, or kernel tooling;
- stale-while-revalidate: preserve the last good block and mark its age while a
  background refresh runs;
- first read: show a compact collecting state in the block that is loading;
- failure: preserve the last value and render the error/age/coverage. Never
  replace unreadable with zero.

Pane state is snapshotted under a short lock and rendered after releasing it.
Notebook disk reads and ytrace summaries need caches. A document-version tick
is notification that data changed, not permission to block the GUI.

## Actions

Actions sit beside the thing they affect. Process rows expose `Kill…`, which
opens `TERM`, `INT`, and `KILL`; no default signal is fired. Explain TERM as the
normal graceful choice and KILL as immediate last resort. PID 1 and Ytop itself
are protected.

Notebook actions declare their risk and evidence. Read-only drill-downs can be
direct. Mutations need an explicit verb; broad or irreversible operations need
confirmation and a recorded result. Agent-authored alert actions follow the
same contract as human actions.

## Visualizations

Plots use a grammar of graphics rather than bespoke dashboard widgets. Data,
transform, mappings, marks, scales, facets, coordinates, annotations, and theme
are separable and serializable. The default is publication quality: restrained
ink, meaningful axes, honest zero/baseline decisions, units on labels, legible
facets, color-safe palettes, uncertainty when known, and export fidelity.

R/ggplot2 is the quality reference, not a runtime dependency. The shared
`emd-renderer` typed component block is the renderer boundary; Python/R/Rust
may all produce the same declarative tree. Version 1 provides nested grids and
panels, plots, sparklines, metrics, query blocks, data grids, and agent findings.
This is the substrate for Axiom-like workbenches as well as finance and tracing
books. A plot also carries an evidence record and nearby table or text summary
so an agent and an accessibility reader receive the same finding.
Details live in `docs/spec-observability-graphics.md`.

## Theme

Ytop inherits yggterm's palette, reading font, gradients, and status vocabulary.
The notebook layer adds no competing brand colors. Color encodes a variable or
state; it is never the sole carrier of meaning. Emoji are prose punctuation at
most, never structural icons.

Dense scientific content is allowed; cramped content is not. Prefer aligned
tables, small multiples, and deliberate whitespace. Avoid giant headings,
rainbow badges, ASCII progress bars as the final visualization language, raw
HTML styling in markdown, and decorative cards around every block.

## Proof standard

Visual work is complete only after screenshots from a shadow client at the live
desktop size. Inspect Top and Dash, a short and long book, loading and loaded
states, and the signal chooser. Validate both pixels and app protocol. Record
residual defects precisely; a schema that looks correct in JSON is not visual
proof.
