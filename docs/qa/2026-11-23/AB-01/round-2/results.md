# AB-01 round 2: results

The question: what does a first-time tester need on v0.1.0 and on
v0.2.1-rc.2 to change a request's body (T3) and add two checks (T4) in an
existing test, and where do they turn the wrong way. The sheet, the start
state and the pass check are in [task.md](task.md). Nothing here is
classed: that is the planning model's, after round 4.

Run on 2026-11-23, Linux, the `kwin` MCP server at 1280 × 800, isolated
home, simulation only. Sixteen counted runs, none void, about 73 minutes
of agents. Each cell reads **shortest path · run 1 · run 2 · run 3**, in
the order the runs were made; never an average. Figures leave out the
lines a driver marked `DRIVER`.

## T3 — the second booking asks for the first booking's room

| | v0.1.0 | v0.2.1-rc.2 |
| --- | --- | --- |
| Done | yes · yes · yes · yes | yes · yes · yes · yes |
| Pointer actions | 12 · 14 · 12 · 15 | 10 · 23 · 10 · 14 |
| Text entries | 2 · 1 · 2 · 1 | 1 · 1 · 1 · 1 |
| Typed / syntax | 3/3 · 1/1 · 3/3 · 2/2 | 1/1 · 1/1 · 1/1 · 1/1 |
| Scrolls | 2 · 2 · 2 · 3 | 1 · 3 · 1 · 2 |
| Wrong turns | 1 · 3 · 4 · 5 | 0 · 8 · 1 · 3 |
| Wrong result | none | none |
| The diff | 3 (+6) · 3 (+6) · 3 (+6) · 3 (+6) | 0 · 0 · 0 · 0 |

- Every run, on both versions, changed `items[1]` to `items[0]` inside the
  body's `{{rooms.body.items[1].id}}` placeholder, so every character
  typed is placeholder syntax. No run used a picker for the value.
- Keys pressed apart from text (BackSpace, arrows, End, Escape): v0.1.0
  18 · 2 · 15 · 5, v0.2.1-rc.2 1 · 3 · 5 · 3. On v0.1.0 most are the
  repairs of a wrong edit in the body (below).
- *The diff* on v0.1.0: `on_error stop` and a `@config` line on each
  `POST` (3); in brackets, the six lines of `calendar`, which the save
  moves out of the `if` branch and chains with `@after(booked.then,
  refused)`. The run's order is the same (`check.sh` compares the order
  edges). v0.2.1-rc.2 writes the one changed line and nothing else.
- Maximised: v0.1.0 none; v0.2.1-rc.2 runs 1 and 3.

## T4 — two checks: the error code, and the id is text

| | v0.1.0 | v0.2.1-rc.2 |
| --- | --- | --- |
| Done | yes · yes · yes · yes | yes · yes · yes · yes |
| Pointer actions | 21 · 24 · 39 · 40 | 14 · 20 · 19 · 17 |
| Text entries | 0 · 0 · 1 · 1 | 0 · 0 · 0 · 0 |
| Typed / syntax | 0/0 · 0/0 · 13/0 · 13/0 | 0/0 · 0/0 · 0/0 · 0/0 |
| Scrolls | 7 · 4 · 6 · 11 | 1 · 1 · 8 · 1 |
| Wrong turns | 3 · 4 · 5 · 5 | 0 · 5 · 4 · 3 |
| Wrong result | none | none |
| The diff | 4 (+6) · 4 (+6) · 2 (+6) · 2 (+6) | 2 · 2 · 2 · 2 |

- **The way.** On v0.2.1-rc.2 every run and the shortest path used *Add
  a check* on a row of the response (the step's *Output data* tab in run
  1 and 2, the run results in run 3), which offers `error.code equals
  "slot_conflict"` and `id is a string` as menu items: nothing typed. On
  v0.1.0, run 1 used *Check* in the run results, which compares the id
  with this run's value `"bk-1"`, and changed the check to *Has type*;
  runs 2 and 3 clicked the *Add comparison* tick on the response row in
  *Output data*, which added nothing, then built two *Comparison* steps
  from the *Library* or *Add step*, chose each value through *Result of
  another step* and *Choose a response field*, and typed `slot_conflict`.
