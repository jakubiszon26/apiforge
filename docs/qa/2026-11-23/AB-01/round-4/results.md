# AB-01 round 4: results

The question: what does a first-time tester need on v0.1.0 and on
v0.2.1-rc.2 to put a new request between two steps and run one step
earlier (T6), and to change a value in the code and see it on the
diagram, then the other way (T7). Where do they turn the wrong way? The
sheet, the starts and the pass check are in [task.md](task.md). Nothing
here is classed; the planning model classes all four rounds together.

Run on 2026-11-23, Linux, the `kwin` MCP server at 1280 × 800, isolated
home, simulation only. **The round reached its budget early.** It made
six counted first-time runs (T6 run 1 and T7 runs 1 and 2, on both
versions) and three shortest paths, in 84.6 of its 90 minutes of agents;
one attempt void by the coordinator is counted in that time (see [what
was not run](#what-was-not-run)). Each cell reads **shortest path · run
1 · run 2**, in the order the runs were made; `—` is a run not made.
Never an average.
Figures leave out the lines a driver marked `DRIVER`.

## Read this first: the first screen

v0.1.0 opens the project on *Code*. For T6 its first screen shows lines
1 to 39 of *Room booking day*, the `@after` lines that order it among
them; for T7 the whole of `cart.afs`, which is where T7 starts. The T6
driver on v0.1.0 switched to *Diagram* as its first action. It says it
did not knowingly act on the source, but "cannot rule out that having
seen it shaped how I read the order". v0.2.1-rc.2 opens both on
*Diagram*; for T7 its first screen shows the `created` card with "Body ·
productId: 123, quantity: 2", which the driver of run 1 used only to
check the starting values.

## T6 — a new request between profile and team; policy before equipment

| | v0.1.0 | v0.2.1-rc.2 |
| --- | --- | --- |
| Done | — · yes · — | yes · yes · — |
| Pointer actions | — · 89 · — | 44 · 58 · — |
| Text entries | — · 6 · — | 4 · 4 · — |
| Typed / syntax | — · 86/0 · — | 52/0 · 52/0 · — |
| Scrolls | — · 14 · — | 8 · 5 · — |
| Wrong turns | — · 14 · — | 4 · 11 · — |
| Wrong result | none | none |
| The diff | — · 4 (+7) · — | 2 · 2 · — |

- **Typed.** Every run typed the address `/rooms/room-a/photos` and a JSON
  sample body for the new request (v0.1.0 `{ "items": ["photo-1.jpg"] }`,
  28 characters; v0.2.1-rc.2 `{"photos": []}`, 14). The simulation
  refuses a request without a sample ("No sample response for the
  simulation."), on both versions. No language syntax was typed. On
  v0.1.0 the run also typed the name `photos` three times (once
  garbled, below); on v0.2.1-rc.2 the names `policy` and `short_enough`
  of the copies, and the new request kept the name `request`.
- **The way, v0.1.0.** The new request: *Add step* with `profile`
  selected puts it loose at the top of the board; the right-click menu
  "After: profile" adds one that the run then skips ("Step outside the
  active execution path"); the run deleted both, dragged the first card
  beside `profile` in four drags with right-drag pans, and drew two
  order wires, `profile` → `photos` → `team`. The order of `policy`: in
  the *Execution order* list at the foot of a step's side panel, *Next →
  equipment* on `short_enough` was refused, "This connection would
  create a cycle"; the run then removed `has_screen` → `policy` and
  `short_enough` → `holidays`, and set `has_screen` → `holidays`, `rooms`
  → `policy` and `short_enough` → `equipment`. The file has the orders as
  `@after` lines, which `check.sh` confirms follow from the file.
- **The way, v0.2.1-rc.2.** *Add step* with `profile` selected puts the
  new request right after `profile`, in the chain. There is no way to
  move a step (below), so `policy` and its check were copied with the
  checks shown as steps, pasted after `rooms`, the originals deleted and
  the copies renamed from `policy_2` and `short_enough_2`.
- **Both:** the sample body is set under *Contract and sample response*
  at the foot of the side panel, below *Execution order*; both runs
  reached it only after collapsing the run results and dragging the
  panel's scrollbar.
- *The diff* on v0.1.0: `on_error stop`, and a `@label`, a `@config` and a
  `timeout 10s` line on the new request; in brackets `slot` moved above
  the new request (it now runs after `profile`) and the six lines of
  `calendar` moved out of the `if` branch as in rounds 2 and 3. On
  v0.2.1-rc.2: a `@config` and a `timeout 10s` line on the new request.
  The moves the task asked for are not counted.
- The new request has no `Authorization` header on either version, and is
  sent to `…/demo/rooms/rooms/room-a/photos`, as the base address ends
  in `/rooms`; the same on both.
- Keys pressed apart from text: v0.1.0 11; v0.2.1-rc.2 3 · 8.
  Maximised: every run.
- Minutes: v0.1.0 40.6; v0.2.1-rc.2 9.3 · 18.4.

## T7 — a value changed in Code seen on the diagram, and the other way

| | v0.1.0 | v0.2.1-rc.2 |
| --- | --- | --- |
| Done | yes · yes · yes | yes · yes · yes |
| Pointer actions | 7 · 9 · 6 | 8 · 9 · 7 |
| Text entries | 2 · 2 · 2 | 2 · 2 · 2 |
| Typed / syntax | 4/0 · 4/0 · 2/0 | 2/0 · 2/0 · 2/0 |
| Scrolls | 6 · 7 · 2 | 2 · 4 · 3 |
| Wrong turns | 1 · 4 · 0 | 1 · 3 · 0 |
| Wrong result | none | none |
| The diff | 2 · 2 · 2 | 0 · 0 · 0 |

- **The way, on both.** In *Code*, over-type the `2`; *Diagram*; find
  the value; edit `123` in the side panel's *Body* field (JSON);
  *Code*; *Save*. No run used a field on the card.
- **Where 3 is seen.** On v0.2.1-rc.2 on the `created` card itself, "Body
  · productId: 123, quantity: 3", at the zoom the board opens at (after
  maximising, at 45 %, the card reads "Body · 2 fields"). On v0.1.0 the
  card shows "{ 2 fields }" at every zoom tried and nothing opens it
  (run 1 clicked and double-clicked it); the value is seen only in the
  side panel's *Body* box, below *Arguments*.
  [capture](t7-v0.1.0-value-only-in-side-panel.png): v0.1.0, T7 run 1,
  the card "{ 2 fields }" beside the panel's `"quantity": 3`.
- **The side panel's foot.** On both versions the *Body* field is below
  the screen in a window 836 px tall on an 800 px screen, and the wheel
  stops short of it. Run 1 and the shortest paths maximised the window,
  then dragged the panel's scrollbar or scrolled with the pointer on a
  heading; in run 2 on both versions the window filled the screen by
  itself and two or three wheel scrolls reached *Body*.
- *The diff* on v0.1.0: `on_error stop` and `@config({ method: "POST" })`,
  written when the value is changed on the diagram. v0.2.1-rc.2 writes
  the one changed line.
- Keys pressed apart from text: v0.1.0 1 · 1 · 3; v0.2.1-rc.2
  3 · 3 · 3.
- Minutes: v0.1.0 2.5 · 3.8 · 1.7; v0.2.1-rc.2 2.4 · 3.3 · 2.0.

## The shortest paths

Each round's cell had one first-time run when the shortest paths were
made, so each list is that run with its wrong turns, the actions that
only undid them, hovers and `DRIVER` lines taken out, replayed by a
fresh driver given the brief and the list.

| | v0.1.0 | v0.2.1-rc.2 |
| --- | --- | --- |
| T6 | **not run**: the round's budget. Run 1 is 122 lines; its list would be about 90 actions | from run 1: 48 actions, 44 pointer. Replayed with four wrong turns: a ctrl+click that dropped the first of two selected steps, a right-drag pan that did nothing while the run results were open, and two looks for the sample response; the replay added scrolls to reach steps and the expander off screen |
| T7 | from run 1: 15 actions. Replayed with one wrong turn (the panel's wheel before maximising) | from run 1: 15 actions. Replayed with one wrong turn (the same) |

## Wrong turns

Each with its version, its run and what the driver expected.

**T6, v0.1.0 (run 1)**

- **Placing the new request.** *Add step* with `profile` selected put it
  loose at the top of the board (8); a request dropped from the
  *Library* beside `team` went to the bottom of the column (53); the
  right-click menu, expected to hold *Delete*, is the add-step menu (45).
- **Ordering.** *Next → equipment* on `short_enough` refused as a cycle,
  while `equipment` still ran before `policy` (98); the board's
  *Execution order* setting was not found at first (24, found at line
  74).
  [capture](t6-v0.1.0-order-cycle-refused.png): v0.1.0, T6 run 1, the
  refusal above the column, `short_enough` → `holidays` in the panel.
- **The sample body.** *Output data* says "A request can also have a
  sample response" with no field (69); the side panel did not scroll to
  the opened *Contract and sample response* (76).
- **Navigation.** The wheel did not move the board (2) or the side panel
  (22, the window cut off); the button taken for a layout opened the
  wire-style menu (3); *fit* and *zoom out* stop at 25 % with one tall
  column (26, 51).
- **Typing.** The name `photos` came out `hotosp`, the caret jumping to
  the start after the first letter (11).
- A failed check's *Inputs* name neither request nor field (87): round
  3's finding, counted here.

**T6, v0.2.1-rc.2 (run 1; shortest path)**

- **No way to move a step** (run 1, eight turns): dragging a card moves
  it on the canvas only, also when dropped on the order line (10, 22);
  the right-click menu only adds a step (8); ctrl+x does nothing (21);
  *Selection actions* holds Copy, Duplicate, Save as component and Delete
  (24); the board-view button is the wire menu (16); a hover shows no
  handle (14); the side panel's scroll stopped above *Execution order*
  while the run results were open (12).
  [capture](t6-v0.2.1-rc.2-no-move-in-selection-actions.png):
  v0.2.1-rc.2, T6 run 1, *Selection actions* on `policy`.
- **Copying a step leaves its check behind** (33); the copy then has "No
  check reads this step yet."
- **The sample body**, as on v0.1.0: *Output data* has no field (shortest
  path 36); the panel ends above it while the results are open (run 1
  56; shortest path 41); "Sample responses" in the results header does
  nothing (run 1 58).
- Shortest path: a ctrl+click dropped the first selected step (20); a
  right-drag pan did nothing with the results open (32).

**T7, both versions**

- **The side panel's wheel** stops before *Body* (v0.1.0 run 1 at 6 and
  9, shortest path 5; v0.2.1-rc.2 run 1 at 10 and 12, shortest path 8):
  the window is cut off until maximised, and after it the wheel moves
  the panel only with the pointer off a field.
- **A double click on `2` in the code** also takes the space after it
  (v0.2.1-rc.2 run 1 at 2, given up; on v0.1.0 run 1 and the shortest
  path it gave `quantity: 3}`, put right by the round trip).

**T7, v0.1.0 only**

- The card's "{ 2 fields }" does not open on a click or a double click
  (run 1, 12 and 13).

## Seen in the runs, not counted as wrong turns

- **The window is 1280 × 836 on an 800 px screen** on both versions in
  most runs (in T7 run 2 on both it filled the screen by itself); every
  T6 run maximised it; on v0.2.1-rc.2 maximising re-fits the board
  (T7: the card's values become "Body · 2 fields").
- **v0.1.0 board.** One tall column at 25 %, the order wires in one
  bundle (the v0.1.0 board order of round 3), counted here.
- **A warning that leaves the window, v0.2.1-rc.2** (T7 run 1 and the
  shortest path): "A far step reads this value directly; save it as a
  variable" on line 11 shows when the project opens and is gone from
  *Problems* after the edit on the diagram, though line 11 is
  untouched; the CLI's `check` still prints it on the saved file.
- **Typing a digit in the code** opens a list of step names on both
  versions (T7).
- The T6 run on v0.1.0 left the cycle message on screen to the end; the
  run order also moved `slot` (a value with no reads) to after
  `profile`.
- No run named the test by its title: T6 opened on *Room booking day*
  in both windows, T7 on the project's only test.

## Void runs

- **None by a driver.** No driver opened *Code* or *Split* in T6, used a
  terminal, the CLI or an `apiforge` MCP tool, or sent a live request.
  In T6 run 1 on v0.1.0 one line (91) was written one action late, with
  the next line; the rest of the log was written as it went, and the run
  is counted.
- **One attempt void by the coordinator**: the first attempt at T7's
  shortest path on v0.2.1-rc.2 started on a folder that had not been
  copied; the window showed "Invalid project-relative path." and the
  driver stopped before its first action. Made again (counted above).

## What was not run

The budget of 90 minutes of agents: T6 run 1 on v0.1.0 took 40.6
minutes on its own. Not made: runs 2 and 3 of T6 on both versions, run 3 of T7 on both, and the shortest path of T6 on v0.1.0. Total counted: 84.6
minutes.

## What differs between the versions, as counted

- **T6.** Done in the one run on each version, with no wrong result.
  v0.1.0: 89 pointer actions, 14 wrong turns, 40.6 minutes, no shortest
  path. v0.2.1-rc.2: 58 pointer actions, 11 wrong turns, 18.4 minutes;
  its shortest path 44 pointer actions and 9.3 minutes. On v0.1.0 the
  new request was put in place by dragging its card and drawing two
  order wires, and `policy` by four changes in *Execution order* lists,
  one refused as a cycle. On v0.2.1-rc.2 *Add step* put the request in
  place in one click, and `policy` was moved by copy, paste, delete and
  two renames, as nothing moves a step; eight of its eleven wrong turns
  are the search for a move. On both the sample response a new request
  needs sits at the foot of the side panel and was hard to reach.
- **T7.** Done in every run on both, with no wrong result. Pointer actions 9 · 6 on v0.1.0 against 9 · 7; shortest paths 7 against 8; wrong turns 4 · 0 against 3 · 0, most of them the side panel's wheel; 1.7 to 3.8 minutes against 2.0 to 3.3. The way is the same on both. On v0.2.1-rc.2 the card shows the changed value; on v0.1.0 only the side panel does. v0.1.0 writes two lines more when the value is changed on the diagram.
