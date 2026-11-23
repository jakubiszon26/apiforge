# AB-01 round 3: results

The question: what does a first-time tester need on v0.1.0 and on
v0.2.1-rc.2 to say why a test fails (T5) and to read, without running
it, what a test sends and in what order (T8), and where do they turn
the wrong way. The sheet, the key and the pass check are in
[task.md](task.md). Nothing here is classed: that is the planning
model's, after round 4.

Run on 2026-11-23, Linux, the `kwin` MCP server at 1280 × 800, isolated
home, simulation only. Twelve counted runs, one void run repeated, three
shortest paths; the fourth shortest path (T8 on v0.1.0) was not run, as
the round reached its 90 minutes of agents (about 86 minutes counted,
plus an interrupted replay). Each cell reads **shortest path · run 1 ·
run 2 · run 3**, in the order the runs were made; never an average.
Figures leave out the lines a driver marked `DRIVER`.

## Read this first: v0.1.0 opens on the source

v0.1.0 opens the project on *Code* with `booking-day.afs`, so its first
screen shows lines 1 to 39 of the scenario's source: the sign-in,
`profile`, `team`, `big_enough team.body.size > 0`, `buildings`,
`floors`, `rooms`, `equipment` and the start of `policy`. The brief
forbids opening *Code*, not reading the screen the window opens on (R2,
#548). Every driver on v0.1.0 switched to *Diagram* as its first action,
and every one says its answer came from the diagram and the run results;
two T8 drivers (run 1 and run 3) say they had read those lines first and
that their answers agree with them. **For T8 this is a possible
advantage for v0.1.0**: the check after the team and the order of the
first eight requests were on screen before any action. For T5 the
source shows nothing of the failure. v0.2.1-rc.2 opens on *Diagram*.

## T5 — which check of which request fails, what came back, what was expected

| | v0.1.0 | v0.2.1-rc.2 |
| --- | --- | --- |
| Answer | right · right · right · right | right · right · right · right |
| Pointer actions | 7 · 15 · 7 · 6 | 6 · 8 · 6 · 6 |
| Text entries | 0 · 0 · 0 · 0 | 0 · 0 · 0 · 0 |
| Typed / syntax | 0/0 each | 0/0 each |
| Scrolls | 3 · 7 · 2 · 1 | 5 · 2 · 4 · 4 |
| Wrong turns | 0 · 5 · 0 · 1 | 0 · 1 · 0 · 0 |
| Wrong result | none | none |
| The diff | 0 each | 0 each |

- **The way, on both.** Run the test; the run results open on the failed
  check `invited`, "The comparison failed. EXPECTED 5 → ACTUAL 4"; enlarge
  the results; open the request just above it, `invite`, and its
  *Outputs* (`invited: 4`). v0.1.0 adds one click (*Diagram* first).
- **Where 5 comes from.** The failed check's *Inputs* show `actual: 4`,
  `expected: 5` and do not name the request or field either reads, on
  both versions. On v0.2.1-rc.2 every run and the shortest path named 5
  as the team's size, after the diagram marked the `team` card "6, 21 · ×
  1/2 failed"; on v0.1.0 only run 1 did (from `team` and its size), runs
  2 and 3 and the shortest path gave 5 without saying where it comes
  from.
- **The key's words.** Every answer names `invited`, `invite` / `POST
  /invitations`, 4 and 5. The sheet asks for "the expected 5 (the size of
  the team)"; the answers that give 5 without the team are graded right,
  as the expected value is named.
- Keys pressed apart from text: v0.1.0 0 · 1 · 0 · 0 (ctrl+z),
  v0.2.1-rc.2 none. Maximised: none.
- Minutes: v0.1.0 1.6 · 4.7 · 1.1 · 1.1; v0.2.1-rc.2 1.8 · 1.5 · 1.3 ·
  1.3.

## T8 — which requests are sent, in what order, and what is checked after the team

| | v0.1.0 | v0.2.1-rc.2 |
| --- | --- | --- |
| Answer | not run · right in part · right · right in part | right · right · right · right |
| Pointer actions | not run · 49 · 25 · 31 | 21 · 18 · 17 · 21 |
| Text entries | 0 each | 0 each |
| Typed / syntax | 0/0 each | 0/0 each |
| Scrolls | not run · 6 · 5 · 3 | 6 · 2 · 7 · 4 |
| Wrong turns | not run · 10 · 6 · 5 | 0 · 4 · 5 · 0 |
| Wrong result | none | none |
| The diff | 0 each | 0 each |

- **Right in part, v0.1.0.** Run 1: the thirteen requests of the main
  path and the branch in order, the check after the team right; cleanup
  left out. Run 3: the check after the team right, the first nine
  requests in order, `hold` put before `first`, `second` and `calendar`
  listed after `notice` without the branch, cleanup left out; the
  driver says the order "from holidays on" is inferred from what each
  step reads, not seen.
- **The check after the team** (`big_enough`, size above 0) is right in
  every run on both versions.
- **The branch.** Every right answer, on both versions, gives `second`
  and `calendar` as the *yes* branch of `booked` (first's status is 201)
  without saying that it is the one taken; graded right, as the requests
  and their order are those of the key.
- **The way, v0.2.1-rc.2.** The board is three rows in step-number order
  with a dashed order line; the `team` card's chip "6, 21 · 2 checks"
  opens its *Checks* tab with `big_enough`; the *IF / ELSE* panel lists
  "Yes → second, refused, calendar, one_booking; No → not_booked" under
  *Execution order*; the *Cleanup* tab shows the two `DELETE`s; *Open
  component* shows the sign-in's `POST /sessions`.
- **The way, v0.1.0.** The board opens as one column of 29 cards at 25 %,
  not in run order (from the top: `not_booked`, `one_booking`, `second`,
  … `big_enough`, `team`, `begin`, `token`, …), with every order wire in
  one dashed bundle beside it. Run 1 read the order from the list of
  token's *Next: Connect to…* drop-down, assuming that list follows the
  run, and checked a few *Next* links. Run 2 pressed *Tidy up* ("Lay out
  the whole board"), which lays the board out as one row in run order,
  and read it through the minimap; this marks the file unsaved (not
  saved). Run 3 walked the column by right-drag and inferred the order;
  its click on *Tidy up* was refused by the session's own permission
  guard as a possible change, not by the window.
- Keys pressed apart from text: v0.1.0 4 · 1 · 0, v0.2.1-rc.2 0 · 0 · 0
  · 0. Maximised: every run on v0.1.0 (the window is 836 px tall on an
  800 px screen); none on v0.2.1-rc.2.
- Minutes: v0.1.0 18.0 · 8.0 · 8.9; v0.2.1-rc.2 3.0 · 4.5 · 5.1 · 4.2.

## The shortest paths

Each is a clean passing first-time run (no wrong turn, no `DRIVER`
line), replayed by a fresh driver given the brief and the list.

| | v0.1.0 | v0.2.1-rc.2 |
| --- | --- | --- |
| T5 | from run 2: 9 actions, 7 pointer. Replayed with no wrong turn; four scrolls of the results list fell short (`DRIVER`) | from run 2: 10 actions, 6 pointer. Replayed with no wrong turn; four short scrolls (`DRIVER`) |
| T8 | not run: the round's budget was reached. The only right first-time run, run 2, has six wrong turns and rests on *Tidy up* | from run 3: 25 actions, 21 pointer. Replayed with no wrong turn; two scrolls added because the side panel keeps its scroll position |

- T5 on v0.1.0: run 3 is one pointer action shorter once its wrong turn
  is out, but it has a wrong turn and a `DRIVER` line; run 2 is clean
  and was replayed, as round 2 learned.
- T8 on v0.2.1-rc.2: run 2 (17 pointer, five wrong turns, all hovers
  and wheel misses) and run 1 (18, four) are shorter once their wrong
  turns are out; run 3 is the clean one and was replayed. The first
  replay of it was cut off at its 15th action by a usage limit on the
  coordinator; its session was stopped and the replay was made again
  with a fresh driver, which is the one counted.

## Wrong turns

Each with its version, its run and what the driver expected.

**T5, v0.1.0**

- **The run results list cannot be scrolled to the failed check** (run 1,
  twice): the list stops at `equipment` / `policy` with steps below.
  Other runs reached `invite` after several wheel tries (`DRIVER` lines
  on both versions).
  [capture](t5-v0.1.0-results-list-stops.png): v0.1.0, T5 run 1, the
  list stopped at `slot` / `equipment`.
- **The failed check's *Outputs* / *Inputs* do not name the request**
  (run 1 *Outputs*, run 3 *Inputs*). The same panel on v0.2.1-rc.2 shows
  the same two numbers; only the run of v0.1.0 counted it a wrong turn.
- Run 1 also looked for the check on the board: the wheel moved one card,
  and `calendar` was "Skipped, step outside the active execution path".

**T5, v0.2.1-rc.2**

- The wheel on the check's detail did nothing (run 1).

**T8, v0.1.0**

- **The board does not show the order.** One column not in run order,
  the order wires bundled and untraceable; selecting a card lights none
  of its wires (run 1, run 3); no panel says what runs before or after
  (run 1 three times, run 2, run 3); *Variables* and the arrow beside
  *Run simulation* looked for a list of steps (run 1, run 3); the
  *Next → token* link in the panel is not clickable (run 1).
  [capture](t8-v0.1.0-board-not-in-run-order.png): v0.1.0, T8 run 3,
  `big_enough` above `team`, `begin` below it, the order wires in one
  bundle at the left.
- **Navigation.** Ctrl+wheel does not zoom (run 1, 2, 3); the wheel and a
  left drag do not move the board before the window is maximised (run
  2, and the wheel in run 3); the button beside *Tidy up* taken for a
  layout opened the wire-style menu (run 2; run 1 opened it looking for
  options); a right-drag pan left the row (run 2).
- Run 1: ctrl+z after a click on `invite` marked the file changed; the
  mark stayed.

**T8, v0.2.1-rc.2**

- **The *IF / ELSE* outputs carry no *Yes* / *No* label** and no
  tooltip, on the wire or the port (run 1 twice, run 2 twice; `second`'s
  panel does not say which branch leads to it, run 1). The branches are
  read at the bottom of `booked`'s panel.
  [capture](t8-v0.2.1-rc.2-branch-unlabelled.png): v0.2.1-rc.2, T8 run
  2, the pointer on the green branch wire; no label appears.
- The wheel over a side panel's field does not scroll the panel (run 1,
  run 2); Ctrl+wheel does not zoom (run 2); a cut-off address shows no
  full path on hover (run 2).

## Seen in the runs, not counted as wrong turns

- **One click marks the file changed, v0.1.0.** A single click on the
  `invite` card marked `booking-day.afs` modified, with *Save* enabled
  and "Graph changed · results from an earlier version" (T5 run 1, T8
  run 1). Neither saved; the files are byte for byte the start. Round 2
  saw selecting a step mark the file unsaved on v0.1.0.
- **The failure badge on the wrong card, v0.2.1-rc.2.** After the run,
  the `team` card is drawn red "6, 21 · × 1/2 failed", while `team`
  passed and the failed check follows `invite` (T5 run 1, 2, 3 and the
  shortest path). No driver was misled in the end; three said a tester
  could be.
- **Cut-off addresses, v0.2.1-rc.2.** Long paths are cut off on the card
  and in the panel's address field (`floors`, `equipment`, `hold`,
  `notice`); the side panel keeps its scroll position from one step to
  the next.
- **The test's title.** The task names *Room booking day*; every driver
  took `booking-day.afs`, which both windows open on. No wrong turn was
  spent on the title this round (round 2's "no scenario title in either
  window").
- The window is 1280 × 836 on an 800 px screen on both versions; on
  v0.1.0 the zoom bar and the hint are below the screen until it is
  maximised.
- R1 (#547), the request's *Checks* tab: on v0.2.1-rc.2 the *Checks* tab
  of `team` showed the check after the team, which is what T8 asked; not
  a wrong turn here.

## Void runs

- **T8 on v0.1.0, run 1, first attempt: void.** The driver made one
  terminal call (a bare `echo` that printed nothing), against the
  brief. Its answer was right in part (the check after the team and the
  first eight requests; the rest of the order unknown, cleanup not
  found) after 19 minutes. Repeated once with a fresh driver; the repeat
  is run 1 above.
- No driver opened *Code* or *Split*, ran the test in T8, sent a live
  request or saved a file: in every run the `.afs` files and the
  manifest are byte for byte the start.

## What could not be checked

- The shortest path of T8 on v0.1.0: the round reached its budget.
- Whether the v0.1.0 T8 answers would have been the same without the
  source on the first screen.
- Time is written down only (above).

## What differs between the versions, as counted

- **T5.** Right in every run on both. Pointer actions 15 · 7 · 6 on
  v0.1.0 against 8 · 6 · 6; shortest paths 7 and 6. Wrong turns 5 · 0 · 1
  against 1 · 0 · 0. The way is the same on both: run, the failed check,
  the request above it. On v0.2.1-rc.2 every answer said the 5 is the
  team's size; on v0.1.0 one of four did.
- **T8.** On v0.2.1-rc.2 right in every run and the shortest path, in
  4.2 to 5.1 minutes, with 17 to 21 pointer actions and 0 to 5 wrong
  turns. On v0.1.0 right in one run of three (run 2, by *Tidy up*), right
  in part in two, in 8.0 to 18.0 minutes, with 25 to 49 pointer actions
  and 5 to 10 wrong turns, every run maximised; the v0.1.0 drivers had
  the start of the source on their first screen. Most v0.1.0 wrong turns
  are a board that does not show the order; on v0.2.1-rc.2 they are the
  unlabelled branch.
