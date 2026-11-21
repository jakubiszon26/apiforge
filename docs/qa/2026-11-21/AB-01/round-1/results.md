# AB-01 round 1: results

The question: what does a first-time tester need on v0.1.0 and on v0.2.0
to finish T1 and T2 in the window, and where do they turn the wrong way.
The sheet, the start state and the pass check are in [task.md](task.md).

Run on 2026-11-21, Linux, the `kwin` MCP server at 1280 × 800, isolated
home, live against `apiforge-demo-api`. Sixteen counted runs, none void.
Each cell reads **shortest path · run 1 · run 2 · run 3**, in the order
the runs were made; never an average. Figures leave out the lines a
driver marked `DRIVER`.

## T1 — a new test with one request, run live

| | v0.1.0 | v0.2.0 |
| --- | --- | --- |
| Done | yes · yes · yes · yes | yes · yes · yes · yes |
| Pointer actions | 12 · 12 · 12 · 16 | 13 · 14 · 20 · 19 |
| Text entries | 2 · 2 · 2 · 3 | 2 · 2 · 3 · 3 |
| Typed / syntax | 59/0 · 59/0 · 59/0 · 62/0 | 59/0 · 59/0 · 62/0 · 62/0 |
| Scrolls | 0 · 0 · 0 · 1 | 0 · 0 · 1 · 1 |
| Wrong turns | 0 · 0 · 0 · 0 | 0 · 1 · 1 · 1 |
| Wrong result | none | none |
| The diff | 3 · 3 · 3 · 2 | 3 · 3 · 2 · 2 |

*The diff* counts lines the window wrote that the driver did not set:
on v0.1.0 `on_error stop`, `timeout 10s` and the `@label("Check id")` of a
check made from the run results; on v0.2.0 `begin = start`, `timeout 10s`
and the same `@label`. Every driver also added a check the task did not
ask for: from *Check* in the run results (`body.id == 123`), or a
*Comparison* on `status == 200` (v0.1.0 run 3 by dragging from the
*HTTP status* output; v0.2.0 runs 2 and 3 from the *Library*).

## T2 — create, read back, check, delete; twice; the delete always

| | v0.1.0 | v0.2.0 |
| --- | --- | --- |
| Done | yes · **no** · yes · yes | yes · yes · yes · yes |
| Pointer actions | 108 · 118 · 131 · 123 | 60 · 108 · 70 · 124 |
| Text entries | 25 · 21 · 22 · 27 | 11 · 24 · 14 · 33 |
| Typed / syntax | 402/66 · 360/57 · 390/79 · 418/66 | 257/44 · 479/92 · 263/48 · 599/109 |
| Scrolls | 17 · 5 · 19 · 4 | 8 · 10 · 12 · 9 |
| Wrong turns | 0 · 15 · 16 · 9 | 0 · 9 · 9 · 8 |
| Wrong result | none | none |
| The diff | 6 (+3) · 6 (+6 `@after`) · 6 (+3) · 6 (+3) | 9 · 7 · 9 · 5 |

- *Done*: v0.1.0 run 1 stopped at the last part: the test checked and
  passed live twice with everything in *Main*, and no request could be
  added to *Cleanup* (below). v0.1.0 run 3 passes the check by its
  letter: the delete and its check stay in *Main* and a second `DELETE`
  with no check was built in *Cleanup*; on a passing run that one answers
  404 `{ "deleted": false }` and is shown as passed. Its body is an
  *Object* step wired into the POST; the body sent is `productId` 123,
  `quantity` 2.
- *Syntax* is the `{{…}}` placeholders typed into the URL, wrong ones
  included (`{{createCart.response.id}}`, `{{createCart.id}}`). The id
  reached the GET and the DELETE only by typing it, in every run on both
  versions. Response paths typed into *Edit path manually* (`id`,
  `quantity`) and the handles typed into *Name in code* are not counted
  as syntax.
