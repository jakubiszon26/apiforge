# MCP-08a: Observer mode — editing paused while another holds the project; the window follows every commit

- Size: M · Owner review: 30 min · Risk: `visible UI`, `default`, `removal`
- State: Backlog (issue #90) · MCP-00 decisions 10 and 12 = A taken
  2026-10-27 · mock-up M1 approved 2026-10-27 · after MCP-07
- Analysis model: Fable · Implementation model: —
- Analysis: [MCP-00](MCP-00.md)

## Goal

While someone else holds the project — an agent, another window, `apply` —
the window is an observer: it shows the committed state within two seconds
of every commit, exactly as the holder sealed it, keeps the person's place
in the project, and does not let them type into a file they cannot save.

## What the owner will see

- New: the badge reads *Observing* instead of *Read-only*; the notice names
  the holder (MCP-07) and says *editing is paused here; this window follows
  every change*; a line under it says what changed last (*10:43 ·
  scenarios/promotion.afs saved*). A new document appears in the explorer, a
  changed one in its tab, a new revision in the component's tab — without a
  reload notice each time.
- Changed: the active document, its tab, its camera and the selected block
  stay where they were across a reload when they still exist; today a
  reload drops the active document.
- Removed or hidden: typing in the code view, editing the board and the
  inspector, the explorer's create, rename and delete, and the public part of
  *Environment* are disabled while observing (decision 12: a `removal` of
  edits that could never be saved). *Project settings* opens with every
  control disabled (MCP-09 enables its toggle).
- Default behaviour that changes: the observer reloads the whole project on
  every commit instead of following files one by one.

## Mock-up

M1 (shared with MCP-07, MCP-08b), round 4, approved 2026-10-27:
`docs/qa/2026-10-27/MCP-00/m1-observer-*.png` — badge, bottom bar and the
paused code view at 960 × 540 and 1600 × 1000, Code, Diagram and Split.

## Acceptance criteria

1. With the lock held by another process, the window shows *Observing* and
   the holder's bar; the code view does not accept keystrokes, the board
   does not accept a drop from the library or a wire, the inspector fields
   are disabled, the explorer's create, rename and delete are disabled, *Run*
   (simulation) works, *Run live requests* is disabled with a hover text.
2. The holder saves a changed scenario (the test writes it with
   `apiforge apply` or the session): within two seconds the code view shows
   the new text, the diagram shows the blocks with the identities and
   positions of the holder's checkpoint (compared with `diagram.json`), and
   Problems shows the new diagnostics; the active tab, the camera and the
   selected block (by identity) are unchanged.
3. The holder registers a new document (session `create_document`): within
   two seconds the explorer lists it; no *reloaded* notice; the active tab is
   unchanged.
4. The holder publishes a revision: within two seconds the component's tab
   shows the new revision and a scenario calling it analyses without error.
5. A commit of source and checkpoint together is never shown half: a test
   that holds the transaction between its staged files (storage seam)
   observes either the old text with the old checkpoint or the new with the
   new.
6. An analysis result for the previous text that arrives after the reload is
   rejected: the diagram never goes back to the earlier state (a test delays
   the analysis worker across a commit).
7. A tab whose document the holder deleted closes with a notice; every other
   tab stays.
8. The window's own checkpoint baseline is the one on disk after each reload:
   after take-back (MCP-07) a save in the window succeeds with no conflict on
   `diagram.json` — the gap that exists today beside `apply`.
9. At 960 × 540 the badge and the bar with its last-change text stay inside
   the window in Code, Diagram and Split.
10. The QA report compares the implementation against M1; the verification
    commands of the development guide pass.

## Not in scope

The hand-over request, the channel and the immediate rescan on `saved`
(MCP-08b; this ticket meets the two-second target with the one-second poll);
the prompt with unsaved work (MCP-08c); the live toggle (MCP-09); run status
(MCP-10).

## Technical appendix

- Observer state: `read_only` in the session already; the chrome gains
  `observing()` and the code view takes `interactive(false)`, the board's
  drop and wire handlers check it, `explorer.rs` disables its commands,
  `settings.rs::open_project_settings` opens the dialog with
  `enabled(false)` instead of returning.
- Reload instead of follow: in `apply_external`, when the session is
  read-only and any committed file changed (source, checkpoint, manifest,
  revisions), call `reload()` (new epoch) and report `project_reloaded`; the
  watcher's check must include `diagram.json` and the revisions directory in
  its fingerprints (today only sources and the manifest).
- Keeping the place: `follow_external` keeps `shown` when
  `session.document(shown)` exists after the reload, keeps `self.canvases`
  by document id (the camera), and re-selects the block by its identity when
  it survived.
- Last-change text in the bar: from the external check's diff (paths whose fingerprint
  changed) and the time of the scan.
- Tests: `folder/acceptance_tests.rs` with the lock taken by the test process
  and writes through a second `ProjectSession` (exclusive) in the test;
  `project_interaction_tests.rs::follow_disk` is the pattern.
