# ADR 0006: Automatic layout and wire routing

- Status: accepted and implemented, with the deviations recorded under
  *As implemented*
- Date: 2026-10-23; amended 2026-11-03 (UX-28, the spline style withdrawn),
  2026-11-12 (LAY-06, shape D and the rail), 2026-11-15 (LAY-03, the gap
  between lines) and 2026-11-16 (LAY-28, the order of lines in a corridor;
  LAY-29, no line under its own card)
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
   their places, so showing them moves nothing.
4. **The rail.** An ordering wire leaves the top of its source, above its
   control socket, and enters the top of its target, so it runs on the rail
   above the row instead of through the gap between two cards where the data
   wires run; `@after` and branch wires are dashed, as every ordering wire
   is. A wire into a card hung right under its source still leaves from the
   socket at the bottom. The router routes wires between two top ports
   first, so a data wire coming down into the row crosses a rail once.
   *Changed by LAY-29, below: every ordering wire leaves its socket at the
   bottom.*
5. **Continuation mark.** The last spine card of a row carries a mark,
   `↳ <next step>`, naming the first card of the row below. It is drawn under
   the wires and is no obstacle to them.

Tidy up of a selection, and the board of the workspace window, keep the
layered layout above. Tidy up ends with the camera fitted to the board.

### The gap between lines (LAY-03)

Amended by LAY-03 (2026-11-15, the owner's finding on #34: lines 2–4 px
apart or merged into a band must never happen; a sprawling board is better
than packed lines). It replaces step 5 of the routing above and the
spacing of shape D.

1. **One gap, 20 px** (`GAP`): the router's clearance from a card, the
   length of a stub and the distance between two lines. A line keeps it
   from every card, its own two included, and from every line routed
   before it, side by side and end to end; lines cross at right angles as
   before. The first and last 20 px of a line, at its ports, are not
   judged. Wires of one port share the straight piece out of it and turn
   off it one by one, 20 px apart; ports nearer each other than the gap
   count as one port in this.
2. **Tracks.** The search grid has a line every 20 px beside the sides of
   the cards. Running within the gap of an earlier line costs 200 times
   its length, more than any detour in the search window, so it happens
   only where the window holds no other route; a wall instead of a price
   would need a second search for that case. The stub end of every port is
   kept free of lines running into it. Nudging is gone.
3. **Order.** Rails first, then the shorter wires first (the longer first
   before): a wire between neighbours has one good route. On the reference
   boards this gives a third fewer crossings than the longer first.
4. **Tidy up makes the room.** Shape D's gaps are 100 px between columns,
   80 px under a row and 24 px under a card. For the lines the board draws when
   Tidy up runs — folded checks and chips leave lines out — each wire is
   routed alone, allowed between two cards at a quarter of the price of
   the way round, and a corridor that `n` of them want gets room for `n`
   lines; then the board is routed as drawn and every place a line still
   runs within the gap widens the gap it lies in by 20 px, until none is
   left. The gap between two columns is the same in every row. A scenario
   opened from code is laid out the same way.
5. **Without Tidy up** nothing moves: lines keep the gap by longer routes,
   and give it up only where cards and the lines already drawn leave no
   route within the search window.

`apiforge_layout::packed` lists the breaches; the `quality` example prints
their number per board, 0 on every reference board.

**Cost.** The track grid makes a search larger. Full routing of the
budget's board (200 cards, 398 wires, laid out in layers 48 px apart, where
most wires have no route that keeps the gap) takes 118 ms on the reference
machine against a budget of 30 ms and 10 ms before; rerouting while
dragging takes 0.6 ms of its 8 ms. The budget for a full routing is
therefore **150 ms** from this amendment on: it runs once when a board
changes, not per frame, and `shop-50` with every wire and check drawn (56
cards, 107 wires) routes in 30 ms. The board wired at random takes 0.9 s
(0.3 s before). The bench prints all three.

### The order of lines in a corridor (LAY-28)

Amended by LAY-28 (2026-11-16). The router gives each line the nearest
free track in the order it routes them, so the lines of a corridor lie in
an order nobody chose, and a line leaving the others crosses the ones in
its way. After every wire has its route and before the bridges are marked,
a pass over the board puts that right (`order::untangle`):

1. **A move** takes one piece of a line that lies between two others to
   another track of its corridor, or lets two pieces of two lines in one
   corridor change tracks. The two pieces it joins get longer or shorter;
   a line keeps its ports, its bends and the side it passes each card on.
   The corridor of a piece is the room between the nearest card on either
   side of it, less the gap; the tracks are the multiples of the gap, up
   to 12 away.
2. **A move is made** when the board has fewer crossings for it and it
   breaks nothing: no breach of the gap (`packed`) that was not there for
   the pieces moved, no line into the mouth of another port. Of a piece's
   moves the one that saves most is taken, then the shortest.
3. **It ends** because every move takes a crossing off the board: rounds
   over the wires in their order until none moves, at most 8, and at most
   60 000 moves weighed, which only a board wired at random reaches. The
   result is deterministic.
4. Lines that stay while others are brought back (`route_among`) are
   counted and not moved. Tidy up's measuring pass routes each wire alone
   and has nothing to order. `Routing::ordered` switches the pass off.

It does not look for another route: a crossing that needs one stays. On
the reference boards it takes 15 to 30 % of the crossings off where every
wire or every check is drawn. **Cost:** the budget's board routes in
132 ms (122 ms before, budget 150 ms), `shop-50` with every wire and check
in 35 ms (31 ms).

### No line under its own card (LAY-29)

Amended by LAY-29 (2026-11-16, the owner's note on #382 and decision A on
its mock-ups). A card is painted over its lines, so a line that begins
where the card draws no socket, or passes under the card, seems to come
out of the card from nowhere.

1. **An ordering wire leaves its socket.** *Next*, *Yes* and *No* are on
   the bottom edge, and every ordering wire starts there, downwards for a
   stub, and goes round its card: to the rail above the row when its
   target stands in the row, as rule 4 of shape D sent it, or down into a
   card hung under it. Rule 4's start on the top edge, above the socket,
   is withdrawn. The top edge of a card only receives. Tidy up makes the
   room the lines under a row take, as for any line (LAY-03).
2. **A wire's own two cards are in its way on every attempt** of the
   search, at the clearance of the attempt. The last attempts, which let a
   wire under a card it joins where nothing else was found, are gone: such
   a wire has no route, is marked incomplete and drawn as the plain corner
   of an enclosed port.
3. **A port inside the wire's other card** — cards lie on each other — is
   left through its stub, long enough to leave that card, exactly as a
   port inside a foreign card is. The guarantee's one exception covers
   both, and `violations` judges both the same.

`apiforge_layout::under_own` lists the wires with a piece inside a card
they join, past such a stub: none on the 24 reference boards and on
generated tidied boards, and none on the generated piles of cards, where
71 of 256 wires had one and 3 now have no route.

**Price**, accepted by the owner on `shop-30` in the default view: 6
crossings for 4, the board 60 px taller, Fit at 1600 × 1000 49 % for
51 %. With every wire drawn `shop-30` has 82 crossings for 75 with its
checks folded and 106 for 105 with them drawn; `shop-50` and `fan-out`
with every wire and check drawn have fewer than before (315 for 340, 79
for 90). **Cost:** none measurable; the budget's board
routes in 133 ms (132 ms before, budget 150 ms).

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
