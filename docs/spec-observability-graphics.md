# Spec: publication-quality observability graphics

**Status:** EMD version 1 is implemented in `emd-renderer` with bounded typed
components and deterministic SVG scenes. This document also records later
grammar-of-graphics, export, faceting, and linked-interaction work. Ytop must
not implement a private chart renderer.

## 1. Quality target

R/ggplot2 is the reference for analytical discipline: a grammar of graphics,
clear defaults, controlled themes, faceting, statistical transforms, and
publication-grade export. Python, Rust, R, and agents should all be able to
produce the same declarative plot; the renderer, not the producer, owns pixels.

A graph is valid only if it answers a named question more clearly than prose or
a small table. It carries units, window, source, transformation, freshness,
coverage, and a text/table fallback. A sparkline without scale, time span, and
current/peak values is decoration, not evidence.

## 2. Grammar

The typed plot model separates:

- **data:** inline bounded table or reference to a live evidence id;
- **transform:** filter, group, summarize, bin, window, rate, normalize,
  confidence interval, and explicit missing-data policy;
- **mapping:** x, y, color, fill, shape, size, group, label;
- **layer:** line, point, interval/ribbon, bar, area, tile, density, box,
  violin, rule, text, flame, graph/node, and topology edge;
- **stat:** identity, count, summary, smooth, quantile, histogram, ECDF;
- **scale:** continuous, discrete, time, log, symlog, limits, breaks, units,
  palette, and out-of-domain behavior;
- **facet:** rows, columns, wrap, shared/free scale policy;
- **coordinate:** Cartesian, flipped, polar only when justified, and topology;
- **annotation:** thresholds, incidents, deployments, uncertainty, missing
  intervals, labels, and operator notes;
- **theme:** publication default plus compact/print variants;
- **interaction:** hover detail, brush/window, select-to-filter, and linked
  evidence drill-down without changing the source document;
- **accessibility:** title, question, summary, table fallback, palette test, and
  keyboard-readable marks.

The plot node is immutable input. Live updates replace its data reference or
bounded inline table while preserving scales where specified, preventing a
chart from visually jumping merely because a new sample arrived.

## 3. Extended-markdown representation

`emd-renderer` is source-decorated and lossless. The grammar is a fenced `emd`
block containing versioned JSON. Version 1 admits nested `grid` and `panel`
composition plus `plot`, `sparkline`, `metric`, `query`, `data-grid`, and
`agent-finding` components. That vocabulary can express an Axiom-like
query/results/analysis workbench without making tracing a special case; the
same blocks serve finance, browser diagnosis, and scientific reports.

The implemented tranche includes:

1. a typed `MdBlock` variant;
2. parser and source-range support;
3. byte-faithful round-trip tests;
4. safe parsing with bounded rows/marks and no executable code;
5. Dioxus rendering in yggterm;
6. deterministic SVG scene generation and responsive host rendering;
7. embedded evidence metadata exposed in source and the rendered footer;
8. stale-while-revalidate updates through ordinary document-version refreshes.

PNG/vector export, facets and transforms beyond identity, app-routed EMD
controls, persistent scale domains, live evidence references, and canonical
analysis-record export remain subsequent grammar revisions rather than hidden
Ytop-only features.

Unknown versions fail visibly with the source preserved. Raw HTML/JavaScript is
never a plot escape hatch.

## 4. Publication theme

Defaults:

- neutral panel integrated with yggterm, minimal border, no glossy card;
- readable document font for labels and tabular numerals for measures;
- axis titles include units; time axes include timezone when material;
- light major grid, restrained or absent minor grid;
- color-blind-safe qualitative and sequential palettes;
- direct labels when they reduce legend travel;
- uncertainty intervals and missing spans rendered explicitly;
- no 3-D, gradients as data-independent decoration, dual axes without an
  explicit transform, truncated bars, or smoothed lines that hide raw points;
- print/export remains legible in grayscale where practical.

Small multiples are preferred over overlapping many series. Side-by-side plots
are allowed when their windows and alignment are explicit. Flamegraphs and
topology/flow diagrams are specialized layers under the same provenance and
fallback contract.

## 5. Performance

Parsing stays incremental at block granularity. Plot data is bounded and
downsampled before it reaches the GUI. Long histories use server-side windowed
aggregation and stable level-of-detail. Rendering occurs off the pane state
lock; animation is optional and suppressed for high-frequency updates or
reduced-motion users.

Notebook switching must not wait for a plot query. The plot renders cached data
or a collecting skeleton and updates in place. A plot error is contained to its
block and preserves the prose around it.

## 6. Scientific and agent reproducibility

Every rendered plot can emit a canonical analysis record containing the source
ids, exact query window, transform pipeline, scale decisions, renderer version,
and bounded result table. An agent consumes that record rather than performing
OCR on pixels. Exported figures embed or accompany the same record so a paper
claim can be regenerated after the UI session ends.
