# ADR 0006: Automatic layout and wire routing

- Status: accepted and implemented, with the deviations recorded under
  *As implemented*
- Date: 2026-10-23; amended 2026-11-03 (UX-28, the spline style withdrawn),
  2026-11-12 (LAY-06, shape D and the rail) and 2026-11-12 (LAY-04, the ports
  decide the rows)
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
Withdrawn by UX-28 (see *Settings*): the board offers only the orthogonal style.

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

*Wire style* (orthogonal / spline, default orthogonal; spline withdrawn by the
amendment below) and *Bridges at crossings*
(default on) live in application settings, not in the document.

Amended by UX-28 (2026-11-03, owner decision 18 = A of the UX-1 plan): the
spline style is withdrawn until it draws curves. What it drew was the
orthogonal route with 24 px fillets, which the *Wire style* menu called
*Curves*; the menu now offers *Right angles* and *Bridges at crossings*. A
preferences file that stores `spline` still loads, as orthogonal, and the next
save of preferences writes `orthogonal`. The rounding in `apiforge-layout`
(`smooth`) stays, with its guarantees and tests, for a future curved style;
no style the board offers uses it today.

### Shape D and the rail (LAY-06)

Amended by LAY-06 (2026-11-12, owner decision Q1 = A and Q3 = A of the
[LAY plan](../sprints/lay.md#51-decisions-taken-2026-11-05)). The flow
direction stays left to right. *Tidy up* of a whole board in the folder
window no longer lays out layers; it gives **shape D**, the hybrid of a
spine and satellites:

1. **Spine.** *Start*, requests, component calls and `if`s, in the order
   the engine reaches them (`graph::sequence`), left to right in 280 px
   columns 96 px apart. The spine wraps to a new row after the columns the
   row width holds: five at the default width, 1784 px, a parameter
   (`layered::Rows`).
2. **Satellites.** Every other block hangs in the column of the spine step
   it reads last, stacked below it in the engine's order, 24 px apart (twice
   the router's clearance). The column is the first of these that exists:
   for a check, the subject LAY-11 folds it into, or else the latest step
   it verifies; the latest spine step among the block's dependencies, data
   or order; the latest column among the satellites it reads; the spine
   step the engine reaches just before it.
3. **Rows.** A row is as tall as its tallest stack, at the heights the board
   draws its cards; the next row starts 96 px lower. Folded checks keep
   their places, so showing them moves nothing. Since LAY-04 a stack may
   start below the row's top, where its ports put it (*As implemented*).
4. **The rail.** An ordering wire leaves the top of its source, above its
   control socket, and enters the top of its target, so it runs on the rail
   above the row instead of through the gap between two cards where the data
   wires run; `@after` and branch wires are dashed, as every ordering wire
   is. A wire into a card hung right under its source still leaves from the
   socket at the bottom. The router routes wires between two top ports
   first, so a data wire coming down into the row crosses a rail once.
5. **Continuation mark.** The last spine card of a row carries a mark,
   `↳ <next step>`, naming the first card of the row below. It is drawn under
   the wires and is no obstacle to them.

Tidy up of a selection, and the board of the workspace window, keep the
layered layout above. Tidy up ends with the camera fitted to the board.

## As implemented

The decisions above hold, with these corrections found while building it:

- **Coordinates: the ports decide the rows (LAY-04).** Brandes–Köpf's four
  alignments were not needed. The layout reads the ports the board draws: an
  edge carries the offset of its output down the right side of the source and
  of its input down the left side of the target, from the cards' tops
  (`Edge::with_offsets`; an ordering edge has none). The first column stacks.
  In each later column a card asks for the top at which a wire from the
  previous column with ports runs straight; of several, the one that
  straightens the most, then the one nearest the centre of its feeders. The
  cards that straighten the most wires are placed first, each as near its
  straight row as the cards already placed leave room, keeping the order
  crossing reduction gave and the 48 px row gap; the others follow at the
  mean of their feeders' centres. A pass from the right then moves a card none
  of whose wires runs straight to straighten a wire into the next column, when
  that card's target has no straight wire yet and the column leaves room. The
  result is deterministic. A layer's cards are centred in its column.
  Crossing reduction breaks barycentre ties by these ports, and counts two
  wires that meet at one card as crossing when their ports there are in the
  opposite order to their other ends, so two wires leaving one card do not
  cross right after it.
- **Shape D and the ports (LAY-04).** The spine of a row is no longer level:
  a stack starts as far below the row's top as makes a wire with ports from
  the stack before into its spine step straight, the drop of the stack before
  plus the wire's misalignment. Within the room the row already has under the
  stack any such wire may decide; a wire from the spine step before may also
  grow the row, as long as the board stays no taller than its width times
  `Rows::aspect`, the shape of the view a fit fills (1134 × 760 px at
  1600 × 1000), so growing never lowers the fit zoom. The first stack of a row
  stays at the top, and so does a spine step at either end of an ordering
  wire, so the rail meets it there; a drop that cannot make the wire straight
  is not taken. A row is as tall as its tallest stack from the row's top. On
  the reference scenarios few wires join two neighbouring columns, so few
  become straight; the generated data-only boards, laid out in layers, are
  where the rule shows.
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
