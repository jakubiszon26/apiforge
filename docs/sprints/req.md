# REQ: the request-and-results sprint — findings, decisions and plan

- State: **decided on first-round mock-ups** by the owner's quiz of
  2026-11-19; not started. It follows LANG part A (0.2.0), which the
  owner started on 2026-11-20. FIX-01 merged that day (#485). Three
  tickets are ready and do not depend on the language (DIAG-02, REP-08,
  REP-07); the rest wait for the
  design-review captures of REQ-00. Four proposals of the planning model
  were accepted into scope on 2026-11-20 and are to be analysed (REQ-07 to
  REQ-10, [section 5](#5-proposals-of-the-planning-model)). On
  2026-11-21 the owner added one mandatory acceptance scenario and took
  REQ-11, REQ-12 and REQ-13 into scope
  ([section 7](#7-the-mandatory-scenario-against-the-plan-2026-11-21)).
- Analysis model: Fable 5.2.
- Inputs: the owner's run of build `1d11f07` on 2026-11-19 with ten
  findings; the code on `main` at `eb2dc01`; the open issues REP-07, DIAG-02,
  DIAG-04, REP-04, RUN-00 and UX-19; the [LAY plan](lay.md), section 13.
- The scenario of the run is the owner's and is not in the repository.
  Everything here was reproduced on a synthetic scenario of the same
  shape, *Label validation* — a fictional parcel API, sixteen requests to
  one endpoint that differ in one value, thirty-three checks, no request
  reading another — which becomes the reference scenario `labels`
  (FIX-01, [#454](https://github.com/jakubiszon26/apiforge/issues/454)), and on `rooms/booking-day`.

## 1. What the run showed

Every board LAY measured has a dense data flow. A validation test has
none: the steps are independent, so the board has nothing to draw between
them, and what matters — how the bodies differ, what is checked, what came
back — sits behind clicks. Six of the ten findings are about reading
results and editing a request; three are about the board and went to
LAY-2 ([LAY plan, 13.7](lay.md#137-closed-2026-11-19)); one was the
scenario itself.

| # | Finding | Cause on `main` | Ticket |
| --- | --- | --- | --- |
| 1 | A row of Run results is a handle and *Passed*; fifty equal rows | `report.rs`, `step_row_button`: the title is the label or the handle, the second line the status word; the row does not look at the document's node. The list is flat and 300 px wide | REP-07 [#381](https://github.com/jakubiszon26/apiforge/issues/381) |
| 2 | A check card does not say what it checks | `canvas/layout.rs`, `sentence`: step name first, expected value last, one line, cut from the right. Three wordings of one predicate: "is equal to" (card), "equals" (Inspector), `==` (folded line) | DIAG-02 [#145](https://github.com/jakubiszon26/apiforge/issues/145) |
| 3 | The body is hard to reach; headers and authorization are hand-typed | The body is the eighth section of the Inspector's *Configuration* tab, about 500 px down at 1600 × 1000, in a box about 320 px wide; the language has no shared headers | REQ-02a, REQ-02b, REQ-03 |
| 4 | No view of one request with its response beside it | The window is a board, an Inspector for the definition and a panel for the result; a request's definition, what was sent, the response and its checks are four places | REQ-00, REQ-02a |
| 5 | Outputs and inputs fold; nothing opens them all | `report.rs`, `value_branch`: only the first level opens, and what is open is remembered per step | REP-08 [#455](https://github.com/jakubiszon26/apiforge/issues/455) |
| 6 | No list of steps | Never built; no issue existed | REQ-00, REQ-01 |
| 8 | The scenario: independent requests, each with one to four checks | A shape no reference board had | FIX-01 [#454](https://github.com/jakubiszon26/apiforge/issues/454) |

Captures of `main` on `labels`: [Run results](../qa/2026-11-19/owner-run/before-run-results.png),
[the board folded](../qa/2026-11-19/owner-run/before-board-folded-gaps.png),
[All wires](../qa/2026-11-19/owner-run/before-all-wires.png).

## 2. Decisions (owner, 2026-11-19)

Recommended options were put first; the owner chose them except in 8 and
14.

| | Decision | Lands in | Mock-up |
| --- | --- | --- | --- |
| 1 = A | Run results: a request with its checks under it; a check's row is its predicate | REP-07 | [M1a](../qa/2026-11-19/owner-run/m1a.png); not chosen [M1b](../qa/2026-11-19/owner-run/m1b.png) |
| 2 = A | The predicate is a check card's title; the handle a small line | DIAG-02 | [M4](../qa/2026-11-19/owner-run/m4.png) |
| 3 = A | One notation for a predicate everywhere: the signs of the code | DIAG-02 | [M4](../qa/2026-11-19/owner-run/m4.png) |
| 4 = A | A value in Run results opens to its leaves; *Tree \| JSON*; *Expand all*, *Collapse all*; the choice holds between steps | REP-08 | [M2](../qa/2026-11-19/owner-run/m2.png) |
| 5 = A | The request view lives in *Diagram → Steps*: list, request, response | REQ-02a | [M3a](../qa/2026-11-19/owner-run/m3a.png); not chosen [M3b](../qa/2026-11-19/owner-run/m3b.png) |
| 6 = A | *Steps* is a second presentation beside *Canvas*; the choice is remembered; the first open stays *Canvas* | REQ-01 | [M6](../qa/2026-11-19/owner-run/m6.png) |
| 7 = A | A step's card: READS, SENDS (what differs from the step above), CHECKS; SETS when a later step reads it | REQ-01 | [M6](../qa/2026-11-19/owner-run/m6.png) |
| 8 = B | *Run to here* and *Send this request alone* with the values of the last run; a live request keeps its separate permission | REQ-04, REQ-05 | [M3a](../qa/2026-11-19/owner-run/m3a.png) |
| 9 = A | Headers as a table and authorization from a list, written per request; no format change | REQ-03 | [M3c](../qa/2026-11-19/owner-run/m3c.png) |
| 10 = A | A check that reads two steps is drawn once, with a reference under the other step | LAY-43, LAY-2 | [M7](../qa/2026-11-19/owner-run/m7.png) |
| 11 = A | A folded check keeps no room; *Checks* on moves the rows apart | LAY-44, LAY-2 | [M5](../qa/2026-11-19/owner-run/m5.png) |
| 12 = A | The ordering lines of a chain run side to side, row by row | LAY-45, LAY-2 | [M8](../qa/2026-11-19/owner-run/m8.png) |
| 13 = A | The run is LAY's acceptance; LAY is closed | [#296](https://github.com/jakubiszon26/apiforge/issues/296) | — |
| 14 = B | LANG part A to 0.2.0 first, as planned; this sprint after it | [backlog](../backlog.md) | — |
| 15 = A | The synthetic scenario may be a fixture and appear in issues | FIX-01 | — |

The mock-ups are first-round drawings, not captures of the application.
For the three tickets that change text and rows inside views that exist
(DIAG-02, REP-07, REP-08) the owner's choice is the approval of the
direction. Each of their pull requests is accepted before merge on the
captures of its QA section, which [testing](../process/testing.md#2-the-functional-qa-report)
defines: taken from the release binary of the branch, at 960 × 540, and at
1600 × 1000 where layout matters. A headless design-review capture may
accompany them as an aid; it is not the evidence. The tickets that add a view
(REQ-01 to REQ-05) get their captures and numbered criteria from REQ-00
first.

## 3. Built on the one-chain language

With decision 14 = B the views are designed for the language of 0.2.0:
the steps of a block run in the order written (LANG-13). The list of
*Steps* is the engine's order (`graph::sequence`), which is then the
written order and the board's badge numbers; a check belongs to the step
it is written after, in the same braces. REQ-00 decides whether a step can
be moved in the list, together with DIAG-06 for the board. DIAG-02, REP-08
and REP-07 read neither `@after` nor the order of the text and can be
built before or after LANG-13 unchanged.

## 4. Tickets and order

| Order | Issue | Ticket | Estimate · risk | Depends on |
| --- | --- | --- | --- | --- |
| 1 | [#454](https://github.com/jakubiszon26/apiforge/issues/454) | FIX-01 the reference scenario `labels` | S · 5 | merged in #485 |
| 2 | [#145](https://github.com/jakubiszon26/apiforge/issues/145) | DIAG-02 a check reads the same everywhere | M · 15 · `visible UI`, `removal` | FIX-01 (merged) |
| 2 | [#455](https://github.com/jakubiszon26/apiforge/issues/455) | REP-08 a value opens expanded; Tree \| JSON | S · 15 · `visible UI`, `default` | FIX-01 (merged) |
| 3 | [#381](https://github.com/jakubiszon26/apiforge/issues/381) | REP-07 a request with its checks under it | M · 25 · `visible UI`, `default` | DIAG-02 (the text), REP-08 (the same file) |
| 4 | [#456](https://github.com/jakubiszon26/apiforge/issues/456) | REQ-00 captures and decisions | S · 30 · `visible UI`, `default` | LANG-13 merged |
| 5 | [#457](https://github.com/jakubiszon26/apiforge/issues/457) | REQ-01 Diagram → Steps | M · 30 · `visible UI` | REQ-00, DIAG-02, REP-07 |
| 6 | [#458](https://github.com/jakubiszon26/apiforge/issues/458) | REQ-02a the request and its response side by side | M · 30 · `visible UI` | REQ-01, REP-08 |
| 7 | [#459](https://github.com/jakubiszon26/apiforge/issues/459) | REQ-02b the body editor; closes DIAG-04 [#365](https://github.com/jakubiszon26/apiforge/issues/365) | M · 20 · `visible UI` | REQ-02a |
| 7 | [#460](https://github.com/jakubiszon26/apiforge/issues/460) | REQ-03 headers table and Auth | M · 15 · `visible UI` | REQ-02a |
| 8 | [#461](https://github.com/jakubiszon26/apiforge/issues/461) | REQ-04 Run to here | S · 15 · `visible UI` | REQ-02a |
| 9 | [#462](https://github.com/jakubiszon26/apiforge/issues/462) | REQ-05 Send this request alone | M · 25 · `visible UI`, `default`, a permission control | REQ-04; five questions on the issue |
| 10 | [#464](https://github.com/jakubiszon26/apiforge/issues/464) | REQ-07 a request in and out as cURL; a failed check copied as a report | S–M · 15 · `visible UI` · to analyse | REP-07 (the report), REQ-02a (the paste) |
| 10 | [#465](https://github.com/jakubiszon26/apiforge/issues/465) | REQ-08 duplicate as a new case; run the failed ones again | S and M · 30 · `visible UI` · to analyse | REQ-01 (the duplicate), REQ-04 (the subset run) |
| 10 | [#477](https://github.com/jakubiszon26/apiforge/issues/477) | REQ-09 hints for repetition, each with a fix | S · 10 · `visible UI` · to analyse | none for the host hint; LANG-18 for the header hint |
| 10 | [#478](https://github.com/jakubiszon26/apiforge/issues/478) | REQ-10 a live response saved as the sample; *Expect this response* | M · 20 · `visible UI` · to analyse | REP-07, REQ-02a; REQ-05 produces the response of a single send |
| 10 | [#538](https://github.com/jakubiszon26/apiforge/issues/538) | REQ-11 a value of a response used in a later request by choosing it | M · 20 · `visible UI` · to analyse, with an A/B round | REQ-02a, REQ-02b; DIAG-08 and DIAG-10 to DIAG-14 for the place |
| 10 | [#539](https://github.com/jakubiszon26/apiforge/issues/539) | REQ-12 a check from a field offers what to compare with | S–M · 15 · `visible UI`, `default` · to analyse, with an A/B round | DIAG-08 |
| 10 | [#540](https://github.com/jakubiszon26/apiforge/issues/540) | REQ-13 the window writes only what was set | S–M · 15 · `default` · three questions open | none |
| 11 | [#463](https://github.com/jakubiszon26/apiforge/issues/463) | REQ-06 the owner's own run | S · 10 | the rest |

235 owner review minutes for the twelve tickets decided on 2026-11-19,
and about 75 for REQ-07 to REQ-10, accepted into scope on 2026-11-20; their
estimates are the proposals' own and are replaced by their analyses. Rows with the same order number touch different
files and may run side by side: DIAG-02 is `canvas/` and
`inspector/checks.rs`, REP-08 is `report.rs`; REP-07 follows both.
UX-19 [#227](https://github.com/jakubiszon26/apiforge/issues/227) edits the list of REP-07 and merges beside it, not with it.
REP-04 [#148](https://github.com/jakubiszon26/apiforge/issues/148), the right-click menu on the JSON text, stays in the
backlog; RUN-00 [#242](https://github.com/jakubiszon26/apiforge/issues/242) keeps run inputs and settings.

## 5. Proposals of the planning model

The findings name where time is lost in the window. Most of the time a
scenario of this shape costs is lost earlier, in writing it: the same
headers in fifteen requests, sixteen bodies that differ in one value, a
name invented for every check. None of these was asked for. **The owner
accepted all eight into scope on 2026-11-20**: each is analysed and
planned where the last column says; no design is approved by that, and
each analysis brings its own questions and mock-up.

| Proposal | Problem it removes | Size · trade-off | Goes to |
| --- | --- | --- | --- |
| LANG-17 [#470](https://github.com/jakubiszon26/apiforge/issues/470) a table of cases, or a request based on another | Sixteen copies of one request; a case is added by copying, a change means sixteen edits | L, to split · `format`; must be analysed with LANG-08, and every view has to show a table and its rows | LANG, second half, with LANG-16 and LANG-08 |
| LANG-18 [#471](https://github.com/jakubiszon26/apiforge/issues/471) headers and authorization shared by a scenario | A token repeated in every request; the one request without it is not visible as the exception | M · `format`; needs a way to leave a shared header out | LANG, second half |
| LANG-19 [#472](https://github.com/jakubiszon26/apiforge/issues/472) a check without an invented name; one check for several fields | A third of the text is one-line checks whose names nothing reads | M–L · `format`; the document, the board and MCP identify a check by its handle today | LANG, second half, with LANG-02 and LANG-04 |
| REQ-07 [#464](https://github.com/jakubiszon26/apiforge/issues/464) a request in and out as cURL; a failed check copied as a report | Requests are retyped from elsewhere, failures are described by hand | S–M; paste must refuse what it cannot represent, copy must not leak a secret | REQ |
| REQ-08 [#465](https://github.com/jakubiszon26/apiforge/issues/465) duplicate a request with its checks as a new case; run the failed ones again | Each new case is a hand copy with renames; a re-run repeats everything | S and M; the duplicate matters less once LANG-17 exists | REQ |
| GEN-01 [#473](https://github.com/jakubiszon26/apiforge/issues/473) suggested negative cases of a field | Which boundaries to test is the same knowledge for every field, written by hand each time | M–L; proposes inputs only, never what the API should answer | no sprint yet: analysed after LANG-17 |
| REQ-09 [#477](https://github.com/jakubiszon26/apiforge/issues/477) hints for repetition: the same host or header in every request, each with a fix | Sixteen requests spell out the same host; the project has a base URL | S; a problem with a fix, like the far-value hint — easy to make noisy; the header hint has no fix before LANG-18 | REQ |
| REQ-10 [#478](https://github.com/jakubiszon26/apiforge/issues/478) a live response saved as the request's sample; *Expect this response* | A response is typed twice, as the sample and as checks, and a hand-typed sample drifts from the API | M; turns today's behaviour into the test unless the person picks the fields; a saved response can carry a secret into a tracked file | REQ |

## 6. Exit criteria

In the [REQ milestone](https://github.com/jakubiszon26/apiforge/milestone/3). The
acceptance is the owner's own run ([REQ-06](../tickets/REQ-06.md)), in
two parts:

- **One mandatory scenario** (owner, 2026-11-21): POST → take the ID from
  the response → GET with that ID → an assertion → DELETE, everything
  from the interface, with no hand editing of the `.afs` file. "If after
  REQ this cannot be done comfortably, in my view the sprint has not
  fully solved the functional tester's problem." It is run against the
  demo API, live, twice; the DELETE first as a step with its own check,
  then moved to Cleanup; and by agents before the owner
  ([A/B rounds](../process/ab-testing.md)), whose round must show no
  language syntax typed, green twice, no wrong turn in at least two of
  three first-time runs and fewer actions than v0.2.0.
- Read, edit, run and diagnose `labels` in the window without opening
  Code.

## 7. The mandatory scenario against the plan (2026-11-21)

The planning model walked the scenario on the released v0.2.0
([REQ-06, the walk](../tickets/REQ-06.md#the-scenario-against-v020-and-the-plan)).
It can be built in the window today in about 30 pointer actions; it
cannot be built without typing `{{create.body.id}}` by heart, and the
check one click makes from the response compares with that run's id and
fails on the next run. The demo API needs nothing: carts are created,
read and deleted by id.

| Step | Covered by | Not covered |
| --- | --- | --- |
| A new scenario, its first request | the board today | adding a step in *Steps* — a question for REQ-00, a criterion of REQ-01 |
| POST with a body | REQ-02a, REQ-02b | — |
| Send it, see the response | REQ-04, REQ-05, UX-42 | REQ-05: a request that reads nothing, sent alone before any run |
| Take the id from the response | REQ-02b's completion, in part | **REQ-11** [#538](https://github.com/jakubiszon26/apiforge/issues/538) |
| GET with the id in its path | REQ-02a | REQ-02b: placeholders as chips in the URL, query and headers too |
| An assertion on the GET | Run results' *Check*; DIAG-08 for a type | **REQ-12** [#539](https://github.com/jakubiszon26/apiforge/issues/539) |
| DELETE with the id, Cleanup | the Cleanup board today; REQ-04 keeps Cleanup running | Cleanup in the list of *Steps* — REQ-00, REQ-01 |
| The file holds what was done | — | the window writes `timeout 10s`, `@config(…)` and `@label` nobody set: **REQ-13** [#540](https://github.com/jakubiszon26/apiforge/issues/540) |

Decided by the owner on 2026-11-21
([REQ-06](../tickets/REQ-06.md#decisions-owner-2026-11-21)): REQ-11,
REQ-12 and REQ-13 are in the sprint, to be analysed, each
`decision-needed` until its mock-up and questions are answered; the
criteria of the right-hand column are on REQ-00 (questions 7 and 8),
REQ-01, REQ-02b and REQ-05. REQ-11 and REQ-12 both put an action on a
field of a response and take their place from DIAG-08 (#521) and the
redesign of *Output data* (DIAG-10 to DIAG-14); REQ-02a is not changed by
this section.

A/B rounds in this sprint
([REQ-06](../tickets/REQ-06.md#ab-rounds-in-the-req-order)): the counted
baseline of the scenario is task T2 of AB-01
([#537](https://github.com/jakubiszon26/apiforge/issues/537), v0.1.0
against v0.2.0); a variants round in the analysis of REQ-11 and of
REQ-12; a baseline of the request view after REQ-05; and the baseline on
the release build before the owner's run. The views of REQ-00 are decided
and drawn, not driven: no round for the shape.