- Keys pressed apart from text: v0.1.0 0 · 1 · 1 · 0, v0.2.1-rc.2
  0 · 1 · 0 · 1.
- *The diff* on v0.1.0: `on_error stop`, a `@config` line, and a `@label`
  per check made from the run results (run 1 and the shortest path); in
  brackets the six lines of `calendar` moved as in T3. On v0.2.1-rc.2: a
  `@label` per check.
- Maximised: every run on both versions.

## The shortest paths

Each is the shortest passing first-time run with its wrong turns (and the
actions that only undid them) taken out, replayed by a fresh driver given
the brief and the list.

| | v0.1.0 | v0.2.1-rc.2 |
| --- | --- | --- |
| T3 | from run 1: 14 actions, 10 pointer. The replay missed the caret by one character, deleted `]` and typed `0` into `items[10.id`; one wrong turn and 18 keys to repair it, then passed | from run 2 (run 3 ties at 10): no wrong turn |
| T4 | from run 1: 24 actions, 21 pointer, kept the two zoom clicks and the click on *first* the run made. The replay passed with three wrong turns: the wheel did not move the run results list or the *Check id* panel | from run 3 (run 1 ties at 14): no wrong turn |

Run 3 of T3 on v0.1.0 is one pointer action shorter than run 1 once its
wrong turns are out (it closes the suggestion list with Escape, not a
click), but its edit rests on a missed click that deleted `]`; run 1 was
replayed.

## Wrong turns

Each with its version, its run (1–3 as above) and what the driver
expected.

**Both tasks, the same on both versions**

- **The test's title is shown nowhere.** The task names *Double booking
  is refused*; neither window shows a scenario's title, only file names.
  Drivers looked for it on the *begin* step, on the empty board, in the
  *Project* menu, under the arrow beside *Run*, in the run results, in the
  file's tooltip and right-click menu, on the *Scenario* label beside the
  file name and in *Checks*. Every driver then chose
  `double-booking.afs` by its name, rightly.
  - T3: v0.1.0 2 · 3 · 5 turns, v0.2.1-rc.2 8 · 0 · 2.
  - T4: v0.1.0 2 · 1 · 1, v0.2.1-rc.2 4 · 2 · 3.
  - [capture](t3-v0.2.1-rc.2-no-test-title.png): v0.2.1-rc.2, T3 run 1,
    the *Scenario* label clicked; the header holds the file name only.
- **A left drag on the board does not pan** (v0.2.1-rc.2 T3 runs 2, 3;
  v0.1.0 T4 run 3). The hint "Pan: right-drag" is below the screen until
  the window is maximised or the results open.
- **The fit button** leaves the board at 22–25 % or only jumps to the top
  (v0.1.0 T4 runs 1, 2, 3; v0.2.1-rc.2 T4 run 2).
- **The wheel** did not move the board (v0.1.0 T4 run 1), the run results
  list (v0.2.1-rc.2 T4 run 2; v0.1.0 T4 shortest path) or the *Check id*
  panel (v0.1.0 T4 shortest path, twice). Other wheel misses are marked
  `DRIVER` in nearly every log.

**T3, v0.1.0 only**

- **Editing the index in the body.** Run 1: a double click on `1`
  selected `items[1`. Run 2: shift+Right over `1` and `0` typed left
  `items[10.id}}` with "Invalid body reference … the last valid body is
  kept", repaired with End, seven Left and two BackSpace. Run 3 and the
  shortest path: the caret landed after `]` (a missed click, `DRIVER`),
  BackSpace deleted `]`; in the shortest path this became a wrong turn.
  The suggestion list opens over the field as soon as a character is
  typed and hides what was typed, on both versions; on v0.2.1-rc.2 the
  same edit gave no wrong turn (shift+Left or shift+Right over the `1`).
  [capture](t3-v0.1.0-body-field-caret.png): v0.1.0, T3 run 2.

