# AB-01 round 1: the differences, classed

Proposed by the planning model on 2026-11-23 from [results.md](results.md);
**the owner confirms the list and the classes**
([regressions](../../../../process/ab-testing.md#regressions)). Each line
names the cell or the list of results.md it rests on. A difference inside
the spread of the three runs is reported as none. The new issues carry no
sprint and no `ready` label.

## Worse on v0.2.0

| | Finding | Evidence | Class | Issue |
| --- | --- | --- | --- | --- |
| R1 | The request's *Checks* tab is where a tester goes to add a check, and it cannot add one. The tab is new in v0.2.0 | T1 *Wrong turns* 0 · 0 · 0 · 0 against 0 · 1 · 1 · 1; the same turn in T2 runs 1, 2, 3 | **2** — a wrong turn in every first-time run | DIAG-15 [#547](https://github.com/jakubiszon26/apiforge/issues/547), new |
| R2 | An empty project opens on Code with the *Library* showing: one action more before the first request | T1 shortest path 12 against 13 | **3** | WS-04 [#548](https://github.com/jakubiszon26/apiforge/issues/548), new |

T1's pointer actions (12 · 12 · 16 against 14 · 20 · 19 in the first-time
runs) overlap and are not a finding of their own: what they hold beyond
R1 and R2 is the unasked check each driver added, by a longer way in
v0.2.0 runs 2 and 3.

## Better on v0.2.0

- T2 is finished by every first-time run (v0.1.0: two of three, and run 3
  only by the letter of the check).
- A step can be added in *Cleanup*: refused eleven times in three runs on
  v0.1.0, once on v0.2.0. The one that is left is DIAG-17
  [#551](https://github.com/jakubiszon26/apiforge/issues/551).
- A new step joins the chain: nothing is connected by hand, and no line
  per connection is written (v0.1.0: `@after`, +3 to +6 lines).
- The delete can be made to run always (v0.1.0: *Stop on error* disabled,
  no such option on a block).
- T2's shortest path: 60 pointer actions and 11 text entries against 108
  and 25. T2's wrong turns: 9 · 9 · 8 against 15 · 16 · 9.

## The same on both

| Finding | Evidence | Issue |
| --- | --- | --- |
| The id reaches the GET and the DELETE only typed as `{{…}}`; wrong placeholders typed | syntax typed in every run; *Wrong turns*, T2 | REQ-11 [#538](https://github.com/jakubiszon26/apiforge/issues/538), REQ-02b [#459](https://github.com/jakubiszon26/apiforge/issues/459) |
| A typed path is refused in *Choose a response field* | six runs of six | DIAG-16 [#550](https://github.com/jakubiszon26/apiforge/issues/550), new |
| No way to move a step from *Main* to *Cleanup*; a pasted copy loses its checks | five runs of six; no run moved a step | REQ-14 [#549](https://github.com/jakubiszon26/apiforge/issues/549), new; REQ-00 question 8 [#456](https://github.com/jakubiszon26/apiforge/issues/456) |
| The window writes lines nobody set, `begin = start` among them on v0.2.0 | *The diff*: 2–3 in T1, 6 and 5–9 in T2 | REQ-13 [#540](https://github.com/jakubiszon26/apiforge/issues/540) |
| A click on a completion suggestion in the URL inserts nothing | v0.1.0 run 1, v0.2.0 run 2 | DIAG-18 [#552](https://github.com/jakubiszon26/apiforge/issues/552), new |
| A new scenario opens in Code | every run; not counted as a wrong turn | WS-04 [#548](https://github.com/jakubiszon26/apiforge/issues/548) |
| *Check* in the run results compares with this run's value | v0.2.0 run 2, rewired by hand | REQ-12 [#539](https://github.com/jakubiszon26/apiforge/issues/539) |
| The bottom of the side panel is out of reach at 1280 × 800 | four runs maximised or scaled the window | the body: REQ-02a [#458](https://github.com/jakubiszon26/apiforge/issues/458), DIAG-04 [#365](https://github.com/jakubiszon26/apiforge/issues/365); the wheel that does not move the panel depends on aiming and is to be tried by hand |

## Within the spread, or not told by three runs

- T2 first-time pointer actions (118 · 131 · 123 against 108 · 70 · 124),
  text entries, characters typed, syntax typed, scrolls.
- A wrong placeholder typed in one run on v0.1.0 and in three on v0.2.0:
  the product is the same; REQ-11's round counts it.
- Seen on v0.2.0 only, in one or two runs: *Output data* shows a sample
  status and not the response (runs 1, 3; the tab exists in v0.1.0 too;
  DIAG-10 [#531](https://github.com/jakubiszon26/apiforge/issues/531),
  REQ-02a, REQ-10 [#478](https://github.com/jakubiszon26/apiforge/issues/478));
  *Next: Connect to…* opens an empty list (run 2); a check card does not
  open the check (run 1; in DIAG-15). *Connect to…* is a control of the
  older language left in the Inspector: whether it goes is the owner's.

No class 1: no task was impossible, gave a wrong result or lost work on
v0.2.0.

## For round 2

- Each driver saves a capture of every wrong turn before the session
  stops; round 1 kept none.
- T2's pass check asks for exactly one `DELETE` with its check in the
  same block, and tries a failing check, so that a result like v0.1.0
  run 3 does not pass and "the delete runs always" is tested.
- The pass check becomes a script kept with the task sheets.
- R1 will meet T4 again; it is counted there and not classed twice.
