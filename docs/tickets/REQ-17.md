# REQ-17: One request tried without making a test of it

- Size: L, to split once question 1 is answered (17a the try, 17b
  keeping it; each M) · Owner review: 25 min · Risk: `visible UI`,
  `default`, a permission control
- State: **Analysis; five questions wait for the owner.** From the
  owner's wish of 2026-11-24. Not scheduled, no `ready:*` label ·
  issue [#603](https://github.com/jakubiszon26/apiforge/issues/603)
  · REQ sprint
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

"The program should be comfortable also when someone wants to experiment
and test something exploratorily, e.g. send a single request to an API."
A person sends one request, reads the answer, changes the address, a
header or the body and sends again, as often as needed, without first
naming a file or adding a test to the project. What is worth keeping is
kept afterwards ([REQ-18](REQ-18.md) for a component).

## Today's path, and what the decided tickets change

Walked on `main` at `1923fcd3` from the code, with AB-01's counted T1
(13 pointer actions and 2 text entries for one GET, v0.2.0) as the
check; not driven in the window for this analysis. The task: an
initialised empty project, one POST with a body sent live, the response
read; then three tries — the address, a header, the body — each sent
and read.

| | Today (`main`) | After the decided tickets | After this ticket (A) |
| --- | --- | --- | --- |
| To the first response | 13 pointer · 3 text · 1 scroll | 13 pointer · 3 text | 6 pointer · 2 text |
| Three tries | 7 pointer · 4 text · 2 scrolls | 9 pointer · 4 text | 9 pointer · 4 text |
| Named before the first send | a file | a file | nothing |
| Left in the project | a registered scenario | a registered scenario | nothing until kept |

The decided tickets do not shorten the path; they take away the scrolls,
the bottom panel and the run of everything else. What stands in the way:

| Obstacle | Taken away by | Remains |
| --- | --- | --- |
| The left panel shows *Library*; a new scenario opens in Code | WS-04, in REQ-01b (criterion 6) | — |
| A file is named before anything is sent (*New scenario* → name → *Create*) | — | **this ticket** |
| The file is a scenario of the project at once: in *Files*, in Git, run by `apiforge run` and by an agent | — | **this ticket** |
| A request is a step of a scenario: *Add step*, then select it | REQ-01b | one action too many for a single request |
| The body is the eighth section of a 320 px Inspector; headers are typed | REQ-02a, REQ-02b, REQ-03 | — |
| One request is sent by running the scenario; the response is in the bottom panel | REQ-05 (criterion 2: with no last run), REQ-02a | — |
| The project starts in *Simulation*; a request with no sample is led to *Add a sample response* (REQ-15) | — | question 3 |
| A request behind a login cannot be sent alone | REQ-16 = A (owner, 2026-11-24) | — |
| A request copied from elsewhere is retyped | REQ-07, to analyse | proposal 4 |
| Each send replaces the response of the one before | — | proposal 1 |

A run does not ask for a save (`prepare_run` settles the open text), so
"unsaved" is not an obstacle; the file made by *Create* is.

## What the owner will see

- New (A): *New request* beside *New scenario* and on an empty
  project's first screen; a tab *Request 1* with REQ-02a's request and
  response panes and no list of steps; *Send*; *Keep ▾*.
- Default behaviour that changes: nothing for a scenario. How a try is
  sent is question 3.
- Removed, hidden, migrated: nothing. The project format is not changed
  under A with question 2 = A or B.

## Mock-up

Throwaway branch `mock/req-17` (on `mock/req-00`, so the panes are the
decided ones), commit `df0b06c1`; captures under
`docs/qa/2026-11-24/REQ-17/` of that commit.
[Board, 960 × 540](https://raw.githubusercontent.com/jakubiszon26/apiforge/df0b06c18e689eb1f05d8c24bcd6c0188d0ced34/docs/qa/2026-11-24/REQ-17/board-try-960.png) ·
whole windows:
[A at 960](https://raw.githubusercontent.com/jakubiszon26/apiforge/df0b06c18e689eb1f05d8c24bcd6c0188d0ced34/docs/qa/2026-11-24/REQ-17/window-req17-a-960.png) ·
[A at 1600](https://raw.githubusercontent.com/jakubiszon26/apiforge/df0b06c18e689eb1f05d8c24bcd6c0188d0ced34/docs/qa/2026-11-24/REQ-17/window-req17-a-1600.png) ·
[*Keep* open](https://raw.githubusercontent.com/jakubiszon26/apiforge/df0b06c18e689eb1f05d8c24bcd6c0188d0ced34/docs/qa/2026-11-24/REQ-17/window-req17-a-keep-960.png) ·
[B at 960](https://raw.githubusercontent.com/jakubiszon26/apiforge/df0b06c18e689eb1f05d8c24bcd6c0188d0ced34/docs/qa/2026-11-24/REQ-17/window-req17-b-960.png).
Data: a fictional `shop.example.test`. Known defects of the mock, not to
be copied: the address shortened to `…/shop/carts` with room left
(REQ-01a 9); *PASSED* in the bottom bar of a try, which has no run;
"1 steps" in B.

## Questions for the owner

Recommended first.

1. **What a try is.** **A — a request with no file**: its own tab, the
   two panes, nothing in the project until kept. · B — a scenario made
   for it in one action (*Try a request* writes `scratch/request-1.afs`
   with one request and opens it in *Steps*): no new kind of tab, S, but
   it is a registered test that `apiforge run` and Git see. · C — nothing
   new: the decided path, and only the acceptance task of
   [REQ-06](REQ-06.md#the-task) part 3.
2. **Where a try lives** (under A). **A — with the person, outside the
   project**: kept in the application's data folder per project, as the
   run mode is, back after a restart; never in the folder, in Git or
   before an agent. · B — in the window only; closing asks. · C — a
   folder of the project that is not registered: changes what Git sees
   (`format`).
3. **How a try is sent.** **A — by the run-mode switch, as every run**
   (REQ-00's 3.3 stands); in *Simulation* with no sample the pane
   offers the switch, not a sample. · B — a try is always live, confirmed
   once per project. · C — always live, never asked.
4. **Keeping.** **A — *Keep ▾*: *Add to a scenario…* (after its selected
   step, or last), *Save as a new scenario…*, *Save as component…*
   ([REQ-18](REQ-18.md)); a name is asked only here.** · B — a new
   scenario and a component only.
5. **How many.** **A — several, a tab each, listed in *Files* under
   *Requests, not saved*.** · B — one at a time.

## Acceptance criteria

Written for 1 = A; numbered again per split ticket once decided.

1. *New request* opens a try with the caret in the address; no prompt,
   no file, the manifest unchanged.
2. The panes are REQ-02a's own (`steps/request.rs`,
   `steps/response.rs`), not copies; the project's base URL and
   environment apply as in a scenario.
3. *Send* sends exactly one request through `request_run`'s gates
   (REQ-05, criterion 6); a live send is journalled.
4. A changed field and *Send* again replace the response; nothing asks
   to save.
5. `apiforge run`, `apiforge check`, the MCP server's `project_overview`
   and Git see no try.
6. Each *Keep* entry writes what typing the request into that place
   writes; kept to a scenario, the try's checks go with it.
7. At 960 × 540 both panes hold 320 px and nothing is cut.
8. QA step: part 3 of REQ-06.

## Not in scope

A collection of saved requests beside scenarios; importing another
client's collections (INT-01 #141); cURL (REQ-07).

## Proposals beyond the request

| # | Proposal | Problem it removes | Size · trade-off | Goes to |
| --- | --- | --- | --- | --- |
| 1 | The sends of a try are kept as a short history; two can be compared | "What did it answer before I changed that?" is lost on every send | S–M · memory only; a response can hold a secret, so never written | with 17b, or later |
| 2 | Values changed between sends are the suggested inputs of a component | The person already showed which values vary | S, inside REQ-18 (its question 1) | REQ-18 |
| 3 | *Try this request* on a request of a scenario: a copy as a try, with the values of the last run | Exploring around a test without changing the test | S after 17a | REQ, after 17a |
| 4 | A `curl …` pasted into an empty try's address fills it | A try usually starts in a browser's network tab | a criterion of REQ-07 | REQ-07 (#464) |
| 5 | Ctrl+Enter sends | One pointer action per try | REQ-00's P5, which waits for the owner | — |

## Technical appendix

- Reuse: REQ-02a's panes, REQ-04a's `Partial::Alone`, REQ-05's result
  kept beside a report, `run_mode.rs` as the model for a per-person,
  per-project file outside the project.
- Open: `prepare_run` takes a document of the session
  (`apiforge-project/src/execution.rs:112`); a try is not one. Either
  the session learns a document that is never in the manifest, or the
  window builds the one-request graph itself. This decides whether 17a
  is M; to be read before the split is labelled.
- Under B the whole ticket is S on REQ-01b and REQ-05.
