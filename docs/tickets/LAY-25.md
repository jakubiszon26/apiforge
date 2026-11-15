# LAY-25: A value used far away is saved in a variable — proposed by the board

- Size: M · Owner review: 30 min · Risk: `visible UI`, `default`
- State: Analysis · waits for the owner's decisions
  ([questions](#questions-for-the-owner)) · issue #335 · LAY sprint, after
  LAY-23 stage 1 (merged, #337)
- Analysis model: Fable · Implementation model: —

## Goal

The owner's request of 2026-11-15: when something from an early step is used
far down the scenario — a cart made once, its `cartId` read by almost every
later request — no wire should run back to the first request each time. The
workflow should assume that such a value is saved in a variable and read
through it, and should propose that by itself.

It revisits decision 6 of LAY-23 (2026-11-12): *Save as variable* as an
action with a suggestion at three readers, never automatic.

## What the board does today

Looked at in captures of the merged board, on a synthetic scenario of the
owner's shape written **without** variables (`far-value`: a cart component
read directly by nine steps, a session value read by two):

- A value read by three or more steps already draws no wire back: its far
  readers carry `← cart.id` and the port says `→ 10`
  ([capture 1](#mock-up)). It makes no difference whether the value is a
  variable.
- What still crosses the board is a far value read by **one or two** steps
  (the two long wires from `session` in capture 1). Decision 3 of LAY-23
  leaves those as wires, and a variable changes nothing there: its port
  fans out as the response's did.
- *Assign to variable* exists in the response tree of the Inspector and of
  Run results (`edit::assign_response_variable`): it creates the variable
  and leaves every reader reading the response. It is half of this ticket.
- Nothing proposes anything while a person connects a value.

So two things are missing: the proposal, and a board on which saving a
value is what removes its wire.

## Where the proposal is made

| Place | Reaches | Clicks · undo | Verdict |
| --- | --- | --- | --- |
| **The drop**: a wire from a port, or a field dragged from the response tree, released on an input that is not beside its source | Values as they are connected on the board | 1 (Enter) · 1 | Recommended; how strongly is [question 1](#questions-for-the-owner) |
| Typing a reference in an Inspector field or in code | — | — | No proposal while typing: text is the person's. The mark below appears once the text is read |
| A third reader appears | — | — | Not a place of its own: it is the moment the mark below appears |
| **The port**: a mark `+ var` under the count `→ 10` of a step's output read by three or more steps; its menu lists the fields, one row each | Boards that exist, scenarios written in code or by an agent | 2 (mark, row) · 1 | Recommended ([question 3](#questions-for-the-owner)) |
| A scenario-wide or project-wide *Tidy values* | Every shared value at once | 1 · 1 per document | Not now: names nobody chose, a diff across many statements and files ([question 4](#questions-for-the-owner)) |

"Far" is LAY-23's rule turned round: the reader is not in the source's row
in its own or the next column. The count does not matter at the drop: the
owner's case is distance.

## What "variable with proxy" becomes

| | A. Variable, chips drawn by the board (recommended) | B. Variable and saved proxy chips |
| --- | --- | --- |
| What is written | One line of the scenario | The line, and one proxy per far reader in `diagram.json` |
| Size | The LAY-23 chip, fits the 96 px gap | 112 × 36, does not fit: gaps grow to about 150 px, `fan-out` from 1784 to about 2000 px, past LAY-06's bound of 1920 |
| Kept in step | Nothing to keep | Every reader added, moved or deleted, in the document |
| A scenario from code or an agent | Chips at once | None until something places them |
| Measured (LAY-23, crossings) | `fan-out` 50 → 9 | `fan-out` 50 → 6, `shop-30` rewritten 72 → 25 |

A needs one change to LAY-23's rule to mean what the owner asked: **a
variable's far reader is a chip whatever the count** — saving a value is
how a person says "do not wire this" ([capture 4](#mock-up): no long wire
left, the two-reader `currency` included). Without it a variable read by
two steps keeps its long wires. That revises decision 3 of LAY-23
([question 2](#questions-for-the-owner)).

Saved proxies after this: still the only way a **parameter or an
environment value** is drawn, so they stay. For a **variable** they are no
longer needed to cut wires; the ones a person places stay and win (a reader
beside a placed proxy keeps its short wire), nothing creates them, nothing
removes them. Retiring them would be a removal and a migration and is not
proposed.

## Assumed, asked or offered

| | What happens at the drop on a far input | Whose decision |
| --- | --- | --- |
| **Assumed** | The variable is made at once and every reader moved to it; a status line says so with *Undo*; a setting turns it off | The owner's: it changes what a gesture does by default and writes lines of the `.afs` nobody asked for; it reverses "never automatic" |
| **Asked** (recommended) | A two-row choice opens at the input, *Read via variable cartId* first and under Enter, *Wire directly* second, Esc connects nothing ([capture 2](#mock-up)) | The owner's: a drop no longer connects at once |
| **Offered** | The drop connects as today; only the port mark proposes | Decided on 2026-11-12; nothing new |

Also the owner's: the chip rule for variables (a default of the view), the
port mark (new and always visible), anything that writes `diagram.json` by
itself (B above). The agent's: the edit, the naming rule, the tests.

## Consequences

- **Code.** One more line after the source step, shorter references:
  `var cartId = cart.cart.id`, then `{{cartId}}`. The diff is one line plus
  one per reader, in a commit the person makes.
- **Run results and badges.** A variable is a step: one more row and one
  more number per value, and every later badge moves by one (LAY-07 numbers
  are the rows). `far-value`: 23 → 26 steps for three values, a taller
  first row (captures 1 and 4). A value that fails has its own row.
- **Components.** A component's output is already named; the variable is a
  second name for it. It pays where readers want a field of the output
  (`cart.cart.id` → `cartId`), which is the owner's case; it is proposed
  like any step's output and never touches the component.
- **Agents writing code.** They have no positions and no proxies; A gives
  their scenarios chips at once. The style goes into the language
  reference, *Values and var* (decided 2026-11-12). A `check` warning is
  [question 5](#questions-for-the-owner).
- **Names.** The last field of the path; the step and the field
  (`cartId`) where the field alone is `id`, `value`, `status` or `body`, or
  is taken; then a number. A variable that already holds exactly this value
  is reused: the first row says *Read cartId* and writes no new line. The
  variable is selected afterwards, its name editable in the Inspector.
- **Undo.** One edit, one undo: the line and every reader come back; the
  text is byte for byte what it was.
- **Deleting the variable.** Deleting a step empties every input that reads
  it (`graph::delete_nodes`), so deleting `cartId` would empty nine
  requests. The ticket adds the inverse, *Read directly again*, on a
  variable that holds a plain reference; Delete itself is unchanged.

## What the owner will see

- New: the choice at the drop of a far value; the mark `+ var` and its menu
  at a port; *Read directly again* on a variable; a paragraph in the
  language reference.
- Changed: a variable's far readers are chips whatever the count.
- Removed or hidden: the long wires of a variable read by one or two far
  steps, one hover or one click away as every chip's wire is.
- Migrated or converted: nothing. No scenario is rewritten unless a person
  chooses a row; `diagram.json` gains nothing.
- Default behaviour that changes: a drop on a far input asks before it
  connects (question 1); the chip rule (question 2).

## Mock-up

`mock/lay-25-variables` at `91fc2d9` (view `review_lay25`, fixtures
`far-value.afs` and `far-value-saved.afs`, synthetic). Rendered by the real
board; the choice and the menu are drawn over it and edit nothing. Each
capture was opened and asserts a card in view and chips drawn.

| # | Capture | Shows |
| --- | --- | --- |
| 1 | [before, Fit](https://raw.githubusercontent.com/jakubiszon26/apiforge/91fc2d9cb752748e677188ce08ca5e09f4cfc2a5/docs/qa/2026-11-15/LAY-25/lay25-1-before-fit.png) | Today: `← cart.id` at nine readers, `→ 10` at `cart`; two long wires from `session` to `checkout` and `lookup` |
| 2 | [the choice at the drop, 60 %](https://raw.githubusercontent.com/jakubiszon26/apiforge/91fc2d9cb752748e677188ce08ca5e09f4cfc2a5/docs/qa/2026-11-15/LAY-25/lay25-2-connect-offer-60.png) | At `shipping`'s input: *Read via variable cartId* (Enter) with the line it writes and "The 8 steps that read it read cartId too. One undo.", then *Wire directly to cart.cart.id* |
| 3 | [the port mark and its menu, 60 %](https://raw.githubusercontent.com/jakubiszon26/apiforge/91fc2d9cb752748e677188ce08ca5e09f4cfc2a5/docs/qa/2026-11-15/LAY-25/lay25-3-port-suggestion-60.png) | `+ var` under `→ 10` at `cart`; the menu has two rows because the port has two fields: `var cartId = cart.cart.id` (9 steps), `var productId = cart.cart.productId` (6 steps) |
| 3b | [the same on `shop-30`, 60 %](https://raw.githubusercontent.com/jakubiszon26/apiforge/91fc2d9cb752748e677188ce08ca5e09f4cfc2a5/docs/qa/2026-11-15/LAY-25/lay25-3b-port-suggestion-shop-30-60.png) | `var token = login.body.token` (10 steps) at `login`; a second mark at `cart` stands on the row of the next output — criterion 4 forbids that |
| 4 | [after saving, Fit](https://raw.githubusercontent.com/jakubiszon26/apiforge/91fc2d9cb752748e677188ce08ca5e09f4cfc2a5/docs/qa/2026-11-15/LAY-25/lay25-4-after-saving-fit.png), [60 %](https://raw.githubusercontent.com/jakubiszon26/apiforge/91fc2d9cb752748e677188ce08ca5e09f4cfc2a5/docs/qa/2026-11-15/LAY-25/lay25-4b-after-saving-60.png) | `currency`, `cartId`, `itemId` hang under their sources; readers say `← cartId`, `← itemId`; no long wire is left; 26 steps. At this zoom the third chip of a card (`currency`) has no room and is an arrow |
| 5 | [the choice at 960 × 540](https://raw.githubusercontent.com/jakubiszon26/apiforge/91fc2d9cb752748e677188ce08ca5e09f4cfc2a5/docs/qa/2026-11-15/LAY-25/lay25-5-connect-offer-960.png) | The choice fits the minimum window; in the mock-up it runs over the Library panel, which criterion 1 forbids |

A capture has no pointer and no drag: the gestures are named in the
criteria. The captures the owner decides on are committed with the ticket.

## Recommendation

1. **Asked at the drop**, the variable first, and **the mark at the port**
   for what already exists. Never a rewrite nobody chose.
2. **A variable's far readers are chips whatever the count**; no saved
   proxy is created.
3. One edit in the core for all of it, with its inverse.

Said plainly: for a value read by three or more steps the board is the same
before and after saving, except that the chip says `cartId` instead of
`cart.id` and there is one more card. What saving buys is the code, the
name, and the one- and two-reader wires. If only the wires matter, LAY-23
measured the rule that needs no variable at all — every far wire a chip,
whatever the count: `shop-30` 20 lines · 1 crossing against 26 · 5 today,
22 of 34 data wires not drawn against 16. It is option C of question 2.

## Questions for the owner

1. **How strongly is the variable proposed when a far value is connected?**
   A. Asked: the choice at the drop, variable first under Enter
   (recommended) · B. Assumed: made at once, *Undo* in the status line, a
   setting to turn it off · C. Offered: the drop connects as today, only
   the port proposes.
2. **What stands at the far readers of a variable?** A. The board's chip,
   whatever the count; nothing saved (recommended) · B. Saved proxy chips
   placed beside each far reader, written to `diagram.json`, wider column
   gaps · C. No variable needed: every far wire is a chip whatever the
   count · D. As today: chips from three readers only.
3. **The proposal at the port.** A. `+ var` under the count, always shown
   in *Chips* (recommended) · B. Shown only while the source is selected or
   pointed at · C. No mark: the action lives in the Inspector's response
   tree only.
4. **Existing scenarios in one go.** A. Not now: one value at a time from
   the port (recommended) · B. *Save shared values as variables* in the
   board menu, one edit per scenario.
5. **Agents writing code.** A. A paragraph in the language reference only
   (recommended) · B. Also a `check` warning on a value read directly by
   three or more steps.

## Acceptance criteria

For the recommended options; a different answer rewrites the criterion it
names. Tests in `folder/acceptance_tests/lay25.rs`, on `far-value.afs` and
`shop-30.afs`, at 1600 × 1000 and 960 × 540.

1. Releasing a wire from `cart`'s port, or a field dragged from its
   response tree, on an input of a step that is not beside `cart` opens the
   choice at that input, inside the board, the first row focused. Enter or
   a click on it writes `var cartId = cart.cart.id`, makes every step that
   read exactly that value read `cartId`, the new one included, and selects
   the variable. Esc connects nothing and writes nothing. The second row
   connects as today.
2. The same drop on a step beside its source connects as today, with no
   choice.
3. Where a variable already holds exactly the value, the first row reads
   *Read cartId* and no variable is added.
4. A step's output with a field read by three or more steps, not held by a
   variable, shows `+ var` under its count; the mark covers no card text
   and no port. Its menu has one row per such field with the line it writes
   and the number of steps; a row does the edit of criterion 1.
5. One undo after either gesture restores the scenario's text byte for
   byte; redo applies it again.
6. The name follows the rule in *Consequences*: `cart.cart.id` → `cartId`,
   `login.body.token` → `token`, a taken name gets the step, then a number.
7. A variable's far reader is a chip whatever the number of readers; a
   reader beside the variable keeps its wire; after every gesture above
   `diagram.json` has no new proxy.
8. *Read directly again* on `cartId` makes its readers read `cart.cart.id`
   and removes the variable, in one undoable edit; it is absent on a
   variable that holds anything but a plain reference.
9. After a simulation the variable has its row in Run results and the
   badges equal the rows (LAY-07).
10. The language reference, *Values and var*, says when to save a value;
    `review_lay25` renders the choice, the menu and the saved board at both
    sizes and asserts a card in view.

## Not in scope

Creating or removing saved proxies; proxies for parameters and environment
values; *Assign to variable* in the response tree, which keeps creating a
variable without moving readers; a proposal while typing; a scenario-wide
action (question 4); rewriting the demo scenarios.

## Estimate

**M · 30 min · `visible UI`, `default`.** Complexity: one new edit in the
core and its inverse, two small popups and one predicate in the canvas.
Uncertainty: low for the edit; the drop has two sources (a port, a response
field) and neither may be missed. Regression
potential: the tests that drop a wire on a far input (LAY-23 criterion 9)
now meet the choice. Verification: window tests and the review view.
With answer 1 = C the ticket is S–M · 20; with 2 = B it is two tickets
(LAY-23 priced the second at M · 20, `default`).

## Technical appendix

- The edit: `edit::assign_response_variable` makes the variable;
  `Binding::map_leaves` over every node rewrites the references with the
  same node, port and path; the inverse is the same walk backwards. Both
  go through `apply_edit`, so each is one undo step.
- The drop: `Command::Connect` and `Command::Bind` in `canvas/mod.rs`
  (`handle_wire_drag`, the `Payload::Variable` drop); "beside" is
  `readers::beside`. The choice is a popup like `canvas/menu.rs`.
- The rule: one condition in `readers::chipped`; LAY-23's criterion 1
  ("a port read by fewer than three steps keeps all of its wires") is
  reworded for variables.
- The mark: beside `readers::lay_count`, with its own place in the gap so
  that it does not stand on the next output's row.
- Files: `crates/apiforge-core/src/edit/nodes.rs`,
  `crates/apiforge-desktop/src/canvas/{mod,readers,menu}.rs`,
  `crates/apiforge-desktop/src/folder/acceptance_tests/lay25.rs`,
  `docs/language/reference.md`, `fixtures/scenarios/lay/far-value.afs`.
- Found on the way, on `main`: the statement of a chip from a component's
  output reads `cart.cart.cart.id → quantity.url` where the code says
  `cart.cart.id` (`readers::texts` joins the port and a path that already
  starts with it). Not this ticket.
- The fixtures model the owner's board and hold none of its names; the
  owner's project is not in the repository.