**T4, v0.1.0 only**

- **The *Add comparison* tick on a response row in *Output data* adds
  nothing** (runs 2, 3; clicked twice and dragged onto the board in run
  2). Both runs then built the checks by hand.
  [capture](t4-v0.1.0-add-comparison-does-nothing.png): v0.1.0, T4 run 2,
  the tick on `error.code` pressed, still 11 steps.
- A button taken for a layout of the whole board opened the wire-style
  menu (run 2); ctrl+scroll did not zoom (run 3).

**T4, v0.2.1-rc.2 only**

- The request's *Checks* tab, expected to add a check (run 1): R1, #547,
  counted here and not classed again. Run 2 opened the tab and left it
  without counting a wrong turn.

## Seen in the runs, not counted as wrong turns

- v0.1.0 opens the project on *Code* with `booking-day.afs` (every run's
  first action is *Diagram*); v0.2.1-rc.2 on *Diagram* with
  `booking-day.afs` and the *Library* (the first action is *Files*): R2,
  #548.
- The window is 1280 × 836 on an 800 px screen on both versions; the
  zoom bar and the hint under the board are cut off until it is
  maximised.
- v0.1.0: selecting a step marks the file unsaved (T3 runs 1, 2, 3 and
  the shortest path; T4 runs 1, 3 and the shortest path).
- v0.1.0: the board opens as one tall column at 25 %, the steps out of
  order (`second` above `first` above `begin`); a *Comparison* dropped on
  the board lands at its far left (T4 run 2).
- v0.1.0 T4 run 1: *Check* in the run results compares with this run's
  value `"bk-1"` (REQ-12, #539).
- v0.2.1-rc.2 puts the new error-code check inside the `if` branch;
  v0.1.0 at the top level, after it. Both run and pass.
- v0.2.1-rc.2: Escape on the suggestion list also closes the side panel
  (T3 run 3).
- T3 run 1 on v0.2.1-rc.2 also ran `double-booking-notice.afs` in
  simulation while looking for the title; nothing else was changed in
  any run (only `scenarios/double-booking.afs` differs from the start).

## Void runs

None. No driver opened Code or Split, used a terminal, the CLI or an
`apiforge` MCP tool, or sent a live request. Two drivers (T4 run 1 on
v0.2.1-rc.2, T4 run 3 on v0.1.0) added `WRONG TURN` to a line when
writing the next one; that is the line's own mark, not a rebuilt log.

## What could not be checked

- Whether a driver would have found the test by its title: no window
  shows it, so every run chose by file name.
- Time is written down only: T3 3.4, 4.3, 4.8 minutes on v0.1.0 and 5.6,
  2.3, 3.3 on v0.2.1-rc.2; T4 5.6, 10.5, 10.0 against 4.2, 4.2, 2.4.

## What differs between the versions, as counted

- **T3.** Done in every run on both. Pointer actions 14 · 12 · 15 on
  v0.1.0 and 23 · 10 · 14 on v0.2.1-rc.2, inside each other's spread;
  shortest paths 12 (a list of 10) and 10. Wrong turns 3 · 4 · 5 against
  8 · 1 · 3, most of them on both versions the hunt for the test's title.
  Only on v0.1.0: the edit of the index went wrong in run 2 and in the
  replayed shortest path (15 and 18 keys pressed), and the save
  writes 3 lines and moves 6.
- **T4.** Done in every run on both. Pointer actions 24 · 39 · 40 against
  20 · 19 · 17; shortest paths 21 against 14; text typed 0 · 13 · 13
  against none; minutes 5.6 · 10.5 · 10.0 against 4.2 · 4.2 · 2.4. Wrong
  turns 4 · 5 · 5 against 5 · 4 · 3, again mostly the title. On
  v0.2.1-rc.2 each check is one menu item on a response row; on v0.1.0
  the tick in *Output data* added nothing in two runs of three, which
  then built the checks from *Comparison* steps by hand. No run on either
  version gave a wrong result.