- *The diff* counts `timeout 10s` on each request, the POST's
  `@config({ method: "POST", timeoutMs: 10000.0 })`, `on_error stop`
  (v0.1.0) or `begin = start` (v0.2.0), and the `@label`s written by
  *Check* in the run results. v0.1.0 also writes an `@after` line for
  every step chained by hand (in brackets). Steps the drivers built
  beyond the task: v0.1.0 run 1 a `GET` after the delete and a 404 check;
  run 3 the *Object* step and the second `DELETE`.
- Keys pressed apart from text (Delete, Return, Escape, ctrl+a, ctrl+c,
  ctrl+v, ctrl+x, ctrl+z, ctrl+−) are not in the table: v0.1.0 19 · 3 · 3,
  v0.2.0 6 · 18 · 3 in the three runs, most of them `ctrl+a` before a
  field.
- Every driver ran the test twice with the delete in *Main*, as the task
  says, before making it run always.

## The shortest paths

Each is the shortest passing first-time run with its wrong turns (and the
actions that only undid them) taken out, replayed by a fresh driver
given the brief and the list. No wrong turn in any replay.

| | v0.1.0 | v0.2.0 |
| --- | --- | --- |
| T1 | from run 2 (run 1 is the same list) | from run 1, without the *Checks* tab; it begins with *Diagram* and *Files*, because the window opens on Code with the *Library* tab showing |
| T2 | from run 3 (123 pointer actions against run 2's 131); passes as run 3 does, with the delete and its check in *Main* and an unchecked `DELETE` in *Cleanup* that answers 404 | from run 2; the replay's *Create* opened *Diagram*, so its *Diagram* click did nothing and is counted |

REQ-06's walk on v0.2.0 was to be the starting list for T2 on v0.2.0;
the repository holds its figures and not its actions, so run 2 was used.

## Wrong turns

Each with its version, its run (1–3 as above) and what the driver
expected.

**T1**

- v0.2.0 runs 1, 2, 3 — the request's *Checks* tab: expected to add a
  status or id check; it says "No check reads this step yet" and offers
  no way to add one.

**T2, the same on both versions**

- The request's *Checks* tab (v0.2.0 runs 1, 2, 3): as in T1.
- The id in the URL typed as `{{createCart.response.id}}` or
  `{{createCart.id}}` (v0.1.0 run 2; v0.2.0 runs 1, 2, 3): expected the
  response's id; "`createCart` has no output `response`" / "`id`".
- Clicking a completion suggestion in the URL (v0.1.0 run 1; v0.2.0
  run 2): expected it to be inserted; the list closed.
- *Choose a response field*: a path typed and Return (v0.1.0 runs 1, 2,
  3; v0.2.0 runs 1, 2, 3, twice in run 1): expected the typed path to be
  taken ("You can also type a path"); it shows 0 fields and nothing
  accepts it. *Edit path manually* works.
- Moving the delete from *Main* to *Cleanup* (v0.1.0 runs 1, 2; v0.2.0
  runs 1, 2, 3): right-click on the block (expected a move; got *What
  should happen next?*), ctrl+x, ctrl+v, ctrl+click to add to a
  selection, dragging the block onto the *Cleanup* tab. No run moved a
  step; each rebuilt the delete in *Cleanup* or copied it (v0.2.0 run 2:
  copy, then the toolbar's paste icon, which drops its checks).
- Adding a step in *Cleanup* refused with "The edited model is not a
  valid definition for this document." (v0.1.0: every request in run 1,
  five times; a *Comparison* in runs 2 and 3, three times each; v0.2.0:
  a *Comparison* once in run 1). Where the driver could tell, *Main*
  already had a step of the default name (`request`, `check`). In v0.1.0
  run 2 a check in *Cleanup* was made only by *Check* in the run results;
  in run 3 none was made.

**T2, v0.1.0 only**

- A new step lands unconnected, often above the others (runs 1, 2, 3):
  *Add step* placed it before *create cart* (run 1, undone);
  *Tidy up* did not reorder (run 2); the order was set by hand through
  each block's *Next: Connect to…*, five times in run 2.
- The *Stop on error* control disabled (run 3), and no "always run"
  option on a block (run 1): expected a way to make the delete always
  run.
- Other single turns: *Body "Optional"* on the block does nothing (run 1);
  *All steps* closed the side panel, ctrl+a on the canvas, ctrl+− scaled
  the whole interface, the canvas wheel panned away (run 2); the + picker's
  *Data from* closed it (runs 2, 3); a top-bar button after *Create*,
  and a click on the canvas to reach the side panel (run 3).

**T2, v0.2.0 only**

- *Output data* tab of a request (runs 1, 3): expected the response
  fields; it shows a sample HTTP status.
- *Next: Connect to…* opens an empty list (run 2).
- A check card in the *Checks* panel does not open its settings; the
  *Expected value* picker shows 0 fields after an edit (run 1).

## Seen in the runs, not counted as wrong turns

- Both versions open a new scenario in *Code* by themselves (v0.1.0 after
  *Create* every time; v0.2.0 on opening the project, and after *Create*
  unless *Diagram* was chosen first). Every driver left at once with
  *Diagram*; no driver acted in Code. The *Diagram* click is counted.
- At 1280 × 800 the bottom of the side panel (the POST's *Body*, a
  check's *Expected value*) is out of reach on both versions: drivers
  maximised the window (v0.1.0 runs 1, 3; v0.2.0 run 3) or pressed
  ctrl+− (v0.2.0 run 1). The mouse wheel often does not move the panel;
  drivers dragged its scrollbar.
- *Check* in the run results compares with this run's value: on v0.2.0
  run 2 the id check was first written with the literal id and rewired by
  hand to `createCart.body.id`. No final file kept a literal id; no run
  passed once and failed the second time.
- v0.2.0 run 1: one exploration run stopped at a half-built check before
  the delete, and one cart was left on the demo API.
- v0.1.0 writes the new scenario at the project root, v0.2.0 under
  `scenarios/`.

## Void runs

None. No driver used Code, a terminal, the CLI or an MCP tool of the
application.

## What could not be checked

- The shortest path of T2 on v0.1.0 replays run 3, the shortest passing
  run, whose driver rebuilt its log for actions 1–33 from its click
  positions (marked as such in its log).
- Whether the delete in *Cleanup* runs when a check fails: no driver
  made a check fail, and the pass check does not try it.
- No captures: the drivers' screenshots are deleted when a `kwin` session
  stops, and none was kept.
- Time is written down only: T1 about 1–2 minutes a run on both
  versions; T2 8.5, 10.5 and 9.4 minutes on v0.1.0 and 8.6, 6.4 and 8.7
  on v0.2.0.

## What differs between the versions, as counted

- **T1.** Done in every run on both versions, with no syntax typed.
  Pointer actions 12 · 12 · 12 · 16 on v0.1.0 and 13 · 14 · 20 · 19 on
  v0.2.0. Every first-time run on v0.2.0 had one wrong turn, the
  request's *Checks* tab; no run on v0.1.0 had one. The shortest paths
  are 12 and 13: v0.2.0's begins with *Diagram* and *Files*.
- **T2.** On v0.1.0, two of three first-time runs passed: run 1 could add
  no request to *Cleanup*, and in run 3 (and its shortest path) the
  delete with its check stayed in *Main* and *Cleanup* got a second,
  unchecked `DELETE`. On v0.2.0 all three passed with the delete in
  *Cleanup*. Pointer actions 118 · 131 · 123 against 108 · 70 · 124 (the
  shortest paths 108 against 60); wrong turns 15 · 16 · 9 against
  9 · 9 · 8; syntax typed 57 · 79 · 66 against 92 · 48 · 109, inside each
  other's spread. The same on both: the id typed by hand as `{{…}}`, a
  typed path refused in *Choose a response field*, no way to move a step
  from *Main* to *Cleanup*, and the lines the window writes (5 to 9 per
  file). Only on v0.1.0: *Cleanup* refused a new step in every run, and
  steps were chained by hand. Only on v0.2.0: the *Checks* tab and the
  *Output data* tab as dead ends. No run on either version gave a wrong
  result; no run opened Code.
