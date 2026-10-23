# ADR 0006: Automatic layout and wire routing

- Status: accepted
- Date: 2026-10-23
- Scope: new crate `apiforge-layout`, the desktop board
- Plan: [scenario code and canvas](../plans/code-and-canvas.md)

## Context

Blocks created from code have no position, and wires are cubic curves drawn
straight through other blocks (`canvas/paint.rs`). The user wants blocks to arrange
themselves sensibly and wires that avoid blocks, either orthogonal or as smooth
curves.

## Decision

### Crate boundary

`apiforge-layout` is pure geometry without `apiforge-core` or `egui`. Its input is
rectangles with ids, ports (a side and an offset), edges between ports and pinned
flags. Its output is positions and polylines. The desktop translates cards and
wires to and from these types. The algorithms are therefore tested with generated
geometry and benchmarked without a window.

### Layered layout

Flow runs left to right (data and ordering edges both point forwards):

1. **Layers.** The longest path from the sources, in the order of a topological
   sort that ignores back edges. Graphs are acyclic by validation; cycles in the
   input are broken defensively by reversing DFS back edges.
2. **Order within layers.** Long edges get dummy nodes, then barycentre sweeps run
   down and up, at most 24 sweeps or until no crossings improve, with port order as
   a tie-breaker.
3. **Coordinates.** Brandes–Köpf: four alignments, balanced, then compacted. Layer
   gaps use the widest card of the layer plus 96 px.

**Incremental placement.** Pinned nodes are fixed obstacles and keep their
coordinates. Unpinned nodes are laid out around them. Of the positions the layered
layout proposes, each unpinned node takes the nearest free slot in its layer
column, scanning up then down in 24 px steps. A node therefore never moves because
of an unrelated edit unless it is unpinned and its own layer or order changed.
Results are deterministic: ties are broken by node order in the document.

*Tidy up* runs the full layout over the selection, or the whole board when nothing
is selected, ignores pins and clears them. It is one undoable edit.

### Wire routing

1. **Obstacles.** Card rectangles inflated by 12 px, except the source and target
   cards of the wire being routed.
2. **Port stubs.** Each wire leaves its port perpendicular to the card side for
   16 px: data outputs to the right, data inputs from the left, order outputs
   downwards, order inputs from the top.
3. **Graph.** The orthogonal visibility graph over the stub ends and the obstacle
   corners (interesting x/y lines of a sparse grid). Edges blocked by obstacles are
   omitted.
4. **Search.** A\* with cost = length + 40 px per bend + 120 px per crossing of an
   already routed wire. Wires are routed longest first. The graph always includes
   the frame around the bounding box of all cards inflated by the margin, so a route
   around everything exists whenever both stubs end outside other cards.
5. **Nudging.** Segments sharing a channel are spaced evenly (minimum 6 px), and
   wires keep a consistent order along the channel to avoid extra crossings.
6. **Bridges.** At each crossing, the wire routed later draws a small semicircle.

**Spline style** renders the same polyline with rounded corners (radius up to
24 px, limited by half of each adjacent segment) blended into cubic segments.

### Guarantees

A **final route** is one drawn when no card is being dragged. For final routes:

- No segment, rounded corner or bridge intersects the rectangle of a card other
  than the wire's own source and target. The geometry is checked after nudging
  and after smoothing. A nudged segment that would collide falls back to its
  un-nudged channel; a corner that would collide gets a smaller radius, down to a
  sharp corner.
- The only exception is a port that itself lies inside another card, because cards
  overlap. Then the wire may cross that card only within its stub. Overlapping
  cards are a layout problem that *Tidy up* resolves.
- There is no fallback through obstacles. When the search fails, which by
  construction only happens for ports inside overlapping cards, the wire takes the
  frame route.

**Interactive routes.** While cards are dragged, wires whose rerouting exceeds the
frame budget are drawn as direct, lighter curves, which may cross cards. They are
replaced by final routes on the first idle frame. Tests assert the guarantees on
final routes only and the budget on interactive frames.

### Performance

| Operation (release build) | Budget |
| --- | --- |
| Full routing, 200 cards, 400 wires | ≤ 30 ms |
| Rerouting wires of dragged cards | ≤ 8 ms per frame |
| Layered layout, 200 cards | ≤ 20 ms |

Routes are cached by the rectangles they depend on. A drag reroutes only the
dragged cards' wires and the wires whose cached route crosses a moved rectangle.
When the budget is exceeded, the remaining wires are drawn as direct curves for
that frame and rerouted when input is idle. Benchmarks live in the crate and run
in CI with a regression margin.

### Settings

*Wire style* (orthogonal / spline, default orthogonal) and *Bridges at crossings*
(default on) live in application settings, not in the document.

## Consequences

- Wire appearance no longer depends only on the endpoints, so screenshots and the
  minimap must use routed wires.
- The algorithms are standard and in our hands. If quality is insufficient, the
  crate boundary allows replacing them without touching the board.
- Hit testing of wires (hover, selection, deletion) uses the routed polylines.
