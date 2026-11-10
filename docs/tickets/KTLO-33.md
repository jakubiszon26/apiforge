# KTLO-33: Tests fail now and then on CI and pass on a rerun

- Size: M (uncertainty: two causes are established from the CI evidence,
  one from the code alone, two are not yet established and need the Windows
  loop) · Owner review: 15 min · Risk: none visible beyond decisions 1 and 2
- State: Implementation (issue #122); decisions 1 = A and 2 = A taken by
  the owner on 2026-11-10; the Windows hover family of #279 added the same
  day
- Analysis model: Fable · Implementation model: Opus 5.5

## Goal

A red CI run means a regression, nothing else. Seven tests have failed at
least once on a commit that passed on its other run or on a rerun, six of
them on windows-2022 only (issue #122; the seventh,
`mcp08a::the_holders_save…`, joined on 2026-11-02 by the owner's decision). Each gets its cause named from the
code and the CI logs, a fix that waits on the right condition or removes
the race — never a sleep, never a looser assertion — and a loop of runs
that proves it.

## What the owner will see

- New: nothing.
- Changed (decision 1, option A recommended): nothing. The bar's *since*
  time after a hand-over keeps its one-second provisional form.
- Changed (decision 2, option A recommended): an observing window whose
  take-back fails for a passing reason tries again at its next check
  instead of giving up until the project is reopened; the notice *This
  window could not take the project back* appears only when three checks
  in a row failed.
- Removed, hidden, migrated, default behaviour: nothing.

## Mock-up

Not applicable: no interface change.

## The flakes, their causes and their fixes

The runs named below were read with `gh run view <id> --log-failed`
(attempt 1 where the run was rerun green). A ✔ marks a cause established
from the evidence; a ? marks the most likely cause, to be confirmed by the
loop of acceptance criterion 7 before the fix is merged.

### 1. `mcp08b::take_back_sits_at_the_right_end_of_the_bar_under_run` ✔ — test defect, with a product nuance (decision 1)

Runs 40704347762 (ubuntu, 2026-10-28), 41309802283 and 41313721850
(windows, 2026-11-01). Each failure's accessibility dump holds both texts:
the test asked for *Claude Code via apiforge mcp · since 14:22 · editing is
paused here…* and the bar said *… since 14:21 …*; the same one-minute
difference in all three (13:17 vs 13:16, 14:03 vs 14:02), and each test
ended within 2.3 s after a full minute (14:22:00.9, 13:17:02.2,
14:03:01.4 UTC).

Cause. When the window hands the project over, `handed_over`
(`folder/handover.rs`) shows a provisional description of the new holder,
`Holder::new(role, client)`, whose `since` is the window's clock at the
grant. The agent's own description was created earlier, in `take_lock`
(`apiforge-mcp/src/agent.rs:680`), before it asked the window, and it is
written to `holder.json` after the grant. The watcher's next probe reads
the file and `follow_holder` replaces the provisional `Observed` because
the two differ (`since`, `endpoint`). When the ask and the grant straddle a
minute boundary the bar's text changes by one minute once, a second or so
after the hand-over. The test reads the bar's text right after the write
is answered (`label_containing`, before the probe) and later asks for that
exact text again, after the probe replaced it. Pure timing of the test
against a one-second product transition; Windows is only slower.

Fix. The test waits until the bar shows the holder's own description —
the observed holder has an endpoint, which only the file's description has
(`folder(harness).holder.as_ref().map(Observed::holder).and_then(Holder::endpoint).is_some()`),
or equals `holder(root)` — before it reads the text, in a helper every
mcp08b/mcp10 test that reads the holder text uses. `mcp10::await_since`
has the same gap (`since` is present in the provisional text too) and takes
the same helper. The product keeps its provisional description (decision
1 A), or drops the time from it (1 B).

### 2. `mcp06_criterion_5_a_change_after_the_start_does_not_stop_the_run` ✔ — test defect

Run 41356200906 (windows, PR #184): `LiveDisabled`, *off in the project's
settings*.

Cause. `run_for` (`apiforge-mcp/src/server.rs`) claims the run slot, then
takes the writer lock (`agent.writer`: probe, `open_exclusive_as`, holder
file, staging directory), opens the session from a stable read of the
whole project, prepares the run, opens the journal, and only then checks
the permission (`run_live`, "immediately before the start", decision 16 A)
and tells the observers `run { running }`. The test waits for the slot
(`wait_until_busy`) and 250 ms, then turns the window's consent off. On a
loaded Windows runner the lock, the stable read and the journal take
longer than 250 ms, so the consent is off at the check. The product is
right: the permission is checked at the start, and a change after the
start does not stop the run. The test turned it off before the start.

Fix. The test waits for the start itself: `server.runs().observe(...)`
delivers the `run { state: running }` message that `execute` sends after
the permission check (`tools.rs:1148`); the test turns the consent off on
receiving it. No sleep, no bound to tune; the ordering in the product is
what the test now depends on, which is what criterion 5 says.

### 3. `header_tests::a_narrowing_diagram_row_drops_the_kind_then_shortens_the_name_then_the_summary` ? — cause not established; test hardened and instrumented

Runs 41307273018 and 41309802283 (windows, 2026-11-01): *hovering the
shortened name shows its full text*.

What the code says. The name is an `egui::Label::truncate()`; its tooltip
is egui's own `show_tooltip_when_elided` (`on_hover_text(job)`,
egui 0.36.2 `widgets/label.rs:285`). Every gate of that tooltip —
`tooltip_delay` 0.5 s, `is_still`, `time_since_last_movement`,
`tooltip_grace_time` — runs on the harness's simulated clock
(`RawInput::time` is `None`, so egui advances `time` by `predicted_dt`
= 0.25 s per step), not on wall time; 60 steps are 15 simulated seconds.
Nothing in the tooltip path reads the wall clock, so the runner's speed
cannot change *when* the tooltip appears, only *whether* the pointer is
over an elided label when the frames are drawn. What can differ between
frames is the layout: the header's room is computed from the toolbar "as
last drawn" (`canvas::header_room`) and from the steps summary, and the
once-a-second watcher check and the storage indicator (`notices`: a
spinner while a job runs) redraw the window on their own schedule. The two
candidates are (a) a relayout during the 60 steps that moves or widens
the name so the pointer, fixed at the old centre, is no longer over an
elided label; (b) the hover harness at 880 px not eliding the name at all
on that run (the test asserts the level on *another* harness of the same
size, never on the one it hovers). Local evidence: the test passed 30
of 30 consecutive runs on Linux (3 cores, two CPU burners), so the Linux
build does not reproduce it and the cause is read, not measured.

Fix. (1) The test asserts, on the harness it hovers, that the name is
elided (`given_up == 2`) before hovering, and hovers the *current* centre
of the name. (2) It then steps until the tooltip node appears, bounded by
simulated time (the same 15 s, 60 steps), rather than asserting after a
fixed number of steps — the same assertion, waited for. (3) On failure it
prints the diagnosis the next CI failure needs: the name's rect per step,
the pointer position, `egui::Context::input(|i| i.pointer.is_still())`,
and whether `egui::Tooltip::was_tooltip_open_last_frame` — so a recurrence
on Windows names (a) or (b) at once. The loop of criterion 7 is the
evidence; if the loop still fails, the ticket is reopened with that
diagnosis.

### 4. `mcp07::a_transaction_left_by_the_holder_opens_the_recovery_view` ? — product defect, two candidates, both closed

Run 40656690200 (windows, PR #113): *the window never took the project
back* within 10 s after *Reload project*.

What the code says. After the reload the watcher probes once a second;
with the lock free, `follow_holder(None)` calls `take_back`
(`folder/holder.rs:165`), which runs `take_writer` on the storage worker.
The take-back is refused for good — `take_back.stopped = true`, *This
window could not take the project back* — on any error other than
`Locked`/`StaleStorage`, and nothing resets `stopped` short of a reload.
On Windows the first thing `take_writer` writes is the holder file,
through `tempfile::NamedTempFile::persist`, which is `MoveFileExW(…,
MOVEFILE_REPLACE_EXISTING)`: it fails with *Access is denied* while any
handle to `holder.json` is open — the window's own check thread reading
it a moment earlier, or an antivirus scanner looking at the file the
recovery view's temporary writer had just rewritten (`ProjectStore::open`
in `review_transaction`). One such failure stops the take-back for good;
the test's ten seconds pass. The replace primitive is MCP-19's subject
(#191): the same call, under an agent's commit, is what refuses a write in
front of the person. Candidate (b), a stale `RecoveryRequired` check
result re-arming the recovery screen after the reload, was ruled out from
the code: while the recovery view's writer holds the lock, `scan_tree`
answers `Busy`, never `RecoveryRequired` (`explorer.rs:97-115`).

Fix. (1) MCP-19 replaces the primitive, so a reader beside the write no
longer fails it (this ticket does not duplicate that change; it depends on
it for the Windows loop). (2) In this ticket: a take-back that fails for a
reason that is not final is tried again at the next probe; the notice is
shown after three consecutive failures and the window keeps trying
(decision 2 A), or the behaviour stays and only MCP-19 narrows the failure
(2 B). A deterministic test drives the first attempt into an I/O error
through the storage fault seam (`project::faults`, `Boundary::Replace` on
`holder.json`) and shows the window the writer at the next probe.

### 5. `mcp06_criterion_6_a_cancelled_or_timed_out_live_run` ? — test defect

Run 40684628910 (windows, PR #117): *no request was sent* — the listener
accepted no connection within 10 s of the run thread's start.

What the code says. Before the first request the run thread takes the lock
(with its 200 ms lock confirmation window when contended), reads the
project, prepares, opens the journal, and `reqwest` resolves and connects
to `127.0.0.1`. `wait_for_journal` waits for the first *accepted*
connection with a 10 s bound and no other exit: a run that was refused
(`held_by_other`, a storage error, a Windows replace failure of the
journal — the same primitive as above) is indistinguishable from a slow
one; the test panics before `running.join()` could say what happened. Ten
seconds under four parallel test threads, Defender scanning a fresh
checkout, and the acceptance suite's other servers is a timing assumption.

Fix. The wait exits on either condition: a request accepted, or the run
thread finished — in which case the test fails with the run's actual
result, which names the cause the next time. The bound stays only as a
guard against a hang and is raised to the suite's other bound (45 s,
`answer`), since it no longer decides the outcome. If the Windows loop
shows a refusal rather than slowness, that refusal is MCP-19's.

### 6. `folder::tests::deleting_a_folder_closes_its_tabs_and_the_trash_restores_unsaved_work` ✔ (from the code) — product defect, MCP-19's

Run 40593992618 (windows, docs-only PR #108, 2026-10-27): `tests.rs:1042`,
a tab of the deleted document still open after the delete job finished.

Cause. The delete is a transaction on the storage worker; its callback
closes the tabs of documents that are gone (`queue_delete` →
`follow_external`). The tab is still open only when the delete failed and
the window said *Could not delete*. On Windows the watcher's check thread
hashes every source file once a second (`scan_tree` → `hash_file`), and
`scan_tree` deliberately does not take the store's worker lock
(`explorer.rs:98`), so it runs beside the delete's own file operations in
the same process. A `remove_file` of a source the hasher holds open
succeeds but leaves the name delete-pending until the handle closes, and
the transaction's `remove_dir` of `scenarios/` then fails with *The
directory is not empty*; a replace in the same transaction (registry,
recovery) fails with *Access is denied* through the `MoveFileExW`
primitive. Either is after activation, so the window shows recovery and
the delete reports an error. Linux has neither limitation, which is why
only Windows sees it.

Fix. MCP-19 (#191): the replace primitive, and the scan serialised with
the worker in the same process. This ticket only records the flake and
runs the loop once MCP-19 is in.

### 7. `mcp08a::the_holders_save_is_shown_within_two_seconds_where_the_person_was` ✔ — product defect, MCP-19's (added 2026-11-02 by the owner's decision)

Run 41449932414 (windows, pull_request run of PR #199 at `04a11f2`; the
push run of the same commit passed): `acceptance_tests.rs:1297`, the
holder's `owner.save(&scenario)` returned `Storage(Io { path:
"…\.apiforge\diagram.json", Os { code: 5, PermissionDenied, "Access is
denied." } })`.

Cause. The holder is a second `ProjectSession` in the test process that
took the writer lock first (`mcp08a::holder`), as the MCP server's does;
the window observes it. Its save is one transaction that replaces the
scenario's `.afs` and the diagram checkpoint `.apiforge/diagram.json`
through `project::io::atomic` → `NamedTempFile::persist` →
`MoveFileExW(MOVEFILE_REPLACE_EXISTING)`. The observing window's check
thread, once a second and at once after a change, hashes the tree and
reads the project with a stable read whose file list includes
`.apiforge/diagram.json` (`load_locked`), and, as an observer, reads the
checkpoint's state to see commits outside the sources
(`external.rs::run`, `committed_outside_sources`). While that read holds
the file open, the writer's `MoveFileExW` is refused with *Access is
denied* (os error 5): the precise collision MCP-19 (#191) names — the
observer's stable read against the writer's replace — on another file.
On Linux the rename succeeds and the reader keeps the old bytes. Whether
the refusal came before or after activation decides whether the window
also showed recovery; the holder's error is the same either way, and the
test's `unwrap` is right to fail on it: a save refused in front of the
person.

Fix. MCP-19, part 1 (the replace primitive), and nothing here: the test
is unchanged, and the Windows loop of criterion 7 runs it after MCP-19
merges. The fix stays in one place.

The issue's first entry (the ubuntu push run of #118, 2026-10-28) is run
40704347762, flake 1 above: the minute boundary falls on Linux as well.

### 8. The Windows hover family (#279) and flake 3 ✔ — test defect, one shared hover

Added by the planning model on 2026-11-10. Failing on most Windows runs
and passing on a rerun: `ux33_the_icon_shows_one_tooltip`,
`ux33_criterion_1_a_component_board_has_the_button_a_scenario_none` and
`ux34_criterion_4_a_chip_alone_cannot_be_copied` (all through
`ux02::hover_shows`), once `ux18::fit_the_view_still_fits_beside_the_inspector`;
flake 3 is the same family.

Cause, established in the implementation. The window protects unsaved
work and the view one second of **wall** time after the last change
(`touched`: a mode, a tab, a panel; `saving.rs::protect`,
`RECOVERY_DELAY`). The job runs on the storage worker; when it runs
longer than 150 ms the storage band (*Protecting unsaved work…* with a
spinner, KTLO-24) shows under the tabs and moves everything below it down
by its height until the job ends. A hover test moved the pointer to the
widget's centre, settled, and took sixty steps; a fast Linux runner ends
the steps before the job starts or finishes the job within 150 ms, a slow
Windows runner is in the middle of it at the last step, with the widget
below the pointer. The tooltip delay itself runs on the harness's
simulated clock, as the analysis said, so only the layout differs.
Reproduced on Linux by slowing the storage worker 200 ms per file
operation through its fault seam: the *Component settings* icon moved from
y 146 to y 184, the old hover failed and the new one passes; the UX-18 fit
ended with exactly the CI failure's rectangles (cards
`[[258 261] - [568 407]]`, focus `[[272 204] - [568 464]]`): the band
moves the board's viewport, which re-applies UX-18's rule that keeps the
one selected card whole in view, panning the centred board 7 px.

Fix. `testing::quiesce` settles, makes the pending protection due now
(`FolderApp::debug_protect_now`) and waits for it. `testing::hover_until`
is the one hover every hover test uses (`hover_shows`, the narrowing row,
the header name, the run label, *Follow*, *Run simulation*): it quiesces,
hovers the widget's current centre, asserts that the pointer is over it,
steps until the expected text shows within fifteen simulated seconds
(the old sixty steps), and on failure prints per frame the widget's
rectangle, the pointer, `is_still`, whether a tooltip layer is open and
whether the storage band shows. The narrowing-row test asserts the name is
shortened on the window it hovers. UX-18's fixture quiesces after
choosing Diagram. No assertion changed.

Seen, not changed (a product matter for the owner, written on #122): on a
slow disk the band appears for a moment a second after a mode or tab
change and the board below it jumps by its height; beside a selected card
at 960 × 540 the jump leaves a fitted board panned.

### 9. `mcp16` follow bounds ✔ — test defect

`mcp16_criterion_4_a_publication_is_followed` failed on windows-2022 in
run 42881644458 (PR #313, documents only): *at the document 2.1471401s
after the write*, against the two-second bound; `mcp16_criterion_2_*`
did the same once at 2.3 s (run 41554907272). The bound is the window's
(MCP-16: the window follows a commit within two seconds), but the tests
started the clock before the agent's call, so it also held the agent's
own transaction: for a publication a revision, the registry and several
replaces, the slowest work a loaded Windows runner does. Not the
storage-band family. Fix: the clock starts when the agent's answer
arrives, which is when its commit is done; the bound and the assertion
are unchanged.

## As built

- Flake 4 (decision 2 A): `TakeBack::stopped` became `failures`; any
  failure other than `Locked`, `StaleStorage` and a transaction to
  recover is tried again at the next check, the notice appears at the
  third failure in a row and the window keeps trying; a success resets
  the count. The tests inject the failure at the flush of
  `.apiforge/local` that `open_exclusive_as` makes right after it takes
  the lock (`Boundary::DirectorySync`): the holder file's own replace goes
  through `apiforge_storage::write_atomic`, which the fault seam does not
  reach. Tests: `ktlo33_a_take_back_that_fails_once_is_tried_again_at_the_next_check`,
  `ktlo33_three_failed_take_backs_in_a_row_show_the_notice_and_the_window_keeps_trying`.
- Flake 1: `mcp08b::await_holder_file`, used by
  `take_back_sits_at_the_right_end_of_the_bar_under_run` and by
  `mcp10::await_since`; `holder.rs` test
  `the_provisional_description_after_a_hand_over_names_no_endpoint`.
- Flake 2: the consent goes off from the run's own `running` message
  (`Runs::observe`); moving the check after the run fails the test with
  `LiveDisabled` (checked by hand once).
- Flake 5: `wait_for_journal` watches the run (`Running`) and fails with
  its result when it ends first; the bound is 45 s. Test:
  `ktlo33_a_run_refused_before_any_request_ends_the_wait_with_its_refusal`.
- Flakes 3 and 8: see 8.
- Flakes 6 and 7: MCP-19 (#191) is merged; no change here, the loop runs
  them.
- The loop: `.github/workflows/flake-loop.yml`, by hand once on `main`,
  or by a push whose head commit says `[flake-loop]`.

## Decisions

Taken by the owner on 2026-11-10 (quiz on the analysis, recorded on #122):
**decision 1 = A** — the bar keeps its provisional *since*; the tests wait
for the holder's own description — and **decision 2 = A** — an observing
window whose take-back fails for a passing reason tries again at its next
check, and the notice appears only after three failed checks in a row. The
earlier answer of 2026-11-02 (2 = B) is superseded.

1. **The provisional holder description after a hand-over.**
   - **A (recommended):** keep it. For up to a second after the grant the
     bar names the agent with the window's own time of the grant; then the
     holder's own description, read from `holder.json`, replaces it, and
     once in sixty hand-overs the minute shown changes by one. The test
     waits for the holder's own description. No change on screen.
   - **B:** the provisional description shows no time (*Claude Code via
     apiforge mcp · editing is paused here…*) until the holder's own is
     read; the time then appears. Honest, visible, and a `visible UI` change
     to MCP-08b's approved bar for a one-minute flicker nobody has seen at
     the window.

2. **A take-back that fails for a passing reason.**
   - **A (recommended):** the window tries again at its next check; after
     three consecutive failures the bar's notice says so and the window
     keeps trying. A reader or a scanner that held a file for a moment
     costs one second, not a reopen. The notice text is unchanged.
   - **B:** today's behaviour: the first failure stops the take-back and
     tells the person, who reopens the project. MCP-19 alone makes the
     failure rarer on Windows.

## Acceptance criteria

1. Flake 1: `take_back_sits_at_the_right_end_of_the_bar_under_run` and
   every mcp08b/mcp10 test that reads the holder's text wait for the
   holder's own description through one helper; a test in `holder.rs`
   shows that the provisional and the file's description differ in
   `endpoint`, so the wait has a condition. Under decision 1 B, the
   provisional text has no time and a `holder.rs` test says so.
2. Flake 2: `mcp06_criterion_5` turns the consent off on the run's
   `running` message; a test seam or the existing `Runs::observe` carries
   it. The test fails before the fix when the check is moved after the
   first step (shown once in the acceptance commit's message, not kept).
3. Flake 3: the hover test asserts elision on the harness it hovers, waits
   for the tooltip bounded by simulated time, and prints the diagnosis on
   failure. The assertion is the same.
4. Flake 4 (decision 2 A): a desktop test forces the first take-back's
   holder-file replace to fail through the fault seam and shows the window
   the writer at the next probe, with no notice; a second test with three
   failures shows the notice and the window still trying. Under 2 B this
   criterion is dropped and criterion 7 alone covers the flake.
5. Flake 5: `wait_for_journal` exits on the run's end and reports its
   result; a test with a server whose run is refused before any request
   fails within the run's own time with the refusal in the message.
6. Flakes 6 and 7: recorded here, no test changes; their proof is criterion
   7 after MCP-19 merges.
7. **The loop.** A `workflow_dispatch` workflow `flake-loop.yml` builds the
   test binaries once and runs a named test filter N times on a chosen
   runner (`ubuntu-24.04`, `windows-2022`), failing on the first failure
   and uploading the output. Each of the seven tests passes 20 of 20 on
   windows-2022 and 20 of 20 on ubuntu-24.04 with the fixes, and the run
   ids are in the QA report. Flakes 4, 6 and 7 are looped after MCP-19 merges
   or on a branch that carries both.
8. No assertion of any of the seven tests is weakened; `git diff` of the
   assertions is quoted in the QA report.
9. The verification commands of the development guide pass.

## Not in scope

- The Windows replace and removal semantics and the scan beside the
  worker: MCP-19 (#191), which this ticket's flakes 4, 6 and 7 depend on.
- The watcher's once-a-second full hash of every source (its cost; a
  cheaper probe): backlog, *Later*.
- `settle()` not waiting for a running watcher check: backlog, *Later*.
- `resolving_one_documents_conflict_leaves_another_documents_unsaved_work_and_history`
  (backlog *Later*, one failure after #19): not seen since; it joins this
  ticket only if the loop reproduces it.

## Technical appendix

- Evidence files: the failed logs are read with `gh run view <id>
  --log-failed` (`--attempt 1` for 41309802283, 41313721850, 40704347762,
  40593992618). The three take-back dumps carry the two texts at tree
  lines *By { label: … since 14:22 … }* and *value: … since 14:21 …*.
- Flake 1 code path: `handover.rs::handed_over` (provisional
  `Holder::new`), `holder.rs::follow_holder` (replacement on inequality),
  `agent.rs:680` (the agent's `since` before the ask),
  `watcher.rs::poll` (probe after each check, held until the first
  `saved`). Helper to add beside `mcp08b::await_until`: `await_holder_file`.
- Flake 2 code path: `server.rs::run_for` (slot before lock),
  `run_live` (check after prepare), `tools.rs:1148`
  (`runs.tell(Running)`), `tools.rs::Runs::observe`.
- Flake 3: egui 0.36.2 `containers/tooltip.rs:238-375`,
  `input_state/mod.rs:374` (`time = new.time.unwrap_or(self.time +
  predicted_dt)`), `egui_kittest` `builder.rs:40` (`step_dt` 0.25),
  `lib.rs::_try_run` (stops at a delayed repaint). Local loop of
  2026-11-02: 0 failures in 30 runs of the unmodified test on Linux with
  two CPU burners.
- Flake 4 code path: `holder.rs::take_back` (`stopped`),
  `external.rs::take_writer`, `transaction.rs::open_exclusive_as` →
  `write_holder` → `file.rs::write_atomic` → `NamedTempFile::persist`
  (tempfile 3.27 `file/imp/windows.rs:92`: `MoveFileExW`); the recovery
  view's own writer `transaction_view.rs:49`. `explorer.rs:97-115` for the
  `Busy`-not-`RecoveryRequired` argument.
- Flake 5: `acceptance.rs::wait_for_journal` and `Listener`.
- Flake 6: `file_prompts.rs::queue_delete`, `explorer.rs::scan_tree` (no
  worker lock), `io.rs::remove` (`fs::remove_dir`).
- The loop workflow: a matrix of one, `cargo test --no-run` then a shell
  loop over the built binary with `--exact <filter>`; Windows in
  PowerShell as `rust.yml` does. About 60 lines.
- Estimate: M · 15 min · about 450 changed lines (tests 220, product 40
  for decision 2 A, workflow 60, this file and the QA report the rest).
