# AB-01 rounds 2 to 4: the differences, classed

Proposed by the planning model on 2026-11-23 from the results of
[round 2](round-2/results.md) (T3, T4), [round 3](round-3/results.md)
(T5, T8) and [round 4](round-4/results.md) (T6, T7; pull request #573):
v0.1.0 against v0.2.1-rc.2, in simulation. **The owner confirms the list
and the classes** ([regressions](../../../process/ab-testing.md#regressions)),
in one quiz for the three rounds. Each line names the cell or the list it
rests on. A difference inside the spread of the runs is reported as none.
The new issues carry no sprint and no `ready` label.
[Round 1](../../2026-11-21/AB-01/round-1/classes.md) is classed apart; its
R1 (#547) and R2 (#548) and its *same on both* list are counted here and
not classed again.

## How the cells were read

- **v0.1.0 opens on Code**, with lines 1 to 39 of the test's source on
  its first screen (R2, #548). Wherever that could have helped v0.1.0 it
  is discounted: nothing below is called worse on v0.2.1-rc.2 on a
  difference the source could explain. That touches T8 (the check after
  the team and eight requests were on screen), T6 (the `@after` lines)
  and T7 (the task starts in Code: v0.1.0's shortest path of 7 against 8
  is that one click). In T8 and T6 v0.1.0 came out worse all the same, so
  what is called better there is, if anything, understated.
- **Round 4 is short.** T6 has one first-time run per version and no
  shortest path on v0.1.0; T7 has two runs per version. T6's counts are
  *not decidable*; only what is a difference of kind is named, and marked
  as resting on one run. T7's two runs agree with each other and with
  the shortest paths.
- T8 has no shortest path on v0.1.0; its three first-time runs are
  complete.

## Per task

| Task | Result | Rests on |
| --- | --- | --- |
| T3 change a body value | **Within the spread** on actions and wrong turns; **better** on what is written | *Pointer actions* 14 · 12 · 15 against 23 · 10 · 14; *Wrong turns* 3 · 4 · 5 against 8 · 1 · 3; *The diff* 3 (+6) in every run against 0 |
| T4 add two checks | **Better** | *Pointer actions* 24 · 39 · 40 against 20 · 19 · 17 (no overlap), shortest 21 against 14; *Typed* 0 · 13 · 13 against 0; *Wrong turns* 4 · 5 · 5 against 5 · 4 · 3 is within the spread |
| T5 why does it fail | **The same**: right in every run on both, by the same way | *Answer*; *Pointer actions* 15 · 7 · 6 against 8 · 6 · 6 and *Wrong turns* 5 · 0 · 1 against 1 · 0 · 0 overlap |
| T8 read the order | **Better**, the clearest difference of the four rounds | *Answer* right in part · right · right in part against right in all four; *Pointer actions* 49 · 25 · 31 against 18 · 17 · 21 (no overlap) |
| T6 insert and reorder | **Not decidable**: one run each | *Done* yes on both; 89 pointer actions and 14 wrong turns against 58 and 11 |
| T7 Code and diagram | **Within the spread** on actions; **better** on what is shown and written | *Pointer actions* 9 · 6 against 9 · 7; *The diff* 2 against 0 in every run |

## Worse on v0.2.1-rc.2

No class 1 and no class 2: no task was impossible, gave a wrong result or
lost work, and no wrong turn that v0.1.0 lacks was made in every
first-time run of a task.

| | Finding | Evidence | Class | Issue |
| --- | --- | --- | --- | --- |
| R3 | After a failing run the `team` card is drawn red, "6, 21 · × 1/2 failed", while `team` passed: the failed check runs after `invite` and is counted under every step it reads. v0.1.0's cards carry no such count | Round 3, *Seen in the runs*: T5 runs 1, 2, 3 and the shortest path; no wrong turn, three drivers said a tester could be misled | **3** — worse to look at. It is also why every v0.2.1-rc.2 answer said where the 5 comes from | LAY-43 [#466](https://github.com/jakubiszon26/apiforge/issues/466), open, decided by the owner on 2026-11-19 |
| R4 | Escape on a suggestion list closes the side panel with it; on v0.1.0 the list only | Round 2, *Seen in the runs*: v0.2.1-rc.2 T3 run 3; v0.1.0 T3 run 3 closed the list with Escape | **3** — one run | [#577](https://github.com/jakubiszon26/apiforge/issues/577), new |
| R5 | A warning that `check` still prints leaves *Problems* after an edit on the diagram. The warning is new in 0.2, so this is a defect of the new build more than a step back | Round 4, *Seen in the runs*: T7 run 1 and the shortest path | **3** | [#578](https://github.com/jakubiszon26/apiforge/issues/578), new |

Counted again and not classed again: R1, the request's *Checks* tab (T4
run 1; fixed by DIAG-15 #547, awaiting acceptance), and R2 (#548).

## Not decidable: reordering a step (T6)

One first-time run per version. Both finished; v0.2.1-rc.2 with fewer
actions (58 against 89), fewer wrong turns (11 against 14) and less than
half the time. What differs in kind:

- **Placing the new request is better**: *Add step* puts it in the chain
  after the selected step in one click (v0.1.0: loose at the top of the
  board, four drags and two order wires).
- **Moving a step was not found on v0.2.1-rc.2.** The build has a move —
  an ordering wire from another step's *Next* dropped on the step
  (DIAG-06a, DIAG-06c) — and the driver did not meet it in eight tries:
  the menu, a drag of the card, a drag onto the line, ctrl+x, *Selection
  actions*. It copied, pasted, deleted and renamed. v0.1.0's driver did
  reorder with the *Execution order* lists, in four changes of which one
  was refused as a cycle. With one run each this is **not called a
  regression**; had the same turn come in three runs it would be class 2
  ("a step that did not exist before"). It is covered by DIAG-06b
  [#491](https://github.com/jakubiszon26/apiforge/issues/491) (*Move
  earlier* and *Move later*, `ready:opus`), and for the drag by DIAG-21
  [#580](https://github.com/jakubiszon26/apiforge/issues/580), new.

## Better on v0.2.1-rc.2

- A check is one menu item on a row of the response, nothing typed (T4).
  On v0.1.0 the *Add comparison* tick added nothing in two runs of three
  (DIAG-09), and the checks were built by hand.
- The board reads in run order when it opens: three rows, numbered, and
  the branches listed in the `if`'s panel (T8). v0.1.0 opens as one
  column of 29 cards at 25 %, not in run order; its one right answer
  needed *Tidy up*, which marks the file changed.
- A save writes the line that changed (T3, T4, T6, T7: 0 or the lines
  asked for). v0.1.0 writes `on_error stop` and `@config`, and moves the
  six lines of `calendar` out of its branch (LAY-34).
- A value changed in Code is seen on the request's card at the zoom the
  board opens at (T7). v0.1.0's card reads "{ 2 fields }" at every zoom.
- A click on a step no longer marks the file changed (v0.1.0: rounds 2
  and 3, nine runs).
- The run results reach the failed check (v0.1.0 T5 run 1: the list
  stops above it).
- The index inside a placeholder was edited without a wrong turn in
  every run (v0.1.0: run 2 and the shortest path, 15 and 18 keys of
  repair). It depends in part on aiming.
- A new step joins the chain (T6, one run): as round 1 found.

## The same on both

| Finding | Evidence | Issue |
| --- | --- | --- |
| A scenario's title is shown nowhere; the test is guessed by its file name | Round 2, *Wrong turns*: T3 2 · 3 · 5 against 8 · 0 · 2, T4 2 · 1 · 1 against 4 · 2 · 3 — most of round 2's wrong turns on both | WS-05 [#574](https://github.com/jakubiszon26/apiforge/issues/574), new |
| A failed check shows `actual: 4`, `expected: 5` and names neither the request nor the field either comes from | Round 3, T5: a wrong turn on v0.1.0 runs 1 and 3; the same panel on v0.2.1-rc.2; round 4, T6 v0.1.0 line 87 | REP-07 [#381](https://github.com/jakubiszon26/apiforge/issues/381), `ready:opus` in REQ |
| A new request stops the simulation until it has a sample response, which is set at the foot of the side panel and led to from nowhere | Round 4, T6: v0.1.0 run 1 (69, 76); v0.2.1-rc.2 run 1 (56, 58) and the shortest path (36, 41) | REQ-15 [#576](https://github.com/jakubiszon26/apiforge/issues/576), new; REQ-02a [#458](https://github.com/jakubiszon26/apiforge/issues/458) |
| The wheel does not scroll the side panel with the pointer over a field | Round 4, T7 on both (run 1 twice, the shortest paths); round 3, T8 and T5 on v0.2.1-rc.2; round 2, T4 shortest path on v0.1.0 | [#579](https://github.com/jakubiszon26/apiforge/issues/579), new. Round 1 left it to be tried by hand |
| A copied step leaves its check behind | Round 4, T6 v0.2.1-rc.2 run 1 (33); round 1 | REQ-08 [#465](https://github.com/jakubiszon26/apiforge/issues/465), REQ-14 [#549](https://github.com/jakubiszon26/apiforge/issues/549) |
| The value of another step reaches a body only as a typed `{{…}}`; the suggestion list covers what is typed | Round 2, T3: every character typed in every run is syntax | REQ-11 [#538](https://github.com/jakubiszon26/apiforge/issues/538), REQ-02b [#459](https://github.com/jakubiszon26/apiforge/issues/459) |
| A new request has no `Authorization` header, and its address is joined to a base that already ends in `/rooms` | Round 4, T6, both | LANG-18 [#471](https://github.com/jakubiszon26/apiforge/issues/471), REQ-09 [#477](https://github.com/jakubiszon26/apiforge/issues/477) |
| The icon-only button beside the layout tools is taken for a layout and opens the wire-style menu | v0.1.0 T4 run 2, T8 runs 1 and 2, T6; v0.2.1-rc.2 T6 run 1 | none; for WS-01 [#366](https://github.com/jakubiszon26/apiforge/issues/366) or the next pass over the board's bar |
| A double click on a number in Code takes the space after it; typing a digit opens a list of step names | Round 4, T7, both | none; too small for a ticket of its own |

## Within the spread, seen once, or not told by these rounds

- T3 and T5 pointer actions and wrong turns; T4 wrong turns; T7 pointer
  actions, shortest paths (7 against 8) and wrong turns.
- **The outputs of *IF / ELSE* carry no *Yes* / *No*** at the zoom the
  board opens at (v0.2.1-rc.2 T8 runs 1 and 2, two turns each; the
  shortest path and run 3 had none). No v0.1.0 run got as far as reading
  a branch, so it is not shown to be a step back; both builds have the
  words in their code. DIAG-20
  [#575](https://github.com/jakubiszon26/apiforge/issues/575), new.
- **Anything about aiming and the session**: a left drag that does not
  pan, Ctrl+wheel that does not zoom, wheel misses, *fit* at 25 %, a
  right-drag pan with the results open. They appear on both versions and
  in `DRIVER` lines; a round cannot tell them
  ([what a round cannot tell](../../../process/ab-testing.md#what-a-round-cannot-tell)).
- **The window was 1280 × 836 on an 800 px screen** on both versions:
  the foot of the side panel and the board's hint were cut off until a
  driver maximised. A matter of the session first (KTLO-48 below);
  KTLO-38 (#215) fixed the first window's height once and is worth one
  look by hand.
- Seen on v0.2.1-rc.2 in one run each: long addresses cut off on the
  card and in the panel with no full text on hover (T8 run 2); the side
  panel keeps its scroll position from one step to the next (T8 shortest
  path); a ctrl+click that dropped the first of two selected steps (T6
  shortest path). No issue yet; REQ-02a redraws the panel.

## Fable's proposals

- **DIAG-06b (#491) before anything else of the board.** It is the one
  place where a tester's ordinary task — "run this before that" — has
  no way a first-time run found, and it is already analysed and labelled.
  After it merges, one first-time run of T6 on `main` (a baseline round,
  20 minutes) says whether DIAG-21 is needed.
- **DIAG-21** [#580](https://github.com/jakubiszon26/apiforge/issues/580):
  a card dropped on an ordering line moves the step.
- **WS-05** [#574](https://github.com/jakubiszon26/apiforge/issues/574)
  into the REQ sprint: it is small, and the REQ acceptance task names
  tests by their titles.
- **KTLO-48** [#581](https://github.com/jakubiszon26/apiforge/issues/581):
  a cap of 15 minutes on a first-time run, every cell run once before
  any is run twice, the variants started on the same view, the window
  checked against the screen. Round 4 lost seven runs to one run of 40
  minutes.
- The missing runs of T6 are **not worth a fifth round on these builds**:
  v0.1.0 is not going to change, and what T6 has to answer is whether
  the next build lets a tester move a step — a question for a baseline
  round on `main` after DIAG-06b.

## Verdict on AB-01

Across T1 to T8, 0.2.x is worse than v0.1.0 nowhere that stops or
misleads a tester's task. Round 1 found one class 2 (R1, the *Checks*
tab; fixed by DIAG-15, awaiting acceptance) and one class 3 (R2). Rounds
2 to 4 add three of class 3, each a detail of something 0.2 added. On
the tasks a tester does most — adding checks, reading what a test does,
finding why it failed, saving without noise — v0.2.1-rc.2 is equal or
clearly better, and in T8 better although v0.1.0 showed its source. The
costs that remain are the same on both versions and are what REQ is
for: the title, the placeholder typed by hand, the failed check that
does not say what it read, the sample response. The one open point is
moving a step, which rests on a single run and on DIAG-06b.
