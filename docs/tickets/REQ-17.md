# REQ-17: One request tried without making a test of it

- Size: split into four tickets of at most M · Risk: per ticket
- State: **Decided** by the owner on 2026-11-24 ([#603](https://github.com/jakubiszon26/apiforge/issues/603)): 1 A,
  2 A, 3 A, 4 A, 5 A; the board is approved, its named defects are
  criteria of REQ-17b. REQ-17c's question was answered the same day (A).
- Analysis model: Fable 5.2 · Implementation model: Opus

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
- Removed, hidden, migrated: nothing. The project format is not
  changed.

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

## Decisions (owner, 2026-11-24)

| | Decided | Not chosen |
| --- | --- | --- |
| 1 = A | A try is a request with no file, in its own tab | a scenario made for it; nothing new |
| 2 = A | Kept with the person, outside the project, back after a restart | the window only; an unregistered folder of the project |
| 3 = A | Sent by the run-mode switch, as every run | always live |
| 4 = A | *Keep ▾*: *Add to a scenario…*, *Save as a new scenario…*, *Save as component…* | without *Add to a scenario* |
| 5 = A | Several, a tab each, listed in *Files* | one at a time |

## Tickets and order

| Ticket | Issue | Size · review | Risk | After | Label |
| --- | --- | --- | --- | --- | --- |
| [REQ-17a](REQ-17a.md) tries held beside the session | [#607](https://github.com/jakubiszon26/apiforge/issues/607) | M · 10 | none visible; the session's edit code factored | — | none: redesigned after the second read, read once more first |
| [REQ-17b](REQ-17b.md) the try in the window | [#603](https://github.com/jakubiszon26/apiforge/issues/603) | M · 25 | `visible UI`, `permission` | REQ-17a, REQ-02a, REQ-05, REQ-16 | none yet |
| [REQ-17c](REQ-17c.md) listed in *Files*, kept across a restart | [#608](https://github.com/jakubiszon26/apiforge/issues/608) | M · 15 | `visible UI`, `permission` | REQ-17b | none yet |
| [REQ-17d](REQ-17d.md) *Keep* | [#609](https://github.com/jakubiszon26/apiforge/issues/609) | M · 15 | `visible UI` | REQ-17b, REQ-01b, REQ-18 | none yet |

No ticket has a `format` flag: nothing a project keeps changes.
REQ-17c adds a file of the person's own, versioned in itself. The
secret in that file is decided: A, not written (#608). A `ready:opus`
label follows the owner's recorded approval of these files.

**`prepare_run`, resolved from the code.** A run does not need a
session document: the window hands the runner a `RunRequest` holding a
whole `Project` and a scenario id (`folder/runs.rs`, `start_run_with`).
Editing needs the session's code, not its list: writing a model to
text, reading it back and undo are `install_model`, `install_step` and
`History`, which need the context, a target and the document's own
text and model. A try inside the session's list was the first design
and is dropped: the session is replaced whole on a finished save, a
reload, a hand-over and a take-back, and a try would have to be
carried across each ([REQ-17a](REQ-17a.md#why-not-a-document-in-the-sessions-list)).
A try lives in a set of its own beside the session and calls the same
functions. REQ-05 is not needed by the engine (a scenario of one
request with no Cleanup is the request); it is needed for the place in
`folder/runs.rs` that keeps a result away from the document's report.

## Not in scope

A collection of saved requests beside scenarios; importing another
client's collections (INT-01 #141); cURL (REQ-07).

## Proposals beyond the request

| # | Proposal | Problem it removes | Size · trade-off | Goes to |
| --- | --- | --- | --- | --- |
| 1 | The sends of a try are kept as a short history; two can be compared | "What did it answer before I changed that?" is lost on every send | S–M · memory only; a response can hold a secret, so never written | with 17b, or later |
| 2 | Values changed between sends are the suggested inputs of a component | The person already showed which values vary | decided: REQ-18's 1 = A | REQ-18 |
| 3 | *Try this request* on a request of a scenario: a copy as a try, with the values of the last run | Exploring around a test without changing the test | S after 17a | REQ, after 17a |
| 4 | A `curl …` pasted into an empty try's address fills it | A try usually starts in a browser's network tab | a criterion of REQ-07 | REQ-07 (#464) |
| 5 | Ctrl+Enter sends | One pointer action per try | REQ-00's P5, which waits for the owner | — |
