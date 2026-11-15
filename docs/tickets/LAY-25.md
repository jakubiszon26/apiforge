# LAY-25: A value used far away is saved in a variable — proposed by the board

- Size: M · Owner review: 35 min · Risk: `visible UI`, `default`
- State: Decided 2026-11-15 ([decisions](#owner-decisions-2026-11-15)) ·
  issue #335 · LAY sprint, after LAY-23 stage 1 (merged, #337) · the
  warning in code is split off as [LAY-26](LAY-26.md) (#341)
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
  ([capture 1](#mock-up)), variable or not.
- What still crosses the board is a far value read by **one or two** steps
  (the two long wires from `session` in capture 1), and a variable changes
  nothing there: its port fans out as the response's did.
- *Assign to variable* exists in the response tree of the Inspector and of
  Run results (`edit::assign_response_variable`): it creates the variable
  and leaves every reader reading the response. It is half of this ticket.
- Nothing proposes anything while a person connects a value.

## Where the proposal is made

| Place | Reaches | Clicks · undo | Decided |
| --- | --- | --- | --- |
| **The drop**: a wire from a port, or a field dragged from the response tree, released on an input that is not beside its source | Values as they are connected on the board | 1 (Enter) · 1 | Yes, asked (decision 1) |
| Typing a reference in an Inspector field or in code | — | — | No proposal while typing; code gets a warning afterwards ([LAY-26](LAY-26.md)) |
| **The port**: a mark `+ var` under the count `→ 10`; its menu lists the fields, one row each | Boards that exist, scenarios written in code or by an agent | 2 (mark, row) · 1 | Yes (decision 3) |
| **The board's menu**: *Save shared values as variables* | Every shared value of the scenario at once | 1 · 1 | Yes (decision 4) |

"Far" on the board is LAY-23's rule turned round: the reader is not in the
source's row in its own or the next column. The count does not matter at
the drop: the owner's case is distance.

## What "variable with proxy" becomes

The variable, and the chips the board draws (decision 2): **a variable's
far reader is a chip whatever the count** — saving a value is how a person
says "do not wire this" ([capture 4](#mock-up)). Nothing is saved besides
the one line of the scenario.

Saved proxy chips were the other way: 112 × 36 does not fit the 96 px gap
(gaps grow to about 150 px, `fan-out` from 1784 to about 2000 px, past
LAY-06's bound of 1920), each has to be kept in step in `diagram.json`,
and a scenario from code has none. They stay the only way a **parameter or
an environment value** is drawn. For a **variable** the ones a person
places stay and win (a reader beside a placed proxy keeps its short wire);
nothing creates them, nothing removes them.

## Consequences

- **Code.** One more line after the source step, shorter references:
  `var cartId = cart.cart.id`, then `{{cartId}}`. The diff is one line plus
  one per reader, in a commit the person makes.
- **Run results and badges.** A variable is a step: one more row and one
  more number per value, and every later badge moves by one (LAY-07).
  `far-value`: 23 → 26 steps for three values (captures 1 and 4). A value
  that fails has its own row.
- **Components.** A component's output is already named; the variable is a
  second name for it. It pays where readers want a field of the output
  (`cart.cart.id` → `cartId`); it is proposed like any step's output and
  never touches the component.
- **Agents writing code.** Their scenarios get chips at once. The style is
  a paragraph in the language reference, *Values and var*; MCP `check`
  says nothing about it (decision 5).
- **Names.** The last field of the path; the step and the field (`cartId`)
  where the field alone is `id`, `value`, `status` or `body`, or is taken;
  then a number. A variable that already holds exactly the value is reused.
  The new variable is selected, its name editable in the Inspector.
- **Undo.** One edit, one undo; the text is byte for byte what it was.
- **Deleting the variable.** Deleting a step empties every input that reads
  it (`graph::delete_nodes`), so deleting `cartId` would empty nine
  requests. The ticket adds the inverse, *Read directly again*; Delete
  itself is unchanged.

## What the owner will see

- New: the choice at the drop of a far value; the mark `+ var` and its menu
  at a port; *Save shared values as variables* in the *Board view* menu;
  *Read directly again* on a variable; a paragraph in the language
  reference.
- Changed: a variable's far readers are chips whatever the count.
- Removed or hidden: the long wires of a variable read by one or two far
  steps, one hover or one click away as every chip's wire is.
- Migrated or converted: nothing. No scenario is rewritten unless a person
  chooses a row or the menu entry; `diagram.json` gains nothing.
- Default behaviour that changes: a drop on a far input asks before it
  connects; the chip rule.

## Mock-up

`mock/lay-25-variables` (views `review_lay25` at `91fc2d9`,
`review_lay25_bulk` at `2cff241`; fixtures `far-value.afs` and
`far-value-saved.afs`, synthetic). Rendered by the real board; the choice,
the menu and the entry are drawn over it and edit nothing. Each capture
was opened and asserts a card in view.

| # | Capture | Shows |
| --- | --- | --- |
| 1 | [before, Fit](https://raw.githubusercontent.com/jakubiszon26/apiforge/91fc2d9cb752748e677188ce08ca5e09f4cfc2a5/docs/qa/2026-11-15/LAY-25/lay25-1-before-fit.png) | Today: `← cart.id` at nine readers, `→ 10` at `cart`; two long wires from `session` |
| 2 | [the choice at the drop, 60 %](https://raw.githubusercontent.com/jakubiszon26/apiforge/91fc2d9cb752748e677188ce08ca5e09f4cfc2a5/docs/qa/2026-11-15/LAY-25/lay25-2-connect-offer-60.png) | *Read via variable cartId* (Enter) with the line it writes, then *Wire directly to cart.cart.id* |
| 3 | [the port mark and its menu, 60 %](https://raw.githubusercontent.com/jakubiszon26/apiforge/91fc2d9cb752748e677188ce08ca5e09f4cfc2a5/docs/qa/2026-11-15/LAY-25/lay25-3-port-suggestion-60.png), [on `shop-30`](https://raw.githubusercontent.com/jakubiszon26/apiforge/91fc2d9cb752748e677188ce08ca5e09f4cfc2a5/docs/qa/2026-11-15/LAY-25/lay25-3b-port-suggestion-shop-30-60.png) | `+ var` under `→ 10`; one row per field. On `shop-30` a second mark stands on the next output's row, which criterion 4 forbids |
| 4 | [after saving, Fit](https://raw.githubusercontent.com/jakubiszon26/apiforge/91fc2d9cb752748e677188ce08ca5e09f4cfc2a5/docs/qa/2026-11-15/LAY-25/lay25-4-after-saving-fit.png), [60 %](https://raw.githubusercontent.com/jakubiszon26/apiforge/91fc2d9cb752748e677188ce08ca5e09f4cfc2a5/docs/qa/2026-11-15/LAY-25/lay25-4b-after-saving-60.png) | Three variables under their sources, `← cartId` at the readers, no long wire, 26 steps. The third chip of a card has no room at this zoom and is an arrow |
| 5 | [the choice at 960 × 540](https://raw.githubusercontent.com/jakubiszon26/apiforge/91fc2d9cb752748e677188ce08ca5e09f4cfc2a5/docs/qa/2026-11-15/LAY-25/lay25-5-connect-offer-960.png) | It fits the minimum window; in the mock-up it runs over the Library panel, which criterion 1 forbids |
| 6 | [the menu entry on `shop-30`](https://raw.githubusercontent.com/jakubiszon26/apiforge/2cff2411ea43be64df53fd8b51f665fd504f3f7d/docs/qa/2026-11-15/LAY-25/lay25-6-bulk-menu-shop-30.png), [nothing to save](https://raw.githubusercontent.com/jakubiszon26/apiforge/2cff2411ea43be64df53fd8b51f665fd504f3f7d/docs/qa/2026-11-15/LAY-25/lay25-6b-bulk-menu-nothing.png) | *Save shared values as variables (3)* last in *Board view*, after a separator; with nothing to save it is grey and says `(0)`. The board it leaves is capture 4's kind |

A capture has no pointer and no drag: the gestures are named in the
criteria. Captures 1 to 5 were decided on; capture 6 came with decision 4
and its place in the menu is still to be seen by the owner.

## Owner decisions (2026-11-15)

One quiz round on the analysis (issue #335).

1. **At the drop of a far value: asked.** A choice at the input, the
   variable first under Enter, *Wire directly* second. Not assumed, not
   only offered.
2. **At a variable's far readers: the board's chip, whatever the count.**
   Nothing saved; no proxy created. Revises decision 3 of LAY-23 for
   variables.
3. **At the port: `+ var` under the count, always shown in *Chips*.**
4. **Existing scenarios in one go: yes** — *Save shared values as
   variables* in the board's menu, one undoable edit per scenario. The
   analysis recommended one value at a time; the owner chose the action.
5. **Agents: a paragraph in the language reference only**, no warning from
   MCP `check`.
6. **Outside MCP a warning, with a fix**, in the Code view and in
   `apiforge check`, at every far read: [LAY-26](LAY-26.md).

## Acceptance criteria

Tests in `folder/acceptance_tests/lay25.rs`, on `far-value.afs` and
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
4. A request's or a component call's output with a field read by three or
   more steps, one of them far, and not held by a variable, shows `+ var`
   under its count; the mark covers no card text and no port. Its menu has
   one row per such field with the line it writes and the number of steps;
   a row does the edit of criterion 1.
5. *Save shared values as variables* in *Board view* saves, in one edit,
   exactly the values that have a row in a `+ var` menu, in Main and
   Cleanup together. On `shop-30`: `token` (ten steps), `cartId` and
   `orderId` (three each); the text gains three `var` lines, each after its
   source step, the sixteen reading steps change in their references only,
   30 steps become 33. On `far-value`: `cartId` and `productId`; not
   `session.body.currency` (two readers), not a value step such as
   `expected_total`, which is a name already. The entry says how many
   values it takes.
6. In that edit a value a variable already holds gets no second variable:
   the steps still reading it directly move to the one there. Names follow
   criterion 8, a clash between two values of one run settled in document
   order. No card moves; the new variables are selected.
7. With nothing to save the entry is shown, disabled, with `(0)` and the
   reason on hover; on a board that cannot be edited it is disabled too.
   Nothing is written.
8. The name follows the rule in *Consequences*: `cart.cart.id` → `cartId`,
   `login.body.token` → `token`, a taken name gets the step, then a number.
9. One undo after any of the three gestures restores the scenario's text
   byte for byte; redo applies it again.
10. A variable's far reader is a chip whatever the number of readers; a
    reader beside the variable keeps its wire; after every gesture above
    `diagram.json` has no new proxy.
11. *Read directly again* on `cartId` makes its readers read `cart.cart.id`
    and removes the variable, in one undoable edit; it is absent on a
    variable that holds anything but a plain reference.
12. After a simulation the variable has its row in Run results and the
    badges equal the rows (LAY-07).
13. The language reference, *Values and var*, says when to save a value;
    `review_lay25` renders the choice, the port menu, the board's menu and
    the saved board at both sizes and asserts a card in view.

## Not in scope

The warning in the Code view and the command line (LAY-26); creating or
removing saved proxies; proxies for parameters and environment values;
*Assign to variable* in the response tree, which keeps creating a variable
without moving readers; a proposal while typing; a project-wide action
across scenarios; rewriting the demo scenarios.

## Estimate

**M · 35 min · `visible UI`, `default`.** Complexity: one edit in the core
taking one value or many, and its inverse; two small popups, one menu
entry and one predicate in the canvas. Uncertainty: low for the edit; the
drop has two sources (a port, a response field) and neither may be missed.
Regression potential: the tests that drop a wire on a far input (LAY-23
criterion 9) now meet the choice; LAY-23's criterion 1 is reworded for
variables. Verification: window tests and the review views.

The scenario-wide action adds 5 minutes and no new machinery: it is the
same edit over a list. With the warning in code the ticket would pass M,
so that part is LAY-26 (S–M · 15), after this one merges.

## Technical appendix

- The edit: `edit::assign_response_variable` makes the variable;
  `Binding::map_leaves` over every node rewrites the references with the
  same node, port and path, in Main and in Cleanup; the inverse is the same
  walk backwards. The scenario-wide action folds it over the list of
  criterion 5. Each goes through `apply_edit`, so each is one undo step.
- The list of values (the `+ var` rows, the menu's count, LAY-26's
  warning) comes from one function in `apiforge-core`; the mock-up's
  `lay25::shared_values` is its sketch.
- The drop: `Command::Connect` and `Command::Bind` in `canvas/mod.rs`
  (`handle_wire_drag`, the `Payload::Variable` drop); "beside" is
  `readers::beside`. The choice is a popup like `canvas/menu.rs`.
- The rule: one condition in `readers::chipped`.
- The mark: beside `readers::lay_count`, with its own place in the gap so
  that it does not stand on the next output's row.
- Files: `crates/apiforge-core/src/edit/nodes.rs`,
  `crates/apiforge-desktop/src/canvas/{mod,readers,menu}.rs`,
  `crates/apiforge-desktop/src/folder/acceptance_tests/lay25.rs`,
  `docs/language/reference.md`, `fixtures/scenarios/lay/far-value.afs`.
- The fixtures model the owner's board and hold none of its names; the
  owner's project is not in the repository.
