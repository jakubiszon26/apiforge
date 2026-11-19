# LANG-13: The steps of a block run in the order written; `@after` leaves the language

- Size: M (the upper end) · Owner review: 30 min · Risk: `format` (what a
  valid `.afs` is, what the window writes)
- State: **Rules approved; ready for implementation; four consequences to
  confirm at acceptance.** The rules of language 2, the fix and what was cut
  are the owner's decisions of 2026-11-16 and 2026-11-17
  ([LANG-00](LANG-00.md): [section 4](LANG-00.md#the-rules-of-language-2),
  section 2 rule 4, section 3, [Confirmed at approval](LANG-00.md#confirmed-at-approval) 3;
  [FLOW-00](FLOW-00.md): decisions 1 and 3–7, consequences 2–4). This file
  derives the criteria from them and decides nothing new about the
  language. Where the approved text had to be read for a case it does not
  spell out, the reading is listed [below](#to-confirm-at-acceptance) with
  the sentence it comes from.
- Issue [#386](https://github.com/jakubiszon26/apiforge/issues/386) · LANG
  sprint, first half · branch `feat/lang-13-order-written`
- Split on 2026-11-20: `apiforge fix` for a whole project and the raised
  language version are **LANG-20** (#481); the board's gestures are DIAG-06 (#391).
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

A scenario reads in the order it runs. The steps of a block run top to
bottom, a step written in a block always runs, and `@after` is gone from
the files a person writes. A file written for 0.1.0 shows an error with a
fix that deletes the `@after` lines and changes nothing else.

## What the owner will see

- Changed: a scenario, a component, a branch of an `if` and the cleanup run
  their steps in the order written. Checks and values stand anywhere and
  run when what they read exists (RUN-03).
- New: `@after` in a source is an error at its line, with a fix. A main
  flow or a component with a step and no `start`, or with a step written
  above its `start`, is an error.
- New: the fix of a file — a click on the error in the editor (the fix a
  diagnostic already carries) — deletes the `@after` lines of a file that
  is written in the order it runs today, and writes `begin = start` where
  the flow has none. Comments and layout stay. A file it cannot fix
  without changing what runs is left untouched and says which statement
  the person places.
- Changed: the window writes a chain without `@after`; the board's
  ordering wires run from step to step (the arrow of LAY-24 reads `after
  <step>`, never `after <check>`).
- Changed: every `.afs` of the repository, the reference and the examples
  are in language 2.
- Default behaviour that changes (decided, FLOW-00 and LANG-00): a file
  valid in 0.1.0 with `@after`, without `start`, or with a request that
  ran nowhere does not pass the check until fixed.
- Until DIAG-06: see consequence 3.
- Removed or hidden on screen: nothing. Migrated: nothing automatically.

## To confirm at acceptance

Readings of the approved text, each the planning model's recommendation
and the owner's to veto. None changes the work by more than a few lines.

1. **A component needs its `start` too, and the fix writes a missing
   `start`.** Rule 1 says "a flow with a step has a `start`" and FLOW-00's
   decision 4 excepts only the cleanup and a branch; decision 5 gives every
   new error a fix "that writes the chain in the order the file runs
   today". `rooms/components/sign-in.afs` is such a file. The LANG quiz's
   "the fix rewrites only the `@after` lines" is kept in its sense: nothing
   is reformatted.
2. **A published revision keeps the spelling of 0.1.0, also one published
   by 0.2.0.** LANG-00 keeps `@after` readable in a stored revision so that
   what is published runs as before. Reading old revisions with the new
   rule added on top can change them (a request that ran nowhere would
   start to run), and telling old from new would need a mark in the
   manifest. So a revision file is written and read exactly as today; it
   is never edited by hand, and the reference describes sources only.
3. **Until DIAG-06 merges, the board places a step where the window
   already writes it.** A block added from the library is written last in
   the main flow and now runs last; a step added from a card's *Next* is
   written after that step and is thereby inserted into the chain; a
   removed step joins its neighbours. Drawing an ordering wire between two
   steps that both exist, and deleting an ordering wire, change nothing
   until DIAG-06 gives them their meaning (the owner's decision 6). No
   release is cut in between (REL-05); the owner may merge this ticket and
   DIAG-06 one after the other.
4. **A model that is not a chain is not written as a source.** A new
   project's demo scenarios are chained in the order they run before they
   are written. A legacy workspace whose document holds a step that runs
   nowhere stops its migration with the error an unprintable document
   gives today (KTLO-04: a refused migration reports and stops; the
   original is untouched).

## Mock-up

Not applicable: no new element. Captures of `shop-30` and `labels` (when
FIX-01 has merged) at 960 × 540 go into the pull request, because the
ordering wires move (FLOW-00, consequence 3).

## Acceptance criteria

New test files: `crates/apiforge-script/tests/lang13.rs` (S),
`crates/apiforge-runner/tests/lang13.rs` (R),
`crates/apiforge-project/tests/lang13.rs` (P),
`crates/apiforge-desktop/src/folder/acceptance_tests/lang13.rs` (D).

1. The steps of a scenario body, a component body, each branch and the
   cleanup run in the order written, wherever their checks and values
   stand; `graph::sequence` of the lowered file lists the steps in text
   order. Tests: S `lang13_criterion_1_*` (one per block kind), R
   `lang13_criterion_1_the_run_follows_the_text`.
2. The step written after an `if` runs after the whole `if`: for `then`,
   for `else`, for a branch without a step, and for an `if` inside a
   branch. Tests: S `lang13_criterion_2_*`; LANG-14's tests pass with
   their sources fixed and their assertions unchanged.
3. `@after` in a source is an error at the attribute. When the file is
   written in the order it runs today, every such error carries a fix that
   deletes its line; otherwise none does, and the first statement out of
   place is named. Tests: S `lang13_criterion_3_*`.
4. A scenario or a component with a step and no `start` is an error with a
   fix that writes `begin = start` (the handle the board gives a new
   *Start*) as the first step; a step written above `start` is an error.
   A cleanup and a branch need none. Tests: S `lang13_criterion_4_*`.
5. `fix_source` on a file written in run order returns the text without
   its `@after` lines, with the `start` of criterion 4 where needed, every
   other byte unchanged and every comment kept; the fixed text lowers with
   no error; the order of its steps is the order the 0.1.0 reading gave;
   applied again it changes nothing. Tests: S `lang13_criterion_5_*` over
   the language-1 texts kept under
   `crates/apiforge-script/tests/fixtures/lang1/` — `checkout`, `lookup`,
   `shop-30`, `booking-day`, `sign-in` as they are on `main`, and `labels`
   as written on #454.
6. `fix_source` changes nothing and reports the statement when: a request
   or call runs nowhere today; the steps are not written in the order they
   run; a statement outside braces is ordered into a branch
   (`@after(x.then)`) or after branches of two `if`s. Tests: S
   `lang13_criterion_6_*`, one per case, each asserting the text is
   returned unchanged.
7. The window writes a chain without `@after`, steps in chain order: text
   → document → text is unchanged for every `.afs` under `fixtures/`; the
   generated-graph round trip (`tests/topology.rs`) passes for generated
   chains with `if`s. Tests: S `lang13_criterion_7_*`, `topology.rs`.
8. A published revision is written and read in the stored spelling: the
   graph lowered from `rooms/.apiforge/revisions/sign-in/1.afs` and from a
   test revision with `@after`, a fan-out and a request that runs nowhere
   equals the graph `main` lowers today (compared with a recorded JSON);
   publishing a language-2 component gives a revision whose graph equals
   the source's. Tests: S `lang13_criterion_8_*`, P
   `lang13_criterion_8_publication_keeps_the_stored_spelling`.
9. Board edits through a session each leave a text that passes the check,
   holds no `@after` and reads back as the document: a block from the
   library (runs last), a step from *Next* of a step in the middle (runs
   between), a step from a branch port, a removed middle step. An ordering
   wire between two existing steps and a deleted ordering wire leave text
   and document unchanged. Tests: P `lang13_criterion_9_*`, D
   `lang13_criterion_9_*` (library drop and *Next* at the window).
10. `prepare_project` with the demo shop writes sources that pass the
    check and whose step order equals `graph::sequence` of the demo's
    model; `fixtures/workspace.json` migrates and every project passes the
    check. Tests: P `lang13_criterion_10_*`; `tests/migration.rs`
    unchanged in what it asserts.
11. Every `.afs` under `fixtures/` (stored revisions apart), every `afs`
    example of `README.md` and of `docs/` outside `docs/qa/`, `docs/tickets/`
    and `docs/sprints/` holds no `@after` and passes the check; the
    simulated step order of each fixture equals the order recorded from
    `main` in the acceptance commit
    (`crates/apiforge-script/tests/fixtures/lang13-step-order.txt`).
    Tests: S `lang13_criterion_11_*`, R
    `lang13_criterion_11_fixtures_run_in_the_recorded_order`.
12. The reference describes language 2: *Start and ordering* and
    *Branches* are rewritten, `@after` appears only in *Diagnostics and
    what to do*; completion does not offer `@after`. Tests:
    `crates/apiforge-project/tests/reference.rs` passes,
    S `lang13_criterion_12_completion_has_no_after`.
13. ADR 0003 and ADR 0005 are amended (a flow with a step has a start and
    no step stands outside it; order is the order written; the stored
    spelling). QA step: read in the pull request.

## Regression potential

| What can break | Covered by |
| --- | --- |
| The order a repository scenario runs in | criterion 11, recorded before the change |
| A published component (the owner's `sign_in`) runs differently | criterion 8; `rooms` window tests unchanged |
| The legacy workspace window and the migration | they keep the stored spelling: `crates/apiforge-cli/tests/document.rs`, `tests/migration.rs`, smoke list 9 |
| A board edit writes a file that does not pass the check | criterion 9; the suites of LAY-34 (`edit.rs`), DIAG-07 and #433 pass with fixed sources |
| RUN-01 and the undecided `if` read the braces from the branch edge every statement in braces carries | lowering keeps that edge on every statement in braces; R `lang13_criterion_1` asserts it |
| Layout: ordering wires now leave steps, not checks, so the measured boards change | measurements are re-recorded and listed in the pull request with before and after; a **limit** that no longer holds (20 px clearance, `packed` 0, fit zoom, no crossing of a card) is not changed and the test is not ignored: it is reported on the issue with its numbers and the pull request stays a draft |
| The owner's room-booking material | `fixtures/scenarios/lay/rooms` is fixed by `fix_source`, not by hand; its window tests pass |

## Not in scope

- `apiforge fix --folder`, the manifest's raised version, the MCP surface
  of the project fix: LANG-20 (#481).
- What an ordering wire between two existing steps does, where a dropped
  step is shown to land, undo of a move: DIAG-06 (#391).
- A step kept in a file without running: rule 3 says a step written in a
  block runs; a line is taken out by a comment. A switch for it would be a
  new ticket.
- Free comments under Format and the attribute order: LANG-06 (#143).
- Running in parallel. The document stays a graph of ordering edges — the
  condition of the owner's approval. **This ticket adds no field to the
  model and stores no list of steps.**

## Technical appendix

**Two spellings.** *Written* (language 2): sources of a folder project.
*Stored* (today's reading, unchanged code): published revisions under
`.apiforge/revisions/` and the legacy JSON workspace
(`LowerContext::new`, `::document`). `LowerContext` and `Printer` carry
which; `LowerContext::folder` becomes *written*, and the loader that reads
a revision and `publish.rs` that writes one ask for *stored*.

**Lowering, written** (`lower.rs`, `walk`): keep the previous step of the
braces; a step (`start`, request, call, `if`) gets an edge from it; after
an `if` that edge is the one `after_whole_ifs` already replaces by the ends
of its branches. Every statement in braces keeps the branch edge it has
today. Checks and values get no ordering edge. `@after` is the error of
criterion 3; to know whether the file is in run order the same tree is
lowered in the stored spelling and the two step orders
(`graph::sequence`, steps only) compared.

**Printing, written** (`print.rs`): no `after` attribute; a block's steps
in chain order (the greedy order already follows the edges). A document
whose printed text does not lower back to it is not printable — `verify`
says so today.

**`chained`** (`apiforge-core/src/graph.rs`, new): the graph with its
ordering edges rebuilt as one chain per braces, in the order
`sequence` gives; an error naming the step for one that is not reachable
or not expressible. Used by `prepare.rs` and `migration.rs` before
printing, and by the session.

**Board edits** (`apiforge-project/src/session.rs`, `install_model`): the
edited model's ordering edges are rebuilt from the order of its steps in
`graph.nodes` and their braces before the text is written, so the model
follows the text (KTLO-26's read-back already keeps the model the text
says). An ordering edge the edit adds between two steps that both
existed before, or takes away, is undone before the rebuild (criterion 9),
so a statement never leaves its braces by it.
`apiforge-core/src/edit/` is **not** touched: DIAG-06 owns it.

**Files.** `crates/apiforge-script/src/{lower,print,edit,branches,complete,far,lib}.rs`
and a new `fix.rs`; `crates/apiforge-core/src/{graph,validation}.rs`;
`crates/apiforge-project/src/{session,prepare,migration,publish,context,analysis}.rs`;
test sources that contain `@after` in `apiforge-script`, `apiforge-core`,
`apiforge-runner`, `apiforge-project`, `apiforge-cli`, `apiforge-mcp` and
`apiforge-desktop` (rewritten with `fix_source`, an example binary
`crates/apiforge-script/examples/fix.rs` applies it to files);
`fixtures/scenarios/**`, `fixtures/scripts/*`, `README.md`,
`docs/language/reference.md` (not line 1: LANG-15, LANG-20),
`docs/decisions/0003-*.md`, `0005-*.md`. Not touched:
`crates/apiforge-runner/src/` (RUN-01), `crates/apiforge-storage/`,
`crates/apiforge-cli/src/`, `crates/apiforge-mcp/src/` (LANG-15, LANG-20),
`crates/apiforge-core/src/edit/` (DIAG-06), `fixtures/runs/`,
`fixtures/projects/`, `fixtures/workspace.json` (generated).

**Commits.** Acceptance tests with the recorded orders and the language-1
texts first, failing; then the language; then the repository's files and
test sources, by the tool. Merge `main` before the last push: a ticket
merged meanwhile (RUN-01's `run01.rs`, FIX-01's `labels.afs`) is fixed by
the same tool.
