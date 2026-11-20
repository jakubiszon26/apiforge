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
- Split on 2026-11-20: `apiforge fix` for a whole project is **LANG-20**
  (#481); the board's gestures are DIAG-06 (#391).
- **Amended on 2026-11-20** after the first implementation (two
  escalations on #386) and an independent review: see
  [Amendments](#amendments-of-2026-11-20), which a line above it yields to.
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
- New: `@after` in a source is an error at its line, with a fix. A scenario
  whose main flow has a step and no `start`, or a step written above its
  `start`, is an error; a component needs no `start`.
- New: the fix of a file — a click on the error in the editor (the fix a
  diagnostic already carries) — deletes the `@after` lines of a file that
  is written in the order it runs today, and writes `begin = start` where
  a scenario's main flow has none. Comments and layout stay. A file it cannot fix
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

1. **A scenario's main flow needs its `start`, and the fix writes a
   missing one; a component needs none.** *Withdrawn in part on
   2026-11-20:* the first reading asked a `start` of a component too. The
   board does not offer *Start* on a component (UX-21, UX-40: "*Start*
   only a scenario runs"), and FLOW-00's criterion speaks of "a main flow
   with a step and no `start`". A component's body runs from its first
   step written; a `start` it already has stays valid. For a scenario,
   decision 5 gives the error a fix "that writes the chain in the order
   the file runs today"; nothing is reformatted.
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
   removes **the attribute** — its line when it stands alone on it,
   otherwise only its own text — and no comment; otherwise none does, and
   the first statement out of place is named. Tests: S
   `lang13_criterion_3_*`, among them `@after(a) b = GET "/b"` on one
   line, a comment beside the attribute, and an attribute broken over two
   lines.
4. A scenario whose main flow has a step and no `start` is an error with
   a fix that writes a `start` as the first step, under the handle the
   board would give it (`begin`, or `begin_2` when a statement is already
   called `begin`); a step written above `start` is an error. A
   component, a cleanup and a branch need none, and a component's `start`
   that is there stays. Tests: S `lang13_criterion_4_*`, among them the
   taken handle and a component without `start` that passes.
5. `fix_source` on a file written in run order returns the text without
   its `@after` lines, with the `start` of criterion 4 where needed, every
   other byte unchanged and every comment kept; the fixed text lowers with
   no error; the order of its steps is the order the 0.1.0 reading gave;
   a text with nothing to fix is returned as it is, so applying the fix
   to its own result changes nothing. Tests: S `lang13_criterion_5_*` over
   the language-1 texts kept under
   `crates/apiforge-script/tests/fixtures/lang1/` — `checkout`, `lookup`,
   `booking-day`, `sign-in` and `labels` as they were on `main` before
   this ticket. (`shop-30` is a case of criterion 6.)
6. `fix_source` changes nothing and reports the statement when: a request
   or call runs nowhere today; the steps are not written in the order they
   run; a statement outside braces is ordered into a branch
   (`@after(x.then)`) or after branches of two `if`s. Tests: S
   `lang13_criterion_6_*`, one per case, each asserting the text is
   returned unchanged; the language-1 `shop-30`, whose `refund` is written
   after the braces of `paid` and ordered into its `then`, is the third
   case and names `refund`.
7. The window writes a chain without `@after`, steps in chain order: text
   → document → text is unchanged for every `.afs` under `fixtures/`
   (stored revisions and the kept language-1 texts apart); the
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

## Amendments of 2026-11-20

After the first implementation (draft pull request #488, two escalations
and a report of limits on #386) and an independent review of this file.
Where a line above disagrees, this section holds.

**Rulings of the planning model**

- **A1 — `shop-30` (criteria 5 and 6).** The fix is right to refuse it
  and criterion 5 was wrong to list it. A file the fix refuses is
  converted by hand **so that what runs stays the same**: `refund`,
  `refund_amount` and `logout` are written inside the `then` braces of
  `paid`, where the 0.1.0 reading already ran them. The boards are
  measured again after that edit.
- **A2 — a check or value that reads no step.** Rule 5 of language 2
  ("checks and values stand anywhere and run when what they read exists")
  is the approved rule, and the runner did not follow it for a flow with
  a `start`: it reached only what follows or feeds a reached step. A data
  statement whose every source is reached, or that reads no statement, is
  on the path; no request or call is enabled by that. The change in
  `apiforge-runner/src/plan.rs` (`reachable_from`) belongs to this ticket.
  Consequence 5 below is what it means for something already published.
- **A3 — no `start` for a component** (consequence 1 as rewritten above).
  What the branch added for it — the fix writing a `start` into a
  component, a `start` in `sign-in.afs` and in component test sources —
  is taken out.
- **A4 — the language version moves here.** A build must not run a
  language-1 project with the new meaning, not even on `main` between two
  merges (LANG-00 section 2, rule 3). So this ticket, after LANG-15
  (#412) has merged, sets `LANGUAGE_VERSION = 2`, removes the `#[ignore]`
  of LANG-15's older-project tests, raises the manifest of
  `fixtures/scenarios/lay/rooms` and sets the number in the reference's
  first line. LANG-20 keeps the command only; until it merges, the
  refusal names a command that is not there yet, which is safe.
- **A5 — in a project still in language 1 the errors carry no fix.** The
  unit of conversion is the project (LANG-20): a file fixed alone under a
  language-1 manifest could no longer be told from a language-1 file
  whose steps run nowhere. In such a project the `@after` and `start`
  errors are shown without their fix and the project's problem names the
  command; in a language-2 project (a pasted line, an agent's old habit)
  the fix is offered as criteria 3 and 4 say.
- **A6 — crossings on `shop-30` with every wire drawn, and the header at
  960 px**: not ruled here; questions 2 and 3 to the owner on #386. The
  tests stay failing and the pull request a draft until they are
  answered.

**Consequence 5, to confirm at acceptance.** With A2, a published
revision that has a `start` and a check or value that neither follows nor
feeds a step now runs it; 0.1.0 skipped it without a word. No request is
sent that was not sent before. The board never offered *Start* on a
component, so only a hand-written component can be such a revision.

**Criteria added or restated**

- 9, also: *Save as component* on a selection of steps writes a component
  whose steps run in the order they ran in the scenario, with no `start`
  and no `@after`. Test: D `lang13_criterion_9_save_as_component`.
- 10, restated: after a migration or a new project, every scenario read
  back equals the original **with its ordering edges rebuilt as a chain**
  (`chained`), identities and positions unchanged, and its steps in the
  order `graph::sequence` gave before; every published revision is equal
  to the original exactly; a working component equals its published base
  in the same chained sense. `tests/migration.rs` asserts that instead of
  plain equality, and says so.
- 11, restated: "every `.afs`" is the working sources — `fixtures/scripts`,
  `fixtures/scenarios` — without `.apiforge/revisions/`, without the
  language-1 texts kept as test data and without `docs/qa/`.
- 14 (new): in a scenario with a `start`, a check that reads only a
  parameter, a value that reads nothing and a check written above the
  first request all run; in a component, an export of an input and of
  `env` has its value. Tests: R `lang13_criterion_14_*`.
- 15 (new, A4 and A5): `LANGUAGE_VERSION` is 2; a project with a
  language-1 manifest opens, shows its errors without a fix, and is
  refused a run with LANG-15's sentence; LANG-15's ignored tests run.
  Tests: those tests; P `lang13_criterion_15_no_fix_in_an_older_project`.

**Files, also:** `crates/apiforge-runner/src/plan.rs`;
`crates/apiforge-script/src/lib.rs` (the constant);
`crates/apiforge-desktop/src/folder/save_component.rs`;
`fixtures/scenarios/lay/rooms/apiforge.project.json`;
`docs/language/reference.md` line 1 (the number and the date).

**Before the pull request leaves draft**

1. A1, A3, the restated criteria 3 and 4 and the new 14 and 15 are in.
2. `main` is merged (FIX-01, RUN-01, then LANG-15) and the sources they
   added are converted: `labels.afs`, `run01.rs`.
3. The boards are measured again after A1 and the table on #386 is
   updated.
4. The owner has answered questions 2 and 3 on #386.

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
