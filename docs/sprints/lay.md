# LAY: the diagram comprehension sprint — audit and proposed plan

- State: **decided** — the owner answered Q1–Q16 in a quiz on 2026-11-05
  after seeing round 3 and the four proofs of round 4
  ([4.8](#48-round-4-the-four-proofs-measured)); the answers are in
  [5.1](#51-decisions-taken-2026-11-05). The sprint is LAY in the
  [backlog](../backlog.md#sprint-lay-in-progress-since-2026-11-05): issues
  LAY-01..19 with their ticket files under `docs/tickets/`. The order of
  work and four tickets were revised by the mid-sprint review of 2026-11-10
  ([section 11](#11-mid-sprint-review-2026-11-10)), which adds LAY-21 and
  LAY-22.
- Analysis model: Fable. Mock-ups rendered by Opus on the throwaway branches
  `mock/lay-00-shapes`, `mock/lay-00-cards`, `mock/lay-00-shapes-2`,
  `mock/lay-00-round-3` and `mock/lay-00-round-4` (captures under
  `docs/qa/2026-11-0{3,4}/LAY-00/` on those branches, posted to #37 and
  #241).
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
  (`login.body.token`) on a pill at its midpoint; the other wires dim.
  Pointing at an input row traces the wires feeding it (a template or an
  object reads several); a click on the row pins them, a click on one wire
  pins that one, until Esc. A router clearance keeps every
  wire 12 px from the borders of cards it does not end on.
- **P10 Off-screen relations (LAY-15).** During focus, a chip on the
  board's edge for each related card out of view, labelled by role
  (`Source: login.body.token ↑`, `Reader: order.URL ←`, grouped as
  `↑ 3 sources` beyond two per edge); a click jumps there and *Back to
  checkout* restores the camera, the selection and the Inspector.
- **P11 Compact request geometry (LAY-16).** A request card's world box
  follows what it shows at the board's density: header, two result lines
  (execution, checks), the ports in use and one summary line per configured
  input (`Body · 2 fields`); unconnected output ports hidden. The box is
  stable while zooming; changing the density is a deliberate action that
  Tidy up repacks.
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

### 4.5 Second opinion, part two: local context

The reviewer's second note names the largest remaining gain as **local
context**: for one step, what feeds it, what it waits for and what happens
after it — which smaller cards and straighter wires do not give. Seven
ideas; three features (1–3) and one task-sheet entry (7) for this sprint:

1. **A hidden check shows when it runs, not only where it belongs.** A
   shared check (`receipt_total` reads `cart` and `receipt`) sits under
   `cart` yet waits for the later `receipt`; the aggregate under `cart`
   suggests otherwise. Membership and moment are two facts, and D sharpens
   the first while collapsing hides the second. → LAY-08's list rows read
   `cart_total — Passed` and `receipt_total — Waiting for receipt ·
   Shared`; hovering a row rings both subjects and the check's badge; a
   check that gates later steps (a *Data condition*, `StepStatus::Blocked`
   when unmet) is counted apart: `2 checks · 1 gate`. Adopted, in LAY-08
   and LAY-11.
2. **Focus helps when the source is off-screen.** Selecting `checkout` with
   `login` three rows up rings a card nobody sees. → LAY-15: during focus,
   chips on the board's edge nearest each off-screen related card
   (`↑ login · token`, `↓ receipt · order.id`; more than two per edge
   collapse into `↑ 3 sources`); a click jumps to the card and *Back to
   checkout* restores the camera and zoom. Adopted, must.
3. **The selected step and the traced value are two levels.** A click on a
   card rings every direct relation; pointing at one input traces the wires
   feeding that input and nothing else; a click on the input or on one wire
   pins the trace until Esc. An input row is an easier target than a thin wire in a trunk. →
   LAY-14 gains input tracing, no toolbar switch. Adopted.
4. **Wrapping should respect dependencies.** A fixed "five columns, next
   row" cuts the scenario at its worst place; the layout could pick the
   wrap among neighbouring boundaries by wires crossing rows, a request
   split from its satellites, a cut branch and the rows' heights, order
   unchanged; run at first layout and Tidy up, never on resize. →
   Deferred to LAY-2 as a LAY-06 follow-up; round 3 reports where the
   fixed wrap cuts shop-30.
5. **Compact shows the data in use**: `body.token → 6 readers`, `status →
   1 check` instead of the four fixed output rows; a click starts the
   trace of item 3; W11's token gets its navigation. → Deferred with Q13
   (it changes what a card shows, the owner's call) and W11; the summary
   form, with no port-model change, is the candidate.
6. **A failure shows its consequences**: `cart_total failed · 4 steps not
   run`, by cause as the engine knows them — stopped (error policy *Stop*),
   blocked (a source failed or a condition unmet, message "unavailable"),
   skipped (an unselected branch, "outside path"), never one grey
   category. The runner already distinguishes these in `StepResult.message`
   (`engine.rs:545–600`). → Deferred to LAY-2 as a RUN ticket; the
   statuses stay distinct in LAY-09's borders now (red failed, amber
   blocked, muted skipped).
7. **A return-to-work task**: select `checkout` → find a value's source →
   look at its check → return to `checkout` and keep editing, measured in
   pans and zoom changes and in whether the person recovers their place. →
   Added to the task sheet as task 8; round 3 walks it in captures.

On process the reviewer is right: the quiz must not be fifteen equal
decisions. Section 5 now separates the owner's questions (behaviour,
defaults, what is hidden, scope) from parameters the planning model
recommends and verifies on the coherent prototype (hues, margins,
thresholds), listed for veto only.

### 4.6 Round 3: the coherent D, measured

`mock/lay-00-round-3` at `ea981d8`, `docs/qa/2026-11-04/LAY-00/lay00r3-*`
(25 captures, posted on #37): shape D on `shop-30` with short satellites,
the continuation mark, badges on every step, checks collapsed after a run
with one failed and one skipped check, a shared check, a gate, the running
step other than the selected one, labelled `@after` wires, the hovered
wire and the hovered input traced, off-screen chips with *Back*, a branch
band across two rows, `shop-50` at 960 × 540, the Inspector list with the
moment of each check, and the task-8 walk in four captures.

| shop-30, board w × h, fit zoom at 1600 × 1000 / 960 × 540 | round 2 | round 3 |
| --- | --- | --- |
| collapsed | 1792 × 2736, 0.28 / — | 1900 × 2296, 0.33 / 0.13 |
| expanded | 1792 × 4100, 0.19 / — | 1900 × 2926, 0.26 / 0.10 |
| shop-50 collapsed | — | 1960 × 3564, 0.21 / 0.084 |

Each column above is that round's own fixture: 30 steps in round 2, 34 in
round 3. LAY-01 restored `shop-30` to 30 steps (a step is a node of the main
graph, the cleanup counted apart) and re-measured both rounds on it,
running the mock branches' own views on the restored file. The same run
reproduces the table above on the original fixtures, so the difference is
the fixture alone:

| restored shop-30, board w × h, fit zoom at 1600 × 1000 / 960 × 540 | round 2 (D) | round 3 |
| --- | --- | --- |
| collapsed | 1792 × 2376, 0.32 / 0.13 | 1900 × 1828, 0.42 / 0.16 |
| expanded | 1792 × 3740, 0.20 / 0.08 | 1900 × 2226, 0.34 / 0.14 |

On equal fixtures round 3's short satellites take 23 % off the height
collapsed and 40 % expanded (the 16 % and 29 % below compare a 30-step
board with a 34-step one). `shop-50` is round 3's, 56 steps, and is
unchanged.

What the captures settle:

- **The integrated promise holds at Fit (0.33)**: every name reads (the
  minimum on-screen label size), every step carries its number, every
  request its aggregate, the failed `checkout` is red with its border, the
  band header `paid → Yes` and the `after cart_total` labels are legible.
  Q7's recommendation follows: the thresholds stay 0.45 / 0.75 with the
  Overview content of P3. Q15's evidence: the red footer and border read at
  0.33 while the band stays green; what the card shows stays the owner's
  question.
- **The four signals read apart** at 60 %: orange frame on `cart`, teal
  outline on its relations, a blue badge and border with *Running…* on
  `payment`, red on `checkout`; `Not run · 3 checks` stays grey.
- **Short satellites take 16 % off the board's height** (2736 → 2296
  collapsed; 4100 → 2926 expanded, 29 %) and the rows no longer stretch
  under `catalogue`.
- **The task-8 walk** needs 6 camera moves on round 2's board, 6 on round
  3's without the chips, and none with the chips and *Back* (3 clicks).
  LAY-15 earns its place in must.
- **What is still weak**: the trunk above row 2 (bundles sharing a channel
  at 6 px — LAY-03/04/05 on this geometry); a request card at Compact is
  47 % empty (unused input rows and the control row), which is Q14's
  evidence; the band header is small at Fit.

Facts about the code learnt in round 3, and where they land:

- **The router's clearance is already 12 px** (`Routing::margin`); the
  closest wire-to-card distance on every board measured is exactly 12.0.
  Raising it to 18 breaks routing between stacked satellites (a 20 px gap
  is narrower than 2 × 18, 64 of 120 segments fall back to straight lines).
  The "hugging" is bundles at 6 px, not the margin. → LAY-14 loses the
  clearance criterion; LAY-01 keeps the metric; satellite spacing (LAY-06)
  must stay ≥ 2 × margin.
- **The route cache ignores routing parameters** (it reuses routes when
  only a parameter changes). → The first ticket that changes a parameter
  keys the cache on it (LAY-04).
- **The card box can shrink**: all card geometry is in one place
  (`canvas/layout.rs`); a `short` flag gave the 70 px satellite without
  touching routing. A Compact box that shrinks with the zoom would move
  cards, so density stays a board-level setting (Q14).
- **Hiding a Data condition drops a real wire**: `valid_cart` reads
  `cart_view.body` and feeds `checkout`; folded under its subject, its data
  wire vanishes. → LAY-11 rule: a gate whose value is read is never hidden,
  or its wire is drawn from the subject, as the `@after` rule.
- **`@after(x.then)` puts a step inside the branch**: the script lowers it
  to the same branch edge as a step written in the `if` block. → LAY-13
  bands include such steps.
- **Input rows are not hover targets** (`Card::input_row` is hit-tested
  only during drag-and-drop). → LAY-14 adds the interaction.
- **The runner has no "started" event**; the running state is injected.
  → LAY-07, as 4.2 said.
- **Selecting a step with the Inspector open marks the file changed**
  (#288) — the mock suppresses the Inspector; the defect is real.
- **At 960 × 540 the Library takes a third of the width.** → WS-00 (#240)
  matters for the small-window goal.
- **Fixtures changed** for the captures (a shared check `checkout_cart`,
  three checks on `checkout` and `refund`, `@after(cart_total)` on
  `address`; `shop-50`'s *Yes* branch has four steps and *No* has
  `cancel`); LAY-01 takes the round-3 fixtures over, with `shop-30`
  restored to 30 steps and `checkout_cart` replaced, as `receipt_total`
  is now the one shared check.

### 4.7 Second opinion on round 3, and the four proofs

The reviewer read the 25 captures, the plan and the mock code. Verdict:
round 3 grounds **approving the direction D, not the interface as
rendered**. The largest gains: short satellites, failures visible with
checks collapsed, moving from a request to its dependencies. The largest
open problems: card geometry, Compact's legibility, and dependencies lost
when a gate is collapsed. The reviewer's table, as recommendations to the
owner:

| Element | Recommendation |
| --- | --- |
| D: order in rows, satellites under their step | approve the direction |
| short value and check cards | approve |
| collapsed checks and the Inspector list | approve once the gate rule is settled |
| red mark of a failed check on a green HTTP result | approve the principle |
| wire and input tracing | approve the direction |
| focus, off-screen chips and *Back* | approve the design; test the interaction |
| today's Compact and the 0.45 / 0.75 thresholds | not yet |
| branch bands | conditional: fix the continuation |
| a large scenario at 960 × 540 | the goal is not reached |

What the captures and the code show, and what the plan adopts:

1. **D wins but still wastes space.** `first`, `second`, `expected_total`
   are short and subordinate, a real gain in hierarchy; `login`, `health`,
   `catalogue`, `cart`, `checkout` remain large forms emptied of content.
   Fit 0.33 is still a small diagram, and shop-30 now has 34 steps, so the
   round 2 → 3 comparison is not an isolated measurement. → A compact
   request whose height follows the content it shows, stable while
   zooming, with density change a deliberate action (Tidy up), becomes
   its own ticket **LAY-16** (must, before the shape and routing get their
   final card sizes); the five columns become a row-width parameter.
   LAY-01 re-measures on fixed fixtures.
2. **Overview improved; Compact does not yet do its job.** Two-digit
   numbers are about 6 px on screen at 0.33; `after …` labels compete
   with names; most successful cards share one green band; expanded
   checks' names are cut. Worse, the mock gives the Overview name
   `max(16 × z, 11)` but the Compact name `16 × z` with no minimum
   (9.6 px at 0.60, details 7.2 px): zooming in across 0.45 shrinks the
   text. → Q7 is **not verified**; the round-3 recommendation is
   withdrawn. LAY-09 gets minimum on-screen sizes for name, number and
   result line at every level, monotone across thresholds (text never
   shrinks when zooming in; information is dropped instead), and its
   acceptance renders 0.44 / 0.46 and 0.74 / 0.76. LAY-07: the badge has a
   minimum on-screen size.
3. **"Used input" was "has a wire"**: `catalogue`'s `query { limit: 20 }`
   and `login`'s body vanish at Compact, suggesting a poorer request than
   the one saved. → Q13's recommended option is split into three rules:
   an unconnected output port may be hidden; a configured input keeps a
   one-line summary (`Query · 1 parameter`, `Body · 2 fields`); a dynamic
   dependency stays traceable.
4. **Collapsed checks: the direction is right, one semantic gap.** The
   Inspector shows name, predicate, result and the values (180.0 vs 172.5),
   and *Waiting for checkout* explains a shared check. The principle "HTTP
   succeeded, the check of the response did not" stands. To fix: `1/2 ✓`
   does not say whether the other check waits, was skipped or has not
   started (`1 ✓ · 1 waiting`); *Shared* repeats and wraps badly (one line
   *Shared with: cart* in the Inspector); the card's result line crams HTTP,
   failures, skipped and shared into one tiny row (two legible lines: the
   execution result, the checks summary). **The gate rule is decided
   before LAY-11**: a gate whose result is read later stays a short,
   visible node with a gate glyph; redrawing its wire from the request is
   rejected because it would suggest the response is used directly,
   skipping the condition. Expand/collapse geometry is unproven (the
   captures have different cameras): proof 2 shows it in one camera.
5. **Wires: tracing works, the base routing is still hard.** The 12 px
   clearance is a board distance: about 4 px on screen at 0.33, and the
   6 px between bundled wires about 2 px; the measurement also skips the
   end cards. The metric stays useful but does not settle legibility of
   ends and bundles, and a larger margin does not help (the 18 px capture).
   → LAY-14's trace reads as a full statement
   (`login.body.token → cart.headers.Authorization`); a row fed by several
   sources shows every wire, never the first one only. LAY-03/04/05 keep
   the bundles.
6. **Focus and running: the colours differ, their coexistence needs
   rules.** Everything still competes by outline; on the selected, failed
   `checkout` several close lines plus wires. → LAY-10: signals differ by
   shape and place too (selection: handles and frame; failure: border and
   a badge glyph; running: an activity glyph and text; relation: a soft
   halo and ringed ports), and focus never dims a failed or a running
   card. The running state in round 3 is injected (`payment` running beside
   a finished `receipt` that depends on it): LAY-07's acceptance uses a
   real partial run and tells current results from a previous run's
   (UX-25).
7. **Off-screen chips: high value, ambiguous labels.** `← expected_total ·
   value` (a source) and `← order · orderId` (a reader) look alike; the
   arrow is read as the dependency's direction. → LAY-15: labels by role
   (`Source: login.body.token ↑`, `Reader: order.URL ←`); a card whose
   header row is not fully in the viewport counts as off-screen. The
   "6 moves → 0" figure is a storyboard (the walk set the camera
   directly), not a measurement: LAY-15's acceptance clicks the chip and
   *Back* and asserts the camera, the selection and the Inspector.
8. **Branch bands: the neutral tint is right, the continuation loses its
   meaning.** The second fragment of `paid → Yes` carries no header, next
   to `paid → No`. → LAY-13: every fragment repeats its header
   (`paid → Yes · continued`), a small status in the header tells the
   taken branch from the skipped one; the row-end marks mean "next step in
   order", never a dependency.
9. **960 × 540 exposes what marks cannot fix.** Fit 0.08 is a map, not a
   scenario; at 0.60 three cards fit and the minimap takes a share. → Two
   honest goals replace "every name readable": at a tiny Fit, orientation
   (where am I, where are the failures); at a working zoom, a few
   neighbouring steps with legible dependencies. Compact cards (LAY-16),
   a collapsible Library (WS-00), lower bands, and labels that yield to
   signals at extreme zoom (LAY-09). LAY-01's metrics and LAY-12's demo
   state both goals.

Five changes to the plan, adopted: Q7 not verified (item 2); compact
request geometry assigned to LAY-16 (item 1); the gate rule decided
(item 4); **LAY-12 moves to must** — checking the whole workflow is the
sprint's goal; the wave-1 tickets share the selection state and the
relation model, so they are ordered, not independent ([section 7](#7-dependencies-and-order)).
And one inconsistency fixed: LAY-06's width criterion said 1800 px while
round 3 measures 1900 at five columns of 280 px; the criterion follows
the row-width parameter (1920 px at the default).

**Round 4** (commissioned 2026-11-04 on `mock/lay-00-round-4`, posted on
#37) renders the four narrow proofs the reviewer asked for instead of
another round of variants: (1) one truly compact request card with
configured and dynamic inputs at 0.44 / 0.46 / 0.60 / 0.74 / 0.76 / 0.90,
beside today's card, and shop-30 compact at Fit; (2) collapse, expand and
*Expand checks here* in one camera with the used gate visible and the
shared check; (3) a coherent partial run, one step selected, another
running, a failure outside the focus not dimmed, stale results told from
current; (4) the walk executed by clicks in a test (chip, *Back*, camera
restored, selection and Inspector kept), at 1600 × 1000 and 960 × 540.

### 4.8 Round 4: the four proofs, measured

`mock/lay-00-round-4` at `df6305b` (built on the round-3 mock),
`docs/qa/2026-11-04/LAY-00/lay00r4-*`, numbers in `lay00r4-metrics.txt`,
posted on #37.

**Proof 1 — the compact request card.** A card's box is the same at every
zoom (`cart` 280 × 190, `login` 280 × 166: the height follows the rows
shown); name, number and result line keep their minimum on-screen sizes
across 0.44 / 0.46 and 0.74 / 0.76; configured inputs keep their summary
(`Body · 2 fields`, `Query · 1 parameter`); unconnected outputs are
hidden; two result lines (`HTTP 201 · 107 ms`, `3 checks ✓ · 1 shared`).

| world px | today | compact |
| --- | --- | --- |
| `login` height | 372 | 166 |
| `cart` height | 424 | 190 |
| shop-30 collapsed, board | 1900 × 2296, fit 0.33 | 1900 × 1452, **fit 0.52** |

At Fit 0.52 the whole 34-step board reads: names, numbers, two result
lines, the gate `valid_cart` with its glyph and wire, the band header
`paid → Yes · active`. This is the first capture where the brief's goal
("open a large scenario and understand it without zooming") is met at
1600 × 1000 for 30 steps. One conflict for the owner (Q16): "the box does
not change with zoom" and "Full shows everything as today" cannot both
hold — at Full the compact card writes the input values but has no
editable fields, because inline editing is drawn at today's geometry.
`if` cards stay at today's geometry in the mock.

**Proof 2 — collapse and expand in one camera** (asserted identical).
Collapsed → expanded moves 15 cards (row 2 +66, row 3 +222, `logout`
+342). *Expand checks here* on `checkout` moves nothing and
`checkout_total` overlaps `refund` by 280 × 60; the alternative shifts the
six cards below by +180 with no overlap. → LAY-11 takes the shift: the
rows below move by the hung checks' height, and the camera stays.

**Proof 3 — a coherent partial run.** Steps 1–22 finished (one failed
check on `checkout`, the gate passed), 23 `order` running, 24–34 pending
(`Not run`), `cart` selected with focus. The failed `checkout` and the
running `order` keep full strength outside the focus; relations carry a
teal halo and ringed ports; the selection has the accent frame. The brief's
"`address` running while `checkout` failed" was impossible (`checkout` is
step 19 after `address`), which the mock caught. Stale versus current
results is **not verified** by this proof: the stale look is the mock's
own, UX-25's bar was not found in the code; LAY-07's acceptance covers it
with the real runner.

**Proof 4 — the walk by clicks.** At 1600 × 1000 and 960 × 540: four
clicks, zero pans and zero zoom changes by the person; after *Back* the
camera is exactly what it was, `checkout` stays selected, the Inspector
shows `checkout` again (asserted in a test). The app moved the camera
three times: the jump, *Back*, and a pan when the Inspector opened on
select.

Facts for the tickets:

- The harness clicks board cards (accessible buttons "name (caption)") and
  any widget with an accessible name; LAY-15's and LAY-11's acceptance
  tests click.
- The camera is zoom and pan, a `Copy` struct; `Canvas::reveal()` forces
  the zoom up to 0.85, so a jump that keeps the zoom needs its own path
  (LAY-15). Opening the Inspector shrinks the board and the canvas pans to
  keep the selection in view: *Back* restores the camera after that pan.
- At 960 × 540 with the Inspector open the board is about 320 px wide and
  the left-edge chips cover part of the selected card → LAY-15: chips
  never cover the selection; WS-00 (#240) decides the panel.
- Chips exist for data relations only (`Source:` / `Reader:`), not for
  order-only ones (`address` before `checkout`); a chip omits the `value`
  port (`valid_cart.id`).
- The trace-two-sources extra was skipped (the mock traces one wire);
  LAY-14's acceptance covers it.

## 5. Owner decisions

Asked once the owner has seen rounds 3 and 4; the recommended option is
first.
**The owner's questions** are 1, 2, 3, 4, 8, 9, 10, 11, 12, 13, 14 and 15
(behaviour, a default, something hidden or shown, scope — rule 1 of the
agent guide; 15 decides what a card shows; 16, after round 4, the compact card at Full). **Recommended by the planning
model**, listed so the owner can veto them: 5 (the aggregate in the
footer), 6 (a shared check counted under every subject, marked `shared`)
and 7 (the thresholds: 0.45 / 0.75 remain the working assumption, **not
verified** — round 3's typography shrinks text across 0.45,
[4.7](#47-second-opinion-on-round-3-and-the-four-proofs); proof 1 of round
4 decides). Decided by the planning model as a rule of the model, not of
looks: a gate whose result is read later is never hidden by collapsing
(4.7 item 4).

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

12. **Capacity.** A. Must and should: sixteen tickets, about 295 owner
    review minutes ([section 6](#6-scope-must-should-deferred)). B. Must
    only: thirteen tickets, 245 minutes; nudge, lanes and bands follow as
    LAY-2. C. Everything including the deferred items.

**After the second opinion** (4.4)

13. **What Compact shows (LAY-09, LAY-16).** A. Name, method and short
    path, two result lines (execution, checks), the ports in use, and a
    one-line summary per configured input (`Body · 2 fields`,
    `Query · 1 parameter`); unconnected output ports, *Optional* and
    separators appear at Full; a dynamic dependency stays traceable. B.
    Only the ports with a wire (round 3's rendering, which hides
    `catalogue`'s configured query). C. Every port as today; only the text
    shrinks.
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

**After round 4** (4.8)

16. **The compact card at Full (LAY-16, LAY-09).** A. The compact box is
    the card at every zoom; at Full it writes the input values and editing
    happens in the Inspector or by double-click (the box never changes
    with the zoom; fit 0.52 for shop-30). B. Below 0.75 the compact box,
    at Full today's geometry with inline fields: cards move when crossing
    0.75, Tidy up keeps both layouts. C. Compact only as a board density
    the person switches on; today's geometry stays the default.

### 5.1 Decisions taken (2026-11-05)

Answered by the owner in an interactive quiz in six rounds, each question
with the recommended option first. One answer differs from the
recommendation (12, wider); the recommendations 5, 6 and 7 were not vetoed.
The same quiz decided UX-19's three and WS-00's four questions
([ux-1.md 6.2](ux-1.md#62-ux-19-and-ws-00-decisions-2026-11-05)).

| Q | Answer | Effect |
| --- | --- | --- |
| 1 | A | Tidy up produces **D hybrid**: a spine in engine order, satellites under their subject, wrapped rows → LAY-06; ADR 0006 amended |
| 2 | A | Badges on every step including checks, equal to the Run results row → LAY-07 |
| 3 | A | `@after` wires drawn, dashed, on the rail → LAY-06 |
| 4 | A | Checks collapsed by default as an application preference; *Expand checks here* is session-only; a gate whose result is read later is never hidden → LAY-11 |
| 5 | A (not vetoed) | The aggregate sits in the subject's footer; a pill under the name at Overview → LAY-11, LAY-09 |
| 6 | A (not vetoed) | A multi-subject check is counted under every subject it reads, marked *shared* → LAY-08, LAY-11 |
| 7 | A (not vetoed) | 0.45 / 0.75 stay the working assumption; LAY-09 verifies them with the minimum sizes and may move them → LAY-09 |
| 8 | A | DIAG-00 (#241) folds into LAY: its questions are 4, 5, 7 and the card captures; DIAG-01 (descriptions, `format`) deferred to its own analysis after LAY |
| 9 | A | Focus is selection behaviour: direct producers, consumers and checks ring, the rest dims, Esc clears → LAY-10 |
| 10 | A | Auto-layout only for a scenario without positions, on open; code edits keep incremental placement → LAY-02 |
| 11 | A | One band per branch, one wire into the band → LAY-13 |
| 12 | **C**, then narrowed | The owner chose C (everything including the deferred items) and, asked which deferred items become tickets, named the three that change what the board says: must, should **and three deferred items** join the sprint as tickets: wrapping by dependencies (LAY-17), Compact's data-in-use summary (LAY-18), failure consequences by cause (LAY-19, a runner-side ticket kept in LAY by this decision). DIAG-01, the component summary, MCP-13, arrowheads and the wire-length metrics stay deferred. 19 tickets, 365 review minutes ([section 6](#6-scope-must-should-deferred)) |
| 13 | A | Compact shows name, method, short path, two result lines, the ports in use and one summary line per configured input (`Body · 2 fields`) → LAY-16, LAY-09 |
| 14 | A | A board-level *Compact cards* application preference; Tidy up repacks; zooming never moves a card → LAY-16 |
| 15 | A | A failed check on a successful request: red footer `HTTP 200 · ✕ 1/3 failed`, 2 px red border at every zoom, the band keeps the HTTP colour → LAY-11, LAY-09 |
| 16 | A | The compact box is the card at every zoom; at Full it writes the input values and editing happens in the Inspector or by double-click → LAY-16 |

## 6. Scope: must, should, deferred

| Class | Tickets | Minutes |
| --- | --- | --- |
| **Must** — the comprehension goal | LAY-01 metrics and reference scenarios · LAY-08 check subjects and the Inspector list with timing · LAY-02 auto-layout and chips · LAY-16 compact request geometry · LAY-07 badges and running · LAY-09 semantic zoom · LAY-11 collapse checks · LAY-10 focus · LAY-14 wire and input tracing · LAY-15 off-screen relations · LAY-06 the shape · LAY-04 ports decide rows · LAY-12 acceptance and demo | 10 + 20 + 15 + 25 + 15 + 20 + 30 + 20 + 20 + 20 + 30 + 20 + 10 = **255** |
| **Should** — geometry after the shape | LAY-13 branch bands · LAY-03 nudge · LAY-05 lanes (measured first) | 20 + 15 + 15 = **50** |
| **Added by the owner** (Q12 = C, 2026-11-05; the three deferred items of [4.5](#45-second-opinion-part-two-local-context) that change what the board says) | LAY-17 wrapping by dependencies (4.5 item 4) · LAY-18 Compact's data-in-use summary with W11's navigation (item 5) · LAY-19 failure consequences by cause (item 6; runner-side, kept in LAY by the decision) | 20 + 20 + 20 = **60** |
| **Deferred** (LAY-2 candidates) | DIAG-01 descriptions (`format`); a component summary on the parent board; MCP-13 layout tool; arrowheads (#167 (3)); wire length and collision metrics beyond LAY-01 | — |

Sprint capacity: 19 tickets, **365** owner review minutes (the section 8
minutes; LAY-09 and LAY-10 were re-estimated at 20 when their criteria grew
in 4.7). The order of work is the work-in-progress limit, four pull requests
within 60 minutes, as in UX-1.

## 7. Dependencies and order

Nothing that depends on a quiz answer starts before the quiz; nothing that
routes wires starts before the shape and collapse are decided and built.

| Wave | Tickets | Why this order |
| --- | --- | --- |
| 0 (may start now, no decision) | LAY-01 (reference scenarios, the metric command), LAY-08 (the subject function in core and the Inspector's check list; no board change) | Pure functions and fixtures every later ticket uses |
| 1 (after the quiz), in this order | LAY-16 (the compact card geometry every later ticket renders into) → LAY-09 (what each level shows, minimum sizes) → LAY-07 (badges, running) → LAY-19 (the not-run causes on the cards, on LAY-07's running state) → LAY-10 (the selection state and the relation model: focus) → LAY-14 and LAY-15 (tracing and chips build on LAY-10's relation model and selection) → LAY-18 (the data-in-use line on LAY-16's card, starting LAY-14's trace) → LAY-11 (collapse, after LAY-08's subjects and the gate rule) | These share the card geometry, the selection state and the relation model; none moves a card, but each builds on the previous one's types |
| 2 | LAY-02, LAY-06, LAY-17, LAY-13 | Positions change: the shape, the first placement, the dependency-aware wrap on the shape, the bands — on LAY-16's card sizes |
| 3 | LAY-04 (must), LAY-03, then LAY-05 after measuring | Routing on the board as it will be; LAY-05 may be unnecessary |
| 4 | LAY-12 (must) | The whole workflow on the reference scenarios, the task sheet timed |

Interplay to name: LAY-11 and LAY-06 share the satellite rule (a hidden
check hangs under its subject); LAY-07 and UX-19 share the row order; LAY-10
and UX-31 share the ring; LAY-10, LAY-14, LAY-15 and LAY-11 share one
selection state and one relation model (producers, readers, checks, gates)
that LAY-10 introduces.

## 8. Proposed tickets

Created as issues after the quiz, with the decisions recorded in
`docs/tickets/<ID>.md`. Criteria are sketches; each ticket's file numbers
them and names the test.

| ID | Result | Size · min · risk | Criteria sketch |
| --- | --- | --- | --- |
| LAY-01 | `fixtures/scenarios/lay/` (14, 30, 50 steps) parse and simulate — the round-3 `shop-30` grew to 34 steps (a shared check, extra checks, `@after`), so LAY-01 restores exactly 30 while keeping one shared check and one used gate, and re-measures rounds 2 and 3 on it; `cargo run -p apiforge-layout --example quality` prints the LAY-01 table plus board width and fit zoom at both sizes; the UX task sheet exists | S · 10 · — | deterministic; `shop-30` has 30 steps; the dev guide names it |
| LAY-08 | `apiforge_core::graph::check_subjects`; the Inspector of a request lists its checks in words with sources, status and moment (`Waiting for receipt · Shared`); gates counted apart; a check's subtitle names subject and predicate | S · 20 · visible UI | two-subject check listed under both with its waiting subject named; a Data condition counted as a gate; standalone check listed nowhere |
| LAY-02 | Auto-layout on open (Q10); chips placed beside their first reader and moved by Tidy up; `place` with every block fresh gives layers | S–M · 15 · default | `order-flow.afs` opens in layers; Fit view within 10 % of the cards' box |
| LAY-16 | Compact request geometry (Q13, Q14): a card's world box follows the content it shows at the board's density (header, two result lines, ports in use, a summary line per configured input); stable while zooming; density change is a deliberate action repacked by Tidy up | M · 25 · visible UI, default | `login` and `cart` at least 40 % shorter than today at the compact density; zooming moves no card; a configured input never disappears (`Body · 2 fields`); routing receives the final sizes |
| LAY-07 | Sequence badges on every step, satellites included (Q2), with a minimum on-screen size; running step's badge glyph and *Running…*; equality with Run results row order | S · 15 · visible UI | the badge of step *k* equals its row; a two-digit number is at least 9 px on screen at Fit; a real partial run shows the running step within one frame of its start, with current results told from a previous run's (UX-25); the capture shows a running step other than the selected one |
| LAY-09 | Overview and Compact contents (P3, Q7, Q13); minimum on-screen sizes for name, number and result line, monotone across thresholds; failed and blocked borders at every zoom; labels yield to signals at extreme zoom | S–M · 20 · visible UI | a failed card is red at 0.35; at 0.25 a name is at least 11 px on screen, a step number and the result line at least 9 px; no text gets smaller when zooming in across 0.45 or 0.75 (rendered at 0.44 / 0.46 and 0.74 / 0.76); zooming moves no card |
| LAY-11 | The *Checks* toggle, two result lines, Inspector list, *Expand checks here*, navigation, Tidy up with hidden checks, the preference (Q4–Q6, Q15); the gate rule of 4.7 | M · 30 · visible UI, default | a failed check is visible collapsed at every zoom; no tick before a run; the checks line names the state of every check (`1 ✓ · 1 waiting`); toggling moves no card and no camera; *Expand checks here* hangs the step's checks under it and shifts the rows below by their height (round 4 proof 2: +180 px on `checkout`, no overlap), the camera stays; two-subject check counted per Q6, one line *Shared with: cart* in the Inspector; an `@after` on a hidden check is drawn from its subject with the check's name; a gate whose value is read stays a short visible node with its wire; Run results row of a collapsed check selects its subject |
| LAY-10 | Focus on selection (Q9): the selection state and the relation model (producers, readers, checks, gates) that LAY-14/15/11 reuse; the four signals by shape and place | S–M · 20 · visible UI, default | selecting `cart` rings login, first, second, cart_total, order with a halo and ringed ports; the selection has handles; a failed or running card is never dimmed; names stay legible on dimmed cards; Esc restores |
| LAY-14 | Wire and input tracing (P9); input rows become hover targets | S–M · 20 · visible UI | hovering `login → cart` or `cart`'s token input shows `login.body.token → cart.headers.Authorization` and rings both ports; a row fed by two sources shows both wires; a click pins the trace until Esc; the 12 px clearance stays a LAY-01 metric |
| LAY-15 | Off-screen relations during focus (P10), labelled by role | M · 20 · visible UI | `checkout` selected with `login` out of view shows `Source: login.body.token ↑`; a card whose header is not fully in view counts as off-screen; the click lands on `login` at the same zoom (its own path, not `reveal()`); *Back* restores camera and zoom exactly with the selection and the Inspector kept (asserted by a test that clicks, as round 4 proof 4); chips never cover the selected card; three sources on one edge read `↑ 3 sources` |
| LAY-06 | The chosen shape in Tidy up (Q1, Q3) with short satellites and the row continuation mark; the row width a parameter; ADR 0006 amended | M · 30 · default | 30-step board ≤ 1920 px wide at the default row width (five columns of 280 px); a row no taller than its tallest short stack; fit zoom ≥ 0.45 for 14 steps; no order wire through a gap; the row-end mark means "next step in order" |
| LAY-13 | Branch bands (Q11): neutral tint, header `created → Yes` repeated on every fragment (`· continued`) with the branch's status, orthogonal entry, a band per row for a wrapped branch; steps lowered into the branch by `@after(x.then)` included | M · 20 · visible UI | one wire per branch; a band contains exactly the branch's steps; every fragment of a wrapped branch carries the header |
| LAY-04 | Ports decide rows | M · 20 · — | as LAY-04 today, measured on the reference scenarios |
| LAY-03 | Nudge | S · 15 · — | as LAY-03 today |
| LAY-05 | Lanes | M · 15 · — | scheduled only if crossings per wire after LAY-04 exceed 0.2 on `shop-30` |
| LAY-12 | The demo (must): the three reference scenarios opened cold, read, run (simulation and live loopback), a failure found; the UX task sheet timed, task 8 by clicks; the two small-window goals of 4.7 item 9 checked | S · 10 · — | the demo plan of [section 10](#10-demo-plan) runs end to end at 1600 × 1000 and 960 × 540 |
| LAY-17 | Wrapping by dependencies (Q12, 4.5 item 4): D's row break is chosen among the neighbouring column boundaries (four or five columns; six would exceed the 1920 px bound) by the wires that would cross rows, a request split from its satellites, a cut branch and the rows' heights; order unchanged; at first layout and Tidy up, never on resize | M · 20 · default | on `shop-30` the wires crossing a row boundary are no more than with the fixed wrap, and no request is split from its satellites; deterministic; the board stays ≤ 1920 px wide |
| LAY-18 | Compact's data-in-use summary (Q12, 4.5 item 5; W11): at the compact density an output line reads `body.token → 6 readers`, `status → 1 check` instead of unused output rows; a click starts LAY-14's trace of those wires; no port-model change | M · 20 · visible UI | `login` at Compact reads `body.token → N readers` with N equal to its wires; clicking the line pins the trace to every reader; a field nothing reads is not listed; the card's box is unchanged by the line's wording |
| LAY-19 | Failure consequences by cause (Q12, 4.5 item 6): a failed step's footer counts the steps it kept from running (`✕ failed · 4 steps not run`); a not-run card names its cause as the engine knows it — *Stopped* (error policy), *Blocked: cart failed* or *condition unmet*, *Skipped: outside path*; Run results' summary uses the same words | M · 20 · visible UI | after a failed simulation of `shop-30` the failed card's count equals the Blocked and Stopped rows of Run results; each not-run card's cause equals its `StepResult` message class; a card on an unselected branch reads *Skipped*, never *Blocked* |

## 9. Metrics and QA

**Technical** (LAY-01, per reference scenario, before and after each
ticket): wires, crossings, bends, straight wires, close and coincident pairs,
card and chip collisions, board width and height, fit zoom at 1600 × 1000
and 960 × 540, long edges. Printed by the quality example on generated
boards and on the three fixtures.

**UX task sheet** (manual, the owner or an agent at the window, on
`shop-30.afs` after a failed simulation; the time and the number of
zoom/pan actions are recorded in the LAY-12 pull request, once before the
sprint and once at its end). The failure is made by changing `total: 172.5`
in `checkout`'s sample to `total: 170`: `checkout_total` fails and the run
stops there, under the policy *Stop* (a test in
`crates/apiforge-runner/tests/acceptance.rs` holds this and what the tasks
below read from the scenario):

1. Say in what order steps 8–12 run. *(Unambiguous? — LAY-07)*
2. Name every check of the `checkout` request. *(seconds — LAY-08, LAY-11)*
3. Say where `order.body.id` used by `receipt` comes from. *(seconds — LAY-10)*
4. With checks collapsed, point at the failed check. *(visible? — LAY-11)*
5. Describe the scenario in three sentences at Fit view without zooming.
   *(LAY-09, LAY-06)*
6. Open the scenario twice; switch Code → Diagram → Split. *(deterministic;
   disorienting? — LAY-02, UX-18)*
7. Say which field of `login` feeds `payment`, by pointing at the wire or
   at the input. *(seconds — LAY-14)*
8. Select `checkout`, find the source of its token, look at that source's
   check, return to `checkout` and keep editing. *(pans and zoom changes;
   is the place recovered? — LAY-15, LAY-10)*

**Deterministic tests** cover what can be asserted: the badge equals the row
(LAY-07); a failed check visible while collapsed (LAY-11); no card moved by a
toggle (LAY-11); the same positions from two opens (LAY-02); dimming never
hides the selection (LAY-10).

## 10. Demo plan

On the three reference scenarios (`order-flow`, `shop-30`, `shop-50`), in
the demo shop project, at 1600 × 1000 and 960 × 540, in simulation and
against the live loopback sample (the steps name `shop-30` and `shop-50`;
`order-flow` runs the same steps as the small case):

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
8. Small-window goals (4.7 item 9), at 960 × 540 on `shop-50.afs` after a
   failed run: at Fit, say where the failures are and which row the
   selection is in without zooming (orientation); at 60 %, name the
   producers and readers of the selected step from the board and the
   chips (neighbouring dependencies). Both timed in the task sheet.

## 11. Mid-sprint review (2026-11-10)

Five days in: LAY-01, LAY-08 and LAY-20 are merged, LAY-16 waits in #310.
The planning model read the plan again, rendered the board of #310 on
`shop-30` (`tools/gui/review.sh review_lay16`) and ran the metric command.
The direction stands. The order of delivery and four gaps do not.

### 11.1 What the real board says

| # | Finding | Evidence | Answer |
| --- | --- | --- | --- |
| R1 | **The value arrives last.** Shape D, short satellites and collapsed checks take `shop-30` from fit 0.205 to 0.52 ([4.8](#48-round-4-the-four-proofs-measured)); the plan put eight card tickets before them. Until then nothing a person sees at a glance improves. | `quality`: `shop-30` 5544 × 1688, fit 0.205 before and after LAY-16 | The new order of 11.2: the coherent board first, the refinements after a checkpoint |
| R2 | **The interim board reads worse than the old one.** With *Compact cards* on, only requests are compact: `cart` (214 px) is the smallest card, a value card `first` that says `items[0]` is taller, and a check card is the tallest. The hierarchy is inverted. | `lay16-compact-60-1600`, `lay16-compact-100-1600` | LAY-21: the compact geometry for every other card kind, pulled out of LAY-06, directly after LAY-16 |
| R3 | **Fit cannot fit.** `MIN_ZOOM` is 0.25 and Fit clamps to it; `shop-30` needs 0.205 today and every board needs 0.06–0.16 at 960 × 540. The plan's fit figures below 0.25 cannot be reached in the window. | `canvas/mod.rs:37`, `fit()` | LAY-09 lowers the limit so that Fit shows every card |
| R4 | **Tidy up can leave an empty board.** The camera fits only when the document changes; a Tidy up that moves every card away leaves the viewport on nothing. The capture named `lay16-fit` shows an empty board and passed the review. | `lay16-fit-1600`, `lay16-fit-960`; `Canvas::show` | LAY-06: Tidy up ends with the board in view. Review captures are looked at by the agent that takes them (11.4) |
| R5 | **The task sheet times reading only.** Q16 = A moved editing off the card (no inline field, hidden ports in a tray, the Inspector for values). None of the eight tasks edits anything, so a loss there would not be seen. | [section 9](#9-metrics-and-qa) | Tasks 9 and 10 below |
| R6 | **All evidence so far is mock-ups read by models.** Four rounds on throwaway branches with injected state; no person has used the result. The findings that changed this project most came from real work. | [4](#4-mock-ups) | The checkpoint of 11.2: the owner uses the real board before the second half is built |
| R7 | **"Compact" names two things**: the zoom level 0.45–0.75 (`CardDetail::Compact`) and the density (*Compact cards*). With Q16 = A the compact box already drops rows that are not legible, so the zoom levels do little for it. | `canvas/layout.rs:54`, `paint.rs` `legible` | LAY-09 renames the zoom level *Medium* in code and documents, and is re-scoped to what LAY-16 left open |
| R8 | **The chain is serial on three files and one test file.** Every wave-1 ticket edits `canvas/{layout,paint,mod}.rs` and appends to `acceptance_tests.rs` (14,862 lines); a Windows CI run takes 35 minutes and the hover flakes (#279) cost a rerun on most pull requests. | `git diff --stat` of #310; #279 | Three tracks by crate (11.3); a test module per ticket in its own file; the flakes fixed first (KTLO-33, #279) |
| R9 | **`quality` measures full cards**, so the exit criterion "fits at 50 %" cannot be read from the command. | #310, *Not verified* | LAY-06's window test is the measure; the command takes card sizes from a file the desktop writes — in LAY-06 |

Two items the review looked at and leaves alone: the tray of LAY-16
(decision 2 = D) is legible at 100 % and reaches every hidden port; the
vocabulary (badge, band, halo, tray, chips, two result lines) is large, and
whether it is too large is a question for the checkpoint, on the real board.

### 11.2 The order from here

The order of [section 7](#7-dependencies-and-order) is replaced. No ticket
is dropped and no decision of 5.1 changes.

**Part A — the coherent board** (the brief's goal at 1600 × 1000):

1. LAY-16 (#310) merges.
2. **LAY-21** every other card compact (new; LAY-06's short satellites).
3. **LAY-11** collapse checks.
4. **LAY-09** what each zoom shows, the minimum sizes, Fit below 25 %.
5. **LAY-07** badges and the running step.
6. In parallel, the layout track: **LAY-02** → **LAY-06**.
7. In parallel, no interface: **LAY-22** (new) the runner's and core's
   groundwork for LAY-07, LAY-19 and LAY-10.

**Checkpoint.** With part A on `main`, LAY-12's task sheet (tasks 1–10) is
timed on the real build and the owner uses `shop-30` and one scenario of his
own. The owner then confirms, reorders or drops the tickets of part B; the
exit criterion "fits at 50 % or more" is read here.

**Part B — the refinements**, in this order unless the checkpoint says
otherwise: LAY-10 → LAY-14, LAY-15 → LAY-19 → LAY-18 → LAY-17, LAY-13 →
LAY-04, LAY-03, LAY-05 → LAY-12.

### 11.3 Tracks

| Track | Files | Tickets |
| --- | --- | --- |
| Cards | `apiforge-desktop/src/canvas/` | LAY-21 → 11 → 09 → 07, then part B's card tickets; one at a time |
| Layout | `apiforge-layout`, `board_edit.rs`, `folder/placement.rs` | LAY-02 → 06 → 17 → 04 → 03 → 05 |
| Engine | `apiforge-runner`, `apiforge-core` | LAY-22 |

A ticket's acceptance module is its own file,
`crates/apiforge-desktop/src/folder/acceptance_tests/<id>.rs`, declared by
one `mod <id>;` line at the head of `acceptance_tests.rs` in alphabetical
order; it reads the helpers through `use super::*`. Nothing is appended to
the end of `acceptance_tests.rs` any more.

### 11.4 Changes to tickets

- **LAY-21** (new, S–M · 20): value, check, *Data condition*, `if` and
  component-call cards at the compact density. LAY-06 loses the short
  satellites and 10 minutes (M · 20).
- **LAY-22** (new, S–M · 10, no interface): the step-started event (from
  LAY-07), the not-run cause (LAY-19 criterion 6), `relations` (LAY-10
  criterion 5). The three tickets keep their minutes; their interface halves
  get smaller.
- **LAY-06** gains: Tidy up ends with the board in view (R4); `quality`
  reads the desktop's card sizes (R9); `shop-30` with checks collapsed fits
  1600 × 1000 at 0.50 or more (the assertion LAY-16 handed over).
- **LAY-09** gains: Fit shows every card (R3); the zoom level is *Medium*
  (R7); it covers every card kind of LAY-21.
- **LAY-12** gains the checkpoint run and tasks 9 and 10 (R5, R6).
- **LAY-11** no longer waits for LAY-18.
- A design-review capture is opened and described by the agent that takes
  it; a capture of a board asserts that a card is in the viewport (R4).

Capacity: 21 tickets, **385** minutes (365 + LAY-21 20 + LAY-22 10 − LAY-06
10); part A is 150 of them.

**Task sheet, added (R5)** — on `shop-30.afs`, timed with the count of
clicks, before (on `main` at `8248b93`, full cards) and at the checkpoint:

9. Add a request after `cart`, give it a JSON body with one field bound to
   `cart.body.id`, and run it. *(editing with no inline field — LAY-16)*
10. Add a check that `cart_view`'s `body.total` equals `expected_total`,
    from the board. *(wiring to a hidden port — LAY-16's tray, LAY-21)*

### 11.5 What published studies say, and what they do not

A short reading of the literature on 2026-11-10 (six searches, abstracts
and summaries, not full texts). Nothing in it guarantees the outcome; it
says where the plan stands on evidence and where it stands on judgement.

| Idea of the sprint | Evidence | Consequence |
| --- | --- | --- |
| Fewer, smaller elements (compact cards, collapse) | Model size is the factor that most affects how well a process model is understood (Reijers and Mendling). | Supported in direction. |
| Hiding checks by default (Q4 = A) | In two experiments on large BPMN models a fully flattened model was understood better than one with collapsed sub-processes (Türetken et al., 2016 and 2020); hidden dependencies are a known cost of visual notations (Green and Petre). | **Not supported as a default.** The aggregate and the visible failure are the mitigation; the checkpoint times tasks 2 and 4 collapsed and expanded, and the default follows the result. |
| The shape before the wires ("comprehension before crossing count") | Edge crossings are the aesthetic with by far the largest measured effect on reading a graph, and path continuity comes next for following a path (Purchase; Ware, Purchase et al.). `shop-30` has 117 crossings on 57 wires today. | **A risk in part A.** Tracing a value is a path task. The checkpoint reads crossings per wire on shape D; above 0.5, LAY-04 and LAY-03 come before every other ticket of part B. |
| A graph at all, for reading a test | Visual programs were slower to read than text in Green and Petre's experiments; Whitley's review finds the opposite for some tasks. | No guarantee either way. Code and Split stay first-class; the task sheet is timed in Diagram and, once, in Code. |
| Focus by dimming, off-screen chips, badges, bands | No task-performance study found for these in this reading. | Judgement only: they are part B and are confirmed at the checkpoint, on use. |

So the checkpoint is the sprint's own experiment and is run as one: the
baseline is timed on a build of `8248b93` (before LAY), the same tasks on
part A, with times, pans and zoom changes, wrong answers, and the metric
table for both.

Sources: Purchase, *Which aesthetic has the greatest effect on human
understanding?* (Graph Drawing 1997); Reijers and Mendling, *A study into
the factors that influence the understandability of business process
models* (2011); Türetken et al., *The influence of using collapsed
sub-processes and groups on the understandability of business process
models* (BISE 2020); Green and Petre, *When visual programs are harder to
read than textual programs* (1992) and *Usability analysis of visual
programming environments* (1996); Whitley, *Visual programming languages
and the empirical evidence for and against* (1997); Yoghourdjian et al.,
*Exploring the limits of complexity* (2018).

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
