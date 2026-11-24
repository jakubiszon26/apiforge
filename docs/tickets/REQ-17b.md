# REQ-17b: One request tried without making a test of it — New request, its tab, Send

- Size: M · Owner review: 25 min · Risk: `visible UI`, `permission`
  (a live send from a new place)
- State: **Decided** ([REQ-17](REQ-17.md): 1 A, 3 A, 5 A). Mock-up
  approved, its defects below as criteria. After REQ-17a, REQ-02a,
  REQ-05 and REQ-16. Issue [#603](https://github.com/jakubiszon26/apiforge/issues/603).
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

One request is sent, read, changed and sent again with nothing named
and nothing added to the project.

## What the owner will see

- New: *New request* beside *New scenario* and on a project with no
  scenario; a tab *Request 1* with the request and response panes and
  no list; *Send* ([board and windows](REQ-17.md#mock-up)). The
  section in *Files* is REQ-17c's.
- Default behaviour that changes: none. Removed or hidden: nothing.

## Acceptance criteria

Tests in `folder/acceptance_tests/req17b.rs`.

1. *New request* opens a try with the caret in the address: no prompt,
   no file; `git status` of the project is empty.
   `req17b_criterion_1_*`
2. The tab shows REQ-02a's two panes by its own functions
   (`steps/request.rs`, `steps/response.rs`) and no list or rail;
   the header reads the try's name and "not saved · not a test of the
   project"; no *Steps | Canvas* switch, no *Run to here*.
   `req17b_criterion_2_*`
3. *Send* passes the gates of a run, which are one function that
   `request_run` and a try's *Send* both call: live only when the
   switch says *Live*; observing → `LIVE_PAUSED`; recovery pending →
   refused; live without a journal → refused when this window writes
   the project and the prompt of a run when it only reads it, as
   `request_run` does today; a live send is journalled; one run at a
   time with the documents' runs. One test per gate.
   `req17b_criterion_3_*`
4. In *Simulation* with no sample the response pane says so and offers
   the switch; *Add a sample response* (REQ-15) is not the first offer.
   `req17b_criterion_4_*`
5. The result is the try's own: no document's report, verdict or
   freshness changes; Run results does not open and **the bar at the
   bottom shows no verdict for a try** (mock defect).
   `req17b_criterion_5_*`
6. A second *Send* replaces the response; nothing asks to save; the tab
   has no unsaved dot. `req17b_criterion_6_*`
7. Several tries: each a tab, named *Request n*. Closing the tab of a
   try that was edited asks once, *Discard* or *Cancel* (until
   REQ-17c lists it in *Files*). `req17b_criterion_7_*`
8. A try survives, unchanged and with its undo: *Save all*, a
   background save finishing, a reload of the project, handing the
   project to an agent and taking it back. While an agent holds the
   project a try is still edited; its live *Send* is paused.
   `req17b_criterion_8_*`
9. **The address is shown whole when it fits and shortened from the
   left only when it does not** (mock defect; REQ-01a's rule 9).
   `req17b_criterion_9_*`, `assert_within_window` at 960 × 540.
10. `apiforge run`, `apiforge check` and the MCP server's
    `project_overview` on the same folder show no try.
    `req17b_criterion_10_*`
11. A secret the try reads from the environment is sent, and is
    `[REDACTED]` in *Request sent*, the response and the checks.
    `req17b_criterion_11_*`
12. A try uses no kept values of REQ-16: it reads no step.
    `req17b_criterion_12_*`

## Captures the owner judges on the pull request

A new try, empty; a try sent live and in simulation with no sample;
two tries' tabs; the question on closing; all at 960 × 540 and one at
1600 × 1000.

## Not in scope

The list in *Files* and keeping across a restart (REQ-17c); *Keep ▾*
(REQ-17d); history of
sends, cURL paste, Ctrl+Enter (proposals).

## Technical appendix

- Files: new `folder/tries.rs` (the window's `TrySet` of REQ-17a, the
  tab kind, the result); `folder/workbench.rs` and `chrome.rs` (a tab
  that is not a file), `folder/explorer.rs` (*New request*),
  `folder/steps_view.rs` (the two panes without the list; a snapshot
  and an edit are asked of the session or of the set by the id's
  kind, in one place), `folder/runs.rs` (the gates as one function;
  the result routed as REQ-05's kind); the test file.
- Regression: the gates of a live run — criterion 3; documents'
  reports — criterion 5 and `report_tests.rs`; the tab band's tests.
