# REQ-00: One place for a request — the Steps view and the request beside its response: captures and decisions

- Size: S · Owner review: 30 min plus the quiz · Risk: `visible UI`, `default`
- State: **Analysis — waits for the owner's decisions** on the boards
  below (issue [#456](https://github.com/jakubiszon26/apiforge/issues/456)).
  Decisions 5, 6, 7, 8, 9 and 14 of 2026-11-19 stand and are not reopened.
- Analysis model: Fable 5.2 · Implementation model: — (no implementation)

## Goal

The owner decides, on captures of the real renderer, how a scenario is
read as a list of steps and how one request is edited with its response
beside it. The decisions unblock REQ-01 to REQ-05 (#457–#462).

## Mock-up

Throwaway branch `mock/req-00`, commit `93e2f89b`: the *Steps*
presentation drawn from the real graph (`graph::sequence`, `check_text`,
`relations`) and a real simulated run, on `labels` (FIX-01; one sample
changed so that one check fails) and on `rooms/double-booking` (a
component call, a value, an `if`, Cleanup). Buttons and menus are drawn,
not wired. Boards are cropped to the view; the list scrolls, so a card
at a crop's edge is cut by the crop, not by the design.

Base of every link: `https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/`
— written below as `…/`.

| Check of the whole window | 960 × 540 | 1600 × 1000 |
| --- | --- | --- |
| Steps before a run | […/window-steps-before-run-960.png](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/window-steps-before-run-960.png) | [1600](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/window-steps-before-run-1600.png) |
| Steps after a run, one check failed | [960](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/window-steps-after-run-960.png) | [1600](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/window-steps-after-run-1600.png) |
| A request selected: Body, Headers, Auth | [960, rail](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/window-q4-a-rail-960.png) | [Body](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/window-request-body-1600.png) · [Headers](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/window-request-headers-1600.png) · [Auth](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/window-request-auth-1600.png) |
| *Send alone*, marked | [960](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/window-q3-sent-alone-960.png) | board 3 |
| `if`, call, value, Cleanup | [960](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/window-rooms-after-run-960.png) | [list](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/window-rooms-after-run-1600.png) · [a Cleanup request](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/window-rooms-cleanup-request-1600.png) |

Known defects of the mock, not proposals: with six tabs the request pane
is too narrow at half of 1600, so *Sample* and *Settings* sit under
*More* (REQ-02a decides the tab row); the check `not_booked`, written in
the `else` branch, is listed under `first`, the step it reads (rule 5.2
below says where it belongs).

## The open questions

**Visible** questions are the owner's, on a board. **Rules** have no
picture worth choosing from; an independent agent may rule on them.
The recommended option is always A.

### 1. SENDS: what "differs" is compared with — visible

Board: [q1-sends-960.png](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/q1-sends-960.png)

- **A · the common request.** Among the requests with the same method
  and the same URL once its `{{…}}` are blanked, each place (a header, a
  query parameter, a body field) has a common value when more than half
  send it; a card lists only what it sends differently, and `no header
  Authorization` for what it leaves out. On `labels`: *No token* → `no
  header Authorization`, *Weight 31 kg* → `weight: 31`, *Unknown parcel*
  → the path. This is what M6 drew.
- B · the step above, literally. *Weight 31 kg* also lists
  `Authorization`, and *Unknown parcel* lists `weight: 2` — the return to
  normal reads as a case.
- C · the first request of the group. Every card after *No token* lists
  `Authorization`.

Why A: the tester's question is "what is special about this case"; B and
C answer "what changed since a neighbour", which depends on the order.

Rules with it (agent): fewer than three comparable requests → the whole
request, at most four lines, caption `SENDS`; nothing differs →
*nothing differs*; a value is cut at 36 characters and a card at four
lines (`+ 2 more`), the full text on hover; SETS replaces SENDS when a
later step reads the request (decision 7).

### 2. What leaves the Inspector — visible (a control moves)

Board: [q2-inspector-960.png](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/q2-inspector-960.png)
(Today and A; B and C are not drawn).

- **A · nothing leaves.** *Steps* has the request pane and no Inspector;
  *Canvas* keeps the Inspector's *Configuration* as today. Both edit the
  same document through the same operations.
- B · the request's fields leave *Configuration*; in *Canvas* the
  Inspector shows a summary and *Edit in Steps*.
- C · the Inspector hosts the request pane's tabs in its narrow column.

Why A: nothing visible is removed, REQ-02a stays M, and REQ-06's run
says whether anyone still edits a request in the Inspector. B is a
removal to decide after that run, not before.

Found while drawing, the owner's (a default): after a run the *Run
results* panel opens over half of *Steps*, where the response already
stands beside the request. The captures fold it by hand. Proposal P4.

### 3. Send this request alone — one visible, four rules

Board: [q3-send-alone-1600.png](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/q3-send-alone-1600.png)

- 3.4 **visible** · where the response of a single send is kept.
  **A · beside the run's**: the pane shows it with the mark *SENT ALONE*
  and a line "not part of a run: the run's verdict is unchanged ·
  *Show the run's response*"; cards and Run results keep the run's.
  B · it replaces the run's response for that step.
- 3.3 **the owner's** (a permission control) · the live permission.
  **A · the run mode as it stands**: a single send is live only when the
  switch says *Live* and the project's consent is given, as a run.
  B · a confirmation each time, naming the method and the URL.
- 3.1 rule · a value the request reads is missing (no last run, or the
  step did not run). **A · refuse, name the step, offer *Run to here***
  (the workspace window words it so today, `app.rs`). B · send with the
  placeholder empty. A request that reads nothing is always sent (G2).
- 3.2 rule · the document is newer than the last run. **A · send**, and
  the pane says the values come from the run of *hh:mm*; the request
  being edited is always newer than the run, so refusing would refuse
  the main use. B · refuse.
- 3.5 rule · the request's checks. **A · evaluated and shown** in the
  pane, counted nowhere else. B · not evaluated.
- Rule · a simulated send shows the sample, marked *SENT ALONE ·
  simulation*, and sends nothing.

### 4. 960 × 540: which column folds — visible

Board: [q4-fold-960.png](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/q4-fold-960.png)

- **A · the list folds to a rail** of numbers and results; the request
  and the response stay side by side (about 340 px each); the rail opens
  over the panes on a click.
- B · the list stays; *Request | Response* become tabs.
- C · the list stays; the response goes under the request (three body
  lines each at 540).

Why A: side by side is decision 5; B and C give it up exactly where
room is short. Without folding the response is cut (first panel).

### 5. Values, `if` and component calls — visible

Board: [q5-kinds-1600.png](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/q5-kinds-1600.png)

- **A · every step is a row**: a value and a call on one line (a call
  says what it SETS), an `if` a line with its branches indented under
  *then* and *else*, a check that stands alone a line. The numbers are
  the board's without gaps in meaning.
- B · requests only; the other steps are named in a grey line between
  cards.

Why A: `slot` and `token` are what the requests READ; in B the reader
meets names with no row. The list equals `sequence(graph)` in A, which
is REQ-01's test.

Rule 5.2 (agent): a check written inside a branch is listed in that
branch even when it reads a step outside it; LAY-43's reference line
stands under the step it reads.

### 6. Moving a step in the list — rule

Capture: [q6-menu-1600.png](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/q6-menu-1600.png)

- **A · the board's commands, no dragging**: *Move earlier* and *Move
  later* in the card's menu with Alt+↑ and Alt+↓, DIAG-06a's operation
  and refusals, as DIAG-06b built them for the board.
- B · dragging a card as well (REQ-06's proposal 5 would A/B it).

Why A: one operation, already tested; a drag in a scrolling list of
tall cards is the costly part and nothing in REQ-06 needs it.

### 7. Adding a step from Steps — visible

Board: [q7-add-960.png](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/q7-add-960.png)

- **A · `+` between two cards under the pointer, and an *Add step* row
  at the end** of Main and of Cleanup; both open the board's *Add step*
  and insert there. An empty scenario shows the row alone: the first
  request.
- B · *Add step* on the bar; it inserts after the selected step, or at
  the end.

Why A: it is DIAG-06b's `+` on a line, and the place is where the
pointer is; B needs a selection first and hides where the step lands.
*Add step after* is also in the card's menu for the keyboard.

### 8. Cleanup in the list — visible

Board: [q8-cleanup-1600.png](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/q8-cleanup-1600.png)

- **A · a section after Main**, headed *CLEANUP · runs after the test,
  also when a check fails*, numbered as the board numbers Cleanup. A
  step is put there by the section's *Add cleanup step* row or by *Move
  to Cleanup* in a card's menu (and *Move to Main* back).
- B · the board's *Main | Cleanup* tabs in *Steps*.

Why A: REQ-06 moves its DELETE to Cleanup and reads the id of a Main
step; in one list both ends are visible, behind a tab one is not.

Rule (agent): *Move to Cleanup* is refused, with the reason, for a step
a later Main step reads; the step keeps its checks.

## What of LANG-17, LANG-18 and LANG-19 the captures account for

Drawn on the language of `main`; none of the three exists.

- LANG-18 (shared headers): option 1 A already treats what most requests
  send as not worth saying and the exception as the line. A shared
  header changes no card. Not drawn: an inherited row in the Headers tab.
- LANG-17 (a table of cases): the common request is the base and each
  SENDS line a row of the table. Not drawn: a table as a card (P2).
- LANG-19 (checks without names): every capture shows a check by its
  predicate, never its handle. Not drawn: one check with several lines.

## Consequences for REQ-01 to REQ-05

- **REQ-01 is over M and splits**: REQ-01a reads (the list, results,
  selection, 960); REQ-01b edits the list (add, move, Cleanup, the empty
  scenario with WS-04 #548).
- REQ-02a takes the fold of question 4 and, under 2 A, removes nothing.
- **REQ-04 and REQ-05 need no engine work**: `RunRequest.target_node_id`,
  `single_node_id` and `previous_run` exist and the workspace window
  uses them (`app.rs`, `RunKind`); the folder window passes `None`
  (`folder/runs.rs`). REQ-04 stays S; REQ-05 stays M for its rules and
  the permission.
- REQ-05 loses `decision-needed` when 3.3 and 3.4 are answered and the
  rules are ruled.

## Draft acceptance criteria (assuming every A)

Tests are `crates/apiforge-desktop/src/folder/acceptance_tests/<ticket>.rs`.

**REQ-01a — Steps reads (M)**
1. A *Steps | Canvas* switch on the board's bar; the first open is
   *Canvas*; the choice is kept per project on this computer.
   `req01a_criterion_1_*`
2. On `labels` the rows and their numbers equal `sequence(graph)`, the
   written order and the board's badges. `req01a_criterion_2_*`
3. Every step is a row (5 A); a check stands once, under its step or in
   its branch (rule 5.2). On `double-booking`: `not_booked` is under
   *else*. `req01a_criterion_3_*`
4. SENDS follows 1 A and its rules; on `labels` *No token* reads `no
   header Authorization` and *Weight 31 kg* `weight: 31`.
   `req01a_criterion_4_*`
5. SETS replaces SENDS for a step a later step reads, one line per field
   and reader. `req01a_criterion_5_*`
6. Cleanup is a section after Main (8 A), numbered as the board.
   `req01a_criterion_6_*`
7. Before a run no row carries a result; after one each row and check
   does, a failed check with its actual value. `req01a_criterion_7_*`
8. Selecting a row selects the step in Canvas, Run results and Code, and
   back. `req01a_criterion_8_*`
9. At 960 × 540 and 1600 × 1000 nothing overlaps; a cut text has its
   hover. `assert_disjoint`, and the QA captures.

**REQ-01b — Steps edits the list (M)**
1. `+` between two cards and the *Add step* row insert where they stand,
   in their braces; the text passes the check. `req01b_criterion_1_*`
2. An empty scenario shows *Add step* alone; a scenario just created
   opens in Diagram and an empty project shows *Files* (WS-04, decision
   A). `req01b_criterion_2_*`
3. *Move earlier*, *Move later*, Alt+↑, Alt+↓ act as on the board, with
   the same refusals. `req01b_criterion_3_*`
4. *Move to Cleanup* and *Move to Main* move the step with its checks;
   refused with the reason when a Main step reads it; *Add cleanup step*
   adds there. `req01b_criterion_4_*`
5. QA step: REQ-06 part 1's DELETE is added, checked, then moved to
   Cleanup, Code never opened.

**REQ-02a — the request and its response (M)**
1. Selecting a request in *Steps* shows the list, the request pane and
   the response pane; no Inspector in *Steps*; *Canvas* unchanged (2 A).
   `req02a_criterion_1_*`
2. Method, URL, Body, Headers, Query, Sample and Settings edit the same
   `.afs` as the Inspector: for each field, the same text after the same
   edit. `req02a_criterion_2_*`
3. The response pane shows the last run's status, time, body (as REP-08
   shows a value), headers and the request sent; *Not run yet* without
   one; a line when the request changed after it. `req02a_criterion_3_*`
4. The request's checks are listed as REP-07 lists them, with results;
   *Add check* adds one. `req02a_criterion_4_*`
5. Under the width where both panes hold 320 px the list is a rail that
   opens on a click (4 A); every tab is reachable at 960 × 540.
   `req02a_criterion_5_*`, `assert_within_window`

**REQ-02b — the body editor (M)**: the issue's five criteria stand,
numbered 1–5 as written (`req02b_criterion_N_*`), on the Body tab of
REQ-02a. 6. At 960 × 540 the editor shows at least five lines beside
the response. QA capture.

**REQ-03 — headers and Auth (M)**
1. Headers are rows of name and value; a row added, changed or removed
   writes only that header. `req03_criterion_1_*`
2. *Auth*: None, Bearer token, Basic, Free header; it shows the header
   it writes; *Bearer* writes `Authorization: "Bearer …"`; *None* removes
   that header and no other. `req03_criterion_2_*`
3. An `Authorization` that fits no form is shown as *Free header*,
   unchanged. `req03_criterion_3_*`
4. `Authorization` is edited in *Auth*; the Headers tab says so.
   `req03_criterion_4_*`
5. A literal token shows the note that a secret belongs in the
   environment; Run results still redact it. `req03_criterion_5_*`
6. QA step: REQ-06 part 2 step 4 leaves a diff of those edits only.

**REQ-04 — Run to here (S)**
1. *Run to here* on the request pane and in the card's menu starts a run
   with `target_node_id`; on `labels`, on *Weight 0 kg*, 15 steps run.
   `req04_criterion_1_*`
2. Later steps read *Not run* — "the run stopped at *name*"; the
   summary says the run was partial; Cleanup ran. `req04_criterion_2_*`
3. Run mode, live permission and inputs are an ordinary run's.
   `req04_criterion_3_*`
4. A step in a branch not taken is *Not run* with the reason.
   `req04_criterion_4_*`

**REQ-05 — Send alone (M)**
1. *Send alone* sends the selected request with `single_node_id` and the
   last run's values; nothing else runs. `req05_criterion_1_*`
2. A request that reads nothing is sent with no last run (G2).
   `req05_criterion_2_*`
3. A missing value refuses, names the step and offers *Run to here*
   (3.1 A). `req05_criterion_3_*`
4. A document newer than the run is sent; the pane names the run the
   values come from (3.2 A). `req05_criterion_4_*`
5. Live only as a run would be (3.3 A); never without that permission;
   MCP's rules unchanged. `req05_criterion_5_*`
6. The response is kept beside the run's, marked *SENT ALONE*, with
   *Show the run's response*; the run's verdict, cards and Run results
   do not change (3.4 A). `req05_criterion_6_*`
7. The checks are evaluated and shown in the pane only (3.5 A).
   `req05_criterion_7_*`
8. A single send of a request that is not a GET says that nothing after
   it runs, Cleanup included (G2). `req05_criterion_8_*`
9. Nothing is written to the document. `req05_criterion_9_*`

## Proposals beyond the request

| # | Proposal | Problem it removes | Size · trade-off | Goes to |
| --- | --- | --- | --- | --- |
| P1 | **Candidate, drawn: SENDS against the common request** (option 1 A) | "The step above" makes order decide what a case is said to send | none beyond REQ-01a; a group under three falls back to the whole request | REQ-01a |
| P2 | **Candidate: a group of same-shape requests reads as a table** — one line per request, a column per place that differs, the status and the checks' result | Sixteen cards are sixteen screens of 85 px for sixteen values; the tester compares down a column. It is LANG-17's table before the language has one | M · `visible UI`; a third reading beside cards, to mock first | after REQ-01a; a round of REQ-06's proposal 6 can measure it |
| P3 | The common request shown once, above its group | What "common" is must be readable, or 1 A is a rule nobody can check | S | REQ-01a, criterion 4, if 1 = A |
| P4 | In *Steps*, a run does not open Run results | The panel covers half of the view that already shows the response | S · `default` — the owner's | REQ-02a |
| P5 | *Run to here* on Enter from the body editor (Ctrl+Enter) | The loop "change a value, read the response" is the sprint's point; a pointer trip each time | S | REQ-04 |

## Not in scope

Implementation; REQ-07 to REQ-13; the redesign of *Output data*.

## Technical appendix

- Mock: `crates/apiforge-desktop/src/mock.rs`, switched by statics from
  `folder/acceptance_tests/req00.rs` (`tools/gui/review.sh
  review_req00`); boards by `tools/mock/req00-boards.py`.
- Reusable as they are: `graph::sequence`, `graph::relations` (READS,
  SETS), `canvas::check_text` and `own_step` (CHECKS),
  `template_text::print_template` (URL, placeholders).
- A Cleanup step reads Main's steps: its texts need a scope over both
  graphs (`check_subjects_in` is the pattern).
