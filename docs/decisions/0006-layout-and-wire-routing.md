# ADR 0006: Automatic layout and wire routing

- Status: accepted and implemented, with the deviations recorded under
  *As implemented*
- Date: 2026-10-23
- Scope: new crate `apiforge-layout`, the desktop board
- Plan: [scenario code and canvas](../roadmap.md)

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

## As implemented

The decisions above hold, with these corrections found while building it:

- **Coordinates.** Brandes–Köpf's four alignments were not needed: rows are
  assigned in the order crossing reduction produced and then pulled towards the
  average row of the cards feeding them, within the gap to their neighbours. The
  result is deterministic and keeps the row order; the balancing is what the
  method's last step does. A layer's cards are centred in its column.
- **Dummy nodes.** They exist for crossing counting, as decided, and they do not
  reserve space in the coordinate phase. A long edge is routed around the cards
  anyway, so a reserved channel bought nothing.
- **The search window.** A route is searched over a window around its two ports,
  not over the whole board, and the window widens when no route is found. A card
  outside the window cannot be met by a route inside it, so this changes nothing
  about the guarantees, and it is what makes the budget reachable. A search is
  also capped at 160 000 grid cells, far above a readable board.
- **The clearance is not the guarantee.** Where cards leave less than the 12 px
  margin, the last attempt searches with no margin at all: the wire then hugs the
  cards instead of crossing them. Dropping a cosmetic clearance is always better
  than crossing a card.
- **Stubs.** A port inside another card gets a stub long enough to leave it, which
  is the exception the guarantee names. A port merely *beside* a card gets a
  shorter stub, down to nothing, instead of one that ends inside its neighbour.
- **Enclosed ports.** When cards enclose a port completely no route exists. Such a
  wire is marked incomplete and drawn as a plain corner, lighter, and the
  guarantee does not apply to it. The *frame route* of the original decision was
  dropped: a frame route out of an enclosed pocket would have to cross a card, so
  it was a worse answer than saying so.
- **Crossing counts.** The cost of crossing an earlier wire is paid per grid edge,
  marked once per search, rather than measured per candidate segment.

**Measured** on the reference machine, release build, 200 cards and 400 wires
whose ports are a layer or so apart, as a scenario's are:

| Operation | Budget | Measured |
| --- | --- | --- |
| Full routing | ≤ 30 ms | 20 ms |
| Reroute while dragging | ≤ 8 ms per frame | 0.2 ms |
| Layered layout | ≤ 20 ms | 0.4 ms |
| Smoothing every route | — | 0.3 ms |

The same board wired at random instead of between neighbours — every wire
spanning the whole board — takes about 700 ms to route in full. No budget covers
that shape and no scenario has it; the number is recorded so the limit is known
rather than assumed. `cargo run -p apiforge-layout --example bench --release`
prints all of it.

## Evidence

Generated boards check the guarantees: no final route reaches into a card that is
not its own, after nudging and after smoothing; the documented exception holds on
boards where cards are piled up; bridges appear at every crossing and nowhere
else; layout and routing repeat exactly; a new card never lands on another and
moves nothing; and a tidied selection keeps clear of the cards outside it. The
fixtures' own boards are routed through the desktop's conversion in its tests, so
the guarantee covers the cards the application really draws.

Both wire styles were also reviewed in the native application on Linux, at
1600 × 1000, through the screenshot probe against the demo workspace, with the
style taken from a preferences file — which is how the setting is reached in a
real session.

## Consequences

- Wire appearance no longer depends only on the endpoints, so screenshots and the
  minimap must use routed wires.
- The algorithms are standard and in our hands. If quality is insufficient, the
  crate boundary allows replacing them without touching the board.
- Hit testing of wires (hover, selection, deletion) uses the routed polylines.
