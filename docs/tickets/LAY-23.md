# LAY-23: A value many steps read draws one wire per reader — thin the board's lines

- Size: M · Owner review: 30 min · Risk: `visible UI`, `default`, `removal`
  (wires not drawn by default) — for the recommended first stage, the
  *Wires* setting with chips; the stages after it are in the
  [estimate](#estimate)
- State: Analysis · **decision open** (issue #333, `decision-needed`): the
  owner chooses the direction and the rule in a quiz
  ([questions](#owner-decisions-open)) · LAY sprint, proposed for part A,
  before the checkpoint (#296)
- Analysis model: Fable · Implementation model: —

## Goal

The owner's finding of 2026-11-12, on a real board after LAY-02, 06, 09 and
11: the biggest problem left is the number of lines. A component call early
in the scenario gives two values that almost every later request reads, so
one port sends 6–8 long wires along the corridors between the rows, on top
of each other, beside the dashed `@after` rails. After this ticket a value
read by many steps no longer covers the board with lines, and a person
still sees at each step where its data comes from.

No ticket covers it today: LAY-04 and LAY-05 straighten and order wires but
keep their number; LAY-03 separates coincident wires, so a fan-out becomes
seven distinct lines; LAY-14, 10, 15 and 18 help on hover, on selection or
from the source card, and leave the default view as it is. It is W11 of the
audit ([lay.md 1.2](../sprints/lay.md#12-what-is-wrong)), which the plan
answered only from the source's side (LAY-18).

## What the owner will see

Four directions were rendered by the real board on a throwaway branch
(A, B and C from the issue, D from the owner on 2026-11-12), each on a
synthetic scenario of the same shape as the owner's board
(`fan-out`, 26 steps: a component call `cart`, the variables `cartId` read
by nine steps and `itemId` read by six, ten `@after` rails) and on
`shop-30` (`login`'s response read by eleven steps), tidied, at Fit and at
60 %.

- **A. Chips at the reader** (recommended). A wire of a fanned-out port is
  not drawn. Its reader shows a small chip at the input, in the gap left of
  the card: `← cartId`, `← login.token`; two sources of one input are two
  chips. The source's port says `→ 9 readers`. The wire comes back, in the
  accent colour, while the pointer is on the chip or the input, and while
  the reader or the source is selected. A reader beside its source keeps
  its wire. Where the gap has no room for the text the chip says the source
  step alone (`← login`), then only the arrow.
- **B. Bus.** A fanned-out port draws one heavier trunk: a riser beside the
  source, a trunk above each row with readers, a short drop into every
  reader, a dot at each junction. Nothing is hidden.
- **C. Data wires on demand.** The board draws the cards, the `@after`
  rails and the branch wires only. The data wires of a card appear while it
  is selected.
- **D. Values saved in variables, with proxy chips.** A value read later
  is saved once in a variable (`var cartId = cart.cartId`) and its readers
  read the variable; a proxy chip of the variable, the saved kind that
  exists today, stands beside each far reader with a short wire into it.
  It changes the scenario, not only the view
  ([its own section](#d-values-saved-in-variables-the-owners-idea-of-2026-11-12)).

- New (A): the reader chip, the reader count at the source's port, the wire
  on hover and on selection; a *Wires* setting in the view menu
  (question 1).
- Changed (A): what the board draws for a port read by three or more steps.
- Removed or hidden (A): the long wires of such a port, by default; every
  one is one hover or one click away, and named at its reader.
- Migrated or converted: nothing. Nothing is saved: the project files and
  `diagram.json` are untouched; proxy chips the person placed stay as they
  are.
- Default behaviour that changes (A): chips are on for a new and an
  existing installation.

## Mock-up

`mock/lay-23-fan-out` at `9bc9a30` (`crates/apiforge-desktop/src/canvas/lay23.rs`,
views `review_lay23_*`; the fixture `fixtures/scenarios/lay/fan-out.afs` is
there too). Captures in [`docs/qa/2026-11-12/LAY-23/`](../qa/2026-11-12/LAY-23/),
each opened and checked, with a card in view asserted by the view:

| View | `fan-out` | `shop-30` |
| --- | --- | --- |
| Today | [Fit](../qa/2026-11-12/LAY-23/lay23-fan-out-main-fit.png), [60 %](../qa/2026-11-12/LAY-23/lay23-fan-out-main-60.png) | [Fit](../qa/2026-11-12/LAY-23/lay23-shop-30-main-fit.png), [checks drawn](../qa/2026-11-12/LAY-23/lay23-shop-30-expanded-main-fit.png) |
| A chips | [Fit](../qa/2026-11-12/LAY-23/lay23-fan-out-a-chips-fit.png), [60 %](../qa/2026-11-12/LAY-23/lay23-fan-out-a-chips-60.png), [a reader selected](../qa/2026-11-12/LAY-23/lay23-fan-out-a-chips-reader-selected-60.png), [the source selected](../qa/2026-11-12/LAY-23/lay23-fan-out-a-chips-source-selected-fit.png) | [Fit](../qa/2026-11-12/LAY-23/lay23-shop-30-a-chips-fit.png), [60 %](../qa/2026-11-12/LAY-23/lay23-shop-30-a-chips-60.png), [checks drawn](../qa/2026-11-12/LAY-23/lay23-shop-30-expanded-a-chips-fit.png) |
| B bus | [Fit](../qa/2026-11-12/LAY-23/lay23-fan-out-b-bus-fit.png) | [Fit](../qa/2026-11-12/LAY-23/lay23-shop-30-b-bus-fit.png) |
| C on demand | [Fit](../qa/2026-11-12/LAY-23/lay23-fan-out-c-on-demand-fit.png), [a reader selected](../qa/2026-11-12/LAY-23/lay23-fan-out-c-on-demand-reader-selected-60.png) | [Fit](../qa/2026-11-12/LAY-23/lay23-shop-30-c-on-demand-fit.png) |
| D variables and proxies | [Fit](../qa/2026-11-12/LAY-23/lay23-fan-out-d-proxies-fit.png), [60 %](../qa/2026-11-12/LAY-23/lay23-fan-out-d-proxies-60.png) | [Fit, rewritten with variables](../qa/2026-11-12/LAY-23/lay23-shop-30-vars-d-proxies-fit.png) |

The hover is not in a capture (a review image has no pointer); criterion 3
names the gesture and the state. Not approved yet: the decision is open.

## Measured

By the definitions of the `quality` example (LAY-01, LAY-06) on the boards
the desktop tidies, compact cards: lines drawn (a wire, or a whole bus),
crossings, coincident pairs (two lines within 0.5 px for 20 px), ink (board
pixels of line). The count of crossings equals the router's bridges on
every baseline (50, 48, 76, 112). Full table with the close pairs, the
longest line and every variant of the rule:
[`lay23-measures.md`](../qa/2026-11-12/LAY-23/lay23-measures.md).

| Checks collapsed (the default) | `fan-out`: lines · crossings · coincident · ink | `shop-30`: lines · crossings · coincident · ink | Data wires not drawn |
| --- | --- | --- | --- |
| Today | 31 · 50 · 76 · 25 100 | 42 · 76 · 54 · 39 400 | 0 |
| A chips | 19 · **9** · **5** · 10 700 | 26 · **5** · **14** · 15 200 | 12 of 21 · 16 of 34 |
| B bus | 21 · 43 · 8 · 18 400 | 29 · 51 · 16 · 24 900 | 0 |
| C on demand | 10 · 8 · 2 · 7 200 | 8 · 0 · 3 · 4 700 | 21 of 21 · 34 of 34 |
| D variables and proxies | 31 · 6 · 14 · 13 000 (12 saved chips) | 45 · 25 · 35 · 25 000 (rewritten: 33 steps, 14 saved chips) | 0 |

| Checks drawn | `fan-out` | `shop-30` | Data wires not drawn |
| --- | --- | --- | --- |
| Today | 45 · 48 · 119 · 34 000 | 57 · 112 · 112 · 53 600 | 0 |
| A chips | 32 · 16 · 23 · 17 400 | 36 · **13** · **23** · 21 000 | 13 of 35 · 21 of 48 |
| B bus | 34 · 50 · 26 · 25 700 | 40 · 86 · 26 · 36 900 | 0 |
| C on demand | 10 · 2 · 3 · 7 900 | 9 · 1 · 3 · 6 200 | 35 of 35 · 48 of 48 |
| D variables and proxies | 45 · 22 · 39 · 19 900 | 60 · 40 · 94 · 39 900 | 0 |

With A and a card selected the wires return on purpose: a reader selected
on `fan-out` draws 21 lines with 16 crossings, the source selected 27 with
37 — the fan again, in one colour, for as long as it is asked for.

## Recommendation

**A, chips at the reader**, with the rule below, on by default, as one
entry of a *Wires* setting that keeps *All wires* and later takes C
([one setting for all three](#one-setting-for-all-three-the-owners-wish-of-2026-11-12)).

1. **It removes what is best evidenced to hurt.** Crossings have the
   largest measured effect on reading a graph and path continuity comes
   next ([lay.md 11.5](../sprints/lay.md#115-what-published-studies-say-and-what-they-do-not)).
   A takes 82 to 93 % of the crossings off both boards (50 → 9, 76 → 5) and
   the coincident bundles with them (76 → 5, 54 → 14). B keeps them
   (50 → 43, 76 → 51): a trunk still has to reach every reader across every
   row, so it crosses what the wires crossed, and it replaces seven lines a
   person cannot tell apart by one line 4 400 to 5 000 px long with eight
   junctions, which is the worst case for continuity. Two buses side by
   side (`cartId`, `itemId`) are a close pair again.
2. **It hides the line, not the dependency.** Hidden dependencies are a
   known cost of visual notations (Green and Petre), and the same reading
   found a collapsed model understood worse than a flat one (Türetken et
   al.). C hides every data dependency of the board until a card is
   clicked: the brief's third question, where data comes from, is no
   longer answered at rest, and nothing on a card says that it reads
   anything. A keeps every dependency written at the place it is used
   (`← cartId` at the input) and counted where it is made (`→ 9 readers`);
   what is one gesture away is following the line by eye. It is the
   off-page connector of a schematic, and the board's own vocabulary:
   proxy chips already stand for variables and parameters.
3. **It keeps the wires that read well.** A reader beside its source keeps
   its wire, so the local flow stays drawn; those are the wires LAY-04
   makes straight.
4. **It is cheap and writes nothing.** A rendering rule in the canvas, no
   router change, no format change, no ADR amendment; B needs the router
   to route a net (an amendment of ADR 0006, `L`).

What A does not do, said plainly: the dependency is read as text, not seen
as a line, and no task study was read for that trade — it is judgement and
measured geometry, to be timed at the checkpoint (tasks 3 and 7 of the
[task sheet](../sprints/lay.md#9-metrics-and-qa), chips on and off). At
Fit of a large board (below about 55 %) a chip shortens to the source step
and then to an arrow: a mark that the input reads something far away, not
its name. And the dashed `@after` rails are untouched: with the data wires
thinned they are the loudest lines left
([capture](../qa/2026-11-12/LAY-23/lay23-shop-30-expanded-a-chips-fit.png));
they were decided drawn (Q3 = A) and are not this ticket.

## The threshold rule

Recommended: **a data wire becomes a chip when its output port is read by
three or more steps and its reader is not beside the source** (the next
column of the same row, or the source's own column). The count is taken on
the document, not on what is drawn, so the *Checks* toggle never turns a
wire into a chip; the rule is applied when a card is dropped, never while
it is dragged.

| Rule, on `shop-30`, checks collapsed | Lines · crossings | Data wires not drawn |
| --- | --- | --- |
| 3 or more readers, neighbours keep their wire (recommended) | 26 · 5 | 16 of 34 |
| 3 or more readers, every reader a chip | 24 · 5 | 18 |
| 2 or more readers, neighbours keep their wire | 21 · 2 | 21 |
| 4 or more readers, neighbours keep their wire | 28 · 11 | 14 |
| Every wire past the next column, whatever the count | 20 · 1 | 22 |

Three is the knee: the fewest wires hidden that still clears the corridors
(two hides a third more for three crossings; four leaves `cart`'s three
long wires and doubles the crossings). On `fan-out` every variant gives the
same board, because both ports are far above any threshold. Keeping the
neighbours' wires costs no crossing. The variants are question 2.

## One setting for all three (the owner's wish of 2026-11-12)

The owner would like A, B and C selectable, like *Compact cards* and
*Collapse checks* in the board's view menu. What that costs, by what the
code and the measurements say:

- **A and C are one scale, B is another thing.** A and C answer *which*
  data wires are drawn; they differ only in the rule: every wire (today) →
  the wires of shared values as chips (A) → no data wire until a card is
  selected (C). C is A's code with another predicate. They do not combine
  side by side, they replace each other; what does combine is A's chip
  with C's rule — C with a chip at every input (`← cart.id`) hides the
  lines and keeps every dependency written, which removes C's main cost.
  B answers *how* a fanned-out port is drawn and needs geometry the router
  does not have.
- **Tidy up does not depend on the setting**, and must not: positions come
  from the document and the card sizes (shape D, LAY-06; the folded checks
  keep their places, LAY-11), and no mode changes a card's box. A's chips
  live in the 96 px gap Tidy up already leaves; a mode that asked for
  wider gaps would move cards when toggled, which *Compact cards* does on
  purpose and this setting should not. B is the exception in waiting: each
  bus takes an 8 px lane in the row gap it shares with the `@after` rails
  and in the column gap, so beyond about six buses the gaps must grow, and
  on a hand-arranged board there are no gaps to rely on.
- **Fit depends on it a little**: chips left of the first column widen the
  bounds (`fan-out` fits at 59 % with every wire, 56 % with chips). The
  rule of UX-18 holds: switching the setting never moves the camera.
- **`quality` and its board file** hold each reference scenario at two
  densities and two check states (12 boards). The setting is measured at
  the compact density only (8 more boards for *chips*); C needs none (it
  draws no data wire); B cannot be measured by the router as it is.
- **Tests.** The window tests that read the wires of `shop-30` (LAY-06
  criteria 4 and 11, LAY-11, LAY-02) say which mode they run in; each mode
  has its own criteria (A ten, C three more, B about six and the router's
  guarantees).
- **The default** is the owner's (question 2).

| Stage | What | Size · minutes · risk |
| --- | --- | --- |
| 1 (LAY-23) | The *Wires* setting in the view menu, an application preference: *All wires* and *Chips for shared values* (A); the `fan-out` fixture; the criteria below | M · 30 · `visible UI`, `default`, `removal` |
| 2 (LAY-23b) | A third entry, *On selection* (C), drawn with a chip at every input | S · 10 · `visible UI` |
| checkpoint | The owner uses the three on `shop-30` and on his own board; tasks 3 and 7 timed in each | — |
| 3, only if still wanted | *Bus* (B): the router routes a net (ADR 0006 amended), then the board's mode | `L`, split: M · 20 and M · 20 |

All four modes together are four tickets and about 80 review minutes; A
with C is two tickets and 40. The recommendation is stages 1 and 2 now and
B not built unless the checkpoint asks for it: B is the dearest mode and
the one the measurements favour least (crossings 50 → 43 and 76 → 51,
against 9 and 5 for A), and the setting stays open for it.

## Why proxy chips are not the default today

What exists (`crates/apiforge-core/src/edit/proxies.rs`, ADR 0004,
[roadmap, *Variable proxies*](../roadmap.md#variable-proxies), merged as
M3 on 2026-10-24):

- A proxy stands for a **variable** (a value marked `var`, of its own
  graph), a **scenario parameter or component input**, or an **environment
  value**. It cannot stand for a step's output: a response field or a
  component's output is "not something the board can place"
  (`placeable` in `canvas/mod.rs`).
- A proxy is **made by the person only**: dragged from *Library →
  Variables* onto the board. Nothing creates one — not opening a scenario,
  not Tidy up. LAY-02 only moves the chips that exist beside their first
  reader. A scenario written in code therefore has none.
- A proxy is **a saved object**: `graph.proxies` in `diagram.json`, with a
  position; a card-like chip 36 px tall and at least 112 px wide, an
  obstacle for the router and for Tidy up.
- **Wire or proxy** is decided per reader when the board is drawn
  (`folded_wires` in `canvas/mod.rs`): a reader takes its value from the
  nearest place it is shown, a proxy or the block itself. Parameters and
  environment values have no block and are drawn only where placed.

No ticket, ADR or sprint document decides against automatic proxies. The
roadmap's goal was "a variable can be placed as a proxy chip anywhere on
the board, so no wire crosses the whole scenario" — placing was the
feature; making them by default was never asked or analysed. It is
history, not a decision. The design does carry two reasons it would not
have been free: a proxy is written to the project (a default that writes
files is the owner's decision), and a 112 × 36 chip does not fit the 96 px
gap between two columns.

What "proxies by default" would mean:

- **For variables** (the owner's board: `cartId` and `itemId` are
  variables): Tidy up would create about fifteen saved chips, one beside
  each far reader. They do not fit the gap, so the columns move apart to
  about 150 px, the five-column board grows from 1784 to about 2000 px —
  past LAY-06's bound of 1920 — and the fit falls. Every later edit has to
  keep them in step (a reader added, moved or deleted), in the document.
- **For step outputs** (`login.body.token`, read by eleven steps of
  `shop-30`; a component's output read directly): the proxy model has no
  such source, so the format and its validation would change — or the chip
  is drawn without being saved, which is option A.

So the owner's belief holds, and A is its cheapest form: **a proxy the
board draws by itself** — for variables and for step outputs alike, small
enough for the gap, written nowhere, with the proxy's own behaviour (the
wire from the nearest place the value is shown, the ring on its readers).
The proxies a person places stay, and win: a reader beside a placed proxy
keeps that wire. If the owner prefers real, movable chips for variables,
that is D, below, and question 6.

## D. Values saved in variables (the owner's idea of 2026-11-12)

Good practice would be to save a request's data in a variable and read the
variable afterwards; a variable can already have proxy chips
(`ProxySource::Variable`), with no change of format.

**What was measured.** The `fan-out` scenario is already written this way
— so is the owner's board: `cartId` and `itemId` are variables — and it is
the board that drowns in lines. The style alone changes nothing: a
variable's port fans out exactly as a response's does (`shop-30` rewritten
with `token`, `cartId` and `orderId` as variables: 45 lines · 72 crossings
· 63 pairs, against 42 · 76 · 54 before, on three more cards). What helps
is the chips, which nobody places today. With a proxy beside every far
reader: `fan-out` 50 → **6** crossings and 76 → 14 pairs, as good as A;
`shop-30` rewritten 72 → 25 and 63 → 35, weaker than A (5 and 14),
because a fan-out that is not a variable stays (`cart`'s response into its
gate and checks, `expected_total`), and a reader hung under a card has no
room for a chip. A saved chip is 36 px tall and at least 112 px wide: it
does not fit the gap between two columns, so the mock-up stands it in the
row gap above its reader, where the `@after` rails run; two fit there, a
third does not.

**How it could be offered.**

- **(a) An action, *Save as variable*,** on an output or on a wire: one
  undoable edit that creates the variable (`var cartId = cart.cartId`,
  named after the last field), points every reader at it and, without A,
  places a chip beside each far reader. A real edit of the scenario:
  the code gains a line and the readers read `cartId`.
- **(b) A suggestion** where an output has three or more readers (at the
  port, `→ 9 readers · Save as variable`, and in the Inspector): cheap on
  top of (a), and it teaches the style where it pays.
- **(c) Automatically: no.** It would rewrite the person's scenario —
  new statements, renamed references, in the `.afs` Git tracks — without
  being asked, each time a third reader appears. The brief rules it out
  (no change of the code to simplify a view), and a project file is never
  changed as a side effect.
- **(d) What it means elsewhere.** *Code:* one more line per value and
  shorter references — arguably clearer text. *Run results and the
  board:* a variable is a step, so one more row, one more card and one
  more number per value (`shop-30`: 30 → 33 steps, taller rows). *A
  failure* in the value has its own row, which helps. *Components:* a
  component's output is already a named value (`cart.cartId`); saving it
  again is a second name for the same thing — it is what the owner's
  scenario does, for the chips' sake. *Scenarios from code or from an
  agent* have no positions and no proxies: D gives them nothing until
  something places chips, which is a default that writes `diagram.json`.
  *Language, guide, demo:* recommending the style is a paragraph in the
  guide; rewriting the demo scenarios changes generated fixtures and is
  not worth doing for it.

**D beside A.** D changes the document and the person's model: it is
kept, it travels with the project, the chips can be moved, and it covers
only what someone saved. A changes only the view: no gesture, every
fan-out covered, scenarios from code included, nothing to keep in step.
They are not rivals. On a board written in D's style, A *is* "variables
with chips by default" — `fan-out` under A is exactly that (9 crossings),
with chips small enough for the gap and no file written. And once A draws
the chips, D's action no longer has to place and maintain proxies: *Save
as variable* is then a small edit whose value is the code, not the board.

Recommended: A first; D as the action (a) with the suggestion (b)
afterwards, a ticket of its own; never (c); a paragraph in the guide, no
demo rewrite. D instead of A is the dearer way to the same board: the
action, plus Tidy up creating and keeping one saved chip per reader.

| D, as | Size · minutes · risk |
| --- | --- |
| *Save as variable* after stage 1, chips drawn by A (LAY-25) | S–M · 20 · `visible UI` — an edit in `apiforge-core`, the menu entry, the code printed |
| The suggestion at three readers | S · 10 · `visible UI` |
| D without A: the action and Tidy up placing and keeping saved proxies per reader | M · 30 and M · 20 · `visible UI`, `default` (Tidy up writes chips) |

## Other ways to cut lines, measured

On the same boards, checks collapsed; lines · crossings · coincident
pairs (today: `fan-out` 31 · 50 · 76, `shop-30` 42 · 76 · 54).

| Idea | `fan-out` | `shop-30` | Verdict |
| --- | --- | --- | --- |
| **No rail into the next step.** An `@after` whose target is the next step of the spine (ten of ten on `fan-out`: every one waits for the previous step's check) is not drawn as a rail; the card says `after quantity_ok` | 21 · **10** · 71 | 39 · 67 · 58 | The largest gain after A on the owner's shape: forty of the fifty crossings are a rail crossing the fan. Changes Q3 = A of 2026-11-05, so it is the owner's (question 4) and needs its own mock-up of the mark |
| The same with A | 9 · **1** · 3 ([capture](../qa/2026-11-12/LAY-23/lay23-fan-out-a-chips-no-next-rails-fit.png)) | 23 · 4 · 14 | The board the two give together |
| No `@after` that the data already implies | 30 · 49 · 76 | 42 · 76 · 54 | One wire on `fan-out`, none on `shop-30`: people write `@after` on a check, which no data implies. Not worth a rule |
| One wire for several wires from one port into one card | 31 · 50 · 76 | 40 · 76 · 42 | Small and only on `shop-30` (coincident 54 → 42); LAY-03's nudge covers the same pairs. Not worth a ticket |
| A threshold by distance instead of by count (every wire past the next column a chip) | 19 · 9 · 5 | 20 · 1 · 10 | Six more wires hidden on `shop-30` for four crossings fewer; a variant of question 3 |
| Collapsing checks | — | — | Done (LAY-11): 57 → 42 lines on `shop-30` |
| Bundling wires that travel together | — | — | It is B drawn by the router; and the opposite of LAY-03, which separates them |
| Dimming long wires instead of hiding them | same lines | same lines | Costs a line of paint, removes no crossing; not measured as a gain |

## LAY-03, LAY-04 and LAY-05

LAY-04's pull request (#332, paused) measured what a routing change does
to a real board: on `shop-30`, checks collapsed, crossings 76 → 73,
coincident pairs 54 → 54, straight wires 0 → 3 of 42. A takes the same
board to 5 crossings and 14 pairs. The clutter is the number of long
wires, not their routes, so:

- **LAY-05** (#36, lanes): **drop from the sprint.** Its own gate is
  crossings per wire above 0.2 after LAY-04; with chips the board is at
  0.19 collapsed and 0.36 with checks drawn, and another routing pass is
  worth a few per cent of what is left. Closed as not planned, reopened
  only if the checkpoint shows crossings among the wires that remain.
- **LAY-04** (#35, PR #332): **stays paused, decided at the checkpoint,
  not before.** Its gain is real only for neighbours' wires, which are
  exactly the ones chips keep, but three straight wires of 42 does not pay
  for a spine that is no longer level. After stage 1 it is refreshed and
  measured on the chips board; it merges if the neighbours' wires it keeps
  come out straight, and is otherwise cut down to the layered layout
  (selections, the workspace window), where it changes no shape.
- **LAY-03** (#34, nudge): **keep, small, after stage 1.** The cap handed
  over from LAY-06 — at most 97 coincident pairs on `shop-30` with checks
  drawn, 112 today — is met by chips alone (23). What is left are wires
  leaving one port together below the threshold, which is what a nudge is
  for. Recommended: the cap moves to the default mode and tightens to 30;
  in *All wires* the number is reported, not asserted (question 5,
  because the cap was the owner's).

## Interplay

- **LAY-03, LAY-04, LAY-05**: the section above.
- **LAY-06 and LAY-11**: Tidy up, the rail and the folded checks do not
  read the setting; LAY-06's criterion 11 runs in the default mode.
- **LAY-14** (#298): a chip is the hover target of its input extended into
  the gap. This ticket brings the minimum — the wire and a tooltip with the
  statement `cartId → checkout.body.cartId` — and LAY-14 adds the pill, the
  pin and the dimming on top. LAY-14's criterion 1 hovers the wire
  `login → cart`, which is a chip under A: it is reworded to hover the chip
  or the input when this ticket is decided.
- **LAY-18** (#302): `→ 9 readers` at the port is LAY-18's line
  (`body.token → 6 readers`) seen from outside the card, and a click on
  either shows every wire of the port. LAY-18 keeps the line inside a
  request card and takes its number from the same count; value cards and
  component calls, which LAY-18 does not cover, keep the mark.
- **LAY-10, LAY-15**: selecting a step brings back the wires of its chips
  at full strength, which is what focus asks for; a chip names a source
  that is off-screen by itself, and LAY-15's edge chips stay the way to
  jump there.
- **Run results**: a chip carries no run state, as a wire carries none;
  rows of Run results select cards as before.
- **LAY-16**: the mark at a port must not sit under the tray handle of a
  selected card (seen in the mock-up, criterion 6).

## Acceptance criteria

For option A. Tests in
`crates/apiforge-desktop/src/folder/acceptance_tests/lay23.rs`, on
`fan-out.afs` and `shop-30.afs`, tidied, at 1600 × 1000 and 960 × 540.

1. On `fan-out` no wire is drawn from `cartId` or `itemId` to a reader
   that is not beside them; each such input carries a chip that names its
   source, two chips where an address reads both; `quantity`, beside them,
   keeps both wires; a port read by fewer than three steps keeps all of its
   wires (`catalogue → first`, `checkout → orderNumber`). Test:
   `lay23_criterion_1_a_fanned_out_port_draws_chips`.
2. The port of `cartId` reads `→ 9 readers`, the number of steps that read
   it in the document; the *Checks* toggle changes neither that number nor
   which wires are chips. Test: `lay23_criterion_2_the_source_counts_its_readers`.
3. Pointing at a chip or at its input draws that wire from the source's
   port to the input at full strength and shows the statement
   `cartId → checkout.body.cartId`; moving the pointer away removes both.
   The chip is found by its accessible name. Test:
   `lay23_criterion_3_pointing_at_a_chip_draws_its_wire`.
4. Selecting a reader draws the wires of all its chips; selecting the
   source, or clicking the mark at its port, draws every wire of the port;
   Esc or a click on the board returns to chips. No card and no camera
   moves, the wires that were drawn keep their routes, and the document is
   not marked changed. Test: `lay23_criterion_4_selection_brings_the_wires_back`.
5. Nothing is written: after Tidy up, hovering and selecting, the scenario
   file and `diagram.json` are byte for byte what they were, and the
   document has no new proxy. Test: `lay23_criterion_5_nothing_is_written`.
6. A chip's text is at least 9 px on screen at every zoom and a chip never
   covers a card, a continuation mark or the tray handle: where the gap has
   no room it says the source step, then the arrow alone (rendered at
   0.25, 0.44, 0.60 and 1.0). Fit shows every chip whole, the first
   column's included. Test: `lay23_criterion_6_chips_fit_the_gap_at_every_zoom`.
7. With chips on, `fan-out` and `shop-30` have at most 0.6 crossings per
   line drawn, checks collapsed and drawn (0.47, 0.19, 0.50 and 0.36 in
   the mock-up; 1.1 to 2.0 today), and `shop-30` with checks drawn at most
   30 coincident pairs. `fan-out` joins the reference scenarios
   (`fixtures/README.md`, the board file of LAY-06) and `quality` prints
   each reference board with chips on and off. Test:
   `lay23_criterion_7_fewer_crossings_per_line`.
8. The view menu has *Wires* with *All wires* and *Chips for shared
   values*, kept in the application preferences, the default as question 2
   says; *All wires* draws exactly the wires the board draws today;
   switching moves no card and no camera, and Tidy up gives the same
   positions in either. Test:
   `lay23_criterion_8_the_setting_moves_nothing_and_restores_every_wire`.
9. Editing follows the rule: a wire dragged from a fanned-out port to a
   far input connects and shows a chip; removing the binding removes the
   chip; while a card is dragged no wire turns into a chip or back, and
   the rule is applied when it is dropped. Test:
   `lay23_criterion_9_editing_follows_the_rule`.
10. The review view `review_lay23` renders both scenarios at Fit and 60 %,
    with a reader and with the source selected, and asserts a card in
    view; the captures are opened by the agent that takes them
    ([lay.md 11.4](../sprints/lay.md#114-changes-to-tickets)).

## Owner decisions (open)

The recommended option is first.

1. **What is built.** The *Wires* setting with *All wires*, *Chips* (A)
   and *On selection* (C); the bus not built unless the checkpoint asks
   for it · all four, the bus included (two more tickets, the router
   changes) · chips only, as the board's rule, with no setting · nothing:
   the wires stay.
2. **The default.** *Chips for shared values* · *All wires*, as today,
   chips for whoever switches them on · *On selection*.
3. **Which wires become chips.** Three or more readers, neighbours keep
   their wire · three or more readers, every reader a chip · every wire
   past the next column, whatever the count · two or more readers.
4. **The `@after` rail into the next step.** A separate ticket (LAY-24)
   mocks up a mark on the card (`after quantity_ok`) in place of the rail,
   and the owner decides on the capture · the rails stay drawn, as decided
   on 2026-11-05 (Q3 = A) · the rail is hidden until its step is selected.
5. **LAY-03, LAY-04, LAY-05.** As recommended above: LAY-05 dropped,
   LAY-04 paused until the checkpoint, LAY-03 kept after stage 1 with its
   cap on the default mode · all three stay as planned · all three
   dropped.
6. **Saving values in variables (D).** After stage 1: an action *Save
   as variable* with a suggestion at three readers, the chips drawn by
   the board (A), a paragraph in the guide · the action only, no
   suggestion · D instead of A: the action, and Tidy up places and keeps
   real proxy chips beside the readers, saved in `diagram.json` · not
   now: the style stays the person's choice, with no action.
7. **When.** Stage 1 before the checkpoint, so that the owner's scenario
   is judged on the thinned board · in part B, with LAY-14.

For veto, the planning model's parameters: the chip says the variable's
name, or the step and the last field (`login.token`), the whole path in
the statement on hover; three as the threshold if question 3 keeps a count;
the chip is quieter than a proxy chip (grey outline, no fill) and takes the
accent only while its wire is drawn.

## Estimate

- **Stage 1 (this ticket): M · 30 min · `visible UI`, `default`,
  `removal`.** Complexity: one crate, the canvas only (the wire set before
  routing, two painters, one hover target, one preference); no state
  machine. Uncertainty: low, the mock-up is the rule in 300 lines.
  Regression potential: real and bounded — the acceptance tests of LAY-06
  and LAY-11 read the wires drawn on `shop-30` and name their mode;
  criterion 8 holds the old board. Verification: window tests and four
  review captures.
- Stage 2 (*On selection*): S · 10. Stage 3 (*Bus*): `L`, split into the
  router (M · 20, ADR 0006) and the board (M · 20). LAY-24 (the mark for
  an `@after` into the next step), if question 4 asks for it: S–M · 20 ·
  `visible UI`, `removal`, after its mock-up. LAY-25 (*Save as variable*),
  if question 6 asks for it: S–M · 20, the suggestion S · 10.

## Not in scope

*On selection* (stage 2) and the bus (stage 3); the `@after` rails
(question 4, LAY-24); LAY-14's
pill, pin and dimming; LAY-18's line inside the request card; a jump from
a chip to its source (LAY-15); creating proxy chips in the document;
parameters and environment values, which are drawn only where a proxy is
placed, as today.

## Technical appendix

- The mock-up's `lay23::plan` is a pure function of the board's wires, its
  cards and the selection, applied between `folded_wires` and the router in
  `Canvas::show`: a wire that is a chip is not routed, so it costs the
  other wires nothing; the chips' boxes join the bounds Fit uses.
- Revealing a wire changes the router's key, and the router routes longest
  first with a crossing cost, so the routes already drawn can shift when a
  long wire returns. Criterion 4 forbids it: the revealed wires are routed
  in a second pass over the cached routes.
- The count per port and the readers come from `apiforge_core`
  (`graph::wires`, or LAY-22's `relations` once merged), not from the
  drawn set.
- The mock-up's bus is a fixed comb on shape D (riser in the column gap,
  trunk in the row gap); it does not avoid other wires, which is why its
  crossings stay. A router-made bus could do better on crossings and
  cannot do better on the trunk's length.
- The fixture models the owner's board and holds none of its names or
  addresses; the owner's project is not in the repository.
