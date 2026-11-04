# LAY: the diagram comprehension sprint — audit and proposed plan

- State: **proposed** — the owner's decisions of [section 5](#5-owner-decisions)
  (Q1–Q15) are asked in a quiz on the evening of 2026-11-04, after the
  second opinion of [4.4](#44-second-opinion-2026-11-04-and-what-it-changes)
  and round 3 of the mock-ups, and recorded in section 5 afterwards. No
  LAY ticket starts, and no new issue is created, before that.
- Analysis model: Fable. Mock-ups rendered by Opus on the throwaway branches
  `mock/lay-00-shapes`, `mock/lay-00-cards`, `mock/lay-00-shapes-2` and
  `mock/lay-00-round-3` (captures under `docs/qa/2026-11-0{3,4}/LAY-00/`
  on those branches, posted to #37 and #241).
- Inputs: the owner's brief of 2026-11-03 ([section 0](#0-the-owners-brief-2026-11-03));
  [LAY-00](../tickets/LAY-00.md) and LAY-01..06 (#32–#37) with their
  comments; #167, #168, #170 (4), #144, #145, #161 (1), #149; the UX-1 plan
  ([ux-1.md](ux-1.md)) and its tickets in flight; the code on `main` at
  `8248b93` (2026-11-03).

The owner closed the geometry question of 2026-10-26 ("fewer crossings, no
wires on top of each other") into a larger one on 2026-11-03: a person
opening a large scenario must understand, without effort or manual tidying,
what runs after what, which checks verify which results and where data comes
from. Geometry serves that; it is not the goal.

## 0. The owner's brief (2026-11-03)

Given in Polish on 2026-11-03 and translated; this is its record in the
repository. **Goal.** A LAY sprint that radically improves how readable
scenario diagrams are, especially with many requests, checks, data
dependencies and components. The user must quickly see: (1) what runs after
what — the real execution order; (2) what checks what; (3) where data comes
from — producers and consumers; (4) what is running now and what failed;
(5) the whole scenario without reading every detail. API Forge stays a graph
editor; it never becomes a linear list of requests.

1. **Existing tickets** LAY-01..06 are re-audited, not assumed optimal.
   LAY-02 gains: auto-layout runs by itself for a scenario without positions.
   LAY-06's key point: a scenario of a dozen steps must not need a diagram
   thousands of pixels wide.
2. **Three relation types, visually distinct.** (A) execution flow: a visual
   hierarchy, step numbers that follow the engine's semantics, the running
   step highlighted, branches shown clearly; (B) assertion relationships:
   selecting a request highlights its checks, selecting a check shows every
   step and value it depends on; a check with several sources is never forced
   onto one request; (C) data dependencies: selecting a step shows producers
   and consumers; a value can be traced to its source; fewer wires visible at
   once. **Hard rule: no visual may imply a false order or change the
   scenario's semantics.**
3. **Collapse checks, one toggle.** Expanded: checks are separate, selectable,
   editable elements. Collapsed: checks aggregate at their request or logical
   group as "✓ 5 checks" or "✕ 1/5 failed"; failures stay visible; a click
   leads to details; a multi-source check is never hidden or misattributed;
   nothing changes in the code, the order or the structure. Open: aggregate
   as part of the card, a separate element or an overlay; checks that belong
   to no single request; selection, Inspector, editing and navigation while
   collapsed; global, per-document or per-session setting; whether it affects
   Tidy up; whether switching keeps layout and camera stable. One toggle, not
   many modes.
4. **Components stay the abstraction mechanism.** No second system of "check
   groups". Collapse is a presentation detail level; focus mode a temporary
   noise reduction. On a parent board components stay closed. A component
   summary (requests, checks, result) may be considered when reliable. Must
   be consistent with UX-31 (the INPUTS strip) and UX-32.
5. **Semantic zoom.** Low: names, main flow, statuses, check counts; medium:
   key inputs and outputs, short dependencies, summaries; high: fields,
   ports, labels, editors. Thresholds are not approved. Names and statuses
   stay readable, labels never anonymous, the structure never jumps,
   consistent with selection and the Inspector, works at common resolutions.
6. **Focus mode.** On selecting a request or check: direct dependencies
   emphasised, relevant inputs and outputs shown, related checks easy to
   find, unrelated elements dimmed, the full view one step away. "Direct
   only" versus the full chain is to be considered; may be selection
   behaviour rather than a mode.
7. **Readability while running.** The running step highlighted; pending,
   running, passed, failed, skipped follow the engine's real states; failed
   checks stay visible when aggregated; a result navigates to its card; the
   current run kept apart from earlier ones; works with Run simulation and
   Run live. Complements Run results, does not duplicate it.
8. **Shapes and routing: comparable mock-ups on the same scenarios.** (A)
   horizontal with a separate ordering bus, (B) wrapped rows of bounded
   width, (C) vertical, (D) hybrid. Test cases: 5, 15, 30 and 50+ steps; many
   checks per request; dependencies that skip steps; components and
   variables; branches; many data dependencies; scenarios from code with no
   positions. Comprehension before crossing count.
9. **Metrics and QA.** Extend LAY-01: technical (crossings, bends, overlaps,
   wire length, collisions, board size, effective zoom) and UX (is the order
   unambiguous; time to find all checks of a request; time to find a value's
   source; is a failed check visible while collapsed; understood without
   constant zooming and panning; deterministic and stable; do view switches
   disorient). Screenshots and geometry alone do not prove good UX.
10. **Working IDs**, to be checked against existing numbers; Fable merges,
    splits and defers: LAY-07 execution-flow hierarchy; LAY-08 check ↔ source
    relationships; LAY-09 semantic zoom; LAY-10 focus mode; LAY-11 collapse
    checks; LAY-12 run status on the diagram.
11. **Constraints.** No DSL or engine change to simplify visuals; no
    persistent project-format data without a real need; components not
    duplicated; aggregation never hides a failure; a check never assigned by
    proximity; order readability never traded for fewer crossings; not only
    for small scenarios; compatible with code editing, the Inspector,
    components, MCP and live preview.
12. **Expected output, no implementation first:** an audit; a UX model for
    the three relations; mock-ups of the alternatives on large realistic
    scenarios; owner decisions with variants, trade-offs and a
    recommendation; must / should / deferred; dependencies and order — do not
    build routing for a layout that is later rejected; small independently
    verifiable tickets; a demo plan on reference scenarios. Decisions are put
    to the owner as quizzes before tickets are created; not everything fits
    one sprint; the owner decides LAY-06 during the sprint.

## 1. Audit — what `main` does today

Read at `8248b93`; file references are to `crates/apiforge-desktop/src/`
unless said otherwise.

### 1.1 What works

- **The engine's order is static and knowable.** Steps run one at a time; a
  step is ready when every step it depends on (data references and the
  ordering edges of `@after` and `if` branches) has finished; among ready
  steps document order decides (`apiforge-runner/src/engine.rs:4–6`,
  `plan.rs`). The same rule computed before a run gives the order the run
  will take; a blocked or skipped step still "finishes" at its turn. Step
  numbers that follow the engine are therefore a derivation, not a guess,
  and they equal the row order of Run results.
- **Two wire kinds already exist.** `graph::wires` emits data wires from
  bindings and ordering wires only for explicit edges (`@after`, branch
  entry); an order implied by data use is not drawn twice
  (`apiforge-core/src/graph.rs:466`). Ordering wires are dashed.
- **Components are closed cards** with an open button; the Inspector and Run
  results nest their steps. UX-31/32 added the INPUTS strip whose pills ring
  the steps that read them (`canvas/mod.rs:538–549`) — the ring is the
  pattern focus and check highlighting should reuse.
- **The router keeps its guarantees** (ADR 0006): final routes never cross a
  card; 200 cards route in 20 ms. The crate is pure geometry and testable
  without a window.
- **Run results update during a run**: the same run's report is replaced as
  steps finish (`folder/runs.rs:115`), so the board already receives partial
  `steps` while running.
- **Camera discipline** from UX-18: the Inspector no longer re-fits the
  board; semantic zoom must respect the same rule (the camera moves only
  for the person).

### 1.2 What is wrong

| # | Finding | Where | Consequence for the person |
| --- | --- | --- | --- |
| W1 | **A check is a full card**, as heavy as a request: 280 px wide, header, predicate subtitle, two input rows, two output rows, footer, control row (`canvas/layout.rs:280–330`). | cards | A scenario with 10 requests and 20 checks is 30 equal boxes; "which checks apply to which block" is read by following wires. The owner's finding on #37 (2026-10-29). |
| W2 | **No execution order is shown** except through wires. No step number; ordering wires only for explicit `@after`; nothing marks that two cards in one column run in document order. | board | A person cannot say what runs after what without the code. Brief (1). |
| W3 | **Status disappears with zoom.** Full detail (≥ 75 %) shows "Passed · 12 ms" in the footer; Compact (45–75 %) a 2 px dot; Overview (< 45 %) nothing. A failed card keeps its green check icon and grey border (`canvas/paint.rs:333–338, 547–589`). | paint | Fit view of a 14-step scenario at 1600 × 1000 lands at 31 %: no status at all. Brief (4), #161 (1). |
| W4 | **Selection emphasises nothing but itself.** A selected card gets an accent border; related cards are ringed only for a selected proxy chip or strip input; no dimming, no producer/consumer emphasis, no check ↔ subject emphasis (`canvas/mod.rs:2133`). | board | Tracing a value or finding a request's checks is done by eye along wires. Brief (2), (6). |
| W5 | **Tidy up spreads a scenario into one long row** — 14 steps into 3600 px, every check and value in its own column (LAY-00 finding 6, [capture](../qa/2026-10-26/LAY/layout-flow-tidied.png)). | `apiforge-layout` | The whole is seen only at 31 %, where nothing is readable (W3). LAY-06. |
| W6 | **A scenario opened from code stacks in one column**; no automatic layout (LAY-00 finding 1; #170 (4)). | `layered::place` | The first view of a generated or MCP-written scenario is unusable; the owner wants auto-layout without Tidy up (#33, 2026-10-29). |
| W7 | **Ordering wires fight the horizontal flow**: bottom-centre to top-centre, four bends, crossing the data wires between the same cards by construction (LAY-00 finding 6; #167 (2)). | router, ports | Every `@after` and every branch edge is the ugliest wire on the board. |
| W8 | **An `if` draws one wire to every step of a branch** (#167 (1)): 8 steps in *Yes* = 8 parallel dashed wires. | `graph.edges` as drawn | A ribbon cable that says nothing a band would not say better. |
| W9 | **Wire geometry defects** of LAY-00: no wire straight (finding 2), coincident bundles (3, 4), long wires taking the inner lanes (5). | router, `coordinates` | Crossings and overlaps that make tracing harder. LAY-03/04/05 stand, but their value depends on how many wires remain after W1 and W8 are addressed. |
| W10 | **Proxy chips are stranded by Tidy up** and distort Fit view (#168 (1–3)). | `board_edit.rs:79` | A tidied board keeps old chips 2000 px away with long wires. |
| W11 | **A value used everywhere is a long wire everywhere.** `login.body.token` feeds five requests in `order-flow.afs`; each is a long wire across the board. | wires | The most common shop pattern (auth token, base id) is the main source of long wires; chips exist for variables and parameters, not for a step's output. |
| W12 | **No description beside a block** (DIAG-01, #144) and the check's subtitle is the predicate only (DIAG-02, #145). | cards | A component call says its name; a check says "is approximately". |

### 1.3 Facts about the model that the design must respect

- A check references its sources through its input bindings; the *subjects*
  of a check are the Request and Component nodes it references **directly**.
  `cart_total = assert cart.body.total ~= total` has one subject (`cart`) and
  one value source (`total`); `receipt_total = assert receipt.body.total ==
  cart.body.total` has two subjects. A check that reads only values or
  parameters has no subject. This is computable in `apiforge-core` from
  `Binding::references()`; nothing about it is geometric.
- `condition all/any` blocks its consumers when it does not hold; it is a
  check with a gate. It aggregates like an `assert` and its blocked
  consumers show *Blocked*.
- Positions live in the project's `diagram.json`, not in `.afs`; a layout
  that only renders differently (badges, aggregates, bands, dimming) writes
  nothing. Collapse state, like wire style and bridges, belongs in the
  application preferences (`preferences.rs`), not in the document.
- The desktop never passes port offsets to the layout crate and the crate
  never reads them (LAY-00); any shape work touches `board_edit.rs:293`
  `layout_edges` and `layered::coordinates`.

### 1.4 Tickets, overlaps and conflicts

| Ticket or issue | Verdict |
| --- | --- |
| LAY-01 #32 metrics | Stands; extended with board size, fit zoom at both window sizes, and the UX task sheet of [section 9](#9-metrics-and-qa). Gains the reference scenarios. |
| LAY-02 #33 layers on open | Stands; gains the 2026-10-29 requirement (auto-layout on open, decision Q10) and the chips of #168 (1–3). |
| LAY-03 #34 nudge, LAY-04 #35 ports, LAY-05 #36 lanes | Stand as routing tickets, **after** the shape and collapse decisions: with checks collapsed and branches banded the wire count of a 30-step board roughly halves, and LAY-04's row rule depends on the shape (D hangs satellites under a spine; rows are the spine's). Measured again on the reference scenarios before scheduling. |
| LAY-06 #37 shape | Re-opened with four options on the same scenarios (brief 8); C is mocked up this time by the brief's list, with the 2026-10-26 caveat (reading direction, ADR 0006). Decision Q1. |
| #167 IF wires and ports | (1) becomes LAY-13 branch bands; (2) the port side is part of the shape decision (a rail makes the bottom/top ports harmless); (3) arrowheads: deferred, badges and focus make direction readable. |
| #168 (1–3) chips | Into LAY-02 (placement near readers; Tidy up moves chips; Fit view sane). (4) is UX-17, merged. |
| #170 (4) | Duplicate of LAY-02 (already recorded). |
| DIAG-00 #241 (UX-1 wave 4) | **Conflict.** Its questions — detail by zoom, the failed state, readable checks — are LAY-09, LAY-11 and LAY-08; rendering them twice would decide the same thing twice. Proposed: DIAG-00 moves into LAY with the card mock-up of this analysis; DIAG-01 (descriptions, a `format` question) stays its own decision, deferred (Q8). |
| DIAG-02 #145 | Answered by LAY-08 (the Inspector's check list names subject and predicate in words) and LAY-11; the check card's subtitle gains the subject (`cart.total ≈ total`) in LAY-09. |
| #161 (1), #170 (2) | LAY-09. |
| UX-19 #227 (Run results navigation, in flight) | LAY-07's numbers equal its row order; LAY-11 must answer "navigate to a collapsed check" (Q4); nothing else overlaps. |
| MCP-13 #149 auto-layout via MCP | After LAY-02 a scenario written by an agent lays itself out when opened, which is most of the need; a `layout` tool stays deferred. |
| UX-18 (camera), UX-28 (Curves removed), UX-31/32 (strip) | Constraints, not overlaps: no re-fit on toggles; orthogonal wires only; the ring pattern. |

## 2. The UX model: three relations, three looks

One board, three ways to read it, each with its own visual vocabulary so
that none can be mistaken for another:

| Relation | Question | Vocabulary | Interaction |
| --- | --- | --- | --- |
| **A. Execution** | what runs after what | a **sequence badge** on every step (its engine order, equal to its Run results row); the running step's badge filled and its border accent; dashed ordering wires only for explicit `@after` and branch entry, on a rail, never through the gaps; a **band** behind each branch of an `if`, labelled *Yes* / *No* | none needed: it is readable at rest, at every zoom |
| **B. Checks** | what checks what | a check is a **satellite** of its subject (placed under it by Tidy up; collapsible into an **aggregate** pill `✓ 5 checks` / `✕ 1/5 failed` in the subject's footer); its wires are thin, in the check colour | select a request → its checks ring and the aggregate turns accent; select a check → its subjects and value sources ring; the Inspector lists a request's checks in words with their sources |
| **C. Data** | where a value comes from | solid data wires from output to input ports as today; proxy chips for variables and parameters | select a step → direct producers and consumers ring, their wires at full strength, everything else dimmed; Esc or a click on the board restores the full view; the Inspector's source rows (`Add to cart → cart.total`) are links that select the producer |

What stays out of the vocabulary on purpose: no second grouping element
(components remain the only abstraction); no colour that means "order"
(order is the number); no attribution of a check by nearness (the subject
rule of 1.3 decides, and a check with no subject stays a card).

## 3. Proposals the mock-ups render

Each is one ticket in [section 8](#8-proposed-tickets); the captures of
[section 4](#4-mock-ups) show them on `order-flow.afs` and the 30-step
reference scenario.

- **P1 Sequence badges (LAY-07).** A 20 px circle at the card's top-left
  with the engine order; the running step's badge filled, accent border,
  "Running…" in the footer; pending steps muted. Numbers are recomputed on
  every edit (pure function of the graph) and shown at every detail level.
- **P2 Collapse checks (LAY-11).** A toolbar toggle *Checks* (expanded /
  collapsed). Collapsed: check cards with a subject and their wires are not
  drawn; each subject shows the aggregate in its footer as a second
  statement after the request's own result (`HTTP 201 · ✓ 2 checks`,
  `HTTP 200 · ✕ 1/3 failed · 1 skipped`, a neutral grey `3 checks` before a
  run, never a tick for a check that did not run); a click on the aggregate
  opens the Inspector's *Checks* list for that step (predicate in words,
  sources, status) with *Expand checks here* (a per-step, session-only
  override). Standalone checks stay cards.
  Navigation from Run results to a collapsed check selects its subject and
  opens the list. Tidy up in collapsed mode lays out the visible cards and
  hangs the hidden checks under their subject, so expanding never needs a
  second tidy. The toggle never moves the camera.
- **P3 Semantic zoom (LAY-09).** The thresholds are decided after round 3
  (Q7); the content changes. Overview: status band, badge, the name drawn at
  a minimum on-screen size (11 px whatever the zoom, clipped to the card),
  aggregate pill. Compact: plus method and short path, the result line and
  the ports in use (Q13); a failed or blocked card has a 2 px red or amber
  border at every level. Full: as today plus the badge; a check's subtitle
  names its subject and predicate (`cart.total ≈ total`). Zooming never
  moves a card: the card's box is its rows, the same at every zoom.
- **P4 Focus on selection (LAY-10).** Direct producers and consumers ring;
  unrelated cards dim to about 60 % (names and statuses stay legible) and
  unrelated wires to about 25 %; a selected request also rings its checks; a
  selected check rings its subjects and value sources in the check colour.
  Four signals are told apart without reading the footer: the selection
  (strong accent frame and handles), the related cards (a lighter outline in
  another hue), the running step (an activity glyph in its badge and
  *Running…*), a failure (red mark and status, independent of the
  selection). No mode, no toggle; the full chain is reached by clicking
  along it (Q6).
- **P5 Run status (LAY-12).** P1 and P3 cover the states; LAY-12 is the
  acceptance: simulation and live, the states follow the engine, a result
  row navigates to its card (UX-19) or aggregate (P2), stale results are
  told apart by UX-25's bar.
- **P6 Shape (LAY-06).** Four captures per scenario: A horizontal with a
  rail, B wrapped rows, C vertical, D hybrid — a spine of Start, requests,
  component calls and `if` in engine order, satellites (checks, values)
  hanging under the step they read, wrapping after five columns at
  1600 × 1000, the rail for `@after`. D is the recommendation: it is the
  only shape where the order is the reading direction, the checks sit by
  their subject and the board is bounded in width, at the cost of long data
  wires from satellites read later (the token, W11).
- **P7 Branch bands (LAY-13).** A tinted band behind the cards of each branch
  with one dashed orthogonal wire from the `if` into the band (rendering
  only; edges unchanged). The band's tint is neutral, not a status colour;
  its header names the condition and the branch (`created → Yes`); a branch
  that wraps gets a band on each row, joined by the continuation mark.
- **P9 Wire tracing (LAY-14).** The hovered or selected wire is drawn at
  full strength with both end ports ringed and its field label
  (`login.body.token`) on a pill at its midpoint; the other wires dim. A
  router clearance keeps every wire 12 px from the borders of cards it does
  not end on.
- **P8 Auto-layout on open and chips (LAY-02).** A scenario without
  positions is laid out when opened; chips are placed beside their first
  reader and move with Tidy up.

## 4. Mock-ups

Rendered on 2026-11-04 by Opus on two throwaway branches and posted with
their raw URLs: the shapes on #37 (`mock/lay-00-shapes` at `40adfad`,
`docs/qa/2026-11-03/LAY-00/lay00-<a|b|c|d>-<scenario>-<size>.png`, plus a
crop per shape), the cards on #241 (`mock/lay-00-cards` at `e1a73ab`,
`lay00-collapse-*`, `lay00-zoom-<35|60|90>-<today|proposed|proposed-collapsed>`,
`lay00-focus-*`, `lay00-running-*`, `lay00-branch-band-*`). The reference
scenarios `order-flow` (14 steps), `shop-30` (30) and `shop-50` (53) are on
the shapes branch under `fixtures/scenarios/lay/`; LAY-01 takes them over.

### 4.1 Shapes, measured

| Board width × height, fit zoom at 1600 × 1000 | order-flow | shop-30 | shop-50 |
| --- | --- | --- | --- |
| A horizontal, rail | 3676 × 968, 0.31 | 5180 × 1720, 0.22 | — |
| B wrapped rows | 1420 × 2668, 0.29 | 1420 × 5367, 0.14 | 1420 × 8799, 0.09 |
| C vertical | 668 × 4312, 0.18 | 1420 × 6684, 0.11 | — |
| D spine and satellites | 1806 × 2232, 0.34 | 1826 × 4132, 0.18 | 1876 × 6840, 0.11 |

At 960 × 540 every shape fits between 0.05 and 0.13. What the captures say:

- In A the reading direction is the dependency layer, not the order: step 9
  (`product`, which depends on nothing) sits left of step 5. Badges make the
  contradiction visible; without them A reads as a false order (brief, hard
  rule). B and C are tall boards where the order zigzags between rows.
- D is the only shape where the order is the reading direction and every
  check sits under the step it reads; its rows are the spine's wrapped
  rows. Its cost: data wires from a satellite read later run across rows,
  and the rails of a branch stack into a band under a row.
- **No shape is readable at Fit view of 30 steps.** Width is not the
  limiting dimension once rows wrap; the card's height (every data row of a
  request) is. Readability at Fit comes from fewer cards (collapsed checks)
  and from what a card shows at Overview (semantic zoom), not from the
  shape. The shape decides whether the order and the subjects can be read
  once the person zooms to 35–60 %.
- The router's defects (LAY-00 findings 2–5) show in every shape: parallel
  token wires along a row in A, a shared gutter line in B, long side lines
  in C, satellite wires overlapping spine wires in D. They are LAY-03/04/05's
  job after the shape is chosen, on the shape's geometry.

### 4.2 Cards, what was learnt

- Collapsed, `order-flow` drops from 14 cards to 9; the aggregates read
  `✓ 1 check`, `✓ 2 checks`, `✕ 1/2 failed`; a two-subject check
  (`receipt_total` reads `cart.body.total`) is counted under both `cart`
  and `receipt`, as Q6 = A says.
- At 60 % the proposed Compact card reads status, subtitle and badge; the
  failed card's red border is visible at 35 %.
- Focus: the ring (UX-31's VARIABLE colour) and the selection border
  (ACCENT) are both orange at a glance; LAY-10 picks a ring colour that
  reads apart from the selection.
- **A step in progress has no result today**: the runner publishes after a
  step finishes, so the board shows the running step as *Ready*. LAY-07's
  running state needs the runner's progress to name the step that started
  (a `Running` status or a "started" event), the one engine-side change in
  the sprint; it changes no semantics.
- A possible defect found on the way, filed as #288: selecting a request
  with the Inspector open marks a saved document as changed and stales its
  results.

### 4.3 Round 2: D and B with checks collapsed, real routing

Round 1 compared the shapes at Fit view with hand-drawn rails — not what a
person looks at. Round 2 (`mock/lay-00-shapes-2` at `f103710`,
`docs/qa/2026-11-04/LAY-00/lay00r2-*`, posted on #37) renders D and B with
checks collapsed, every wire by the real router, badges capped at 14 px,
the camera on rows 1–2 at 60 % and 35 %.

| shop-30 | board w × h | fit zoom |
| --- | --- | --- |
| D expanded | 1792 × 4100 | 0.19 |
| D collapsed | 1792 × 2736 | 0.28 |
| B expanded | 1416 × 5363 | 0.14 |
| B collapsed | 1416 × 3280 | 0.23 |

Collapsing removes 11 check cards and 24 wires from shop-30 and makes the
board a third shorter. At 35 % D's spine reads in order with an aggregate
on every request; B's order zigzags between rows (1, 4, 2, 30, 5 …). The
router's trunk above row 2 (about eight overlapping wires, about eight
crossings) and the bottom-to-top ordering wires are what LAY-03/04/05 fix
on D's geometry. Rule learnt for LAY-11: an ordering dependency on a hidden
check (`@after(cart_total)` on `order`) is drawn from the check's subject
while collapsed, never dropped.

### 4.4 Second opinion (2026-11-04) and what it changes

A second reviewer read the plan and every capture of the three branches.
Verdict: D as the direction, no branch as a finished pattern; collapsing
checks and the Inspector list are the strongest parts; the board's height,
the routing and the consistency of the marks are the weakest. The captures
confirm each finding (the Fit view of D collapsed at 0.28 shows neither
numbers nor aggregates; `✓ 1 check` appears before any run; a value card is
an almost empty box as tall as a request; wires hug `catalogue`'s border).
What the plan adopts, and where:

1. **D's rows are as tall as their tallest satellite stack** (`first`,
   `second`, `total` under `products` push the next row down; empty space
   under `begin`, `login`, `order`); satellites carry no number in the
   captures. → LAY-06: a value card is a header and one row, an expanded
   check likewise short; a continuation mark at the end of each row
   (`↳ 10`) and its mirror at the start of the next; five columns a layout
   parameter, not a rule. LAY-07: every step numbered, satellites included
   (the omission was the mock-ups', which drew badges by hand in round 1 and
   on spine steps only in round 2).
2. **At 60 % collapsed, row 1 and two value cards fill the viewport**, and a
   request shows unused outputs, *Optional* and separators. Hiding text in a
   box that keeps its height gains nothing: a card's box is its rows
   (`Card::height`), the same at every zoom. → Q13 (what Compact shows) and
   Q14 (whether a board-level density setting may shrink the box, repacked
   by Tidy up; zooming itself never moves a card).
3. **The routing baseline is not optional**: wires along card borders and a
   trunk above row 2 whose ends cannot be told apart defeat relation C
   whatever the shape. → LAY-04 moves to Must; a new LAY-14 (wire tracing
   and the 12 px clearance, P9) is Must; LAY-03 and LAY-05 stay Should,
   after the shape. Q12's figures updated.
4. **Collapsed checks: the idea approved, the semantics corrected.** Before
   a run the aggregate is neutral (`3 checks`, no tick); the request's
   result and the checks' result are two statements (`HTTP 200 · ✕ 1/3
   failed · 1 skipped`); a failed check marks its subject at every zoom
   while the status band keeps the HTTP result (Q15 asks how loud); a check
   with two subjects reads `shared` in each aggregate and "also checks
   receipt" in the list, never as two assertions; an `@after` on a hidden
   check is labelled with the check's name on the wire. → LAY-11's criteria
   and P2.
5. **Semantic zoom unproven**: round 2 rendered today's Overview (nothing
   below 0.45), so the integrated promise "names, order and checks at every
   zoom" has not been shown, and 0.45 / 0.75 cannot be approved from these
   captures. → Q7 deferred to round 3; LAY-09 gains a minimum on-screen
   label size (11 px) so that the promise is a rule, not a hope.
6. **Focus and running share one orange** with the selection. → LAY-10's
   four signals (P4); LAY-07's capture shows a running step other than the
   selected one; dimming keeps names and statuses legible.
7. **Branch bands: the concept approved, the sample too easy.** *Yes/No* too
   small and stuck to the entry, diagonal wires, green and orange competing
   with the status colours, no wrapped or nested branch. → LAY-13 (P7) and a
   two-row band in round 3 before the ticket is written.

On substance the plan and the second opinion do not disagree. One
correction of emphasis: the reviewer reads D's remaining cost (satellite
height, row continuation) as a condition on the shape, and the plan agrees
that D is approved only with the short satellites of item 1.

**Round 3** (commissioned 2026-11-04 on `mock/lay-00-round-3`, posted on
#37) renders the one coherent D the reviewer asked for, on `shop-30`:
collapsed after a run with one failed and one skipped check, a shared
check, the running step other than the selected one, a labelled `@after`
on a hidden check, the hovered wire traced, short satellites with the
continuation mark, the proposed Overview and Compact at Fit, 35 % and
60 %, a branch band across two rows, and `shop-50` at 960 × 540. Its table
extends 4.3 and answers Q7, Q13–Q15 with evidence.

## 5. Owner decisions

Asked 2026-11-04; the recommended option is first.

**Shape and order**

1. **The shape Tidy up produces (LAY-06).** A. **D hybrid**: a spine in
   engine order with satellites under their subject, wrapped after five
   columns. B. **B wrapped rows** of the dependency layers. C. **A
   horizontal** with the rail (bounded only by zoom). D. **C vertical** (the
   reading direction changes; ADR 0006 amended).
2. **Sequence badges (LAY-07).** A. On every step, the engine's order, equal
   to the Run results row; checks included. B. On spine steps only (requests,
   components, `if`); checks unnumbered. C. None: order is read from the
   shape and the wires.
3. **Explicit `@after` wires once badges exist.** A. Drawn, dashed, on the
   rail: they say "waits for" without data. B. Hidden until the source or
   target is selected.

**Checks**

4. **Collapse checks (LAY-11) — default and scope.** A. Collapsed by default,
   an application preference like wire style; *Expand checks here* on a step
   is session-only. B. Expanded by default, the preference remembered once
   toggled. C. Per project.
5. **Where the aggregate sits.** A. In the subject card's footer, where the
   status text is; a pill under the name at Overview. B. A separate small
   tile beside the subject. C. A corner badge over the card.
6. **A check with several subjects.** A. Counted under every subject it
   reads directly; the Inspector list shows it under each with "also checks
   receipt". B. Counted under its latest subject in engine order only, with a
   "+ shared" mark.

**Reading the board**

7. **Semantic zoom thresholds (LAY-09).** A. Decide after round 3 shows D
   with collapsed checks and the proposed Overview at 0.28, 0.35 and 0.60;
   until then the thresholds are a parameter. B. Keep 0.45 / 0.75 now and
   change what each level shows. C. Lower Full to 0.60 (#161's suggestion);
   cards hold less at 60 % and inline fields shrink.
8. **DIAG-00 (#241).** A. Moves out of UX-1 into LAY: its questions are 4, 5,
   7 and the card captures; DIAG-01 (descriptions beside blocks, a `format`
   decision) is deferred to its own analysis after LAY. B. Stays in UX-1
   wave 4 as a separate mock-up.
9. **Focus (LAY-10).** A. Selection behaviour only: direct producers,
   consumers and checks ring, the rest dims; Esc clears. B. A *Focus* toggle
   in the toolbar that dims by the full upstream and downstream chain. C.
   Both: A now, B later as a should ticket.
10. **Auto-layout (LAY-02).** A. A scenario whose steps have no positions is
    laid out when opened and the positions are kept until the next save
    writes `diagram.json`, as any edit would; steps added in code keep
    today's incremental placement. B. Also re-lay the board whenever a code
    edit adds three or more steps at once.
11. **Branches (LAY-13).** A. One band per branch, one wire into the band.
    B. One wire per step as today, routed on the rail.

**Scope**

12. **Capacity.** A. Must and should: fifteen tickets, about 255 owner
    review minutes ([section 6](#6-scope-must-should-deferred)). B. Must
    only: ten tickets, 180 minutes; nudge, lanes and bands follow as LAY-2.
    C. Everything including the deferred items.

**After the second opinion** (4.4)

13. **What Compact shows (LAY-09).** A. Name, method and short path, the
    result line with the aggregate, and only the ports in use; unused
    outputs, *Optional* and separators appear at Full. B. Every port as
    today; only the text shrinks.
14. **Card height and density.** A. A board-level *Compact cards* setting
    (an application preference beside the *Checks* toggle) hides the unused
    rows and shrinks the card's box; Tidy up repacks the rows; zooming never
    moves a card. B. The box stays as today, every row, whatever is shown;
    only Tidy up's spacing changes.
15. **A failed check on a successful request (LAY-11).** A. The subject's
    footer turns red (`HTTP 200 · ✕ 1/3 failed`) and the card gets the 2 px
    red border at every zoom; the status band keeps the HTTP colour; Run
    results keeps the check's own row. B. Only the aggregate's text turns
    red; the card is otherwise unchanged (round 1's capture).

## 6. Scope: must, should, deferred

| Class | Tickets | Minutes |
| --- | --- | --- |
| **Must** — the comprehension goal | LAY-01 metrics and reference scenarios · LAY-08 check subjects and the Inspector list · LAY-02 auto-layout and chips · LAY-07 badges and running · LAY-09 semantic zoom · LAY-11 collapse checks · LAY-10 focus · LAY-14 wire tracing and clearance · LAY-06 the shape · LAY-04 ports decide rows | 10 + 15 + 15 + 15 + 15 + 30 + 15 + 15 + 30 + 20 = **180** |
| **Should** — geometry after the shape | LAY-13 branch bands · LAY-03 nudge · LAY-05 lanes (measured first) · LAY-12 acceptance and demo | 20 + 15 + 15 + 10 = **60** |
| **Deferred** | DIAG-01 descriptions (`format`); C vertical if not chosen; fan-out "taps" for a value read by many (W11); a component summary on the parent board; MCP-13 layout tool; arrowheads (#167 (3)); wire length and collision metrics beyond LAY-01 | — |

## 7. Dependencies and order

Nothing that depends on a quiz answer starts before the quiz; nothing that
routes wires starts before the shape and collapse are decided and built.

| Wave | Tickets | Why this order |
| --- | --- | --- |
| 0 (may start now, no decision) | LAY-01 (reference scenarios, the metric command), LAY-08 (the subject function in core and the Inspector's check list; no board change) | Pure functions and fixtures every later ticket uses |
| 1 (after the quiz) | LAY-07, LAY-09, LAY-11, LAY-10, LAY-14 — rendering only, positions untouched | Five independent S/M tickets on different parts of the card and board; none moves a card |
| 2 | LAY-02, LAY-06, LAY-13 | Positions change: the shape, the first placement, the bands |
| 3 | LAY-04 (must), LAY-03, then LAY-05 after measuring | Routing on the board as it will be; LAY-05 may be unnecessary |
| 4 | LAY-12 | The demo on the reference scenarios |

Interplay to name: LAY-11 and LAY-06 share the satellite rule (a hidden
check hangs under its subject); LAY-07 and UX-19 share the row order; LAY-10
and UX-31 share the ring.

## 8. Proposed tickets

Created as issues after the quiz, with the decisions recorded in
`docs/tickets/<ID>.md`. Criteria are sketches; each ticket's file numbers
them and names the test.

| ID | Result | Size · min · risk | Criteria sketch |
| --- | --- | --- | --- |
| LAY-01 | `fixtures/scenarios/lay/` (14, 30, 50 steps) parse and simulate; `cargo run -p apiforge-layout --example quality` prints the LAY-01 table plus board width and fit zoom at both sizes; the UX task sheet exists | S · 10 · — | deterministic; the dev guide names it |
| LAY-08 | `apiforge_core::graph::check_subjects`; the Inspector of a request lists its checks in words with sources and status; a check's subtitle names subject and predicate | S · 15 · visible UI | two-subject check listed under both; standalone check listed nowhere |
| LAY-02 | Auto-layout on open (Q10); chips placed beside their first reader and moved by Tidy up; `place` with every block fresh gives layers | S–M · 15 · default | `order-flow.afs` opens in layers; Fit view within 10 % of the cards' box |
| LAY-07 | Sequence badges on every step, satellites included (Q2); running step's badge glyph and *Running…*; equality with Run results row order | S · 15 · visible UI | the badge of step *k* equals its row; a running simulation shows the running step within one frame of its start; the capture shows a running step other than the selected one |
| LAY-09 | Overview and Compact contents (P3, Q7, Q13); a minimum on-screen label size; failed and blocked borders at every zoom | S · 15 · visible UI | a failed card is red at 0.35; a name is at least 11 px on screen at 0.25; zooming moves no card |
| LAY-11 | The *Checks* toggle, aggregates as a second statement, Inspector list, *Expand checks here*, navigation, Tidy up with hidden checks, the preference (Q4–Q6, Q15) | M · 30 · visible UI, default | a failed check is visible collapsed at every zoom; no tick before a run; toggling moves no card and no camera; two-subject check counted per Q6 and marked `shared`; an `@after` on a hidden check is drawn from its subject with the check's name; Run results row of a collapsed check selects its subject |
| LAY-10 | Focus on selection (Q9) with the four signals of P4 | S · 15 · visible UI, default | selecting `cart` rings login, first, second, cart_total, order in a hue other than the selection; names stay legible on dimmed cards; Esc restores |
| LAY-14 | Wire tracing and clearance (P9) | S · 15 · visible UI | hovering `login → cart` shows `login.body.token` and rings both ports; no routed segment within 12 px of a card it does not end on (LAY-01's metric) |
| LAY-06 | The chosen shape in Tidy up (Q1, Q3) with short satellites and the row continuation mark; the column count a parameter; ADR 0006 amended | M · 30 · default | 30-step board ≤ 1800 px wide at 1600 × 1000; a row no taller than its tallest short stack; fit zoom ≥ 0.45 for 14 steps; no order wire through a gap |
| LAY-13 | Branch bands (Q11): neutral tint, header `created → Yes`, orthogonal entry, a band per row for a wrapped branch | M · 20 · visible UI | one wire per branch; a band contains exactly the branch's steps; a wrapped branch has one band per row |
| LAY-04 | Ports decide rows | M · 20 · — | as LAY-04 today, measured on the reference scenarios |
| LAY-03 | Nudge | S · 15 · — | as LAY-03 today |
| LAY-05 | Lanes | M · 15 · — | scheduled only if crossings per wire after LAY-04 exceed 0.2 on `shop-30` |
| LAY-12 | The demo: the three reference scenarios opened cold, read, run (simulation and live loopback), a failure found; the UX task sheet timed | S · 10 · — | the demo plan of [section 10](#10-demo-plan) runs end to end |

## 9. Metrics and QA

**Technical** (LAY-01, per reference scenario, before and after each
ticket): wires, crossings, bends, straight wires, close and coincident pairs,
card and chip collisions, board width and height, fit zoom at 1600 × 1000
and 960 × 540, long edges. Printed by the quality example on generated
boards and on the three fixtures.

**UX task sheet** (manual, the owner or an agent at the window, on
`shop-30.afs` after a failed simulation; the time and the number of
zoom/pan actions are recorded in the LAY-12 pull request, once before the
sprint and once at its end):

1. Say in what order steps 8–12 run. *(Unambiguous? — LAY-07)*
2. Name every check of the `checkout` request. *(seconds — LAY-08, LAY-11)*
3. Say where `order.body.id` used by `receipt` comes from. *(seconds — LAY-10)*
4. With checks collapsed, point at the failed check. *(visible? — LAY-11)*
5. Describe the scenario in three sentences at Fit view without zooming.
   *(LAY-09, LAY-06)*
6. Open the scenario twice; switch Code → Diagram → Split. *(deterministic;
   disorienting? — LAY-02, UX-18)*
7. Say which field of `login` feeds `payment`, by pointing at the wire.
   *(seconds — LAY-14)*

**Deterministic tests** cover what can be asserted: the badge equals the row
(LAY-07); a failed check visible while collapsed (LAY-11); no card moved by a
toggle (LAY-11); the same positions from two opens (LAY-02); dimming never
hides the selection (LAY-10).

## 10. Demo plan

On the reference scenarios, in the demo shop project, at 1600 × 1000 and
960 × 540:

1. Open `shop-30.afs` from code with no positions: it lays itself out
   (LAY-02) in the chosen shape (LAY-06); the badges read 1…30 (LAY-07).
2. At Fit view, say the flow from the status bands and names (LAY-09).
3. Collapse checks; the board halves its cards; aggregates read `✓ n checks`
   (LAY-11).
4. Run the simulation with a broken sample: the running badge walks the
   board; `checkout` turns `✕ 1/3 failed`; the Run results row selects the
   subject and lists the check (LAY-07, LAY-11, UX-19).
5. Select `checkout`: its checks and its producers ring, the rest dims; select
   the failed check: its two subjects ring (LAY-10, LAY-08).
6. Expand checks here, edit the check in the Inspector, re-run: green.
7. `shop-50.afs`: the same at 960 × 540, with the branch bands (LAY-13).

## Technical appendix

- Subjects: `Binding::references()` over `node.inputs`, filtered by node
  kind; in `apiforge-core` beside `dependencies`, so MCP's check tool can
  reuse it later.
- Sequence: a static scheduler in `apiforge-core` (or exposed by the runner's
  `plan`) that replays the engine's rule without executing; the runner's own
  loop (`engine.rs:518–560`) is the reference, and a test asserts both agree
  on the fixtures.
- Collapse: a `Vec<StepResult>`-independent filter over cards and wires in
  `canvas/mod.rs` before routing (hidden cards are not obstacles either);
  Tidy up in collapsed mode passes the visible set to the layout and places
  hidden checks under their subject with `layered::place`.
- Dimming: a second pass over `paint::card` and `paint::routed_wire` with an
  alpha; no geometry change.
- Shape D: `layered` gains a spine mode — layers by engine order of spine
  nodes, satellites assigned to their latest subject's column and stacked
  below; wrapping is a coordinate step after the rows are known; the rail is
  a router preference for wires whose both ports are vertical.
- Throwaway code of the mock-up branches is not merged; the captures the
  owner chooses are committed under `docs/qa/<date>/<ticket>/` with the
  ticket.
